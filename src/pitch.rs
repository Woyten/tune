use crate::note;
use crate::note::Note;
use crate::parse;
use crate::{key::PianoKey, ratio::Ratio};
use note::PitchedNote;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::ops::{Div, Mul};
use std::str::FromStr;

pub const A4_PITCH: Pitch = Pitch { hz: 440.0 };

#[derive(Copy, Clone, Debug)]
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

    pub fn describe(self, concert_pitch: ConcertPitch) -> Description {
        let semitones_above_a4 =
            Ratio::from_float(self.hz / concert_pitch.a4_pitch().as_hz()).as_semitones();
        let approx_semitones_above_a4 = semitones_above_a4.round();

        Description {
            freq_in_hz: self.hz,
            approx_note: Note::from_midi_number(
                approx_semitones_above_a4 as i32 + note::A4_NOTE.midi_number(),
            ),
            deviation: Ratio::from_semitones(semitones_above_a4 - approx_semitones_above_a4),
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
pub struct Description {
    pub freq_in_hz: f64,
    pub approx_note: Note,
    pub deviation: Ratio,
}

impl Display for Description {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(
            f,
            "{:.3} Hz | MIDI {} | {:5}",
            self.freq_in_hz,
            self.approx_note.midi_number(),
            self.approx_note,
        )?;

        let deviation_in_cents = self.deviation.as_cents();
        if deviation_in_cents.abs() >= 0.001 {
            write!(f, " | {:+.3}c", deviation_in_cents)?;
        }

        Ok(())
    }
}

#[derive(Copy, Clone, Debug)]
pub struct ConcertPitch {
    a4_pitch: Pitch,
}

impl ConcertPitch {
    pub fn from_a4_pitch(a4_pitch: impl Pitched) -> Self {
        Self {
            a4_pitch: a4_pitch.pitch(),
        }
    }

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
    fn default() -> Self {
        Self::from_a4_pitch(A4_PITCH)
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn describe_in_default_pitch() {
        let concert_pitch_440 = ConcertPitch::default();
        assert_eq!(
            [
                format_pitch(220.0, concert_pitch_440),
                format_pitch(330.0, concert_pitch_440),
                format_pitch(440.0, concert_pitch_440),
                format_pitch(550.0, concert_pitch_440),
            ],
            [
                "220.000 Hz | MIDI 57 | A     3",
                "330.000 Hz | MIDI 64 | E     4 | +1.955c",
                "440.000 Hz | MIDI 69 | A     4",
                "550.000 Hz | MIDI 73 | C#/Db 5 | -13.686c",
            ]
        );
    }

    #[test]
    fn describe_in_strange_pitch() {
        let concert_pitch_330 = ConcertPitch::from_a4_pitch(Pitch::from_hz(330.0));
        assert_eq!(
            [
                format_pitch(220.0, concert_pitch_330),
                format_pitch(330.0, concert_pitch_330),
                format_pitch(440.0, concert_pitch_330),
                format_pitch(550.0, concert_pitch_330),
            ],
            [
                "220.000 Hz | MIDI 62 | D     4 | -1.955c",
                "330.000 Hz | MIDI 69 | A     4",
                "440.000 Hz | MIDI 74 | D     5 | -1.955c",
                "550.000 Hz | MIDI 78 | F#/Gb 5 | -15.641c",
            ]
        );
    }

    fn format_pitch(freq: f64, concert_pitch: ConcertPitch) -> String {
        Pitch::from_hz(freq).describe(concert_pitch).to_string()
    }
}
