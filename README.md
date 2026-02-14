# unitrie-rs-core

Reusable Rust crate for RSK Unitrie core operations, without JNI bindings.

## What it provides

- `UnitrieCore` facade with selectable implementation:
  - `legacy-v1`: compatibility-oriented legacy engine.
  - `next`: incremental engine with cached mutation/persistence paths.
- Core trie operations:
  - `get`, `put`, `delete`, `delete_recursive`
  - root hash calculation and snapshotting
  - persisted root loading via `RawStoreAdapter`
- Codec and hashing utilities used by both trie engines.

## Install

Add to your `Cargo.toml`:

```toml
[dependencies]
unitrie-rs-core = { path = "./unitrie-rs" }
```

## Quick example

```rust
use unitrie_rs_core::{UnitrieCore, UnitrieImplementation};

let mut trie = UnitrieCore::new(UnitrieImplementation::LegacyV1);
trie.put(b"hello".to_vec(), b"world".to_vec());
assert_eq!(trie.get(b"hello"), Some(b"world".to_vec()));
```

## Development

- Run tests: `cargo test`
- Run benches: `cargo bench`

## License

LGPL-3.0-or-later
