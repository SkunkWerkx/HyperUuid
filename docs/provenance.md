# Provenance

Every file a release ships verifies from the tag alone, with one command; the main README's
[Provenance](../README.md#provenance) has it.

Underneath that release attestation, every artifact also carries GitHub build provenance from
the run that made it, and there the flags depend on where the signing workflow lives, not on
the registry. The gems, the npm tarballs, the R source tarball, the C archives and
`inventory.json` are signed inside this repository's `ci.yml`, in the preparation run's release
tail at the preparation commit on master, and the published NuGet package inside its
`release.yml` at the tag; all verify with plain `--repo SkunkWerkx/HyperUuid`. The crate, the
jar, the wheels and the sdist, the pre-push NuGet package, every native library, TypeScript
addon and PHP extension zip (the whole story for Swift and PHP, which have no package of their
own; Go ships no binary at all, its core being generated source whose provenance is the
regeneration check) are signed by reusable workflows in `SkunkWerkx/.github`, all at that same
preparation commit. Add `--signer-repo SkunkWerkx/.github` (and `--source-digest <commit>` to
pin either), or `--owner SkunkWerkx` in place of both. Get the
signer wrong and `gh` reports a bare `verifying with issuer "sigstore.dev"`, which reads like a
bad signature but is only an identity mismatch. PyPI additionally holds a
[PEP 740](https://peps.python.org/pep-0740/) publish attestation for every wheel and the sdist, filed by the
publish action and shown on each file's PyPI page.

The gates run on the way in, not just on the way out: the gem job verifies every native
artifact it packs before building a gem, every package is verified against its provenance,
inventoried and preserved in the preparation run before the staging commit exists, that run's
`stage-release` job verifies the Swift zips' attestations before writing their checksums into
`Package.swift`, and the tag run re-verifies every preserved file and the inventory before it
signs the release attestation and uploads anything ([RELEASING.md](../.github/RELEASING.md)),
so a failed check stops the release while nothing is yet irreversible. The READMEs that show the
command on their own artifact: [Rust](../rust/#verifying-provenance),
[C#](../csharp/#native-binary-provenance), [Java](../java/#verifying-provenance),
[Ruby](../ruby/#verifying-provenance), [Python](../python/#verifying-provenance),
[PHP](../php/#verifying-provenance), [Swift](../swift/#verifying-provenance),
[Go](../go/#verifying-build-provenance), [C](../c/#install).
