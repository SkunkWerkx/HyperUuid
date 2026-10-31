//go:build hyperuuid_spike

package w2gaes

import (
	"encoding/hex"
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

var corpusDir = func() string {
	if d := os.Getenv("CORPUS"); d != "" {
		return d
	}
	return "../../../../../../corpus"
}()

func load(t *testing.T, name string) []map[string]any {
	b, err := os.ReadFile(filepath.Join(corpusDir, name))
	if err != nil {
		t.Fatal(err)
	}
	var v []map[string]any
	if err := json.Unmarshal(b, &v); err != nil {
		t.Fatal(err)
	}
	return v
}

func hx(s string) []byte { b, _ := hex.DecodeString(s); return b }

func layoutCode(s string) int32 {
	if s == "sql_server" {
		return 2
	}
	return 1
}

// put writes b at scratch+off and returns its guest address.
func put(off uint32, b []byte) int32 { copy(c.mem()[c.scratch+off:], b); return int32(c.scratch + off) }
func get(off uint32) string          { return hex.EncodeToString(c.mem()[c.scratch+off : c.scratch+off+16]) }

func TestCorpus(t *testing.T) {
	ns := map[string]string{"dns": "6ba7b8109dad11d180b400c04fd430c8", "url": "6ba7b8119dad11d180b400c04fd430c8",
		"oid": "6ba7b8129dad11d180b400c04fd430c8", "x500": "6ba7b8149dad11d180b400c04fd430c8"}
	n := 0
	for _, v := range load(t, "v5.json") {
		name := hx(v["name_hex"].(string))
		nsp, np := put(0, hx(ns[v["namespace"].(string)])), put(64, name)
		if code := c.m.Xuuid_new_v5(nsp, np, int32(len(name)), int32(c.scratch+32)); code != 0 || get(32) != v["expect"] {
			t.Errorf("v5 %v: code %d got %s", v, code, get(32))
		}
		n++
	}
	for _, v := range load(t, "sql_order.json") {
		p := put(0, hx(v["rfc"].(string)))
		if code := c.m.Xuuid_v7_to_sql_order(p); code != 0 || get(0) != v["sql"] {
			t.Errorf("to_sql %v: %d %s", v, code, get(0))
		}
		if code := c.m.Xuuid_v7_to_rfc_order(p); code != 0 || get(0) != v["rfc"] {
			t.Errorf("to_rfc %v: %d %s", v, code, get(0))
		}
		n++
	}
	for _, v := range load(t, "sql_order_refused.json") {
		p := put(0, hx(v["uuid"].(string)))
		var code int32
		if v["direction"] == "to_sql" {
			code = c.m.Xuuid_v7_to_sql_order(p)
		} else {
			code = c.m.Xuuid_v7_to_rfc_order(p)
		}
		if code == 0 || get(0) != v["uuid"] {
			t.Errorf("refused %v: %d %s", v, code, get(0))
		}
		n++
	}
	for _, v := range load(t, "timestamp.json") {
		p := put(0, hx(v["uuid"].(string)))
		ver := c.m.Xuuid_get_timestamp(p, layoutCode(v["layout"].(string)), int32(c.scratch+32))
		ms := uint64(c.mem()[c.scratch+32]) | uint64(c.mem()[c.scratch+33])<<8 | uint64(c.mem()[c.scratch+34])<<16 | uint64(c.mem()[c.scratch+35])<<24 |
			uint64(c.mem()[c.scratch+36])<<32 | uint64(c.mem()[c.scratch+37])<<40 | uint64(c.mem()[c.scratch+38])<<48 | uint64(c.mem()[c.scratch+39])<<56
		want := v["unix_millis"]
		if want == nil {
			if ver != 0 {
				t.Errorf("timestamp %v: version %d", v, ver)
			}
		} else if ver != int32(v["version"].(float64)) || ms != uint64(want.(float64)) {
			t.Errorf("timestamp %v: %d %d", v, ver, ms)
		}
		n++
	}
	for _, v := range load(t, "inspect.json") {
		p := put(0, hx(v["uuid"].(string)))
		lc := layoutCode(v["layout"].(string))
		ver := c.m.Xuuid_version(p, lc)
		if ver != int32(v["version"].(float64)) {
			t.Errorf("inspect version %v: %d", v, ver)
		}
		rfc := c.m.Xuuid_is_rfc(p, ver, lc) != 0
		if rfc != v["is_rfc"].(bool) {
			t.Errorf("inspect is_rfc %v: %v", v, rfc)
		}
		n++
	}
	t.Logf("%d vectors", n)
}
