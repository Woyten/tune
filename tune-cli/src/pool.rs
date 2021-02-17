use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    hash::Hash,
};

pub struct Pool<C, K, L> {
    mode: PoolingMode,
    free: VecDeque<C>,
    tuned: BTreeMap<u64, K>, // Insertion order is conserved
    active: HashMap<K, (u64, C, L)>,
    curr_usage_id: u64,
}

#[derive(Clone, Copy, Debug)]
pub enum PoolingMode {
    Block,
    Stop,
    Ignore,
}

impl<C: Copy, K: Eq + Hash + Copy, L: Copy> Pool<C, K, L> {
    pub fn new(mode: PoolingMode, channels: impl IntoIterator<Item = C>) -> Self {
        Self {
            mode,
            free: channels.into_iter().collect(),
            tuned: BTreeMap::new(),
            active: HashMap::new(),
            curr_usage_id: 0,
        }
    }

    pub fn key_pressed(&mut self, key: K, location: L) -> Option<(C, Option<L>)> {
        let freed_channel = self.insert(key, location);
        if let Some(freed_channel) = freed_channel {
            return Some((freed_channel, None));
        }

        match self.mode {
            PoolingMode::Block => None,
            PoolingMode::Stop => {
                self.find_note_to_steal()
                    .map(|(channel, old_key, old_location)| {
                        self.remove(&old_key);
                        self.insert(key, location).unwrap();
                        (channel, Some(old_location))
                    })
            }
            PoolingMode::Ignore => self.find_note_to_steal().map(|(channel, old_key, _)| {
                self.free(&old_key).unwrap();
                self.insert(key, location).unwrap();
                (channel, None)
            }),
        }
    }

    pub fn key_released(&mut self, key: &K) -> Option<C> {
        let freed_channel = self.channel_for_key(&key);
        self.remove(&key);
        freed_channel
    }

    pub fn channel_for_key(&self, key: &K) -> Option<C> {
        self.active.get(key).map(|&(_, channel, _)| channel)
    }

    fn insert(&mut self, key: K, location: L) -> Option<C> {
        let free_channel = self.free.pop_front()?;
        self.tuned.insert(self.curr_usage_id, key);
        self.active
            .insert(key, (self.curr_usage_id, free_channel, location));
        self.curr_usage_id += 1;
        Some(free_channel)
    }

    fn find_note_to_steal(&mut self) -> Option<(C, K, L)> {
        let key = *self.tuned.values().next()?;
        let &(_, channel, location) = self.active.get(&key)?;
        Some((channel, key, location))
    }

    fn remove(&mut self, key: &K) {
        if let Some((usage_id, freed_channel, _)) = self.active.remove(key) {
            if self.tuned.remove(&usage_id).is_some() {
                self.free.push_back(freed_channel);
            }
        }
    }

