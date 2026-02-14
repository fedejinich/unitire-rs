use criterion::{black_box, criterion_group, criterion_main, Criterion};
use hex::decode as decode_hex;
use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Once;
use std::time::Instant;
use unitrie_rs::next::core_trie::NextUnitrie;
use unitrie_rs::store_adapter::RawStoreAdapter;

static SUMMARY_ONCE: Once = Once::new();
const CORE_CORPUS_ENV: &str = "UNITRIE_JMH_CORE_CORPUS_PATH";
const CORE_OUTPUT_ENV: &str = "UNITRIE_RUST_CORE_BENCH_OUTPUT";

#[derive(Debug, Deserialize)]
struct Corpus {
    workloads: Vec<Workload>,
}

#[derive(Debug, Deserialize, Clone)]
struct Workload {
    name: String,
    #[serde(default = "default_repeat")]
    repeat: usize,
    operations: Vec<Operation>,
}

#[derive(Debug, Deserialize, Clone)]
struct Operation {
    op: String,
    #[serde(default)]
    #[serde(alias = "keyHex")]
    key_hex: Option<String>,
    #[serde(default)]
    #[serde(alias = "valueHex")]
    value_hex: Option<String>,
    #[serde(default)]
    size: Option<usize>,
}

#[derive(Debug)]
struct InMemoryRawStoreAdapter {
    nodes: HashMap<Vec<u8>, Vec<u8>>,
    values: HashMap<Vec<u8>, Vec<u8>>,
}

impl InMemoryRawStoreAdapter {
    fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            values: HashMap::new(),
        }
    }
}

impl RawStoreAdapter for InMemoryRawStoreAdapter {
    fn load_raw_node(&mut self, hash: &[u8]) -> Option<Vec<u8>> {
        self.nodes.get(hash).cloned()
    }

    fn load_raw_value(&mut self, hash: &[u8]) -> Option<Vec<u8>> {
        self.values.get(hash).cloned()
    }

    fn save_raw_node(&mut self, hash: &[u8], serialized_node: &[u8]) {
        self.nodes.insert(hash.to_vec(), serialized_node.to_vec());
    }

    fn save_raw_value(&mut self, hash: &[u8], value: &[u8]) {
        self.values.insert(hash.to_vec(), value.to_vec());
    }
}

fn default_repeat() -> usize {
    1
}

fn core_trie_bench(criterion: &mut Criterion) {
    let corpus = load_corpus();
    SUMMARY_ONCE.call_once(|| {
        if let Err(error) = write_manual_summary(&corpus) {
            panic!("failed to write rust core benchmark summary: {error}");
        }
    });

    let mut group = criterion.benchmark_group("TrieRustCoreBenchmark");
    for workload in &corpus.workloads {
        let workload = workload.clone();
        group.bench_function(workload.name.clone(), move |bencher| {
            bencher.iter(|| {
                let checksum = run_workload(&workload);
                black_box(checksum);
            });
        });
    }
    group.finish();
}

fn load_corpus() -> Corpus {
    let path = resolve_corpus_path();
    let payload = fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "could not read core workload corpus at {}: {error}",
            path.display()
        )
    });
    serde_json::from_str(&payload).unwrap_or_else(|error| {
        panic!(
            "invalid core workload corpus at {}: {error}",
            path.display()
        )
    })
}

fn resolve_corpus_path() -> PathBuf {
    if let Ok(configured) = env::var(CORE_CORPUS_ENV) {
        if !configured.trim().is_empty() {
            return PathBuf::from(configured);
        }
    }

    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    crate_root
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("benchmarks")
        .join("unitrie-corpus")
        .join("workloads-v1.json")
}

fn run_workload(workload: &Workload) -> usize {
    let mut trie = NextUnitrie::new();
    let mut store = InMemoryRawStoreAdapter::new();
    let mut checksum = 0usize;

    for _ in 0..workload.repeat.max(1) {
        for operation in &workload.operations {
            checksum ^= apply_operation(&mut trie, &mut store, operation);
        }
    }

    checksum
}

