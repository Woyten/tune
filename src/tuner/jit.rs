use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    hash::Hash,
};

use crate::{
    note::Note,
    pitch::{Pitch, Pitched, Ratio},
    tuning::Approximation,
};

use super::{Group, GroupBy, IsErr, TunableSynth};

pub struct JitTuner<K, S> {
    model: JitTuningModel<K>,
    synth: S,
}

impl<K, S: TunableSynth> JitTuner<K, S> {
    /// Starts a new [`JitTuner`] with the given `synth` and `pooling_mode`.
    pub fn start(synth: S, pooling_mode: PoolingMode) -> Self {
        Self {
            model: JitTuningModel::new(synth.num_channels(), synth.group_by(), pooling_mode),
            synth,
        }
    }
}

impl<K: Copy + Eq + Hash, S: TunableSynth> JitTuner<K, S> {
    /// Starts a note with the given `pitch`.
    ///
    /// `key` is used as identifier for currently sounding notes.
    pub fn note_on(&mut self, key: K, pitch: Pitch, attr: S::NoteAttr) -> S::Result {
        match self.model.register_key(key, pitch) {
            RegisterKeyResult::Accepted {
                channel,
                stopped_note,
                started_note,
                detuning,
            } => {
                if let Some(stopped_note) = stopped_note {
                    let result = self.synth.note_off(channel, stopped_note, attr.clone());
                    if result.is_err() {
                        return result;
                    }
                }
                let result = self
                    .synth
                    .notes_detune(channel, &[(started_note, detuning)]);
                if result.is_err() {
                    return result;
                }
                self.synth.note_on(channel, started_note, attr)
            }
            RegisterKeyResult::Rejected => S::Result::ok(),
        }
    }

    /// Stops the note of the given `key`.
    pub fn note_off(&mut self, key: K, attr: S::NoteAttr) -> S::Result {
        match self.model.deregister_key(key) {
            AccessKeyResult::Found {
                channel,
                found_note,
            } => self.synth.note_off(channel, found_note, attr),
            AccessKeyResult::NotFound => S::Result::ok(),
        }
    }

    /// Updates the note of `key` with the given `pitch`.
    pub fn note_pitch(&mut self, key: K, pitch: Pitch) -> S::Result {
        match self.model.access_key(key) {
            AccessKeyResult::Found {
                channel,
                found_note,
            } => {
                let detuning = Ratio::between_pitches(found_note.pitch(), pitch);
                self.synth.notes_detune(channel, &[(found_note, detuning)])
            }
            AccessKeyResult::NotFound => S::Result::ok(),
        }
    }

    /// Sets a polyphonic attribute for the note with the given `key`.
    pub fn note_attr(&mut self, key: K, attr: S::NoteAttr) -> S::Result {
        match self.model.access_key(key) {
            AccessKeyResult::Found {
                channel,
                found_note,
            } => self.synth.note_attr(channel, found_note, attr),
            AccessKeyResult::NotFound => S::Result::ok(),
        }
    }

    /// Sets a channel-global attribute.
    pub fn global_attr(&mut self, attr: S::GlobalAttr) -> S::Result {
        self.synth.global_attr(attr)
    }

    /// Stops the current [`JitTuner`] yielding the consumed [`TunableSynth`] for future reuse.
    pub fn stop(mut self) -> S {
        let active_keys: Vec<_> = self.model.active_keys().collect();

        for key in active_keys {
            self.note_off(key, S::NoteAttr::default());
        }

        self.synth
    }
}

/// A more flexible but also more complex alternative to the [`AotTuningModel`](super::AotTuningModel).
///
/// It allocates channels and yields detunings just-in-time and is, therefore, not dependent on any fixed tuning.
pub struct JitTuningModel<K> {
    num_channels: usize,
    group_by: GroupBy,
    pooling_mode: PoolingMode,
    pools: HashMap<Group, JitPool<K, usize, Note>>,
    groups: HashMap<K, Group>,
}

