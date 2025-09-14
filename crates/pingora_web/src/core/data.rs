use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Thread-safe typed map for app-level shared data
#[derive(Default)]
pub struct AppData {
    inner: RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
}

impl std::fmt::Debug for AppData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppData").finish()
    }
}

impl AppData {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }

    pub fn provide_arc<T: Send + Sync + 'static>(&self, value: Arc<T>) -> Option<Arc<T>> {
        let type_id = TypeId::of::<T>();
        let mut map = self.inner.write().expect("AppData poisoned");
        let prev = map.insert(type_id, value as Arc<dyn Any + Send + Sync>);
        if let Some(prev_any) = prev {
            prev_any.downcast::<T>().ok()
        } else {
            None
        }
    }

    pub fn get<T: Send + Sync + 'static>(&self) -> Option<Arc<T>> {
        let map = self.inner.read().expect("AppData poisoned");
        let type_id = TypeId::of::<T>();
        if let Some(stored) = map.get(&type_id) {
            let cloned = stored.clone();
            cloned.downcast::<T>().ok()
        } else {
            None
        }
    }

    pub fn remove<T: Send + Sync + 'static>(&self) -> Option<Arc<T>> {
        let mut map = self.inner.write().expect("AppData poisoned");
        let type_id = TypeId::of::<T>();
        if let Some(prev) = map.remove(&type_id) {
            prev.downcast::<T>().ok()
        } else {
            None
        }
    }
}
