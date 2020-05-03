/// A physical or logical key on a real or virtual instrument without any notion of a pitch.
///
/// It does *not* represent a musical key, like in "F&nbsp;minor", which is why this struct is called [`PianoKey`].
#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PianoKey {
    midi_number: i32,
}

impl PianoKey {
    pub fn from_midi_number(midi_number: i32) -> Self {
        Self { midi_number }
    }

    pub fn midi_number(self) -> i32 {
        self.midi_number
    }

    /// Counts the number of keys [left inclusive, right exclusive) between `self` and `other`.
    pub fn num_keys_before(self, other: PianoKey) -> i32 {
        other.midi_number - self.midi_number
    }
}