fn apply_operation(
    trie: &mut NextUnitrie,
    store: &mut InMemoryRawStoreAdapter,
    operation: &Operation,
) -> usize {
    let operation_name = operation.op.trim().to_ascii_lowercase();
    match operation_name.as_str() {
        "put" => {
            let key = decode_required(&operation.key_hex, "keyHex", "put");
            let value = decode_required(&operation.value_hex, "valueHex", "put");
            let value_len = value.len();
            trie.put(key, value);
            value_len
        }
        "get" => {
            let key = decode_required(&operation.key_hex, "keyHex", "get");
            trie.get_ref(&key).map(|value| value.len()).unwrap_or(0)
        }
        "delete" => {
            let key = decode_required(&operation.key_hex, "keyHex", "delete");
            trie.delete(&key);
            key.len()
        }
        "deleterecursive" | "delete_recursive" | "delete-recursive" => {
            let key = decode_required(&operation.key_hex, "keyHex", "deleteRecursive");
            trie.delete_recursive(&key);
            key.len()
        }
        "getvaluelength" | "get_value_length" | "get-value-length" => {
            let key = decode_required(&operation.key_hex, "keyHex", "getValueLength");
            trie.get_value_length(&key).unwrap_or(0)
        }
        "getvaluehash" | "get_value_hash" | "get-value-hash" => {
            let key = decode_required(&operation.key_hex, "keyHex", "getValueHash");
            trie.get_value_hash(&key)
                .map(|hash| hash.len())
                .unwrap_or(0)
        }
        "collectkeys" | "collect_keys" | "collect-keys" => {
            let size = operation.size.unwrap_or(0);
            trie.collect_keys(size).len()
        }
        "save" => {
            trie.save_to_store(store);
            1
        }
        "savereload" | "save_reload" | "save-reload" => {
            trie.save_to_store(store);
            let root = trie.current_root_hash();
            let rehydrated =
                NextUnitrie::from_persisted_root(&root, store).unwrap_or_else(|error| {
                    panic!("could not rehydrate trie from persisted root: {error}")
                });
            *trie = rehydrated;
            root.len()
        }
        "roothash" | "root_hash" | "root-hash" => trie.current_root_hash().len(),
        _ => panic!("unsupported workload operation: {}", operation.op),
    }
}

fn decode_required(hex: &Option<String>, field_name: &str, operation: &str) -> Vec<u8> {
    let raw_hex = hex
        .as_ref()
        .map(String::as_str)
        .unwrap_or_else(|| panic!("operation {operation} requires {field_name}"));
    decode_hex_value(raw_hex)
}

fn decode_hex_value(raw: &str) -> Vec<u8> {
    let trimmed = raw.trim();
    let without_prefix = trimmed.strip_prefix("0x").unwrap_or(trimmed);
    let normalized = if without_prefix.len() % 2 == 1 {
        let mut prefixed = String::with_capacity(without_prefix.len() + 1);
        prefixed.push('0');
        prefixed.push_str(without_prefix);
        prefixed
    } else {
        without_prefix.to_string()
    };

    if normalized.is_empty() {
        return Vec::new();
    }

    decode_hex(&normalized).unwrap_or_else(|error| panic!("invalid hex value '{raw}': {error}"))
}

fn write_manual_summary(corpus: &Corpus) -> Result<(), String> {
    let mut workloads = Vec::with_capacity(corpus.workloads.len());
    for workload in &corpus.workloads {
        let mut samples_ns = Vec::with_capacity(30);
        let mut checksum = 0usize;
        for _ in 0..30 {
            let started = Instant::now();
            checksum ^= run_workload(workload);
            samples_ns.push(started.elapsed().as_nanos() as f64);
        }

        samples_ns.sort_by(|left, right| left.partial_cmp(right).unwrap());
        let avg_ns = samples_ns.iter().sum::<f64>() / samples_ns.len() as f64;
        let p95_index = ((samples_ns.len() as f64 * 0.95).ceil() as usize).saturating_sub(1);
        let p95_ns = samples_ns[p95_index.min(samples_ns.len() - 1)];
        let throughput_ops_per_sec = if avg_ns <= 0.0 {
            0.0
        } else {
            1_000_000_000.0 / avg_ns
        };

        workloads.push(serde_json::json!({
            "benchmark": workload.name,
            "engine": "rust(next-core)",
            "metrics": {
                "avgMicros": avg_ns / 1_000.0,
                "p95Micros": p95_ns / 1_000.0,
                "throughputOpsPerSec": throughput_ops_per_sec
            },
            "sampleCount": samples_ns.len(),
            "checksum": checksum
        }));
    }

    let output_path = resolve_output_path();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "could not create output directory {}: {error}",
                parent.display()
            )
        })?;
    }

    let payload = serde_json::json!({
        "generatedAt": chrono_like_timestamp(),
        "engine": "rust(next-core)",
        "workloads": workloads
    });

    fs::write(
        &output_path,
        serde_json::to_vec_pretty(&payload)
            .map_err(|error| format!("could not serialize rust core summary JSON: {error}"))?,
    )
    .map_err(|error| format!("could not write {}: {error}", output_path.display()))
}

fn resolve_output_path() -> PathBuf {
    if let Ok(configured) = env::var(CORE_OUTPUT_ENV) {
        if !configured.trim().is_empty() {
            return PathBuf::from(configured);
        }
    }

    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    crate_root
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("rskj-core")
        .join("build")
        .join("reports")
        .join("jmh")
        .join("result_trie_rust_core_summary.json")
}

fn chrono_like_timestamp() -> String {
    // Keep dependency footprint small for benches.
    let now = std::time::SystemTime::now();
    match now.duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => format!("{}s", duration.as_secs()),
        Err(_) => "0s".to_string(),
    }
}

criterion_group!(core_trie, core_trie_bench);
criterion_main!(core_trie);
