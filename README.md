# unitrie-rs

`unitrie-rs` is the reusable Rust core for Rootstock Unitrie.

This crate is intentionally JNI-free so it can be embedded by different hosts (RSKj adapter, future Rust-native clients, and potential Reth integration paths).

## Scope

- Consensus-sensitive trie behavior (`put/get/delete/delete_recursive`)
- Root hash semantics and snapshot support
- Persistence load/save via `RawStoreAdapter`
- Compatibility-focused implementations:
  - `legacy-v1`
  - `next`
- Codec modules used by the trie core:
  - `RSKIP107`
  - `Orchid`

## Install

```toml
[dependencies]
unitrie-rs = { git = "https://github.com/fedejinich/unitire-rs.git" }
```

## Quick start

```rust
use unitrie_rs::{UnitrieCore, UnitrieImplementation};

let mut trie = UnitrieCore::new(UnitrieImplementation::LegacyV1);
trie.put(b"hello".to_vec(), b"world".to_vec());
assert_eq!(trie.get(b"hello"), Some(b"world".to_vec()));
```

## Development

```bash
cargo test
cargo bench --bench core_trie_bench
```

## Validation approach

- Rust parity tests compare `legacy-v1` and `next` deterministically.
- This crate is intended to be validated against Java behavior in host integration repositories.

## License

LGPL-3.0-or-later.