    fn free(&mut self, key: &K) -> Option<C> {
        let &(usage_id, freed_channel, _) = self.active.get(key)?;
        self.tuned.remove(&usage_id).map(|_| {
            self.free.push_back(freed_channel);
            freed_channel
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pooling_mode_block() {
        let mut pool = Pool::new(PoolingMode::Block, 0..3);

        assert_eq!(pool.key_pressed("keyA", "locA"), Some((0, None)));
        assert_eq!(pool.key_pressed("keyB", "locB"), Some((1, None)));
        assert_eq!(pool.key_pressed("keyC", "locC"), Some((2, None)));
        assert_eq!(pool.key_pressed("keyD", "locD"), None);

        assert_eq!(pool.channel_for_key(&"keyA"), Some(0));
        assert_eq!(pool.channel_for_key(&"keyB"), Some(1));
        assert_eq!(pool.channel_for_key(&"keyC"), Some(2));
        assert_eq!(pool.channel_for_key(&"keyD"), None);

        assert_eq!(pool.key_released(&"keyB"), Some(1));
        assert_eq!(pool.key_pressed("keyD", "locD"), Some((1, None)));
        assert_eq!(pool.key_pressed("keyE", "locE"), None);

        assert_eq!(pool.channel_for_key(&"keyA"), Some(0));
        assert_eq!(pool.channel_for_key(&"keyB"), None);
        assert_eq!(pool.channel_for_key(&"keyC"), Some(2));
        assert_eq!(pool.channel_for_key(&"keyD"), Some(1));
        assert_eq!(pool.channel_for_key(&"keyE"), None);

        assert_eq!(pool.key_released(&"keyA"), Some(0));
        assert_eq!(pool.key_released(&"keyB"), None);
        assert_eq!(pool.key_released(&"keyC"), Some(2));
        assert_eq!(pool.key_released(&"keyD"), Some(1));
        assert_eq!(pool.key_released(&"keyE"), None);

        assert_eq!(pool.channel_for_key(&"keyA"), None);
        assert_eq!(pool.channel_for_key(&"keyB"), None);
        assert_eq!(pool.channel_for_key(&"keyC"), None);
        assert_eq!(pool.channel_for_key(&"keyD"), None);
        assert_eq!(pool.channel_for_key(&"keyE"), None);
    }

    #[test]
    fn pooling_mode_stop() {
        let mut pool = Pool::new(PoolingMode::Stop, 0..3);

        assert_eq!(pool.key_pressed("keyA", "locA"), Some((0, None)));
        assert_eq!(pool.key_pressed("keyB", "locB"), Some((1, None)));
        assert_eq!(pool.key_pressed("keyC", "locC"), Some((2, None)));
        assert_eq!(pool.key_pressed("keyD", "locD"), Some((0, Some("locA"))));

        assert_eq!(pool.channel_for_key(&"keyA"), None);
        assert_eq!(pool.channel_for_key(&"keyB"), Some(1));
        assert_eq!(pool.channel_for_key(&"keyC"), Some(2));
        assert_eq!(pool.channel_for_key(&"keyD"), Some(0));

        assert_eq!(pool.key_released(&"keyB"), Some(1));
        assert_eq!(pool.key_pressed("keyD", "locD"), Some((1, None)));
        assert_eq!(pool.key_pressed("keyE", "locE"), Some((2, Some("locC"))));

        assert_eq!(pool.channel_for_key(&"keyA"), None);
        assert_eq!(pool.channel_for_key(&"keyB"), None);
        assert_eq!(pool.channel_for_key(&"keyC"), None);
        assert_eq!(pool.channel_for_key(&"keyD"), Some(1));
        assert_eq!(pool.channel_for_key(&"keyE"), Some(2));

        assert_eq!(pool.key_released(&"keyA"), None);
        assert_eq!(pool.key_released(&"keyB"), None);
        assert_eq!(pool.key_released(&"keyC"), None);
        assert_eq!(pool.key_released(&"keyD"), Some(1));
        assert_eq!(pool.key_released(&"keyE"), Some(2));

        assert_eq!(pool.channel_for_key(&"keyA"), None);
        assert_eq!(pool.channel_for_key(&"keyB"), None);
        assert_eq!(pool.channel_for_key(&"keyC"), None);
        assert_eq!(pool.channel_for_key(&"keyD"), None);
        assert_eq!(pool.channel_for_key(&"keyE"), None);
    }

    #[test]
    fn pooling_mode_ignore() {
        let mut pool = Pool::new(PoolingMode::Ignore, 0..3);

        assert_eq!(pool.key_pressed("keyA", "locA"), Some((0, None)));
        assert_eq!(pool.key_pressed("keyB", "locB"), Some((1, None)));
        assert_eq!(pool.key_pressed("keyC", "locC"), Some((2, None)));
        assert_eq!(pool.key_pressed("keyD", "locD"), Some((0, None)));

        assert_eq!(pool.channel_for_key(&"keyA"), Some(0));
        assert_eq!(pool.channel_for_key(&"keyB"), Some(1));
        assert_eq!(pool.channel_for_key(&"keyC"), Some(2));
        assert_eq!(pool.channel_for_key(&"keyD"), Some(0));

        assert_eq!(pool.key_released(&"keyB"), Some(1));
        assert_eq!(pool.key_pressed("keyD", "locD"), Some((1, None)));
        assert_eq!(pool.key_pressed("keyE", "locE"), Some((2, None)));

        assert_eq!(pool.channel_for_key(&"keyA"), Some(0));
        assert_eq!(pool.channel_for_key(&"keyB"), None);
        assert_eq!(pool.channel_for_key(&"keyC"), Some(2));
        assert_eq!(pool.channel_for_key(&"keyD"), Some(1));
        assert_eq!(pool.channel_for_key(&"keyE"), Some(2));

        assert_eq!(pool.key_released(&"keyA"), Some(0));
        assert_eq!(pool.key_released(&"keyB"), None);
        assert_eq!(pool.key_released(&"keyC"), Some(2));
        assert_eq!(pool.key_released(&"keyD"), Some(1));
        assert_eq!(pool.key_released(&"keyE"), Some(2));

        assert_eq!(pool.channel_for_key(&"keyA"), None);
        assert_eq!(pool.channel_for_key(&"keyB"), None);
        assert_eq!(pool.channel_for_key(&"keyC"), None);
        assert_eq!(pool.channel_for_key(&"keyD"), None);
        assert_eq!(pool.channel_for_key(&"keyE"), None);
    }
}
