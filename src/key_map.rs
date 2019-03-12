use crate::note;
use crate::note::Note;
use crate::pitch::ReferencePitch;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;

#[derive(Clone, Debug)]
pub struct KeyMap {
    pub ref_pitch: ReferencePitch,
    pub root_note: Note,
}

impl KeyMap {
    pub fn as_kbm(&self) -> FormattedKeyMap<'_> {
        FormattedKeyMap(self)
    }
}

impl Default for KeyMap {
    fn default() -> Self {
        KeyMap {
            ref_pitch: ReferencePitch::from_note(note::A5_NOTE),
            root_note: note::A5_NOTE,
        }
    }
}

pub struct FormattedKeyMap<'a>(&'a KeyMap);

impl<'a> Display for FormattedKeyMap<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        writeln!(f, "1")?;
        writeln!(f, "0")?;
        writeln!(f, "127")?;
        writeln!(f, "{}", self.0.root_note.midi_number())?;
        writeln!(f, "{}", self.0.ref_pitch.note().midi_number())?;
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
            root_note: Note::from_midi_number(60),
            ref_pitch: ReferencePitch::from_note_and_pitch(note::A5_NOTE, Pitch::from_hz(430.0)),
        };

        assert_eq!(
            key_map.as_kbm().to_string().lines().collect::<Vec<_>>(),
            ["1", "0", "127", "60", "69", "430", "1", "0"]
        )
    }
}
