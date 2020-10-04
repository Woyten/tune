//! Abstractions for working with notes, letters and octaves.

use crate::math;
use crate::pitch::{Pitch, Pitched};
use crate::tuning::ConcertPitch;
use crate::{key::PianoKey, ratio::Ratio};
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;

/// A musical note encapsulating a clearly defined pitch.
///
/// The pitch can be derived using the [`Pitched`] impl on the [`Note`] type itself, assuming
/// standard 440&nbsp;Hz tuning, or on [`NoteAtConcertPitch`], given a specific concert pitch.
#[derive(Copy, Clone, Debug, Ord, Eq, Hash, PartialEq, PartialOrd)]
pub struct Note {
    midi_number: i32,
}

impl Note {
    pub fn from_midi_number(midi_number: impl Into<i32>) -> Self {
        Self {
            midi_number: midi_number.into(),
        }
    }

    /// Creates a [`Note`] instance given a [`NoteLetter`] and an octave.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::note::HelmholtzOctave;
    /// # use tune::note::Note;
    /// # use tune::note::NoteLetter;
    /// # use tune::note::Octave;
    /// let a4 = Note::from_midi_number(69);
    /// assert_eq!(Note::from_letter_and_octave(NoteLetter::A, 4), a4);
    /// assert_eq!(Note::from_letter_and_octave(NoteLetter::A, Octave::from_octave_number(4)), a4);
    /// assert_eq!(Note::from_letter_and_octave(NoteLetter::A, HelmholtzOctave::OneLined), a4);
    /// ```
    pub fn from_letter_and_octave(note_letter: NoteLetter, octave: impl Into<Octave>) -> Self {
        let semitone = match note_letter {
            NoteLetter::C => 0,
            NoteLetter::Csh => 1,
            NoteLetter::D => 2,
            NoteLetter::Dsh => 3,
            NoteLetter::E => 4,
            NoteLetter::F => 5,
            NoteLetter::Fsh => 6,
            NoteLetter::G => 7,
            NoteLetter::Gsh => 8,
            NoteLetter::A => 9,
            NoteLetter::Ash => 10,
            NoteLetter::B => 11,
        };
        Self::from_midi_number((octave.into().octave_number + 1) * 12 + semitone)
    }

    /// Creates a [`Note`] instance from a [`PianoKey`] assuming standard 12-EDO tuning.
    pub fn from_piano_key(piano_key: PianoKey) -> Self {
        Self::from_midi_number(piano_key.midi_number())
    }

    pub fn midi_number(self) -> i32 {
        self.midi_number
    }

    /// Splits the current note into a [`NoteLetter`] and an [`Octave`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::note::Note;
    /// # use tune::note::NoteLetter;
    /// # use tune::note::Octave;
    /// let a4 = Note::from_midi_number(69);
    /// assert_eq!(a4.letter_and_octave(), (NoteLetter::A, Octave::from_octave_number(4)));
    ///
    /// let midi_root = Note::from_midi_number(0);
    /// assert_eq!(midi_root.letter_and_octave(), (NoteLetter::C, Octave::from_octave_number(-1)));
    /// ```
    pub fn letter_and_octave(self) -> (NoteLetter, Octave) {
        let (midi_octave, semitone) = math::i32_dr_u32(self.midi_number, 12);
        let note_letter = match semitone {
            0 => NoteLetter::C,
            1 => NoteLetter::Csh,
            2 => NoteLetter::D,
            3 => NoteLetter::Dsh,
            4 => NoteLetter::E,
            5 => NoteLetter::F,
            6 => NoteLetter::Fsh,
            7 => NoteLetter::G,
            8 => NoteLetter::Gsh,
            9 => NoteLetter::A,
            10 => NoteLetter::Ash,
            11 => NoteLetter::B,
            other => unreachable!("value was {}", other),
        };
        (note_letter, Octave::from_octave_number(midi_octave - 1))
    }

    /// Retrieves the associated [`PianoKey`] assuming standard 12-EDO tuning.
    pub fn as_piano_key(self) -> PianoKey {
        PianoKey::from_midi_number(self.midi_number())
    }

