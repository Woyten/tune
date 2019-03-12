use crate::note;
use crate::note::Note;
use crate::parse;
use crate::ratio::Ratio;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::ops::Mul;
use std::str::FromStr;

pub const A5_PITCH: Pitch = Pitch { hz: 440.0 };

#[derive(Copy, Clone, Debug)]
pub struct Pitch {
    hz: f64,
}

impl Pitch {
    pub fn from_hz(hz: f64) -> Pitch {
        Pitch { hz }
    }

    pub fn as_hz(self) -> f64 {
        self.hz
    }

    pub fn describe(self, concert_pitch: ConcertPitch) -> Description {
        let semitones_above_a5 = Ratio::from_float(self.hz / concert_pitch.a5_hz()).as_semitones();
        let approx_semitones_above_a5 = semitones_above_a5.round();

        Description {
            freq_in_hz: self.hz,
            approx_note: Note::from_midi_number(
                approx_semitones_above_a5 as i32 + note::A5_NOTE.midi_number(),
            ),
            deviation: Ratio::from_semitones(semitones_above_a5 - approx_semitones_above_a5),
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

impl Mul<Ratio> for Pitch {
    type Output = Pitch;

    fn mul(self, rhs: Ratio) -> Self::Output {
        Pitch::from_hz(self.as_hz() * rhs.as_float())
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
    a5_hz: f64,
}

impl ConcertPitch {
    pub fn from_a5_hz(a5_hz: f64) -> ConcertPitch {
        ConcertPitch { a5_hz }
    }

    pub fn a5_hz(self) -> f64 {
        self.a5_hz
    }
}

impl Default for ConcertPitch {
    fn default() -> Self {
        Self::from_a5_hz(A5_PITCH.as_hz())
    }
}

#[derive(Copy, Clone, Debug)]
pub struct ReferencePitch {
    note: Note,
    pitch: Pitch,
}

impl ReferencePitch {
    pub fn from_note(note: Note) -> Self {
        Self::from_note_and_delta(note, Ratio::default())
    }

    pub fn from_note_and_pitch(note: Note, pitch: Pitch) -> Self {
        Self { note, pitch }
    }

    pub fn from_note_and_delta(note: Note, delta: Ratio) -> Self {
        Self {
            note,
            pitch: note.pitch(ConcertPitch::default()) * delta,
        }
    }

    pub fn note(&self) -> Note {
        self.note
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
            let pitch = pitch
                .parse()
                .map_err(|e| format!("Invalid pitch '{}': {}", pitch, e))?;
            Ok(ReferencePitch::from_note_and_pitch(
                Note::from_midi_number(note_number),
                pitch,
            ))
        } else if let [note, delta] = parse::split_balanced(s, '+').as_slice() {
            let note_number = note
                .parse()
                .map_err(|_| format!("Invalid note '{}': Must be an integer", note))?;
            let delta = delta
                .parse()
                .map_err(|e| format!("Invalid delta '{}': {}", delta, e))?;
            Ok(ReferencePitch::from_note_and_delta(
                Note::from_midi_number(note_number),
                delta,
            ))
        } else if let [note, delta] = parse::split_balanced(s, '-').as_slice() {
            let note_number = note
                .parse()
                .map_err(|_| format!("Invalid note '{}': Must be an integer", note))?;
            let delta = delta
                .parse::<Ratio>()
                .map_err(|e| format!("Invalid delta '{}': {}", delta, e))?;
            Ok(ReferencePitch::from_note_and_delta(
                Note::from_midi_number(note_number),
                delta.inv(),
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
        assert_eq!(
            [
                format_pitch(220.0, ConcertPitch::default()),
                format_pitch(330.0, ConcertPitch::default()),
                format_pitch(440.0, ConcertPitch::default()),
                format_pitch(550.0, ConcertPitch::default()),
            ],
            [
                "220.000 Hz | MIDI 57 | A     4",
                "330.000 Hz | MIDI 64 | E     5 | +1.955c",
                "440.000 Hz | MIDI 69 | A     5",
                "550.000 Hz | MIDI 73 | C#/Db 6 | -13.686c",
            ]
        );
    }

    #[test]
    fn describe_in_strange_pitch() {
        assert_eq!(
            [
                format_pitch(220.0, ConcertPitch::from_a5_hz(330.0)),
                format_pitch(330.0, ConcertPitch::from_a5_hz(330.0)),
                format_pitch(440.0, ConcertPitch::from_a5_hz(330.0)),
                format_pitch(550.0, ConcertPitch::from_a5_hz(330.0)),
            ],
            [
                "220.000 Hz | MIDI 62 | D     5 | -1.955c",
                "330.000 Hz | MIDI 69 | A     5",
                "440.000 Hz | MIDI 74 | D     6 | -1.955c",
                "550.000 Hz | MIDI 78 | F#/Gb 6 | -15.641c",
            ]
        );
    }

    fn format_pitch(freq: f64, concert_pitch: ConcertPitch) -> String {
        Pitch::from_hz(freq).describe(concert_pitch).to_string()
    }
}
