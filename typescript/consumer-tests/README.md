# npm consumer tests

What a consumer gets from the ten npm tarballs, proven by installing them. `run.sh` serves the
packed tarballs from a local [Verdaccio](https://verdaccio.org) registry on port 4873, then
runs each scenario in a fresh directory under `work/<scenario>/` with its own `package.json`,
`.npmrc` and npm cache, so `npm install` resolves, selects platforms and writes lockfiles
exactly as it would against npmjs. Nothing here reaches npmjs for the packages under test:
`hyperuuid` and `@skunkwerkx/*` have no uplink in `verdaccio.yaml`; everything else (the WASI
package's runtime, the bundlers' dependencies) proxies through. `smoke.cjs` is the consumer:
it requires `hyperuuid`, prints `available=<bool> target=<native|wasm32-wasip1|none>`,
generates a v4 and a v7, and checks one v5 corpus vector and one SQL-order vector from
`HYPERUUID_CORPUS`. When nothing loaded it prints the `ERR_HYPERUUID_UNAVAILABLE` error's cause
chain, one line per cause.

The scenarios are the design spec's
([`docs/superpowers/specs/2026-10-31-typescript-wasi-fallback-design.md`](../../docs/superpowers/specs/2026-10-31-typescript-wasi-fallback-design.md),
"The consumer tests: ci.yml's `test-npm-package`"); the header of `run.sh` lists them with what each asserts. In CI,
`test-npm-package` in `ci.yml` runs them on Linux, macOS and Windows (on the pull-request
board when the pull request touched `typescript/`, the core or `.github/`; on the release board
always) after the whole conformance suite under `NAPI_RS_FORCE_WASI=error` on each; the
Docker scenarios (`floor`, `s390x`) and `mac-lockfile` run on Linux. `s390x` asserts a loss,
not a platform: see below.

## Running locally

Pack tarballs from this machine's one native and the WASI build (from the repository root;
`cargo cdylib` and `npm run build` then `npm run build:wasi` in `typescript/` first):

```sh
mkdir -p /tmp/node-artifacts/node-osx-arm64 /tmp/node-artifacts/node-wasm32-wasip1
cp typescript/hyperuuid.darwin-arm64.node /tmp/node-artifacts/node-osx-arm64/
cp typescript/hyperuuid.wasm32-wasip1.wasm typescript/hyperuuid.wasip1* /tmp/node-artifacts/node-wasm32-wasip1/
python3 ../HyperForge/.github/scripts/npm_package.py pack --package typescript \
  --artifacts /tmp/node-artifacts --out /tmp/npm-local --allow-missing-natives
```

Then the host scenarios:

```sh
typescript/consumer-tests/run.sh /tmp/npm-local negative-unpublished plain omit-optional \
  absent-native forced-wasi negative-deleted-package negative-deleted-wasm bundlers
```

And the container scenarios, which need Docker (on Docker Desktop the containers reach the
registry through `host.docker.internal`; on a Linux host leave `REGISTRY_HOST` unset and they
share the host network):

```sh
REGISTRY_HOST=host.docker.internal typescript/consumer-tests/run.sh /tmp/npm-local floor
REGISTRY_HOST=host.docker.internal typescript/consumer-tests/run.sh /tmp/npm-local s390x
```

`mac-lockfile` needs only the darwin-arm64 package in the registry, so it runs from a local
pack too (`run.sh <dir> mac-lockfile`). Negative controls: `CORPUS_DIR=$(mktemp -d) run.sh <dir> plain` fails on the
vector check; `FLOOR_IMAGE=node:22.12.0-alpine run.sh <dir> floor` fails the install with
`EBADENGINE`.

## What the runs established (2026-10-31, Apple M6, Node 26.13.0, npm 11.20.0)