    /// Creates a [`NoteAtConcertPitch`] instance with `self` sounding at a different pitch.
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::note::NoteLetter;
    /// # use tune::pitch::Pitch;
    /// # use tune::tuning::ConcertPitch;
    /// use tune::pitch::Pitched;
    ///
    /// let c4_at_260_hz = NoteLetter::C.in_octave(4).at_pitch(Pitch::from_hz(260.0));
    /// assert_approx_eq!(c4_at_260_hz.pitch().as_hz(), 260.0);
    ///
    /// let (_note, concert_pitch) = c4_at_260_hz;
    /// assert_approx_eq!(concert_pitch.a4_pitch().as_hz(), 437.266136);
    /// ```
    pub fn at_pitch(self, pitched: impl Pitched) -> NoteAtConcertPitch {
        (self, ConcertPitch::from_note_and_pitch(self, pitched))
    }

    /// Convenience function creating a [`NoteAtConcertPitch`] instance.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::note::NoteLetter;
    /// # use tune::pitch::Pitch;
    /// # use tune::tuning::ConcertPitch;
    /// let a4 = NoteLetter::A.in_octave(4);
    /// let concert_pitch = ConcertPitch::from_a4_pitch(Pitch::from_hz(435.0));
    /// assert_eq!(a4.at_concert_pitch(concert_pitch), (a4, concert_pitch));
    /// ```
    pub fn at_concert_pitch(self, concert_pitch: ConcertPitch) -> NoteAtConcertPitch {
        (self, concert_pitch)
    }

    /// Counts the number of semitones [left inclusive, right exclusive) between `self` and `other`.
    pub fn num_semitones_before(self, other: Note) -> i32 {
        other.midi_number - self.midi_number
    }

    /// Retrieves the [`Note`] instance `num_semitones` above `self`.
    pub fn plus_semitones(self, num_semitones: i32) -> Note {
        Note::from_midi_number(self.midi_number() + num_semitones)
    }
}

impl Pitched for Note {
    fn pitch(self) -> Pitch {
        (self, ConcertPitch::default()).pitch()
    }
}

/// [`Note`]s are rendered in scientific pitch notation.
///
/// # Examples
///
/// ```
/// # use tune::note::Note;
/// assert_eq!(Note::from_midi_number(0).to_string(), "C -1");
/// assert_eq!(Note::from_midi_number(69).to_string(), "A 4");
/// assert_eq!(Note::from_midi_number(70).to_string(), "A#/Bb 4");
/// assert_eq!(Note::from_midi_number(71).to_string(), "B 4");
/// assert_eq!(Note::from_midi_number(72).to_string(), "C 5");
/// assert_eq!(Note::from_midi_number(127).to_string(), "G 9");
///
/// // Format flags
/// assert_eq!(format!("{:+}", Note::from_midi_number(70)), "A# 4");
/// assert_eq!(format!("{:-}", Note::from_midi_number(70)), "Bb 4");
/// assert_eq!(format!("{:10}", Note::from_midi_number(70)), "A#/Bb 4   ");
/// assert_eq!(format!("{:<10}", Note::from_midi_number(70)), "A#/Bb 4   ");
/// assert_eq!(format!("{:>10}", Note::from_midi_number(70)), "   A#/Bb 4");
/// ```
impl Display for Note {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let (letter, octave) = self.letter_and_octave();

        let formatted_note = match (f.sign_plus(), f.sign_minus()) {
            (false, false) => format!("{}", letter),
            (true, false) => format!("{:+}", letter),
            (false, true) => format!("{:-}", letter),
            (true, true) => unreachable!("Impossible format string"),
        };

        f.pad(&format!("{} {}", formatted_note, octave.octave_number))
    }
}

/// The speaking name of a note within its octave.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum NoteLetter {
    C,
    Csh,
    D,
    Dsh,
    E,
    F,
    Fsh,
    G,
    Gsh,
    A,
    Ash,
    B,
}

impl NoteLetter {
    /// Shortcut for [`Note::from_letter_and_octave`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::note::Note;
    /// # use tune::note::NoteLetter;
    /// assert_eq!(NoteLetter::C.in_octave(4), Note::from_letter_and_octave(NoteLetter::C, 4));
    /// ```
    pub fn in_octave(self, octave: impl Into<Octave>) -> Note {
        Note::from_letter_and_octave(self, octave)
    }
}

impl Display for NoteLetter {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        enum Sign {
            Sharp,
            Flat,
            Both,
        };

        let sign = match (f.sign_plus(), f.sign_minus()) {
            (false, false) => Sign::Both,
            (true, false) => Sign::Sharp,
            (false, true) => Sign::Flat,
            (true, true) => unreachable!("Impossible format string"),
        };

