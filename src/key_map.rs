use crate::{key::PianoKey, note, pitch::ReferencePitch};
use note::{NoteLetter, PitchedNote};
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;

#[derive(Clone, Debug)]
pub struct KeyMap {
    pub ref_pitch: ReferencePitch,
    pub root_key: PianoKey,
}

impl KeyMap {
    pub fn root_at(note: impl PitchedNote) -> Self {
        KeyMap {
            ref_pitch: ReferencePitch::from_note(note),
            root_key: note.note().as_piano_key(),
        }
    }

    pub fn root_at_a4() -> Self {
        Self::root_at(NoteLetter::A.in_octave(4))
    }

    pub fn as_kbm(&self) -> FormattedKeyMap<'_> {
        FormattedKeyMap(self)
    }
}

pub struct FormattedKeyMap<'a>(&'a KeyMap);

impl<'a> Display for FormattedKeyMap<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        writeln!(f, "1")?;
        writeln!(f, "0")?;
        writeln!(f, "127")?;
        writeln!(f, "{}", self.0.root_key.midi_number())?;
        writeln!(f, "{}", self.0.ref_pitch.key().midi_number())?;
        writeln!(f, "{}", self.0.ref_pitch.pitch().as_hz())?;
        writeln!(f, "1")?;
        writeln!(f, "0")?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::pitch::Pitch;

    #[test]
    fn format_key_map() {
        let key_map = KeyMap {
            root_key: PianoKey::from_midi_number(60),
            ref_pitch: ReferencePitch::from_key_and_pitch(
                NoteLetter::A.in_octave(4).as_piano_key(),
                Pitch::from_hz(430.0),
            ),
        };

        assert_eq!(
            key_map.as_kbm().to_string().lines().collect::<Vec<_>>(),
            ["1", "0", "127", "60", "69", "430", "1", "0"]
        )
    }
}
