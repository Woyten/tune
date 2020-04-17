use crate::{
    note,
    note::Note,
    pitch,
    pitch::{Pitch, Pitched},
    ratio::Ratio,
};

/// A [`Tuning`] maps notes or, in general, addresses of type `N` to a [`Pitch`].
pub trait Tuning<N> {
    fn pitch_of(self, note_or_address: N) -> Pitch;
}

impl<N, T: Tuning<N>> Pitched for (N, T)
where
    (N, T): Copy,
{
    fn pitch(self) -> Pitch {
        self.1.pitch_of(self.0)
    }
}

#[derive(Copy, Clone, Debug)]
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
                * Ratio::from_semitones(f64::from(note.num_semitones_before(note::A4_NOTE))),
        }
    }

    pub fn a4_pitch(self) -> Pitch {
        self.a4_pitch
    }
}

impl Default for ConcertPitch {
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::note;
    /// # use tune::tuning::ConcertPitch;
    /// assert_approx_eq!(ConcertPitch::default().a4_pitch().as_hz(), 440.0);
    /// ```
    fn default() -> Self {
        Self::from_a4_pitch(pitch::A4_PITCH)
    }
}

impl Tuning<Note> for ConcertPitch {
    fn pitch_of(self, note: Note) -> Pitch {
        self.a4_pitch * Ratio::from_semitones(note::A4_NOTE.num_semitones_before(note))
    }
}
