use crate::note;
use crate::note::Note;
use crate::parse;
use crate::{
    key::PianoKey,
    ratio::Ratio,
    tuning::{Approximation, Tuning},
};
use note::PitchedNote;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::ops::{Div, Mul};
use std::str::FromStr;

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct Pitch {
    hz: f64,
}

impl Pitch {
    pub fn from(pitched: impl Pitched) -> Pitch {
        pitched.pitch()
    }

    pub fn from_hz(hz: f64) -> Pitch {
        Pitch { hz }
    }

    pub fn as_hz(self) -> f64 {
        self.hz
    }

    pub fn find_in<N>(self, approx: impl Tuning<N>) -> Approximation<N> {
        approx.find_by_pitch(self)
    }

    pub fn describe<N>(self, approx: impl Tuning<N>) -> Description<N> {
        let approximation = approx.find_by_pitch(self);

        Description {
            freq_in_hz: self.hz,
            approx_value: approximation.approx_value,
            deviation: approximation.deviation,
        }
    }
}

impl FromStr for Pitch {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.ends_with("Hz") || s.ends_with("hz") {
            let freq = &s[..s.len() - 2];
            let freq = freq
                .parse::<Ratio>()
                .map_err(|e| format!("Invalid frequency: '{}': {}", freq, e))?;
            Ok(Pitch::from_hz(freq.as_float()))
        } else {
            Err("Must end with Hz or hz".to_string())
        }
    }
}

impl Div<Ratio> for Pitch {
    type Output = Pitch;

    fn div(self, rhs: Ratio) -> Self::Output {
        Pitch::from_hz(self.as_hz() / rhs.as_float())
    }
}

impl Mul<Ratio> for Pitch {
    type Output = Pitch;

    fn mul(self, rhs: Ratio) -> Self::Output {
        Pitch::from_hz(self.as_hz() * rhs.as_float())
    }
}

pub trait Pitched: Copy {
    fn pitch(self) -> Pitch;
}

impl Pitched for Pitch {
    fn pitch(self) -> Pitch {
        self
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Description<N> {
    pub freq_in_hz: f64,
    pub approx_value: N,
    pub deviation: Ratio,
}

impl<N: Display> Display for Description<N> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{:.3} Hz | {}", self.freq_in_hz, self.approx_value,)?;

        let deviation_in_cents = self.deviation.as_cents();
        if deviation_in_cents.abs() >= 0.001 {
            write!(f, " | {:+.3}c", deviation_in_cents)?;
        }

        Ok(())
    }
}

#[derive(Copy, Clone, Debug)]
pub struct ReferencePitch {
    key: PianoKey,
    pitch: Pitch,
}

impl ReferencePitch {
    pub fn from_note(note: impl PitchedNote) -> Self {
        Self::from_key_and_pitch(note.note().as_piano_key(), note)
    }

    pub fn from_key_and_pitch(key: PianoKey, pitched: impl Pitched) -> Self {
        Self {
            key,
            pitch: pitched.pitch(),
        }
    }

    pub fn key(&self) -> PianoKey {
        self.key
    }

    pub fn pitch(&self) -> Pitch {
        self.pitch
    }
}

impl FromStr for ReferencePitch {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let [note, pitch] = parse::split_balanced(s, '@').as_slice() {
            let note_number = note
                .parse()
                .map_err(|_| format!("Invalid note '{}': Must be an integer", note))?;
            let pitch: Pitch = pitch
                .parse()
                .map_err(|e| format!("Invalid pitch '{}': {}", pitch, e))?;
            Ok(ReferencePitch::from_key_and_pitch(
                PianoKey::from_midi_number(note_number),
                pitch,
            ))
        } else if let [note, delta] = parse::split_balanced(s, '+').as_slice() {
            let note_number = note
                .parse()
                .map_err(|_| format!("Invalid note '{}': Must be an integer", note))?;
            let delta = delta
                .parse()
                .map_err(|e| format!("Invalid delta '{}': {}", delta, e))?;
            Ok(ReferencePitch::from_note(
                Note::from_midi_number(note_number).alter_pitch_by(delta),
            ))
        } else if let [note, delta] = parse::split_balanced(s, '-').as_slice() {
            let note_number = note
                .parse()
                .map_err(|_| format!("Invalid note '{}': Must be an integer", note))?;
            let delta = delta
                .parse::<Ratio>()
                .map_err(|e| format!("Invalid delta '{}': {}", delta, e))?;
            Ok(ReferencePitch::from_note(
                Note::from_midi_number(note_number).alter_pitch_by(delta.inv()),
            ))
        } else {
            let note_number = s
                .parse()
                .map_err(|_| "Must be an expression of type 69, 69@440Hz or 69+100c".to_string())?;
            Ok(ReferencePitch::from_note(Note::from_midi_number(
                note_number,
            )))
        }
    }
}
