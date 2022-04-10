use std::{fmt::Debug, hash::Hash, mem};

use tune::{
    note::Note,
    pitch::{Pitch, Pitched},
    scala::{KbmRoot, Scl},
    tuner::{AotTuner, JitTuner, PoolingMode, SetTuningError, TunableSynth},
    tuning::{Scale, Tuning},
};

use crate::keypress::{IllegalState, KeypressTracker, LiftAction, PlaceAction};

pub struct TunableBackend<K, S> {
    tuner: Tuner<K, S>,
}

impl<K, S> TunableBackend<K, S> {
    pub fn new(synth: S) -> Self {
        Self {
            tuner: Tuner::None { synth },
        }
    }
}

enum Tuner<K, S> {
    Destroyed,
    None {
        synth: S,
    },
    Jit {
        jit_tuner: JitTuner<K, S>,
    },
    Aot {
        aot_tuner: AotTuner<i32, S>,
        keypress_tracker: KeypressTracker<K, i32>,
    },
    AotBroken {
        aot_tuner: AotTuner<i32, S>,
    },
}

impl<K: Copy + Eq + Hash + Debug + Send, S: TunableSynth> TunableBackend<K, S>
where
    S::Result: Debug,
{
    pub fn set_tuning(&mut self, tuning: (&Scl, KbmRoot)) {
        let synth = self.destroy_tuning();

        let lowest_key = tuning
            .find_by_pitch_sorted(Note::from_midi_number(-1).pitch())
            .approx_value;

        let highest_key = tuning
            .find_by_pitch_sorted(Note::from_midi_number(128).pitch())
            .approx_value;

        let mut aot_tuner = AotTuner::start(synth);

        let tuning = tuning.as_sorted_tuning().as_linear_mapping();
        let keys = lowest_key..highest_key;

        self.tuner = match aot_tuner.set_tuning(tuning, keys) {
            Ok(_) => Tuner::Aot {
                aot_tuner,
                keypress_tracker: KeypressTracker::new(),
            },
            Err(err) => {
                match err {
                    SetTuningError::TooManyChannelsRequired(required_channels) => {
                        eprintln!("[WARNING] Cannot apply tuning. The tuning requires {required_channels} channels");
                    }
                    SetTuningError::TunableSynthResult(result) => {
                        eprintln!("[WARNING] Cannot apply tuning: {result:?}");
                    }
                }
                // Tuner::None should be used here. However, if None is used, FluidLite will crash for some unknown reason.
                Tuner::AotBroken { aot_tuner }
            }
        };
    }

    pub fn set_no_tuning(&mut self) {
        let synth = self.destroy_tuning();
        let jit_tuner = JitTuner::start(synth, PoolingMode::Stop);
        self.tuner = Tuner::Jit { jit_tuner };
    }

    pub fn is_tuned(&self) -> bool {
        match self.tuner {
            Tuner::Destroyed | Tuner::None { .. } | Tuner::AotBroken { .. } => false,
            Tuner::Jit { .. } | Tuner::Aot { .. } => true,
        }
    }

    pub fn start(&mut self, id: K, degree: i32, pitch: Pitch, velocity: S::NoteAttr) {
        match &mut self.tuner {
            Tuner::Destroyed | Tuner::None { .. } | Tuner::AotBroken { .. } => {}
            Tuner::Jit { jit_tuner } => {
                jit_tuner.note_on(id, pitch, velocity);
            }
            Tuner::Aot {
                keypress_tracker,
                aot_tuner,
            } => match keypress_tracker.place_finger_at(id, degree) {
                Ok(PlaceAction::KeyPressed) => {
                    aot_tuner.note_on(degree, velocity);
                }
                Ok(PlaceAction::KeyAlreadyPressed) => {
                    aot_tuner.note_off(degree, S::NoteAttr::default());
                    aot_tuner.note_on(degree, velocity);
                }
                Err(id) => {
                    eprintln!(
                        "[WARNING] Key with ID {:?} not lifted before pressed again",
                        id,
                    );
                }
            },
        }
    }

    pub fn update_pitch(&mut self, id: K, degree: i32, pitch: Pitch, velocity: S::NoteAttr) {
        match &mut self.tuner {
            Tuner::Destroyed | Tuner::None { .. } | Tuner::AotBroken { .. } => {}
            Tuner::Jit { jit_tuner } => {
                jit_tuner.note_pitch(id, pitch);
            }
            Tuner::Aot {
                keypress_tracker,
                aot_tuner,
            } => match keypress_tracker.move_finger_to(&id, degree) {
                Ok((LiftAction::KeyReleased(released), _)) => {
                    aot_tuner.note_off(released, S::NoteAttr::default());
                    aot_tuner.note_on(degree, velocity);
                }
                Ok((LiftAction::KeyRemainsPressed, PlaceAction::KeyPressed)) => {
                    aot_tuner.note_on(degree, velocity);
                }
                Ok((LiftAction::KeyRemainsPressed, PlaceAction::KeyAlreadyPressed)) => {}
                Err(IllegalState) => {}
            },
        }
    }

    pub fn update_pressure(&mut self, id: K, pressure: S::NoteAttr) {
        match &mut self.tuner {
            Tuner::Destroyed | Tuner::None { .. } | Tuner::AotBroken { .. } => {}
            Tuner::Jit { jit_tuner } => {
                jit_tuner.note_attr(id, pressure);
            }
            Tuner::Aot {
                keypress_tracker,
                aot_tuner,
            } => {
                if let Some(&location) = keypress_tracker.location_of(&id) {
                    aot_tuner.note_attr(location, pressure);
                }
            }
        }
    }

    pub fn stop(&mut self, id: K, velocity: S::NoteAttr) {
        match &mut self.tuner {
            Tuner::Destroyed | Tuner::None { .. } | Tuner::AotBroken { .. } => {}
            Tuner::Jit { jit_tuner } => {
                jit_tuner.note_off(id, velocity);
            }
            Tuner::Aot {
                keypress_tracker,
                aot_tuner,
            } => match keypress_tracker.lift_finger(&id) {
                Ok(LiftAction::KeyReleased(location)) => {
                    aot_tuner.note_off(location, velocity);
                }
                Ok(LiftAction::KeyRemainsPressed) => {}
                Err(IllegalState) => {}
            },
        }
    }

    pub fn send_monophonic_message(&mut self, message_type: S::GlobalAttr) {
        match &mut self.tuner {
            Tuner::Destroyed | Tuner::None { .. } => {}
            Tuner::Jit { jit_tuner } => {
                jit_tuner.global_attr(message_type);
            }
            Tuner::Aot { aot_tuner, .. } | Tuner::AotBroken { aot_tuner, .. } => {
                aot_tuner.global_attr(message_type);
            }
        }
    }

    pub fn is_aot(&self) -> bool {
        match self.tuner {
            Tuner::Destroyed | Tuner::None { .. } | Tuner::Jit { .. } => false,
            Tuner::Aot { .. } | Tuner::AotBroken { .. } => true,
        }
    }

    fn destroy_tuning(&mut self) -> S {
        let mut tuner = Tuner::Destroyed;
        mem::swap(&mut tuner, &mut self.tuner);

        match tuner {
            Tuner::Destroyed => unreachable!("Tuning already destroyed"),
            Tuner::None { synth } => synth,
            Tuner::Jit { jit_tuner } => jit_tuner.stop(),
            Tuner::Aot {
                mut aot_tuner,
                keypress_tracker,
            } => {
                for pressed_key in keypress_tracker.pressed_locations() {
                    aot_tuner.note_off(pressed_key, S::NoteAttr::default());
                }
                aot_tuner.stop()
            }
            Tuner::AotBroken { aot_tuner } => aot_tuner.stop(),
        }
    }
}