- **Bundlers.** Neither esbuild (`--bundle --platform=node`) nor webpack produces a working
  self-contained bundle: the loader's `require` calls for the platform packages and the WASI
  package cannot be resolved into one file (webpack fails the build on them; esbuild bundles
  and the bundle then cannot load the module in a directory with no `node_modules`). The
  configuration that passes, for both, is `@skunkwerkx/hyperuuid` **external**: esbuild
  `--external:@skunkwerkx/hyperuuid`, webpack
  `externals: { '@skunkwerkx/hyperuuid': 'commonjs @skunkwerkx/hyperuuid' }`, with
  the package installed beside the bundle (`npm install --omit=dev` of the deployment's
  `package.json`). That is the configuration to document for bundled deployments.
- **npm bug 4828 is fixed in npm 11.20.0 and alive in Node 22's npm.** The scenario reproduces
  npm/cli#4828 live: install for darwin-arm64, delete the lockfile, install again beside
  `node_modules`, and the regenerated lockfile names only the platform package on disk; `npm ci`
  from it elsewhere installs no native. With npm 11.20.0 that no longer happens on either side:
  the regenerated lockfile names every platform (CI's first run, 2026-10-31, failed the scenario's
  shape assertion; the same with esbuild's 25 platform packages), and `npm ci` from a lockfile
  missing optional platform packages refuses with `EUSAGE`, naming each missing one. npm 10.9.2,
  the npm Node 22.13.0 bundles, still does both (esbuild: one platform entry after regeneration,
  `npm ci --os=linux --cpu=x64` exits 0 with nothing installed), so the scenario installs that npm
  into `work/` and reproduces with it (`LOCKFILE_NPM`), as the Linux x64 consumer `--os=linux
  --cpu=x64` makes it on any host. Node 22 users with a lockfile a Mac wrote are who this is for.
- **Big-endian hosts.** In `node:24-alpine` for `linux/s390x` under QEMU the install resolves
  no native package, the fallback loads, and every check of `smoke.cjs` passes, but the process
  never exits: `node ./smoke.cjs` sat idle (main thread in `epoll_wait`, 0% CPU) with one
  referenced `MessagePort`. Tracing `MessagePort.prototype.ref` named it: `@emnapi/runtime`'s
  `NodejsWaitingRequestCounter`, ref'd by `napi_create_threadsafe_function` (napi-rs creates one
  at module registration) and never unref'd, because `napi_unref_threadsafe_function` in
  `@napi-rs/wasm-runtime`'s `emnapi-plugins.cjs` reads the count with
  `Atomics.load(new Uint32Array(memory))`, host-endian, where the create wrote it with a
  little-endian `DataView`: the 1 reads as 16,777,216 on s390x (`os.endianness() === 'BE'`;
  reproduced with a bare `WebAssembly.Memory`). The same mix appears at some forty more sites
  in the runtime. `container/consumer.sh` therefore bounds the smoke with `timeout 300` (QEMU on a CI runner is slower than on the Mac, where the checks print well inside two minutes) and
  prints `smoke exit=<status>`, and `scenario_s390x` asserts, after the checks, that the bound
  killed it (busybox's `timeout` reports the child's SIGTERM as 143, GNU's would say 124): an
  exit of 0 means upstream fixed it and the docs are stale, so it fails too. On the Mac the
  checks print within a minute of the container's start. Nothing beyond the smoke's five checks
  is claimed for big-endian hosts.
- **Bun.** Under `NAPI_RS_FORCE_WASI=error`, Bun 1.4.4 cannot initialise the WASI module (its
  `node:wasi` has no reactor `initialize`), so `require('@skunkwerkx/hyperuuid')` succeeds with
  `isAvailable() === false`. The native addon on Bun passes the whole suite. Recorded, not a
  failure of the run.
- **Publishing to Verdaccio.** npm refuses to publish with no credentials at all, whatever the
  registry's rule, so `run.sh` creates one user through the registry's own user endpoint
  (`PUT /-/user/org.couchdb.user:publisher`, `max_users: 1` in `verdaccio.yaml`) and puts its
  token in a user config of its own (`NPM_CONFIG_USERCONFIG=work/npmrc`), so the developer's
  `~/.npmrc` never reaches a scenario.
- **Project config.** npm reads the project `.npmrc` from the nearest directory holding a
  `package.json` or `node_modules`, so every consumer directory gets a minimal `package.json`
  first; without it the harness's own manifest would be the project.
