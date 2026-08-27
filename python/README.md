# hyperuuid

RFC 9562 UUID v4 (random), v5 (deterministic), and v7 (time-sortable) generation,
calling directly into the native `libhyperuuid` shared library via stdlib `ctypes` —
no runtime bridge, no extra dependency.

Ships `linux-arm64` only for now. The same `ctypes.CDLL` code also runs under
[Pyodide](https://pyodide.org/) in the browser given an Emscripten-built
`libhyperuuid.so` "side module" (Pyodide has shipped real `ctypes` support since
0.18) — that additional build just isn't included here yet.

```python
import uuid
import hyperuuid

hyperuuid.new_v4()
hyperuuid.new_v5(uuid.NAMESPACE_DNS, "example.com")
hyperuuid.new_v7()
```

Returns stdlib `uuid.UUID` objects. For v5's namespace argument, use the RFC 9562
Section 6.6 well-known namespaces already in the standard library —
`uuid.NAMESPACE_DNS`, `NAMESPACE_URL`, `NAMESPACE_OID`, `NAMESPACE_X500` — no need
for this package to redefine them.
