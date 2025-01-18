use io::Read;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, io};
use tune::{key::PianoKey, pitch::Pitch, tuning::KeyboardMapping};

use crate::{error::ResultExt, CliError, CliResult};

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum TuneDto {
    Scale(ScaleDto),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ScaleDto {
    pub root_key_midi_number: i32,
    pub root_pitch_in_hz: Option<f64>,
    pub items: Vec<ScaleItemDto>,
}

impl ScaleDto {
    pub fn read(mut input: impl Read) -> CliResult<ScaleDto> {
        let mut buffer = Vec::new();
        input.read_to_end(&mut buffer).unwrap();

        // serde_yml::from_reader seems to be broken which is why we collect the data to a buffer first.
        serde_yml::from_slice(&buffer)
            .handle_error::<CliError>("Could not parse scale file")
            .map(|TuneDto::Scale(scale): _| scale)
    }

    pub fn keys(&self) -> Vec<PianoKey> {
        self.items
            .iter()
            .map(|item| PianoKey::from_midi_number(item.key_midi_number))
            .collect()
    }

    pub fn to_keyboard_mapping(&self) -> impl KeyboardMapping<PianoKey> {
        DtoKeyboardMapping {
            key_map: self
                .items
                .iter()
                .map(|item| {
                    (
                        PianoKey::from_midi_number(item.key_midi_number),
                        Pitch::from_hz(item.pitch_in_hz),
                    )
                })
                .collect(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ScaleItemDto {
    pub key_midi_number: i32,
    pub pitch_in_hz: f64,
}

struct DtoKeyboardMapping {
    key_map: HashMap<PianoKey, Pitch>,
}

impl KeyboardMapping<PianoKey> for DtoKeyboardMapping {
    fn maybe_pitch_of(&self, key: PianoKey) -> Option<tune::pitch::Pitch> {
        self.key_map.get(&key).copied()
    }
}