        let note_name = match (self, sign) {
            (NoteLetter::C, _) => "C",
            (NoteLetter::Csh, Sign::Both) => "C#/Db",
            (NoteLetter::Csh, Sign::Sharp) => "C#",
            (NoteLetter::Csh, Sign::Flat) => "Db",
            (NoteLetter::D, _) => "D",
            (NoteLetter::Dsh, Sign::Both) => "D#/Eb",
            (NoteLetter::Dsh, Sign::Sharp) => "D#",
            (NoteLetter::Dsh, Sign::Flat) => "Eb",
            (NoteLetter::E, _) => "E",
            (NoteLetter::F, _) => "F",
            (NoteLetter::Fsh, Sign::Both) => "F#/Gb",
            (NoteLetter::Fsh, Sign::Sharp) => "F#",
            (NoteLetter::Fsh, Sign::Flat) => "Gb",
            (NoteLetter::G, _) => "G",
            (NoteLetter::Gsh, Sign::Both) => "G#/Ab",
            (NoteLetter::Gsh, Sign::Sharp) => "G#",
            (NoteLetter::Gsh, Sign::Flat) => "Ab",
            (NoteLetter::A, _) => "A",
            (NoteLetter::Ash, Sign::Both) => "A#/Bb",
            (NoteLetter::Ash, Sign::Sharp) => "A#",
            (NoteLetter::Ash, Sign::Flat) => "Bb",
            (NoteLetter::B, _) => "B",
        };

        f.pad(note_name)
    }
}

/// Typed representation of the octave of a note.
#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Octave {
    octave_number: i32,
}

impl Octave {
    pub fn from_octave_number(octave_number: i32) -> Self {
        Self { octave_number }
    }

    pub fn octave_number(self) -> i32 {
        self.octave_number
    }
}

impl From<i32> for Octave {
    fn from(octave_number: i32) -> Self {
        Octave::from_octave_number(octave_number)
    }
}

impl From<HelmholtzOctave> for Octave {
    fn from(helmholtz_octave: HelmholtzOctave) -> Self {
        let octave_number = match helmholtz_octave {
            HelmholtzOctave::SubContra => 0,
            HelmholtzOctave::Contra => 1,
            HelmholtzOctave::Great => 2,
            HelmholtzOctave::Small => 3,
            HelmholtzOctave::OneLined => 4,
            HelmholtzOctave::TwoLined => 5,
            HelmholtzOctave::ThreeLined => 6,
            HelmholtzOctave::FourLined => 7,
            HelmholtzOctave::FiveLined => 8,
            HelmholtzOctave::SixLined => 9,
        };
        Self::from_octave_number(octave_number)
    }
}

/// The speaking name of the octave of a note.
#[derive(Copy, Clone, Debug, Eq, Ord, Hash, PartialEq, PartialOrd)]
pub enum HelmholtzOctave {
    SubContra,
    Contra,
    Great,
    Small,
    OneLined,
    TwoLined,
    ThreeLined,
    FourLined,
    FiveLined,
    SixLined,
}

/// Trait for objects that provide [`Pitch`] and [`Note`] information.
///
/// A [`Note`] has a unique pitch defined by the 440 Hz standard tuning.
/// For a note to sound at a different [`Pitch`] the type alias [`NoteAtConcertPitch`] is used.
pub trait PitchedNote: Pitched {
    /// Retrieves the [`Note`] part of `self`.
    ///
    /// ```
    /// # use tune::note::NoteLetter;
    /// # use tune::pitch::Pitch;
    /// use tune::note::PitchedNote;
    ///
    /// let c4 = NoteLetter::C.in_octave(4);
    /// assert_eq!(c4.note(), c4);
    ///
    /// let c4_altered = c4.at_pitch(Pitch::from_hz(256.0));
    /// assert_eq!(c4_altered.note(), c4);
    /// ```
    fn note(self) -> Note;

    /// Returns a new `PitchedNote` with the same [`Note`] part but a [`Pitch`] altered by `delta`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::note::NoteLetter;
    /// # use tune::ratio::Ratio;
    /// use tune::note::PitchedNote;
    /// use tune::pitch::Pitched;
    ///
    /// let a4 = NoteLetter::A.in_octave(4);
    /// let a4_altered = a4.alter_pitch_by(Ratio::from_float(1.01));
    /// assert_eq!(a4_altered.note(), a4);
    /// assert_approx_eq!(a4_altered.pitch().as_hz(), 444.4);
    /// ```
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

/// Type alias for [`Note`]s that should sound at a [`Pitch`] different from standard 440 Hz tuning.
pub type NoteAtConcertPitch = (Note, ConcertPitch);

impl PitchedNote for NoteAtConcertPitch {
    fn note(self) -> Note {
        self.0
    }
}
