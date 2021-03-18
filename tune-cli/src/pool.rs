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
        if let Some(channel) = self.try_insert(key, location) {
            return Some((channel, None));
        }

        match self.mode {
            PoolingMode::Block => None,
            PoolingMode::Stop => self.find_old_key().map(|(channel, old_key, old_location)| {
                self.key_released(&old_key);
                self.try_insert(key, location).unwrap();
                (channel, Some(old_location))
            }),
            PoolingMode::Ignore => self.find_old_key().map(|(channel, old_key, _)| {
                self.weaken_key(&old_key);
                self.try_insert(key, location).unwrap();
                (channel, None)
            }),
        }
    }

    pub fn key_released(&mut self, key: &K) -> Option<C> {
        self.active.remove(key).map(|(usage_id, freed_channel, _)| {
            self.free_key(usage_id, freed_channel);
            freed_channel
        })
    }

    pub fn channel_for_key(&self, key: &K) -> Option<C> {
        self.active.get(key).map(|&(_, channel, _)| channel)
    }

    fn try_insert(&mut self, key: K, location: L) -> Option<C> {
        let free_channel = self.free.pop_front()?;
        self.tuned.insert(self.curr_usage_id, key);
        self.active
            .insert(key, (self.curr_usage_id, free_channel, location));
        self.curr_usage_id += 1;
        Some(free_channel)
    }

    fn find_old_key(&mut self) -> Option<(C, K, L)> {
        let key = *self.tuned.values().next()?;
        let &(_, channel, location) = self.active.get(&key)?;
        Some((channel, key, location))
    }

    fn weaken_key(&mut self, key: &K) {
        if let Some(&(usage_id, freed_channel, _)) = self.active.get(key) {
            self.free_key(usage_id, freed_channel);
        }
    }

    fn free_key(&mut self, usage_id: u64, freed_channel: C) {
        if self.tuned.remove(&usage_id).is_some() {
            self.free.push_back(freed_channel);
        }
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