impl<K> JitTuningModel<K> {
    pub fn new(num_channels: usize, group_by: GroupBy, pooling_mode: PoolingMode) -> Self {
        Self {
            num_channels,
            group_by,
            pooling_mode,
            pools: HashMap::new(),
            groups: HashMap::new(),
        }
    }
}

impl<K: Copy + Eq + Hash> JitTuningModel<K> {
    pub fn register_key(&mut self, key: K, pitch: Pitch) -> RegisterKeyResult {
        let Approximation {
            approx_value,
            deviation,
        } = pitch.find_in_tuning(());

        let group = self.group_by.group(approx_value);

        let pool = self
            .pools
            .entry(group)
            .or_insert_with(|| JitPool::new(self.pooling_mode, 0..self.num_channels));

        match pool.key_pressed(key, approx_value) {
            Some((channel, stopped)) => {
                self.groups.insert(key, group);
                if let Some(stopped) = stopped {
                    self.groups.remove(&stopped.0);
                }
                RegisterKeyResult::Accepted {
                    stopped_note: stopped.map(|(_, note)| note),
                    started_note: approx_value,
                    channel,
                    detuning: deviation,
                }
            }
            None => RegisterKeyResult::Rejected,
        }
    }

    pub fn deregister_key(&mut self, key: K) -> AccessKeyResult {
        let pools = &mut self.pools;
        match self
            .groups
            .get(&key)
            .and_then(|group| pools.get_mut(group))
            .and_then(|pool| pool.key_released(key))
        {
            Some((channel, found_note)) => {
                self.groups.remove(&key);
                AccessKeyResult::Found {
                    channel,
                    found_note,
                }
            }
            None => AccessKeyResult::NotFound,
        }
    }

    pub fn access_key(&self, key: K) -> AccessKeyResult {
        match self
            .groups
            .get(&key)
            .and_then(|group| self.pools.get(group))
            .and_then(|pool| pool.find_key(key))
        {
            Some((channel, found_note)) => AccessKeyResult::Found {
                found_note,
                channel,
            },
            None => AccessKeyResult::NotFound,
        }
    }

    pub fn active_keys(&self) -> impl Iterator<Item = K> + '_ {
        self.pools.values().flat_map(|pool| pool.active_keys())
    }
}

/// Reports the channel, [`Note`] and detuning of a newly registered key.
///
/// If the key cannot be registered [`RegisterKeyResult::Rejected`] is returned.
/// If the new key requires a registered note to be stopped `stopped_note` is [`Option::Some`].

pub enum RegisterKeyResult {
    Accepted {
        channel: usize,
        stopped_note: Option<Note>,
        started_note: Note,
        detuning: Ratio,
    },
    Rejected,
}

/// Reports the channel and [`Note`] of a registered key.
///
/// If the key is not registered [`AccessKeyResult::NotFound`] is returned.
pub enum AccessKeyResult {
    Found { channel: usize, found_note: Note },
    NotFound,
}

struct JitPool<K, C, N> {
    mode: PoolingMode,
    free: VecDeque<C>,
    tuned: BTreeMap<u64, K>, // Insertion order is conserved
    active: HashMap<K, (u64, C, N)>,
    curr_usage_id: u64,
}

/// Defines what to do when the channel pool is full and a new key cannot be registered.
#[derive(Clone, Copy, Debug)]
pub enum PoolingMode {
    Block,
    Stop,
    Ignore,
}

impl<K: Copy + Eq + Hash, C: Copy, N: Copy> JitPool<K, C, N> {
    fn new(mode: PoolingMode, channels: impl IntoIterator<Item = C>) -> Self {
        Self {
            mode,
            free: VecDeque::from_iter(channels),
            tuned: BTreeMap::new(),
            active: HashMap::new(),
            curr_usage_id: 0,
        }
    }

