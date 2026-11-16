use std::sync::Arc;

use crate::db::Store;

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub ingest_api_key: Arc<str>,
}
