use std::{fmt::Debug, hash::Hash, mem, ops::RangeInclusive};

use tune::{
    note::Note,
    pitch::{Pitch, Pitched},
    scala::{KbmRoot, Scl},
    tuner::{AotTuner, JitTuner, PoolingMode, TunableSynth},
    tuning::{Scale, Tuning},
};

use crate::keypress::{IllegalState, KeypressTracker, LiftAction, PlaceAction};

pub struct TunableBackend<K, S> {
    tuner: Tuner<K, S>,
}

impl<K, S: TunableSynth> TunableBackend<K, S> {
    pub fn new(synth: S) -> Self {
        Self {
            tuner: Tuner::Aot {
                aot_tuner: AotTuner::start(synth),
                keypress_tracker: KeypressTracker::new(),
            },
        }
    }
}

enum Tuner<K, S> {
    Destroyed,
    Jit {
        jit_tuner: JitTuner<K, S>,
    },
    Aot {
        aot_tuner: AotTuner<i32, S>,
        keypress_tracker: KeypressTracker<K, i32>,
    },
}

impl<K: Copy + Eq + Hash + Debug + Send, S: TunableSynth> TunableBackend<K, S>
where
    S::Result: Debug,
{
    pub fn set_tuning(&mut self, tuning: (&Scl, KbmRoot)) {
        let synth = self.destroy_tuning();
        let mut aot_tuner = AotTuner::start(synth);

        let range = range(
            tuning,
            Note::from_midi_number(0),
            Note::from_midi_number(127),
        );
        let tuning = Tuning::<i32>::as_linear_mapping(tuning);

        match aot_tuner.set_tuning(tuning, range) {
            Ok(required_channels) => {
                if !aot_tuner.tuned() {
                    log::warn!(
                        "Cannot apply tuning. The tuning requires {required_channels} channels"
                    );
                }
            }
            Err(err) => {
                log::warn!("Cannot apply tuning: {err:?}");
            }
        }

        self.tuner = Tuner::Aot {
            aot_tuner,
            keypress_tracker: KeypressTracker::new(),
        };
    }

    pub fn set_no_tuning(&mut self) {
        let synth = self.destroy_tuning();
        let jit_tuner = JitTuner::start(synth, PoolingMode::Stop);
        self.tuner = Tuner::Jit { jit_tuner };
    }

    pub fn is_tuned(&self) -> bool {
        match &self.tuner {
            Tuner::Destroyed => false,
            Tuner::Jit { .. } => true,
            Tuner::Aot { aot_tuner, .. } => aot_tuner.tuned(),
        }
    }

    pub fn start(&mut self, id: K, degree: i32, pitch: Pitch, velocity: S::NoteAttr) {
        match &mut self.tuner {
            Tuner::Destroyed => {}
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
                    log::warn!("Key with ID {id:?} not lifted before pressed again",);
                }
            },
        }
    }

    pub fn update_pitch(&mut self, id: K, degree: i32, pitch: Pitch, velocity: S::NoteAttr) {
        match &mut self.tuner {
            Tuner::Destroyed => {}
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
            Tuner::Destroyed => {}
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
            Tuner::Destroyed => {}
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
            Tuner::Destroyed => {}
            Tuner::Jit { jit_tuner } => {
                jit_tuner.global_attr(message_type);
            }
            Tuner::Aot { aot_tuner, .. } => {
                aot_tuner.global_attr(message_type);
            }
        }
    }

    pub fn is_aot(&self) -> bool {
        match self.tuner {
            Tuner::Destroyed | Tuner::Jit { .. } => false,
            Tuner::Aot { .. } => true,
        }
    }

    fn destroy_tuning(&mut self) -> S {
        let mut tuner = Tuner::Destroyed;
        mem::swap(&mut tuner, &mut self.tuner);

        match tuner {
            Tuner::Destroyed => unreachable!("Tuning already destroyed"),
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
        }
    }
}

