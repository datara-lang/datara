# datara.toml / datara.lock Schema (DPM)

## datara.toml (project manifest)

```toml
[package]
name = "myapp"            # required
version = "1.5.1"          # semver string
description = "..."        # optional (publish metadata)
authors = ["..."]           # optional
license = "MIT"             # optional
entry = "src/main.dtr"      # Level-3 entry point
manifest_schema = 1         # optional, at file top

[dependencies]
mathx = "0.1.0"             # registry version req
jsonx = { version = "0.1.0", path = "../jsonx" }  # path dep

[capabilities]              # permission grants (Enterprise tier)
fs-read = ["./**"]
fs-write = ["./out.csv"]
exec = ["*"]
```

## datara.lock (generated; commit it)

Written by `dpm install` / `dpm publish`. Table per locked package:

```toml
[[packages]]
name = "mathx"
version = "0.1.0"
digest = "merkle:sha256:<hex>"   # content-addressed integrity
source = "registry"              # or "local" / a URL
dependencies = []
```

## Package registry layout ($DATARA_HOME)

- `store/bundles/<name>-<version>.dtr-pkg` — the package bundle (JSON)
- `store/bundles/....sig` — detached Ed25519 signature (`ed25519:<pk>:<sig>`)
- `store/<name>/<version>/` — CAS-extracted package sources
- `keys/<key-id>.sk` — signing secret keys (`dpm keygen`)
- `trust/<pk>.pk` — trust anchors (`dpm trust <pk>`)

Verification (`dpm verify`): the merkle digest is recomputed from
packages/ and must match datara.lock; when a bundle `.sig` exists, the
signature is checked against the trust-store — an invalid trusted
signature escalates the package to Mismatch even with a matching digest.
