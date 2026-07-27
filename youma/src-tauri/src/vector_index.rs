// vector_index.rs — In-process HNSW vector index for semantic search.
//
// Wraps hnsw_rs with a global RwLock'd singleton.
// Rebuilt from SQLite embeddings on startup, updated incrementally.
// Thread-safe: concurrent reads (search) use read lock, writes (add) use write lock.

use hnsw_rs::prelude::*;
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

// ── State ─────────────────────────────────────────────────────────────────────

struct HnswState {
    /// HNSW index over f32 vectors with cosine distance.
    /// 'static lifetime because all inserted data is owned (Vec copies) inside hnsw_rs.
    hnsw: Hnsw<'static, f32, DistCosine>,
    /// Sequential key → chunk_id (hnsw_rs uses usize IDs).
    key_to_id: HashMap<usize, String>,
    /// chunk_id → sequential key (dedup guard).
    id_to_key: HashMap<String, usize>,
    next_key: usize,
}

// hnsw_rs uses parking_lot internally and is safe to share across threads
// when guarded by an external RwLock. We access it only while holding
// the write lock for inserts and the read lock for searches.
unsafe impl Send for HnswState {}
unsafe impl Sync for HnswState {}

impl HnswState {
    fn new() -> Self {
        // max_nb_connection=16, max_elements=200_000, max_layer=16, ef_construction=200
        let hnsw: Hnsw<'static, f32, DistCosine> =
            Hnsw::new(16, 200_000, 16, 200, DistCosine {});
        Self {
            hnsw,
            key_to_id: HashMap::new(),
            id_to_key: HashMap::new(),
            next_key: 1,
        }
    }

    fn add(&mut self, chunk_id: &str, vec: &[f32]) {
        if self.id_to_key.contains_key(chunk_id) {
            return; // already indexed
        }
        let key = self.next_key;
        self.next_key += 1;
        // hnsw_rs insert copies the slice into an owned Vec internally.
        self.hnsw.insert((vec, key));
        self.key_to_id.insert(key, chunk_id.to_string());
        self.id_to_key.insert(chunk_id.to_string(), key);
    }

    fn search(&self, query: &[f32], top_k: usize) -> Vec<(String, f32)> {
        if self.key_to_id.is_empty() {
            return vec![];
        }
        let ef = (top_k * 4).max(50).min(self.key_to_id.len());
        let neighbours = self.hnsw.search(query, top_k, ef);
        neighbours
            .into_iter()
            .filter_map(|n| {
                self.key_to_id.get(&n.d_id).map(|id| {
                    // DistCosine returns cosine distance (0=identical, 2=opposite).
                    // Convert to similarity in [0,1].
                    let sim = 1.0 - (n.distance / 2.0);
                    (id.clone(), sim.clamp(0.0, 1.0))
                })
            })
            .collect()
    }

    fn size(&self) -> usize {
        self.key_to_id.len()
    }
}

// ── Global singleton ──────────────────────────────────────────────────────────

static GLOBAL: OnceLock<RwLock<HnswState>> = OnceLock::new();

fn global() -> &'static RwLock<HnswState> {
    GLOBAL.get_or_init(|| RwLock::new(HnswState::new()))
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Load all stored embeddings from SQLite and (re)build the HNSW index.
/// Call on startup after DB init. Returns number of vectors loaded.
pub fn rebuild_from_db() -> Result<usize, String> {
    eprintln!("[vector_index] rebuild_from_db: loading embeddings from SQLite...");

    // embeddings_all_with_meta returns (chunk_id, title, content, topic, Vec<f32>)
    let rows = crate::db::embeddings_all_with_meta()?;
    if rows.is_empty() {
        eprintln!("[vector_index] rebuild_from_db: no embeddings stored yet");
        return Ok(0);
    }

    let mut state = HnswState::new();
    let mut loaded = 0usize;

    for (chunk_id, _title, _content, _topic, emb) in &rows {
        state.add(chunk_id, emb);
        loaded += 1;
    }

    // Replace the global state.
    match global().write() {
        Ok(mut guard) => *guard = state,
        Err(e) => return Err(format!("[vector_index] write lock poisoned: {e}")),
    }

    eprintln!("[vector_index] rebuild_from_db: {loaded} vectors indexed");
    Ok(loaded)
}

/// Add a single embedding to the in-memory index.
/// Called from knowledge::embed_source_chunks / embed_all_pending after db::embedding_set.
pub fn add(chunk_id: &str, vec: &[f32]) {
    match global().write() {
        Ok(mut guard) => guard.add(chunk_id, vec),
        Err(e) => eprintln!("[vector_index] add write lock error: {e}"),
    }
}

/// Search the HNSW index. Returns (chunk_id, similarity) pairs sorted by descending similarity.
/// Returns empty vec if the index is not built or empty.
pub fn search(query: &[f32], top_k: usize) -> Vec<(String, f32)> {
    match global().read() {
        Ok(guard) => {
            let mut results = guard.search(query, top_k);
            results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            results
        }
        Err(e) => {
            eprintln!("[vector_index] search read lock error: {e}");
            vec![]
        }
    }
}

/// True if the index has at least one vector.
pub fn is_ready() -> bool {
    global().read().map(|g| g.size() > 0).unwrap_or(false)
}

/// Number of vectors in the index.
pub fn size() -> usize {
    global().read().map(|g| g.size()).unwrap_or(0)
}
