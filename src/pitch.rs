use crate::math;
use crate::ratio::Ratio;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;

pub(crate) const A5_MIDI_NUMBER: i32 = 69;

#[derive(Copy, Clone, Debug)]
pub struct Pitch {
    freq: f64,
}

impl Pitch {
    pub(crate) fn from_freq(freq: f64) -> Pitch {
        Pitch { freq }
    }

    pub fn describe(self, concert_pitch: ConcertPitch) -> Description {
        let fractional_semitones_above_a5 = self.freq.log2() * 12.0;
        let semitones_above_a5 = fractional_semitones_above_a5.round();

        let approx_midi_number = semitones_above_a5 as i32 + A5_MIDI_NUMBER;
        let (approx_octave, approx_semitone) = math::div_mod_i32(approx_midi_number, 12);
        let approx_note_name = get_note_name(approx_semitone);
        let deviation_in_cents = (fractional_semitones_above_a5 - semitones_above_a5) * 100.0;

        Description {
            freq_in_hz: self.freq * concert_pitch.a5_hz,
            approx_midi_number,
            approx_note_name,
            approx_octave,
            deviation: Ratio::from_cents(deviation_in_cents),
        }
    }
}

fn get_note_name(semitone: u32) -> &'static str {
    match semitone {
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
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Description {
    pub freq_in_hz: f64,
    pub approx_midi_number: i32,
    pub approx_note_name: &'static str,
    pub approx_octave: i32,
    pub deviation: Ratio,
}

impl Display for Description {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(
            f,
            "{:.3} Hz | MIDI {} | {:5} {}",
            self.freq_in_hz, self.approx_midi_number, self.approx_note_name, self.approx_octave,
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
}

impl Default for ConcertPitch {
    fn default() -> Self {
        Self::from_a5_hz(440.0)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn describe_default_concert_pitch() {
        assert_eq!(format_pitch(0.018_581_36), "8.176 Hz | MIDI 0 | C     0");
        assert_eq!(
            format_pitch(0.9),
            "396.000 Hz | MIDI 67 | G     5 | +17.596c"
        );
        assert_eq!(format_pitch(1.0), "440.000 Hz | MIDI 69 | A     5");
        assert_eq!(
            format_pitch(1.1),
            "484.000 Hz | MIDI 71 | B     5 | -34.996c"
        );
        assert_eq!(
            format_pitch(28.508_758),
            "12543.854 Hz | MIDI 127 | G     10"
        );
    }

    fn format_pitch(freq: f64) -> String {
        format!("{}", Pitch::from_freq(freq).describe(Default::default()))
    }
}
