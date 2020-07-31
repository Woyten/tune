use io::Read;
use serde::{Deserialize, Serialize};
use std::io;

#[derive(Debug, Deserialize, Serialize)]
pub enum TuneDto {
    Scale(ScaleDto),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ScaleDto {
    pub root_key_midi_number: i32,
    pub root_pitch_in_hz: f64,
    pub items: Vec<ScaleItemDto>,
}

impl ScaleDto {
    pub fn read(input: impl Read) -> io::Result<ScaleDto> {
        let input: TuneDto = serde_json::from_reader(input)?;

        match input {
            TuneDto::Scale(scale) => Ok(scale),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScaleItemDto {
    pub key_midi_number: i32,
    pub pitch_in_hz: f64,
}
