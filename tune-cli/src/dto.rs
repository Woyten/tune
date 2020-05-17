use serde::{Deserialize, Serialize};

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

#[derive(Debug, Serialize, Deserialize)]
pub struct ScaleItemDto {
    pub key_midi_number: i32,
    pub pitch_in_hz: f64,
}