pub fn range(
    tuning: impl Scale,
    lowest_pitch: impl Pitched,
    highest_pitch: impl Pitched,
) -> RangeInclusive<i32> {
    let lowest_degree = tuning
        .find_by_pitch_sorted(lowest_pitch.pitch())
        .approx_value;

    let highest_degree = tuning
        .find_by_pitch_sorted(highest_pitch.pitch())
        .approx_value;

    lowest_degree - 1..=highest_degree + 1
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use assert_approx_eq::assert_approx_eq;
    use tune::{note::NoteLetter, pitch::Ratio, tuner::GroupBy};

    use super::*;

    struct FakeSynth {
        state: Rc<RefCell<CapturedState>>,
    }

    impl TunableSynth for FakeSynth {
        type Result = ();

        type NoteAttr = ();

        type GlobalAttr = ();

        fn num_channels(&self) -> usize {
            8
        }

        fn group_by(&self) -> GroupBy {
            GroupBy::Channel
        }

        fn notes_detune(
            &mut self,
            channel: usize,
            detuned_notes: &[(Note, Ratio)],
        ) -> Self::Result {
            self.state
                .borrow_mut()
                .notes_detunes
                .push((channel, detuned_notes.to_vec()));
        }

        fn note_on(
            &mut self,
            channel: usize,
            started_note: Note,
            _attr: Self::NoteAttr,
        ) -> Self::Result {
            self.state
                .borrow_mut()
                .note_ons
                .push((channel, started_note));
        }

        fn note_off(
            &mut self,
            _channel: usize,
            _stopped_note: Note,
            _attr: Self::NoteAttr,
        ) -> Self::Result {
        }

        fn note_attr(
            &mut self,
            _channel: usize,
            _affected_note: Note,
            _attr: Self::NoteAttr,
        ) -> Self::Result {
        }

        fn global_attr(&mut self, _attr: Self::GlobalAttr) -> Self::Result {}
    }

    #[derive(Default)]
    struct CapturedState {
        notes_detunes: Vec<(usize, Vec<(Note, Ratio)>)>,
        note_ons: Vec<(usize, Note)>,
    }

    #[test]
    fn tunable_backend_conserve_order_of_scale_items() {
        let state = Rc::new(RefCell::new(CapturedState::default()));
        let synth = FakeSynth {
            state: state.clone(),
        };

        let mut backend = TunableBackend::<usize, _>::new(synth);
        let (scl, kbm) = create_non_monotonous_tuning();

        backend.set_tuning((&scl, kbm));
        let fake_pitch = Pitch::from_hz(0.0);
        backend.start(0, 1, fake_pitch, ());
        backend.start(1, 2, fake_pitch, ());
        backend.start(2, 3, fake_pitch, ());
        backend.start(3, 4, fake_pitch, ());
        backend.start(4, 5, fake_pitch, ());

        let state = state.borrow();

        for ((channel, note_detunes), (expected_channel, expected_detune_cents)) in
            state.notes_detunes.iter().zip([(0, 0.0), (1, 50.0)])
        {
            assert_eq!(channel, &expected_channel);
            assert_eq!(note_detunes.len(), 1);
            assert_approx_eq!(note_detunes[0].1.as_cents(), expected_detune_cents);
        }

        assert_eq!(
            state.note_ons,
            [
                (0, Note::from_midi_number(63)),
                (0, Note::from_midi_number(64)),
                (1, Note::from_midi_number(63)),
                (0, Note::from_midi_number(65)),
                (0, Note::from_midi_number(66)),
            ]
        );
    }

    fn create_non_monotonous_tuning() -> (Scl, KbmRoot) {
        let scl = Scl::builder()
            .push_cents(100.0)
            .push_cents(200.0)
            .push_cents(150.0)
            .push_cents(300.0)
            .build()
            .unwrap();
        let kbm_root = KbmRoot::from(NoteLetter::D.in_octave(4));
        (scl, kbm_root)
    }
}
