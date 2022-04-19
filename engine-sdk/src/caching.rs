use aurora_engine_types::BTreeMap;

/// A naive cache storing all key-value pairs it learns about..
#[derive(Default)]
pub struct FullCache<K, V> {
    inner: BTreeMap<K, V>,
}

impl<K: Ord, V> FullCache<K, V> {
    pub fn get_or_insert_with<F: FnOnce() -> V>(&mut self, k: K, f: F) -> &mut V {
        self.inner.entry(k).or_insert_with(f)
    }
}
