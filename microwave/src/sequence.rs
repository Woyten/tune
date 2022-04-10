use std::{fs::File, time::Duration};

use serde::{Deserialize, Serialize};

pub struct Recorder<A> {
    last_timestamp_microsecs: Option<u64>,
    sequence: Vec<SequencerEvent<A>>,
}

impl<A> Recorder<A> {
    pub fn new() -> Self {
        Self {
            last_timestamp_microsecs: None,
            sequence: Vec::new(),
        }
    }

    pub fn record(&mut self, timestamp_microsecs: u64, action: A) {
        if let Some(last_timestamp_microsecs) = self.last_timestamp_microsecs {
            self.sequence
                .push(SequencerEvent::Pause(Duration::from_micros(
                    timestamp_microsecs - last_timestamp_microsecs,
                )));
        }
        self.last_timestamp_microsecs = Some(timestamp_microsecs);
        self.sequence.push(SequencerEvent::Action(action));
    }

    pub fn get_recording(&self) -> Recording<A>
    where
        A: Clone,
    {
        Recording {
            sequence: self.sequence.clone(),
        }
    }
}

#[derive(Clone)]
pub struct Recording<A> {
    sequence: Vec<SequencerEvent<A>>,
}

impl<A> Recording<A> {
    pub fn load(song_location: &str) -> Self
    where
        for<'a> A: Deserialize<'a>,
    {
        println!("[INFO] Loading song `{song_location}`");
        let file = File::open(song_location).unwrap();
        let sequence = serde_yaml::from_reader(file).unwrap();
        println!("[INFO] Song loaded");

        Self { sequence }
    }

    pub fn save(&self, song_location: &str)
    where
        A: Serialize,
    {
        println!("[INFO] Saving song `{song_location}`");
        let file = File::create(song_location).unwrap();
        serde_yaml::to_writer(file, &self.sequence).unwrap();
        println!("[INFO] Song saved");
    }

    pub async fn play(&self, mut consumer: impl FnMut(&A)) {
        for event in &self.sequence {
            match event {
                SequencerEvent::Action(action) => consumer(action),
                SequencerEvent::Pause(duration) => async_std::task::sleep(*duration).await,
            }
        }
    }
}

#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
enum SequencerEvent<A> {
    Action(A),
    Pause(Duration),
}
