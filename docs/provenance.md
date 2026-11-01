# Provenance

Every file a release ships verifies from the tag alone, with one command; the main README's
[Provenance](../README.md#provenance) has it.

Underneath that release attestation, every artifact also carries GitHub build provenance from
the run that made it, and there the flags depend on where the signing workflow lives, not on
the registry. The gems, the npm tarballs, the R source tarball, the C archives, `inventory.json`
and the published NuGet package are signed inside this repository's `release.yml` and verify
with plain `--repo SkunkWerkx/HyperUuid`. The crate, the jar, the wheels and the sdist, the pre-push NuGet package, every
native library, TypeScript addon and PHP extension zip (the whole story for Swift and PHP,
which have no package of their own; Go ships no binary at all, its core being generated
source whose provenance is the regeneration check) are signed by reusable workflows in
`SkunkWerkx/.github`: the crate and the Maven files at the tagged commit, the wheels, the sdist and the
natives at the preparation commit on master they were built from. Add `--signer-repo SkunkWerkx/.github` (and `--source-digest <commit>` to pin
either), or `--owner SkunkWerkx` in place of both. Get the
signer wrong and `gh` reports a bare `verifying with issuer "sigstore.dev"`, which reads like a
bad signature but is only an identity mismatch. PyPI additionally holds a
[PEP 740](https://peps.python.org/pep-0740/) publish attestation for every wheel and the sdist, filed by the
publish action and shown on each file's PyPI page.

The gates run on the way in, not just on the way out: `stage-native-binaries.yml` verifies the
Swift zips' attestations before writing their checksums into `Package.swift`, the gem
job verifies every native artifact it packs before building a gem, every package is attested
and preserved before anything is uploaded, and the upload itself is a second, deliberate
dispatch of `release.yml` from those preserved bytes ([RELEASING.md](../.github/RELEASING.md)), so
a failed check stops the release while nothing is yet irreversible. The READMEs that show the
command on their own artifact: [Rust](../rust/#verifying-provenance),
[C#](../csharp/#native-binary-provenance), [Java](../java/#verifying-provenance),
[Ruby](../ruby/#verifying-provenance), [Python](../python/#verifying-provenance),
[PHP](../php/#verifying-provenance), [Swift](../swift/#verifying-provenance),
[Go](../go/#verifying-build-provenance), [C](../c/#install).
