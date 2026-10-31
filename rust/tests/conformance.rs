//! Replays the shared conformance corpus (`corpus/*.json` at the repository root) through
//! the public Rust API. The corpus is the cross-language contract: every binding replays
//! the same files against the same core, so a vector that drifts here breaks every binding
//! at once, and a byte moved in the SQL Server permutation (persisted data) fails here
//! before it can reorder anyone's clustered index. `corpus/README.md` has the vector shapes;
//! the two layout files are replayed inside the crate (lib.rs), since they pin the
//! deterministic half of generation that the public API doesn't expose.

use hyperuuid::{Layout, SqlOrderError, Uuid, Variant, get_timestamp, get_timestamp_in, v5, v7};
use serde_json::Value;
use std::path::PathBuf;

fn corpus(name: &str) -> Vec<Value> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../corpus")
        .join(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("reading {}: {error}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("parsing {name}: {error}"))
}

fn uuid(vector: &Value, field: &str) -> Uuid {
    let bytes = hex(vector[field].as_str().expect(field));
    Uuid::from_bytes(bytes.try_into().expect(field))
}

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn v5() {
    for vector in corpus("v5.json") {
        let namespace = match vector["namespace"].as_str().unwrap() {
            "dns" => v5::namespace::DNS,
            "url" => v5::namespace::URL,
            "oid" => v5::namespace::OID,
            "x500" => v5::namespace::X500,
            other => panic!("unknown namespace {other}"),
        };
        let name = hex(vector["name_hex"].as_str().unwrap());
        if let Some(text) = vector.get("name").and_then(Value::as_str) {
            assert_eq!(text.as_bytes(), name, "{vector}");
        }
        assert_eq!(
            v5::new_v5(namespace, &name),
            uuid(&vector, "expect"),
            "{vector}"
        );
    }
}

#[test]
fn sql_order() {
    for vector in corpus("sql_order.json") {
        let rfc = uuid(&vector, "rfc");
        let sql = uuid(&vector, "sql");
        assert_eq!(v7::to_sql_order(&rfc), Ok(sql), "{vector}");
        assert_eq!(v7::to_rfc_order(&sql), Ok(rfc), "{vector}");
    }
}

#[test]
fn sql_order_refused() {
    for vector in corpus("sql_order_refused.json") {
        let convert = match vector["direction"].as_str().unwrap() {
            "to_sql" => v7::to_sql_order,
            "from_sql" => v7::to_rfc_order,
            other => panic!("unknown direction {other}"),
        };
        assert_eq!(
            convert(&uuid(&vector, "uuid")),
            Err(SqlOrderError::WrongVersion),
            "{vector}"
        );
    }
}

fn layout(vector: &Value) -> Layout {
    match vector["layout"].as_str().expect("layout") {
        "rfc9562" => Layout::Rfc9562,
        "sql_server" => Layout::SqlServer,
        other => panic!("unknown layout {other}"),
    }
}

#[test]
fn timestamp() {
    for vector in corpus("timestamp.json") {
        let id = uuid(&vector, "uuid");
        let layout = layout(&vector);
        let version = id.version_in(layout);
        assert_eq!(
            u64::from(version),
            vector["version"].as_u64().unwrap(),
            "{vector}"
        );
        let field = vector["unix_millis"].as_u64();
        // The version-agnostic door answers only for a timestamp the crate would mint: a
        // version 7 dated past the generation cap has no timestamp there, as a version 6 has
        // none, while the per-version field reader below reads the 48 bits as they are.
        let expected = field.filter(|&millis| millis <= v7::MAX_UNIX_MILLIS);
        let found = get_timestamp_in(&id, layout).map(|t| t.to_unix_millis());
        assert_eq!(found, expected, "{vector}");
        if layout == Layout::Rfc9562 {
            assert_eq!(
                get_timestamp(&id).map(|t| t.to_unix_millis()),
                expected,
                "{vector}"
            );
        }
        if let (7, Some(millis)) = (version, field) {
            assert_eq!(v7::unix_millis_in(&id, layout), millis, "{vector}");
            if layout == Layout::Rfc9562 {
                assert_eq!(v7::unix_millis(&id), millis, "{vector}");
            }
        }
    }
}

#[test]
fn inspect() {
    for vector in corpus("inspect.json") {
        let id = uuid(&vector, "uuid");
        let layout = layout(&vector);
        let version = vector["version"].as_u64().unwrap() as u8;
        let is_rfc = vector["is_rfc"].as_bool().unwrap();
        assert_eq!(id.version_in(layout), version, "{vector}");
        assert_eq!(id.is_rfc_in(version, layout), is_rfc, "{vector}");
        // No other version answers true.
        for other in (0..=15).filter(|&v| v != version) {
            assert!(!id.is_rfc_in(other, layout), "{vector} is_rfc_in({other})");
        }
        if let Some(variant) = vector.get("variant").and_then(Value::as_str) {
            let expected = match variant {
                "ncs" => Variant::Ncs,
                "rfc9562" => Variant::Rfc9562,
                "microsoft" => Variant::Microsoft,
                "future" => Variant::Future,
                other => panic!("unknown variant {other}"),
            };
            assert_eq!(id.variant(), expected, "{vector}");
            assert_eq!(id.is_rfc(version), is_rfc, "{vector}");
        }
    }
}
