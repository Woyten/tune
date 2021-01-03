#![allow(unused)] // TODO: Remove

use std::{
    cmp::{Ordering, Reverse},
    collections::BinaryHeap,
};

use crate::model::EventId;

enum EventSpec {
    Note {
        //waveform: f64,
        note: i32,
        num_beats_to_hold: f64,
    },
    Wait {
        num_beats_to_wait: f64,
    },
}

pub struct Sequence {
    beats_per_s: f64,
    events: Vec<EventSpec>,
}

impl Sequence {
    pub fn to_sequence(&self) -> BinaryHeap<Reverse<Event>> {
        let mut curr_beat = 0.0;
        let mut curr_id = 0;
        let mut result = BinaryHeap::new();

        for event in &self.events {
            match *event {
                EventSpec::Wait { num_beats_to_wait } => {
                    curr_beat += num_beats_to_wait;
                }
                EventSpec::Note {
                    note,
                    num_beats_to_hold,
                } => {
                    result.push(Reverse(Event {
                        timestamp: curr_beat,
                        event_type: EventType::NoteOn { id: curr_id, note },
                    }));
                    result.push(Reverse(Event {
                        timestamp: curr_beat + num_beats_to_hold,
                        event_type: EventType::NoteOff { id: curr_id, note },
                    }));
                    curr_id += 1;
                }
            }
        }

        result
    }
}

#[derive(Copy, Clone, PartialEq, PartialOrd)]
pub struct Event {
    timestamp: f64,
    event_type: EventType,
}

impl Eq for Event {}

impl Ord for Event {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

#[derive(Copy, Clone, Eq, PartialEq, PartialOrd, Ord)]
enum EventType {
    NoteOn { id: usize, note: i32 },
    NoteOff { id: usize, note: i32 },
}
