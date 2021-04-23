//! Types for working with musical tunings.

#![allow(clippy::wrong_self_convention)] // Should be fixed in a major release

use crate::{
    note::{Note, NoteLetter},
    pitch::{Pitch, Pitched, Ratio},
};

/// A [`Tuning`] maps keys or notes of type `K` to a [`Pitch`] or vice versa.
pub trait Tuning<K> {
    /// Returns the [`Pitch`] of the given key or note `K` in the current [`Tuning`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::note::NoteLetter;
    /// # use tune::tuning::ConcertPitch;
    /// use tune::tuning::Tuning;
    ///
    /// let standard_tuning = ConcertPitch::default();
    /// let a5 = NoteLetter::A.in_octave(5);
    ///
    /// assert_approx_eq!(standard_tuning.pitch_of(a5).as_hz(), 880.0);
    /// ```
    fn pitch_of(&self, key: K) -> Pitch;

    /// Finds a closest key or note [`Approximation`] for the given [`Pitch`] in the current [`Tuning`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::note::NoteLetter;
    /// # use tune::pitch::Pitch;
    /// # use tune::pitch::Ratio;
    /// # use tune::tuning::ConcertPitch;
    /// use tune::tuning::Tuning;
    ///
    /// let standard_tuning = ConcertPitch::default();
    /// let a5 = NoteLetter::A.in_octave(5);
    /// let detuned_a5_pitch = Pitch::of(a5) * Ratio::from_cents(10.0);
    ///
    /// let approximation = standard_tuning.find_by_pitch(detuned_a5_pitch);
    /// assert_eq!(approximation.approx_value, a5);
    /// assert_approx_eq!(approximation.deviation.as_cents(), 10.0);
    /// ```
    fn find_by_pitch(&self, pitch: Pitch) -> Approximation<K>;

    /// Wraps `self` in a type adapter s.t. it can be used in functions that are generic over [`KeyboardMapping<K>`].
    fn as_linear_mapping(self) -> LinearMapping<Self>
    where
        Self: Sized,
    {
        LinearMapping { inner: self }
    }
}

/// `impl` forwarding for references.
impl<K, T: Tuning<K> + ?Sized> Tuning<K> for &T {
    fn pitch_of(&self, key: K) -> Pitch {
        T::pitch_of(self, key)
    }

    fn find_by_pitch(&self, pitch: Pitch) -> Approximation<K> {
        T::find_by_pitch(self, pitch)
    }
}

/// A key or note `K` paired with an appropriate [`Tuning<K>`] is considered [`Pitched`].
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
/// let concert_pitch = ConcertPitch::from_a4_pitch(Pitch::from_hz(432.0));
/// assert_approx_eq!((NoteLetter::A.in_octave(5), concert_pitch).pitch().as_hz(), 864.0);
/// ```
impl<K: Copy, T: Tuning<K> + ?Sized> Pitched for (K, T) {
    fn pitch(&self) -> Pitch {
        self.1.pitch_of(self.0)
    }
}

/// A [`Scale`] is a tuning whose [`Pitch`]es can be accessed in a sorted manner.
///
/// Accessing pitches in order can be important, e.g. when handling pitches in a certain frequency window.
pub trait Scale {
    /// Returns the [`Pitch`] at the given scale degree in the current [`Scale`].
    fn sorted_pitch_of(&self, degree: i32) -> Pitch;

    /// Finds a closest scale degree [`Approximation`] for the given [`Pitch`] in the current [`Scale`].
    fn find_by_pitch_sorted(&self, pitch: Pitch) -> Approximation<i32>;

    /// Wraps `self` in a type adapter s.t. it can be used in functions that are generic over [`Tuning<i32>`].
    fn as_sorted_tuning(self) -> SortedTuning<Self>
    where
        Self: Sized,
    {
        SortedTuning { inner: self }
    }
}

/// `impl` forwarding for references.
impl<S: Scale + ?Sized> Scale for &S {
    fn sorted_pitch_of(&self, degree: i32) -> Pitch {
        S::sorted_pitch_of(self, degree)
    }

    fn find_by_pitch_sorted(&self, pitch: Pitch) -> Approximation<i32> {
        S::find_by_pitch_sorted(self, pitch)
    }
}

/// Type adapter returned by [`Scale::as_sorted_tuning`].
pub struct SortedTuning<S> {
    inner: S,
}

impl<S: Scale> Tuning<i32> for SortedTuning<S> {
    fn pitch_of(&self, key: i32) -> Pitch {
        self.inner.sorted_pitch_of(key)
    }

    fn find_by_pitch(&self, pitch: Pitch) -> Approximation<i32> {
        self.inner.find_by_pitch_sorted(pitch)
    }
}

/// Similar to a [`Tuning`] but not designed to be surjective or injecive.
///
/// An inversion operation is not provided.
/// In return, zero or multiple keys/notes can point to a [`Pitch`].
pub trait KeyboardMapping<K> {
    /// Returns the [`Pitch`] of the provided key or note.
    fn maybe_pitch_of(&self, key: K) -> Option<Pitch>;
}

/// `impl` forwarding for references.
impl<K, T: KeyboardMapping<K> + ?Sized> KeyboardMapping<K> for &T {
    fn maybe_pitch_of(&self, key: K) -> Option<Pitch> {
        T::maybe_pitch_of(self, key)
    }
}

/// Type adapter returned by [`Tuning::as_linear_mapping`].
pub struct LinearMapping<T> {
    inner: T,
}

