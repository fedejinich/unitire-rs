use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

const DEFAULT_CAPACITY: usize = 256;

#[derive(Debug, Clone)]
struct CacheEntry {
    generation: u64,
    keys: Arc<Vec<Vec<u8>>>,
    packed: Arc<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct StorageIterationCache {
    capacity: usize,
    order: VecDeque<Vec<u8>>,
    entries: HashMap<Vec<u8>, CacheEntry>,
}

impl Default for StorageIterationCache {
    fn default() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }
}

impl StorageIterationCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            order: VecDeque::new(),
            entries: HashMap::new(),
        }
    }

    pub fn get_keys(&self, account_address: &[u8], generation: u64) -> Option<Arc<Vec<Vec<u8>>>> {
        self.entries
            .get(account_address)
            .filter(|entry| entry.generation == generation)
            .map(|entry| Arc::clone(&entry.keys))
    }

    pub fn get_packed(&self, account_address: &[u8], generation: u64) -> Option<Arc<Vec<u8>>> {
        self.entries
            .get(account_address)
            .filter(|entry| entry.generation == generation)
            .map(|entry| Arc::clone(&entry.packed))
    }

    pub fn insert(
        &mut self,
        account_address: Vec<u8>,
        generation: u64,
        keys: Arc<Vec<Vec<u8>>>,
        packed: Arc<Vec<u8>>,
    ) -> (Arc<Vec<Vec<u8>>>, Arc<Vec<u8>>) {
        if self.capacity == 0 {
            return (keys, packed);
        }

        self.touch(&account_address);
        self.entries.insert(
            account_address.clone(),
            CacheEntry {
                generation,
                keys,
                packed,
            },
        );

        while self.entries.len() > self.capacity {
            let Some(evicted) = self.order.pop_front() else {
                break;
            };
            self.entries.remove(&evicted);
        }

        let entry = self
            .entries
            .get(&account_address)
            .expect("inserted cache entry must exist");
        (Arc::clone(&entry.keys), Arc::clone(&entry.packed))
    }

    fn touch(&mut self, account_address: &[u8]) {
        if let Some(position) = self
            .order
            .iter()
            .position(|candidate| candidate.as_slice() == account_address)
        {
            self.order.remove(position);
        }

        self.order.push_back(account_address.to_vec());
    }
}

#[cfg(test)]
mod tests {
    use super::StorageIterationCache;
    use std::sync::Arc;

    #[test]
    fn get_requires_matching_generation() {
        let mut cache = StorageIterationCache::new(16);
        let account = vec![0xaa];
        let keys = Arc::new(vec![vec![0x01], vec![0x02]]);
        let packed = Arc::new(vec![0xfd, 0x01]);
        cache.insert(account.clone(), 3, Arc::clone(&keys), Arc::clone(&packed));

        let hit = cache
            .get_keys(&account, 3)
            .expect("cache entry should be available");
        assert_eq!(hit.as_ref(), keys.as_ref());
        assert!(cache.get_keys(&account, 4).is_none());
        assert_eq!(
            cache
                .get_packed(&account, 3)
                .expect("packed payload should exist")
                .as_ref(),
            packed.as_ref()
        );
    }

    #[test]
    fn evicts_oldest_account_when_capacity_is_exceeded() {
        let mut cache = StorageIterationCache::new(2);
        cache.insert(
            vec![0x01],
            0,
            Arc::new(vec![vec![0xa1]]),
            Arc::new(vec![0x01]),
        );
        cache.insert(
            vec![0x02],
            0,
            Arc::new(vec![vec![0xa2]]),
            Arc::new(vec![0x02]),
        );
        cache.insert(
            vec![0x03],
            0,
            Arc::new(vec![vec![0xa3]]),
            Arc::new(vec![0x03]),
        );

        assert!(cache.get_keys(&[0x01], 0).is_none());
        assert!(cache.get_keys(&[0x02], 0).is_some());
        assert!(cache.get_keys(&[0x03], 0).is_some());
    }

    #[test]
    fn updating_an_existing_account_refreshes_eviction_order() {
        let mut cache = StorageIterationCache::new(2);
        cache.insert(
            vec![0x01],
            0,
            Arc::new(vec![vec![0xa1]]),
            Arc::new(vec![0x01]),
        );
        cache.insert(
            vec![0x02],
            0,
            Arc::new(vec![vec![0xa2]]),
            Arc::new(vec![0x02]),
        );
        cache.insert(
            vec![0x01],
            1,
            Arc::new(vec![vec![0xb1]]),
            Arc::new(vec![0x11]),
        );
        cache.insert(
            vec![0x03],
            1,
            Arc::new(vec![vec![0xc3]]),
            Arc::new(vec![0x03]),
        );

        assert!(cache.get_keys(&[0x02], 1).is_none());
        assert_eq!(
            cache
                .get_keys(&[0x01], 1)
                .expect("entry should exist")
                .as_ref(),
            &vec![vec![0xb1]]
        );
        assert_eq!(
            cache
                .get_packed(&[0x01], 1)
                .expect("entry should exist")
                .as_ref(),
            &vec![0x11]
        );
    }
}
