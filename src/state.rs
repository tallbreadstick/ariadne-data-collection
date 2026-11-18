use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

use crate::db::Store;
use crate::models::DiffDelta;

#[derive(Debug, Clone)]
pub struct BufferedChunk {
    pub delta: DiffDelta,
    pub received_at: Instant,
}

#[derive(Debug, Default)]
pub struct ChunkBuffer {
    /// Maps chunk_id -> (total_chunks, map of chunk_index -> BufferedChunk)
    pub batches: HashMap<String, (u32, HashMap<u32, BufferedChunk>)>,
}

impl ChunkBuffer {
    pub fn cleanup_stale(&mut self) {
        let now = Instant::now();
        // Discard buffers older than 10 minutes
        self.batches.retain(|_, (_, chunks)| {
            chunks.values().any(|c| now.duration_since(c.received_at).as_secs() < 600)
        });
    }
}

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub ingest_api_key: Arc<str>,
    pub chunk_buffer: Arc<Mutex<ChunkBuffer>>,
}