impl<K, T: Tuning<K>> KeyboardMapping<K> for LinearMapping<T> {
    fn maybe_pitch_of(&self, key: K) -> Option<Pitch> {
        Some(self.inner.pitch_of(key))
    }
}

/// The result of a find operation on [`Scale`]s or [`Tuning`]s.
#[derive(Copy, Clone, Debug)]
pub struct Approximation<K> {
    /// The value to find.
    pub approx_value: K,

    /// The deviation from the ideal value.
    pub deviation: Ratio,
}

/// A [`ConcertPitch`] enables [`Note`]s to sound at a [`Pitch`] different to what would be expected in 440&nbsp;Hz standard tuning.
///
/// To access the full potential of [`ConcertPitch`]es have a look at the [`Tuning`] and [`PitchedNote`](crate::note::PitchedNote) traits.
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct ConcertPitch {
    a4_pitch: Pitch,
}

impl ConcertPitch {
    /// Creates a [`ConcertPitch`] with the given `a4_pitch`.
    pub fn from_a4_pitch(a4_pitch: impl Pitched) -> Self {
        Self {
            a4_pitch: a4_pitch.pitch(),
        }
    }

    /// Creates a [`ConcertPitch`] from the given `note` and `pitched` value.
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::note::NoteLetter;
    /// # use tune::tuning::ConcertPitch;
    /// # use tune::pitch::Pitch;
    /// let c4 = NoteLetter::C.in_octave(4);
    /// let fixed_c4_tuning = ConcertPitch::from_note_and_pitch(c4, Pitch::from_hz(260.0));
    /// assert_approx_eq!(fixed_c4_tuning.a4_pitch().as_hz(), 437.266136);
    /// ```
    pub fn from_note_and_pitch(note: Note, pitched: impl Pitched) -> Self {
        Self {
            a4_pitch: pitched.pitch()
                * Ratio::from_semitones(f64::from(
                    note.num_semitones_before(NoteLetter::A.in_octave(4)),
                )),
        }
    }

    /// Returns the [`Pitch`] of A4.
    pub fn a4_pitch(self) -> Pitch {
        self.a4_pitch
    }
}

/// The default [`ConcertPitch`] is A4 sounding at 440&nbsp;Hz.
///
/// # Examples
///
/// ```
/// # use assert_approx_eq::assert_approx_eq;
/// # use tune::tuning::ConcertPitch;
/// assert_approx_eq!(ConcertPitch::default().a4_pitch().as_hz(), 440.0);
/// ```
impl Default for ConcertPitch {
    fn default() -> Self {
        Self::from_a4_pitch(Pitch::from_hz(440.0))
    }
}

/// A [`ConcertPitch`] maps [`Note`]s to [`Pitch`]es and is considered a [`Tuning`].
///
/// # Examples
///
/// ```rust
/// # use assert_approx_eq::assert_approx_eq;
/// # use tune::note::NoteLetter;
/// # use tune::tuning::ConcertPitch;
/// # use tune::pitch::Pitch;
/// use tune::tuning::Tuning;
///
/// let c4 = NoteLetter::C.in_octave(4);
/// let a4 = NoteLetter::A.in_octave(4);
///
/// let standard_tuning = ConcertPitch::default();
/// assert_approx_eq!(standard_tuning.pitch_of(c4).as_hz(), 261.625565);
/// assert_approx_eq!(standard_tuning.pitch_of(a4).as_hz(), 440.0);
///
/// let healing_tuning = ConcertPitch::from_a4_pitch(Pitch::from_hz(432.0));
/// assert_approx_eq!(healing_tuning.pitch_of(c4).as_hz(), 256.868737);
/// assert_approx_eq!(healing_tuning.pitch_of(a4).as_hz(), 432.0);
/// ```
impl Tuning<Note> for ConcertPitch {
    fn pitch_of(&self, note: Note) -> Pitch {
        self.a4_pitch * Ratio::from_semitones(NoteLetter::A.in_octave(4).num_semitones_before(note))
    }

    fn find_by_pitch(&self, pitch: Pitch) -> Approximation<Note> {
        let semitones_above_a4 = Ratio::between_pitches(self.a4_pitch, pitch).as_semitones();
        let round_to_lower_step = Ratio::from_float(1.000001);
        let approx_semitones_above_a4 =
            (semitones_above_a4 - round_to_lower_step.as_semitones()).round();

        Approximation {
            approx_value: Note::from_midi_number(
                approx_semitones_above_a4 as i32 + NoteLetter::A.in_octave(4).midi_number(),
            ),
            deviation: Ratio::from_semitones(semitones_above_a4 - approx_semitones_above_a4),
        }
    }
}

/// Convenience implementation enabling to write `()` instead of [`ConcertPitch::default()`].
///
/// # Examples
///
/// ```
/// # use tune::note::Note;
/// # use tune::pitch::Pitch;
/// use tune::pitch::Pitched;
///
/// assert_eq!(Pitch::from_hz(880.0).find_in_tuning(()).approx_value, Note::from_midi_number(81));
/// ```
impl Tuning<Note> for () {
    fn pitch_of(&self, note: Note) -> Pitch {
        ConcertPitch::default().pitch_of(note)
    }

    fn find_by_pitch(&self, pitch: Pitch) -> Approximation<Note> {
        ConcertPitch::default().find_by_pitch(pitch)
    }
}
