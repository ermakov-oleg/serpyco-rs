use nohash_hasher::{IntMap, IsEnabled};

#[derive(Debug, Clone)]
pub enum FastMap<K: PartialEq, V> {
    Micro(micromap::Map<K, V, 32>),
    Int(IntMap<K, V>),
}

impl<K: PartialEq, V> FastMap<K, V> {
    pub fn new(capacity: usize) -> Self {
        if capacity < 32 {
            FastMap::Micro(micromap::Map::new())
        } else {
            FastMap::Int(IntMap::default())
        }
    }

    pub fn insert(&mut self, key: K, value: V)
    where
        K: std::hash::Hash + Eq + IsEnabled,
    {
        match self {
            FastMap::Micro(map) => map.insert(key, value),
            FastMap::Int(map) => {
                map.insert(key, value);
            }
        }
    }

    #[inline]
    pub fn get(&self, key: &K) -> Option<&V>
    where
        K: std::hash::Hash + Eq + IsEnabled,
    {
        match self {
            FastMap::Micro(map) => map.get(key),
            FastMap::Int(map) => map.get(key),
        }
    }
}
