use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub enum TuneDto {
    Dump(DumpDto),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DumpDto {
    pub items: Vec<DumpItemDto>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DumpItemDto {
    pub key_midi_number: i32,
    pub scale_degree: i32,
    pub pitch_in_hz: f64,
}
