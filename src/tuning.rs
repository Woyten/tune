use crate::{
    note,
    note::Note,
    pitch::{Pitch, Pitched},
    ratio::Ratio,
};
use note::NoteLetter;

pub trait Scale {
    fn sorted_pitch_of(&self, degree: i32) -> Pitch;

    fn find_by_pitch_sorted(&self, pitch: Pitch) -> Approximation<i32>;
}

/// A [`Tuning`] maps notes or, in general, addresses of type `N` to a [`Pitch`] or vice versa.
pub trait Tuning<N> {
    /// Finds the [`Pitch`] for the given note or address.
    fn pitch_of(&self, note_or_address: N) -> Pitch;

    /// Finds the closest note or address for the given [`Pitch`].
    fn find_by_pitch(&self, pitch: Pitch) -> Approximation<N>;
}

/// A scale degree paired with an appropriate [`Tuning`] is considered [`Pitched`].
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
/// let cp = ConcertPitch::from_a4_pitch(Pitch::from_hz(432.0));
/// assert_approx_eq!((NoteLetter::A.in_octave(5), cp).pitch().as_hz(), 864.0);
/// ```
impl<N, T: Tuning<N>> Pitched for (N, T)
where
    (N, T): Copy,
{
    fn pitch(self) -> Pitch {
        self.1.pitch_of(self.0)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Approximation<N> {
    pub approx_value: N,
    pub deviation: Ratio,
}

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct ConcertPitch {
    a4_pitch: Pitch,
}

/// ```rust
/// # use assert_approx_eq::assert_approx_eq;
/// # use tune::note::Note;
/// # use tune::tuning::ConcertPitch;
/// # use tune::tuning::Tuning;
/// # use tune::pitch::Pitch;
/// let c4 = Note::from_midi_number(60);
/// let a4 = Note::from_midi_number(69);
///
/// let standard_tuning = ConcertPitch::from_a4_pitch(Pitch::from_hz(440.0));
/// assert_approx_eq!(standard_tuning.pitch_of(c4).as_hz(), 261.625565);
/// assert_approx_eq!(standard_tuning.pitch_of(a4).as_hz(), 440.0);
///
/// let healing_tuning = ConcertPitch::from_a4_pitch(Pitch::from_hz(432.0));
/// assert_approx_eq!(healing_tuning.pitch_of(c4).as_hz(), 256.868737);
/// assert_approx_eq!(healing_tuning.pitch_of(a4).as_hz(), 432.0);
/// ```
impl ConcertPitch {
    pub fn from_a4_pitch(a4_pitch: impl Pitched) -> Self {
        Self {
            a4_pitch: a4_pitch.pitch(),
        }
    }

    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::note::Note;
    /// # use tune::tuning::ConcertPitch;
    /// # use tune::tuning::Tuning;
    /// # use tune::pitch::Pitch;
    /// let c4 = Note::from_midi_number(60);
    /// let a4 = Note::from_midi_number(69);
    ///
    /// let fixed_c4_tuning = ConcertPitch::from_note_and_pitch(c4, Pitch::from_hz(260.0));
    /// assert_approx_eq!(fixed_c4_tuning.pitch_of(c4).as_hz(), 260.0);
    /// assert_approx_eq!(fixed_c4_tuning.pitch_of(a4).as_hz(), 437.266136);
    /// ```
    pub fn from_note_and_pitch(note: Note, pitched: impl Pitched) -> Self {
        Self {
            a4_pitch: pitched.pitch()
                * Ratio::from_semitones(f64::from(
                    note.num_semitones_before(NoteLetter::A.in_octave(4)),
                )),
        }
    }

    pub fn a4_pitch(self) -> Pitch {
        self.a4_pitch
    }
}

/// The default [`ConcertPitch`] is A4 sounding at 440 Hz.
///
/// # Examples
///
/// ```
/// # use assert_approx_eq::assert_approx_eq;
/// # use tune::note;
/// # use tune::tuning::ConcertPitch;
/// assert_approx_eq!(ConcertPitch::default().a4_pitch().as_hz(), 440.0);
/// ```
impl Default for ConcertPitch {
    fn default() -> Self {
        Self::from_a4_pitch(Pitch::from_hz(440.0))
    }
}

impl Tuning<Note> for ConcertPitch {
    fn pitch_of(&self, note: Note) -> Pitch {
        self.a4_pitch * Ratio::from_semitones(NoteLetter::A.in_octave(4).num_semitones_before(note))
    }

    fn find_by_pitch(&self, pitch: Pitch) -> Approximation<Note> {
        let semitones_above_a4 = Ratio::between_pitches(self.a4_pitch, pitch).as_semitones();
        let approx_semitones_above_a4 = semitones_above_a4.round();

        Approximation {
            approx_value: Note::from_midi_number(
                approx_semitones_above_a4 as i32 + NoteLetter::A.in_octave(4).midi_number(),
            ),
            deviation: Ratio::from_semitones(semitones_above_a4 - approx_semitones_above_a4),
        }
    }
}

/// Convenience implementation enabling to write `()` instead of [`ConcertPitch`]`::default()`.
///
/// # Examples
///
/// ```
/// # use tune::note::Note;
/// # use tune::pitch::Pitch;
/// assert_eq!(Pitch::from_hz(880.0).find_in(&()).approx_value, Note::from_midi_number(81));
/// ```
impl Tuning<Note> for () {
    fn pitch_of(&self, note_or_address: Note) -> Pitch {
        ConcertPitch::default().pitch_of(note_or_address)
    }

    fn find_by_pitch(&self, pitch: Pitch) -> Approximation<Note> {
        ConcertPitch::default().find_by_pitch(pitch)
    }
}
