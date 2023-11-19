//! Operations for working with physical or virtual keyboards.

/// A physical or logical key on a real or virtual instrument without any notion of a pitch.
///
/// This struct does *not* represent a musical key, like in "F&nbsp;minor", which is why its name is [`PianoKey`].
#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PianoKey {
    midi_number: i32,
}

impl PianoKey {
    /// Creates a [`PianoKey`] instance from the given MIDI number.
    pub fn from_midi_number(midi_number: impl Into<i32>) -> Self {
        Self {
            midi_number: midi_number.into(),
        }
    }

    /// Returns the MIDI number of this [`PianoKey`].
    pub fn midi_number(self) -> i32 {
        self.midi_number
    }

    /// Returns the MIDI number of this [`PianoKey`] if it is in the valid MIDI range [0..128).
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::key::PianoKey;
    /// assert_eq!(PianoKey::from_midi_number(-1).checked_midi_number(), None);
    /// assert_eq!(PianoKey::from_midi_number(0).checked_midi_number(), Some(0));
    /// assert_eq!(PianoKey::from_midi_number(64).checked_midi_number(), Some(64));
    /// assert_eq!(PianoKey::from_midi_number(127).checked_midi_number(), Some(127));
    /// assert_eq!(PianoKey::from_midi_number(128).checked_midi_number(), None);
    /// ```
    pub fn checked_midi_number(self) -> Option<u8> {
        u8::try_from(self.midi_number)
            .ok()
            .filter(|midi_number| (0..128).contains(midi_number))
    }

    /// Iterates over all [`PianoKey`]s in the range [`self`..`upper_bound`).
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::key::PianoKey;
    /// let midi_key_62 = PianoKey::from_midi_number(62);
    /// let midi_key_67 = PianoKey::from_midi_number(67);
    ///
    /// assert_eq!(
    ///     midi_key_62.keys_before(midi_key_67).collect::<Vec<_>>(),
    ///     (62..67).map(PianoKey::from_midi_number).collect::<Vec<_>>()
    /// );
    /// assert!(midi_key_67.keys_before(midi_key_62).collect::<Vec<_>>().is_empty());
    /// ```
    pub fn keys_before(
        self,
        upper_bound: PianoKey,
    ) -> impl DoubleEndedIterator<Item = PianoKey> + ExactSizeIterator<Item = PianoKey> + 'static
    {
        (self.midi_number..upper_bound.midi_number).map(Self::from_midi_number)
    }

    /// Counts the number of keys [left inclusive, right exclusive) between `self` and `other`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::key::PianoKey;
    /// let midi_key_62 = PianoKey::from_midi_number(62);
    /// let midi_key_67 = PianoKey::from_midi_number(67);
    ///
    /// assert_eq!(midi_key_62.num_keys_before(midi_key_67), 5);
    /// assert_eq!(midi_key_67.num_keys_before(midi_key_62), -5);
    /// ```
    pub fn num_keys_before(self, other: PianoKey) -> i32 {
        other.midi_number - self.midi_number
    }

    /// Returns the key `num_steps` steps after `self`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::key::PianoKey;
    /// let midi_key_62 = PianoKey::from_midi_number(62);
    /// let midi_key_67 = PianoKey::from_midi_number(67);
    ///
    /// assert_eq!(midi_key_62.plus_steps(5), midi_key_67);
    /// assert_eq!(midi_key_67.plus_steps(-5), midi_key_62);
    /// ```
    pub fn plus_steps(self, num_steps: i32) -> PianoKey {
        PianoKey::from_midi_number(self.midi_number + num_steps)
    }
}
