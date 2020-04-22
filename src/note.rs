use crate::math;
use crate::pitch::{Pitch, Pitched};
use crate::tuning::ConcertPitch;
use crate::{key::PianoKey, ratio::Ratio};
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;

pub const A4_NOTE: Note = Note { midi_number: 69 };

/// A musical note encapsulating a clearly defined pitch.
///
/// The pitch can be derived using the [`Pitched`] impl on the [`Note`] type itself, assuming
/// standard 440&nbsp;Hz tuning, or on [`NoteAtConcertPitch`], given a specific concert pitch.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Note {
    midi_number: i32,
}

pub type NoteAtConcertPitch = (Note, ConcertPitch);

impl Note {
    pub fn from_midi_number(midi_number: i32) -> Self {
        Self { midi_number }
    }

    /// Creates a [`Note`] instance from a [`PianoKey`] assuming standard 12-EDO tuning.
    pub fn from_piano_key(piano_key: PianoKey) -> Self {
        Self::from_midi_number(piano_key.midi_number())
    }

    pub fn midi_number(self) -> i32 {
        self.midi_number
    }

    /// Retrieves the associated [`PianoKey`] assuming standard 12-EDO tuning.
    pub fn as_piano_key(self) -> PianoKey {
        PianoKey::from_midi_number(self.midi_number())
    }

    /// Creates a [`NoteAtConcertPitch`] instance with `self` sounding at a different pitch.
    pub fn at_pitch(self, pitched: impl Pitched) -> NoteAtConcertPitch {
        (self, ConcertPitch::from_note_and_pitch(self, pitched))
    }

    /// Convenience function creating a [`NoteAtConcertPitch`] instance.
    pub fn at_concert_pitch(self, concert_pitch: ConcertPitch) -> NoteAtConcertPitch {
        (self, concert_pitch)
    }

    /// Counts the number of semitones [left inclusive, right exclusive) between `self` and `other`.
    pub fn num_semitones_before(self, other: Note) -> i32 {
        other.midi_number - self.midi_number
    }
}

impl Pitched for Note {
    fn pitch(self) -> Pitch {
        (self, ConcertPitch::default()).pitch()
    }
}

impl Display for Note {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let (octave, semitone) = math::div_mod_i32(self.midi_number, 12);

        let note_name = match semitone {
            0 => "C",
            1 => "C#/Db",
            2 => "D",
            3 => "D#/Eb",
            4 => "E",
            5 => "F",
            6 => "F#/Gb",
            7 => "G",
            8 => "G#/Ab",
            9 => "A",
            10 => "A#/Bb",
            11 => "B",
            other => unreachable!("value was {}", other),
        };

        let width = f.width().unwrap_or(0);
        write!(f, "{:width$} {}", note_name, octave - 1, width = width)
    }
}

pub trait PitchedNote: Pitched {
    fn note(self) -> Note;

    fn alter_pitch_by(self, delta: Ratio) -> NoteAtConcertPitch {
        let new_concert_pitch =
            ConcertPitch::from_note_and_pitch(self.note(), self.pitch() * delta);
        (self.note(), new_concert_pitch)
    }
}

impl PitchedNote for Note {
    fn note(self) -> Note {
        self
    }
}

impl PitchedNote for NoteAtConcertPitch {
    fn note(self) -> Note {
        self.0
    }
}
