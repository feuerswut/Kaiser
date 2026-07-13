use std::sync::Mutex;

use kaiser_core::{AudioManager, KaiserBackend, KaiserConfigStore};
use monarch::MonarchDisplayManager;

pub struct AppState {
    pub manager: Mutex<MonarchDisplayManager<KaiserBackend, KaiserConfigStore>>,
    pub audio: Mutex<AudioManager>,
    pub store_path: std::path::PathBuf,
}

impl AppState {
    pub fn new() -> Self {
        let store_path = KaiserConfigStore::default_path();
        let backend = KaiserBackend::new();
        let store = KaiserConfigStore::new(store_path.clone());
        let manager = MonarchDisplayManager::new(backend, store)
            .expect("failed to initialize display manager");
        Self {
            manager: Mutex::new(manager),
            audio: Mutex::new(AudioManager::new()),
            store_path,
        }
    }

    pub fn new_store(&self) -> KaiserConfigStore {
        KaiserConfigStore::new(self.store_path.clone())
    }
}