    fn key_pressed(&mut self, key: K, note: N) -> Option<(C, Option<(K, N)>)> {
        if let Some(channel) = self.try_insert(key, note) {
            return Some((channel, None));
        }

        match self.mode {
            PoolingMode::Block => None,
            PoolingMode::Stop => self.find_old_key().map(|(channel, old_key, old_location)| {
                self.key_released(old_key);
                self.try_insert(key, note).unwrap();
                (channel, Some((old_key, old_location)))
            }),
            PoolingMode::Ignore => self.find_old_key().map(|(channel, old_key, _)| {
                self.weaken_key(old_key);
                self.try_insert(key, note).unwrap();
                (channel, None)
            }),
        }
    }

    fn key_released(&mut self, key: K) -> Option<(C, N)> {
        self.active
            .remove(&key)
            .map(|(usage_id, freed_channel, location)| {
                self.free_key(usage_id, freed_channel);
                (freed_channel, location)
            })
    }

    fn find_key(&self, key: K) -> Option<(C, N)> {
        self.active
            .get(&key)
            .map(|&(_, channel, location)| (channel, location))
    }

    fn active_keys(&self) -> impl Iterator<Item = K> + '_ {
        self.active.keys().copied()
    }

    fn try_insert(&mut self, key: K, note: N) -> Option<C> {
        let free_channel = self.free.pop_front()?;
        self.tuned.insert(self.curr_usage_id, key);
        self.active
            .insert(key, (self.curr_usage_id, free_channel, note));
        self.curr_usage_id += 1;
        Some(free_channel)
    }

    fn find_old_key(&mut self) -> Option<(C, K, N)> {
        let key = *self.tuned.values().next()?;
        let &(_, channel, location) = self.active.get(&key)?;
        Some((channel, key, location))
    }

    fn weaken_key(&mut self, key: K) {
        if let Some(&(usage_id, freed_channel, _)) = self.active.get(&key) {
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
        let mut pool = JitPool::new(PoolingMode::Block, 0..3);

        assert_eq!(pool.key_pressed("keyA", "locA"), Some((0, None)));
        assert_eq!(pool.key_pressed("keyB", "locB"), Some((1, None)));
        assert_eq!(pool.key_pressed("keyC", "locC"), Some((2, None)));
        assert_eq!(pool.key_pressed("keyD", "locD"), None);

        assert_eq!(pool.find_key(&"keyA"), Some((0, "locA")));
        assert_eq!(pool.find_key(&"keyB"), Some((1, "locB")));
        assert_eq!(pool.find_key(&"keyC"), Some((2, "locC")));
        assert_eq!(pool.find_key(&"keyD"), None);

        assert_eq!(pool.key_released(&"keyB"), Some((1, "locB")));
        assert_eq!(pool.key_pressed("keyD", "locD"), Some((1, None)));
        assert_eq!(pool.key_pressed("keyE", "locE"), None);

        assert_eq!(pool.find_key(&"keyA"), Some((0, "locA")));
        assert_eq!(pool.find_key(&"keyB"), None);
        assert_eq!(pool.find_key(&"keyC"), Some((2, "locC")));
        assert_eq!(pool.find_key(&"keyD"), Some((1, "locD")));
        assert_eq!(pool.find_key(&"keyE"), None);

        assert_eq!(pool.key_released(&"keyA"), Some((0, "locA")));
        assert_eq!(pool.key_released(&"keyB"), None);
        assert_eq!(pool.key_released(&"keyC"), Some((2, "locC")));
        assert_eq!(pool.key_released(&"keyD"), Some((1, "locD")));
        assert_eq!(pool.key_released(&"keyE"), None);

        assert_eq!(pool.find_key(&"keyA"), None);
        assert_eq!(pool.find_key(&"keyB"), None);
        assert_eq!(pool.find_key(&"keyC"), None);
        assert_eq!(pool.find_key(&"keyD"), None);
        assert_eq!(pool.find_key(&"keyE"), None);
    }

    #[test]
    fn pooling_mode_stop() {
        let mut pool = JitPool::new(PoolingMode::Stop, 0..3);

        assert_eq!(pool.key_pressed("keyA", "locA"), Some((0, None)));
        assert_eq!(pool.key_pressed("keyB", "locB"), Some((1, None)));
        assert_eq!(pool.key_pressed("keyC", "locC"), Some((2, None)));
        assert_eq!(
            pool.key_pressed("keyD", "locD"),
            Some((0, Some(("keyA", "locA"))))
        );

        assert_eq!(pool.find_key(&"keyA"), None);
        assert_eq!(pool.find_key(&"keyB"), Some((1, "locB")));
        assert_eq!(pool.find_key(&"keyC"), Some((2, "locC")));
        assert_eq!(pool.find_key(&"keyD"), Some((0, "locD")));

        assert_eq!(pool.key_released(&"keyB"), Some((1, "locB")));
        assert_eq!(pool.key_pressed("keyD", "locD"), Some((1, None)));
        assert_eq!(
            pool.key_pressed("keyE", "locE"),
            Some((2, Some(("keyC", "locC"))))
        );

        assert_eq!(pool.find_key(&"keyA"), None);
        assert_eq!(pool.find_key(&"keyB"), None);
        assert_eq!(pool.find_key(&"keyC"), None);
        assert_eq!(pool.find_key(&"keyD"), Some((1, "locD")));
        assert_eq!(pool.find_key(&"keyE"), Some((2, "locE")));

        assert_eq!(pool.key_released(&"keyA"), None);
        assert_eq!(pool.key_released(&"keyB"), None);
        assert_eq!(pool.key_released(&"keyC"), None);
        assert_eq!(pool.key_released(&"keyD"), Some((1, "locD")));
        assert_eq!(pool.key_released(&"keyE"), Some((2, "locE")));

        assert_eq!(pool.find_key(&"keyA"), None);
        assert_eq!(pool.find_key(&"keyB"), None);
        assert_eq!(pool.find_key(&"keyC"), None);
        assert_eq!(pool.find_key(&"keyD"), None);
        assert_eq!(pool.find_key(&"keyE"), None);
    }

    #[test]
    fn pooling_mode_ignore() {
        let mut pool = JitPool::new(PoolingMode::Ignore, 0..3);

        assert_eq!(pool.key_pressed("keyA", "locA"), Some((0, None)));
        assert_eq!(pool.key_pressed("keyB", "locB"), Some((1, None)));
        assert_eq!(pool.key_pressed("keyC", "locC"), Some((2, None)));
        assert_eq!(pool.key_pressed("keyD", "locD"), Some((0, None)));

        assert_eq!(pool.find_key(&"keyA"), Some((0, "locA")));
        assert_eq!(pool.find_key(&"keyB"), Some((1, "locB")));
        assert_eq!(pool.find_key(&"keyC"), Some((2, "locC")));
        assert_eq!(pool.find_key(&"keyD"), Some((0, "locD")));

        assert_eq!(pool.key_released(&"keyB"), Some((1, "locB")));
        assert_eq!(pool.key_pressed("keyD", "locD"), Some((1, None)));
        assert_eq!(pool.key_pressed("keyE", "locE"), Some((2, None)));

        assert_eq!(pool.find_key(&"keyA"), Some((0, "locA")));
        assert_eq!(pool.find_key(&"keyB"), None);
        assert_eq!(pool.find_key(&"keyC"), Some((2, "locC")));
        assert_eq!(pool.find_key(&"keyD"), Some((1, "locD")));
        assert_eq!(pool.find_key(&"keyE"), Some((2, "locE")));

        assert_eq!(pool.key_released(&"keyA"), Some((0, "locA")));
        assert_eq!(pool.key_released(&"keyB"), None);
        assert_eq!(pool.key_released(&"keyC"), Some((2, "locC")));
        assert_eq!(pool.key_released(&"keyD"), Some((1, "locD")));
        assert_eq!(pool.key_released(&"keyE"), Some((2, "locE")));

        assert_eq!(pool.find_key(&"keyA"), None);
        assert_eq!(pool.find_key(&"keyB"), None);
        assert_eq!(pool.find_key(&"keyC"), None);
        assert_eq!(pool.find_key(&"keyD"), None);
        assert_eq!(pool.find_key(&"keyE"), None);
    }
}
