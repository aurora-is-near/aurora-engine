/// The intention of this struct is to prevent repeating duplicate computations/IO with the
/// same input (key). However, unlike memoization or typical caching, this only remembers the
/// most recent key-value pair. This means it is optimized for consecutive duplicate lookups,
/// as opposed to general duplicated lookups. The benefit is that its memory footprint and
/// internal logic are both minimal, and the drawback is that its use case is very narrow.
#[derive(Default)]
pub struct DupCache<K, V> {
    key: K,
    value: V,
}

impl<K: Copy + Eq, V> DupCache<K, V> {
    pub fn get_or_insert_with<F: FnOnce() -> V>(&mut self, k: &K, f: F) -> &mut V {
        if &self.key != k {
            let new_value = f();
            self.value = new_value;
            self.key = *k;
        }

        &mut self.value
    }
}

/// Same as `DupCache` but optimized for the case that `K = (K1, K2)`.
#[derive(Default)]
pub struct PairDupCache<K1, K2, V> {
    key: (K1, K2),
    value: V,
}

impl<K1: Copy + Eq, K2: Copy + Eq, V> PairDupCache<K1, K2, V> {
    pub fn get_or_insert_with<F: FnOnce() -> V>(&mut self, k: (&K1, &K2), f: F) -> &mut V {
        if (&self.key.0 != k.0) || (&self.key.1 != k.1) {
            let new_value = f();
            self.value = new_value;
            self.key = (*k.0, *k.1);
        }

        &mut self.value
    }
}
