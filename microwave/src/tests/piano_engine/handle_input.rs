use tune::key::PianoKey;
use tune::pitch::Pitch;
use tune::pitch::Ratio;

use super::fixture::*;
use crate::control::LiveParameter;
use crate::piano::InputEvent;
use crate::piano::InputLocation;
use crate::piano::TuningMode;
use crate::toggle::Direction;

fn slightly_off(pitch: Pitch) -> Pitch {
    pitch * Ratio::from_cents(5.0)
}

fn key_c4() -> PianoKey {
    PianoKey::from_midi_number(60)
}

fn key_cs4() -> PianoKey {
    PianoKey::from_midi_number(61)
}

fn key_d4() -> PianoKey {
    PianoKey::from_midi_number(62)
}

fn key_e4() -> PianoKey {
    PianoKey::from_midi_number(64)
}

const DEGREE_EDO12_C4: i32 = -2;
const DEGREE_EDO12_D4: i32 = 0;
const DEGREE_EDO12_DS4: i32 = 1;
const DEGREE_EDO12_E4: i32 = 2;
const DEGREE_EDO12_F4: i32 = 3;
const DEGREE_EDO12_A4: i32 = 7;

const DEGREE_HARMONICS_8_8: i32 = 0;
const DEGREE_HARMONICS_9_8: i32 = 1;
const DEGREE_HARMONICS_10_8: i32 = 2;
const DEGREE_HARMONICS_13_8: i32 = 5;

const VELOCITY_PRESS: u8 = 100;
const VELOCITY_RELEASE: u8 = 64;
const VELOCITY_LOW: u8 = 42;

// Generic test cases

#[test]
fn press_release_with_velocity() {
    let mut f = PianoEngineFixture::new();

    // Pressing a key starts the note with the given velocity on current and background backends
    f.when(|e| {
        e.handle_input(InputEvent::Pressed(
            SRC_A,
            InputLocation::Piano(key_d4()),
            VELOCITY_PRESS,
        ))
    })
    .expect(|e| {
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_A),
            (Some(edo_12_pitch(DEGREE_EDO12_D4)), VELOCITY_PRESS),
        );
        e.pressed_keys
            .insert((BACKGROUND_LEGATO_BACKEND, SRC_A), (None, VELOCITY_PRESS));
        e.keys_version = 2;
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_EDO12_D4,
                pitch: edo_12_pitch(DEGREE_EDO12_D4),
                velocity: VELOCITY_PRESS,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_EDO12_D4,
                pitch: edo_12_pitch(DEGREE_EDO12_D4),
                velocity: VELOCITY_PRESS,
            },
        ));
    });

    // Releasing the key stops the note with the given velocity on all backends
    f.when(|e| e.handle_input(InputEvent::Released(SRC_A, VELOCITY_RELEASE)))
        .expect(|e| {
            e.pressed_keys.clear();
            e.keys_version = 4;
            e.expected_calls.push((
                FOREGROUND_LEGATO_BACKEND,
                RecordedCall::Stop {
                    key_id: SRC_A,
                    velocity: VELOCITY_RELEASE,
                },
            ));
            e.expected_calls.push((
                BACKGROUND_LEGATO_BACKEND,
                RecordedCall::Stop {
                    key_id: SRC_A,
                    velocity: VELOCITY_RELEASE,
                },
            ));
        });

    // Releasing an already-released key is suppressed — no stop calls, no version bump
    f.when(|e| e.handle_input(InputEvent::Released(SRC_A, VELOCITY_RELEASE)))
        .expect(|_e| {});
}

#[test]
fn move_ignored_without_legato_updates_with_legato() {
    let mut f = PianoEngineFixture::new();

    // Press a key to prepare for legato move
    f.when(|e| {
        e.handle_input(InputEvent::Pressed(
            SRC_A,
            InputLocation::Pitch(edo_12_pitch(DEGREE_EDO12_D4)),
            VELOCITY_PRESS,
        ))
    })
    .expect(|e| {
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_A),
            (Some(edo_12_pitch(DEGREE_EDO12_D4)), VELOCITY_PRESS),
        );
        e.pressed_keys
            .insert((BACKGROUND_LEGATO_BACKEND, SRC_A), (None, VELOCITY_PRESS));
        e.keys_version = 2;
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_EDO12_D4,
                pitch: edo_12_pitch(DEGREE_EDO12_D4),
                velocity: VELOCITY_PRESS,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_EDO12_D4,
                pitch: edo_12_pitch(DEGREE_EDO12_D4),
                velocity: VELOCITY_PRESS,
            },
        ));
    });

    // Without legato, a move event is completely ignored
    f.when(|e| {
        e.handle_input(InputEvent::Moved(
            SRC_A,
            InputLocation::Pitch(edo_12_pitch(DEGREE_EDO12_E4)),
        ))
    })
    .expect(|e| {
        e.keys_version = 2;
    });

    // Enable legato
    f.when(|e| e.set_parameter(LiveParameter::Legato, 1.0))
        .expect(|e| {
            e.expected_calls.push((
                FOREGROUND_LEGATO_BACKEND,
                RecordedCall::ControlChange {
                    controller: LEGATO_CONTROLLER,
                    value: 127,
                },
            ));
            e.expected_calls.push((
                BACKGROUND_LEGATO_BACKEND,
                RecordedCall::ControlChange {
                    controller: LEGATO_CONTROLLER,
                    value: 127,
                },
            ));
            e.expected_calls.push((
                FOREGROUND_NO_LEGATO_BACKEND,
                RecordedCall::ControlChange {
                    controller: LEGATO_CONTROLLER,
                    value: 127,
                },
            ));
            e.expected_storage.set_parameter(LiveParameter::Legato, 1.0);
        });

    // With legato on, a move event updates the pitch on backends with the pressed key
    f.when(|e| {
        e.handle_input(InputEvent::Moved(
            SRC_A,
            InputLocation::Pitch(edo_12_pitch(DEGREE_EDO12_E4)),
        ))
    })
    .expect(|e| {
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_A),
            (Some(edo_12_pitch(DEGREE_EDO12_E4)), VELOCITY_PRESS),
        );
        e.keys_version = 3;
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::UpdatePitch {
                key_id: SRC_A,
                degree: DEGREE_EDO12_E4,
                pitch: edo_12_pitch(DEGREE_EDO12_E4),
                velocity: VELOCITY_PRESS,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::UpdatePitch {
                key_id: SRC_A,
                degree: DEGREE_EDO12_E4,
                pitch: edo_12_pitch(DEGREE_EDO12_E4),
                velocity: VELOCITY_PRESS,
            },
        ));
    });
}

#[test]
fn key_pressure_forwarded_to_all_backends_even_when_not_on() {
    let mut f = PianoEngineFixture::new();

    // Key pressure is forwarded to all backends (0.75 * 128 = 96)
    f.when(|e| e.set_key_pressure(SRC_A, 0.75)).expect(|e| {
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::UpdatePressure {
                key_id: SRC_A,
                pressure: 96,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::UpdatePressure {
                key_id: SRC_A,
                pressure: 96,
            },
        ));
        e.expected_calls.push((
            FOREGROUND_NO_LEGATO_BACKEND,
            RecordedCall::UpdatePressure {
                key_id: SRC_A,
                pressure: 96,
            },
        ));
    });
}

#[test]
fn different_sources_coexist_independently() {
    let mut f = PianoEngineFixture::new();

    // Press a key from the mouse source
    f.when(|e| {
        e.handle_input(InputEvent::Pressed(
            SRC_A,
            InputLocation::Piano(key_e4()),
            VELOCITY_PRESS,
        ))
    })
    .expect(|e| {
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_A),
            (Some(edo_12_pitch(DEGREE_EDO12_E4)), VELOCITY_PRESS),
        );
        e.pressed_keys
            .insert((BACKGROUND_LEGATO_BACKEND, SRC_A), (None, VELOCITY_PRESS));
        e.keys_version = 2;
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_EDO12_E4,
                pitch: edo_12_pitch(DEGREE_EDO12_E4),
                velocity: VELOCITY_PRESS,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_EDO12_E4,
                pitch: edo_12_pitch(DEGREE_EDO12_E4),
                velocity: VELOCITY_PRESS,
            },
        ));
    });

    // Press a different key from the keyboard source — both notes coexist
    f.when(|e| {
        e.handle_input(InputEvent::Pressed(
            SRC_B,
            InputLocation::Piano(key_d4()),
            VELOCITY_LOW,
        ))
    })
    .expect(|e| {
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_A),
            (Some(edo_12_pitch(DEGREE_EDO12_E4)), VELOCITY_PRESS),
        );
        e.pressed_keys
            .insert((BACKGROUND_LEGATO_BACKEND, SRC_A), (None, VELOCITY_PRESS));
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_B),
            (Some(edo_12_pitch(DEGREE_EDO12_D4)), VELOCITY_LOW),
        );
        e.pressed_keys
            .insert((BACKGROUND_LEGATO_BACKEND, SRC_B), (None, VELOCITY_LOW));
        e.keys_version = 4;
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_B,
                degree: DEGREE_EDO12_D4,
                pitch: edo_12_pitch(DEGREE_EDO12_D4),
                velocity: VELOCITY_LOW,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_B,
                degree: DEGREE_EDO12_D4,
                pitch: edo_12_pitch(DEGREE_EDO12_D4),
                velocity: VELOCITY_LOW,
            },
        ));
    });

    // Releasing the mouse key only stops that key, the keyboard key remains
    f.when(|e| e.handle_input(InputEvent::Released(SRC_A, 0)))
        .expect(|e| {
            e.pressed_keys.remove(&(FOREGROUND_LEGATO_BACKEND, SRC_A));
            e.pressed_keys.remove(&(BACKGROUND_LEGATO_BACKEND, SRC_A));
            e.pressed_keys.insert(
                (FOREGROUND_LEGATO_BACKEND, SRC_B),
                (Some(edo_12_pitch(DEGREE_EDO12_D4)), VELOCITY_LOW),
            );
            e.pressed_keys
                .insert((BACKGROUND_LEGATO_BACKEND, SRC_B), (None, VELOCITY_LOW));
            e.keys_version = 6;
            e.expected_calls.push((
                FOREGROUND_LEGATO_BACKEND,
                RecordedCall::Stop {
                    key_id: SRC_A,
                    velocity: 0,
                },
            ));
            e.expected_calls.push((
                BACKGROUND_LEGATO_BACKEND,
                RecordedCall::Stop {
                    key_id: SRC_A,
                    velocity: 0,
                },
            ));
        });

    // Releasing the keyboard key stops it independently
    f.when(|e| e.handle_input(InputEvent::Released(SRC_B, 0)))
        .expect(|e| {
            e.pressed_keys.clear();
            e.keys_version = 8;
            e.expected_calls.push((
                FOREGROUND_LEGATO_BACKEND,
                RecordedCall::Stop {
                    key_id: SRC_B,
                    velocity: 0,
                },
            ));
            e.expected_calls.push((
                BACKGROUND_LEGATO_BACKEND,
                RecordedCall::Stop {
                    key_id: SRC_B,
                    velocity: 0,
                },
            ));
        });
}

#[test]
fn same_source_repress_replaces_note() {
    let mut f = PianoEngineFixture::new();

    // First press starts the note
    f.when(|e| {
        e.handle_input(InputEvent::Pressed(
            SRC_A,
            InputLocation::Piano(key_d4()),
            VELOCITY_PRESS,
        ))
    })
    .expect(|e| {
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_A),
            (Some(edo_12_pitch(DEGREE_EDO12_D4)), VELOCITY_PRESS),
        );
        e.pressed_keys
            .insert((BACKGROUND_LEGATO_BACKEND, SRC_A), (None, VELOCITY_PRESS));
        e.keys_version = 2;
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_EDO12_D4,
                pitch: edo_12_pitch(DEGREE_EDO12_D4),
                velocity: VELOCITY_PRESS,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_EDO12_D4,
                pitch: edo_12_pitch(DEGREE_EDO12_D4),
                velocity: VELOCITY_PRESS,
            },
        ));
    });

    // Pressing the same source again starts a new note, overwriting the previous one
    f.when(|e| {
        e.handle_input(InputEvent::Pressed(
            SRC_A,
            InputLocation::Piano(key_c4()),
            VELOCITY_LOW,
        ))
    })
    .expect(|e| {
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_A),
            (Some(edo_12_pitch(DEGREE_EDO12_C4)), VELOCITY_LOW),
        );
        e.pressed_keys
            .insert((BACKGROUND_LEGATO_BACKEND, SRC_A), (None, VELOCITY_LOW));
        e.keys_version = 4;
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::Stop {
                key_id: SRC_A,
                velocity: 0,
            },
        ));
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_EDO12_C4,
                pitch: edo_12_pitch(DEGREE_EDO12_C4),
                velocity: VELOCITY_LOW,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::Stop {
                key_id: SRC_A,
                velocity: 0,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_EDO12_C4,
                pitch: edo_12_pitch(DEGREE_EDO12_C4),
                velocity: VELOCITY_LOW,
            },
        ));
    });

    // A single release stops the note completely
    f.when(|e| e.handle_input(InputEvent::Released(SRC_A, 0)))
        .expect(|e| {
            e.pressed_keys.clear();
            e.keys_version = 6;
            e.expected_calls.push((
                FOREGROUND_LEGATO_BACKEND,
                RecordedCall::Stop {
                    key_id: SRC_A,
                    velocity: 0,
                },
            ));
            e.expected_calls.push((
                BACKGROUND_LEGATO_BACKEND,
                RecordedCall::Stop {
                    key_id: SRC_A,
                    velocity: 0,
                },
            ));
        });
}

// Pitch input test cases

#[test]
fn pitch_quantized_on_press_continuous_preserves_raw_pitch() {
    let mut f = PianoEngineFixture::new();

    // In fixed mode, pressing a slightly detuned pitch snaps to the nearest tuned degree
    f.when(|e| {
        e.handle_input(InputEvent::Pressed(
            SRC_A,
            InputLocation::Pitch(slightly_off(edo_12_pitch(DEGREE_EDO12_D4))),
            VELOCITY_PRESS,
        ))
    })
    .expect(|e| {
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_A),
            (Some(edo_12_pitch(DEGREE_EDO12_D4)), VELOCITY_PRESS),
        );
        e.pressed_keys
            .insert((BACKGROUND_LEGATO_BACKEND, SRC_A), (None, VELOCITY_PRESS));
        e.keys_version = 2;
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_EDO12_D4,
                pitch: edo_12_pitch(DEGREE_EDO12_D4),
                velocity: VELOCITY_PRESS,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_EDO12_D4,
                pitch: edo_12_pitch(DEGREE_EDO12_D4),
                velocity: VELOCITY_PRESS,
            },
        ));
    });

    // Release to prepare for next step
    f.when(|e| e.handle_input(InputEvent::Released(SRC_A, 0)))
        .expect(|e| {
            e.pressed_keys.clear();
            e.keys_version = 4;
            e.expected_calls.push((
                FOREGROUND_LEGATO_BACKEND,
                RecordedCall::Stop {
                    key_id: SRC_A,
                    velocity: 0,
                },
            ));
            e.expected_calls.push((
                BACKGROUND_LEGATO_BACKEND,
                RecordedCall::Stop {
                    key_id: SRC_A,
                    velocity: 0,
                },
            ));
        });

    // Switch to continuous mode
    f.when(|e| e.switch_tuning_mode(Direction::Forward))
        .expect(|e| {
            e.tuning_mode = TuningMode::Continuous;
            e.expected_calls
                .push((FOREGROUND_LEGATO_BACKEND, RecordedCall::SetNoTuning));
            e.expected_calls
                .push((BACKGROUND_LEGATO_BACKEND, RecordedCall::SetNoTuning));
            e.expected_calls
                .push((FOREGROUND_NO_LEGATO_BACKEND, RecordedCall::SetNoTuning));
            e.expected_calls
                .push((FOREGROUND_LEGATO_BACKEND, RecordedCall::RequestStatus));
        });

    // In continuous mode, pressing a slightly detuned pitch preserves the raw pitch in pressed_keys
    f.when(|e| {
        e.handle_input(InputEvent::Pressed(
            SRC_A,
            InputLocation::Pitch(slightly_off(edo_12_pitch(DEGREE_EDO12_D4))),
            VELOCITY_PRESS,
        ))
    })
    .expect(|e| {
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_A),
            (
                Some(slightly_off(edo_12_pitch(DEGREE_EDO12_D4))),
                VELOCITY_PRESS,
            ),
        );
        e.pressed_keys
            .insert((BACKGROUND_LEGATO_BACKEND, SRC_A), (None, VELOCITY_PRESS));
        e.keys_version = 6;
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_EDO12_D4,
                pitch: slightly_off(edo_12_pitch(DEGREE_EDO12_D4)),
                velocity: VELOCITY_PRESS,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_EDO12_D4,
                pitch: slightly_off(edo_12_pitch(DEGREE_EDO12_D4)),
                velocity: VELOCITY_PRESS,
            },
        ));
    });
}

#[test]
fn pitch_quantized_on_move_continuous_preserves_raw_pitch() {
    let mut f = PianoEngineFixture::new();

    // Press a key to prepare for legato move
    f.when(|e| {
        e.handle_input(InputEvent::Pressed(
            SRC_A,
            InputLocation::Pitch(edo_12_pitch(DEGREE_EDO12_D4)),
            VELOCITY_PRESS,
        ))
    })
    .expect(|e| {
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_A),
            (Some(edo_12_pitch(DEGREE_EDO12_D4)), VELOCITY_PRESS),
        );
        e.pressed_keys
            .insert((BACKGROUND_LEGATO_BACKEND, SRC_A), (None, VELOCITY_PRESS));
        e.keys_version = 2;
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_EDO12_D4,
                pitch: edo_12_pitch(DEGREE_EDO12_D4),
                velocity: VELOCITY_PRESS,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_EDO12_D4,
                pitch: edo_12_pitch(DEGREE_EDO12_D4),
                velocity: VELOCITY_PRESS,
            },
        ));
    });

    // Enable legato
    f.when(|e| e.set_parameter(LiveParameter::Legato, 1.0))
        .expect(|e| {
            e.expected_calls.push((
                FOREGROUND_LEGATO_BACKEND,
                RecordedCall::ControlChange {
                    controller: LEGATO_CONTROLLER,
                    value: 127,
                },
            ));
            e.expected_calls.push((
                BACKGROUND_LEGATO_BACKEND,
                RecordedCall::ControlChange {
                    controller: LEGATO_CONTROLLER,
                    value: 127,
                },
            ));
            e.expected_calls.push((
                FOREGROUND_NO_LEGATO_BACKEND,
                RecordedCall::ControlChange {
                    controller: LEGATO_CONTROLLER,
                    value: 127,
                },
            ));
            e.expected_storage.set_parameter(LiveParameter::Legato, 1.0);
        });

    // In fixed mode, moving to a slightly detuned pitch snaps to the nearest tuned degree
    f.when(|e| {
        e.handle_input(InputEvent::Moved(
            SRC_A,
            InputLocation::Pitch(slightly_off(edo_12_pitch(DEGREE_EDO12_E4))),
        ))
    })
    .expect(|e| {
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_A),
            (Some(edo_12_pitch(DEGREE_EDO12_E4)), VELOCITY_PRESS),
        );
        e.keys_version = 3;
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::UpdatePitch {
                key_id: SRC_A,
                degree: DEGREE_EDO12_E4,
                pitch: edo_12_pitch(DEGREE_EDO12_E4),
                velocity: VELOCITY_PRESS,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::UpdatePitch {
                key_id: SRC_A,
                degree: DEGREE_EDO12_E4,
                pitch: edo_12_pitch(DEGREE_EDO12_E4),
                velocity: VELOCITY_PRESS,
            },
        ));
    });

    // Switch to continuous mode
    f.when(|e| e.switch_tuning_mode(Direction::Forward))
        .expect(|e| {
            e.tuning_mode = TuningMode::Continuous;
            e.expected_calls
                .push((FOREGROUND_LEGATO_BACKEND, RecordedCall::SetNoTuning));
            e.expected_calls
                .push((BACKGROUND_LEGATO_BACKEND, RecordedCall::SetNoTuning));
            e.expected_calls
                .push((FOREGROUND_NO_LEGATO_BACKEND, RecordedCall::SetNoTuning));
            e.expected_calls
                .push((FOREGROUND_LEGATO_BACKEND, RecordedCall::RequestStatus));
        });

    // In continuous mode, moving to a slightly detuned pitch preserves the raw pitch in pressed_keys
    f.when(|e| {
        e.handle_input(InputEvent::Moved(
            SRC_A,
            InputLocation::Pitch(slightly_off(edo_12_pitch(DEGREE_EDO12_D4))),
        ))
    })
    .expect(|e| {
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_A),
            (
                Some(slightly_off(edo_12_pitch(DEGREE_EDO12_D4))),
                VELOCITY_PRESS,
            ),
        );
        e.keys_version = 4;
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::UpdatePitch {
                key_id: SRC_A,
                degree: DEGREE_EDO12_D4,
                pitch: slightly_off(edo_12_pitch(DEGREE_EDO12_D4)),
                velocity: VELOCITY_PRESS,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::UpdatePitch {
                key_id: SRC_A,
                degree: DEGREE_EDO12_D4,
                pitch: slightly_off(edo_12_pitch(DEGREE_EDO12_D4)),
                velocity: VELOCITY_PRESS,
            },
        ));
    });
}

#[test]
fn pitch_location_depends_on_tuning_not_layout_or_kbm() {
    let mut f = PianoEngineFixture::new();

    // Press a key on 12-EDO — degree determined by 12-EDO tuning
    f.when(|e| {
        e.handle_input(InputEvent::Pressed(
            SRC_A,
            InputLocation::Pitch(edo_12_pitch(DEGREE_EDO12_A4)),
            VELOCITY_PRESS,
        ))
    })
    .expect(|e| {
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_A),
            (Some(edo_12_pitch(DEGREE_EDO12_A4)), VELOCITY_PRESS),
        );
        e.pressed_keys
            .insert((BACKGROUND_LEGATO_BACKEND, SRC_A), (None, VELOCITY_PRESS));
        e.keys_version = 2;
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_EDO12_A4,
                pitch: edo_12_pitch(DEGREE_EDO12_A4),
                velocity: VELOCITY_PRESS,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_EDO12_A4,
                pitch: edo_12_pitch(DEGREE_EDO12_A4),
                velocity: VELOCITY_PRESS,
            },
        ));
    });

    // Switch layout — pitch-based input still maps to the same degree since layout doesn't affect pitch input
    f.when(|e| e.switch_layout(Direction::Forward))
        .expect(|_e| {});

    f.when(|e| {
        e.handle_input(InputEvent::Pressed(
            SRC_B,
            InputLocation::Pitch(edo_12_pitch(DEGREE_EDO12_A4)),
            VELOCITY_LOW,
        ))
    })
    .expect(|e| {
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_A),
            (Some(edo_12_pitch(DEGREE_EDO12_A4)), VELOCITY_PRESS),
        );
        e.pressed_keys
            .insert((BACKGROUND_LEGATO_BACKEND, SRC_A), (None, VELOCITY_PRESS));
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_B),
            (Some(edo_12_pitch(DEGREE_EDO12_A4)), VELOCITY_LOW),
        );
        e.pressed_keys
            .insert((BACKGROUND_LEGATO_BACKEND, SRC_B), (None, VELOCITY_LOW));
        e.keys_version = 4;
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_B,
                degree: DEGREE_EDO12_A4,
                pitch: edo_12_pitch(DEGREE_EDO12_A4),
                velocity: VELOCITY_LOW,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_B,
                degree: DEGREE_EDO12_A4,
                pitch: edo_12_pitch(DEGREE_EDO12_A4),
                velocity: VELOCITY_LOW,
            },
        ));
    });

    // Switch tuning to Harmonics — pitch-based input now maps via Harmonics tuning
    f.when(|e| e.switch_tuning(Direction::Forward)).expect(|e| {
        e.expected_calls
            .push((FOREGROUND_LEGATO_BACKEND, RecordedCall::SetTuning));
        e.expected_calls
            .push((BACKGROUND_LEGATO_BACKEND, RecordedCall::SetTuning));
        e.expected_calls
            .push((FOREGROUND_NO_LEGATO_BACKEND, RecordedCall::SetTuning));
        e.expected_calls
            .push((FOREGROUND_LEGATO_BACKEND, RecordedCall::RequestStatus));
    });

    // Release previous keys and press the pitch of Harmonics degree 7
    f.when(|e| {
        e.handle_input(InputEvent::Released(SRC_A, 0));
        e.handle_input(InputEvent::Released(SRC_B, 0));
        e.handle_input(InputEvent::Pressed(
            SRC_A,
            InputLocation::Pitch(harmonics_pitch(DEGREE_HARMONICS_13_8)),
            VELOCITY_PRESS,
        ))
    })
    .expect(|e| {
        e.pressed_keys.clear();
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_A),
            (Some(harmonics_pitch(DEGREE_HARMONICS_13_8)), VELOCITY_PRESS),
        );
        e.pressed_keys
            .insert((BACKGROUND_LEGATO_BACKEND, SRC_A), (None, VELOCITY_PRESS));
        e.keys_version = 10;
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::Stop {
                key_id: SRC_A,
                velocity: 0,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::Stop {
                key_id: SRC_A,
                velocity: 0,
            },
        ));
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::Stop {
                key_id: SRC_B,
                velocity: 0,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::Stop {
                key_id: SRC_B,
                velocity: 0,
            },
        ));
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_HARMONICS_13_8,
                pitch: harmonics_pitch(DEGREE_HARMONICS_13_8),
                velocity: VELOCITY_PRESS,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_HARMONICS_13_8,
                pitch: harmonics_pitch(DEGREE_HARMONICS_13_8),
                velocity: VELOCITY_PRESS,
            },
        ));
    });
}

// Isomorphic input test cases

#[test]
fn isomorphic_keys_mapped_to_scale_degree() {
    let mut f = PianoEngineFixture::new();

    // On 12-EDO with Meantone[7] layout (ps=2, ss=1): Isomorphic(1, 0) = degree 2
    f.when(|e| {
        e.handle_input(InputEvent::Pressed(
            SRC_A,
            InputLocation::Isomorphic(1, 0),
            VELOCITY_PRESS,
        ))
    })
    .expect(|e| {
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_A),
            (Some(edo_12_pitch(DEGREE_EDO12_E4)), VELOCITY_PRESS),
        );
        e.pressed_keys
            .insert((BACKGROUND_LEGATO_BACKEND, SRC_A), (None, VELOCITY_PRESS));
        e.keys_version = 2;
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_EDO12_E4,
                pitch: edo_12_pitch(DEGREE_EDO12_E4),
                velocity: VELOCITY_PRESS,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_EDO12_E4,
                pitch: edo_12_pitch(DEGREE_EDO12_E4),
                velocity: VELOCITY_PRESS,
            },
        ));
    });

    // On 12-EDO with Meantone[7] layout: Isomorphic(0, 1) = degree 1
    f.when(|e| {
        e.handle_input(InputEvent::Pressed(
            SRC_B,
            InputLocation::Isomorphic(0, 1),
            VELOCITY_LOW,
        ))
    })
    .expect(|e| {
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_A),
            (Some(edo_12_pitch(DEGREE_EDO12_E4)), VELOCITY_PRESS),
        );
        e.pressed_keys
            .insert((BACKGROUND_LEGATO_BACKEND, SRC_A), (None, VELOCITY_PRESS));
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_B),
            (Some(edo_12_pitch(DEGREE_EDO12_DS4)), VELOCITY_LOW),
        );
        e.pressed_keys
            .insert((BACKGROUND_LEGATO_BACKEND, SRC_B), (None, VELOCITY_LOW));
        e.keys_version = 4;
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_B,
                degree: DEGREE_EDO12_DS4,
                pitch: edo_12_pitch(DEGREE_EDO12_DS4),
                velocity: VELOCITY_LOW,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_B,
                degree: DEGREE_EDO12_DS4,
                pitch: edo_12_pitch(DEGREE_EDO12_DS4),
                velocity: VELOCITY_LOW,
            },
        ));
    });
}

#[test]
fn isomorphic_location_depends_on_tuning_and_layout_not_kbm() {
    let mut f = PianoEngineFixture::new();

    // On 12-EDO Meantone[7] (ps=2, ss=1): Isomorphic(1, 0) = degree 2
    f.when(|e| {
        e.handle_input(InputEvent::Pressed(
            SRC_A,
            InputLocation::Isomorphic(1, 0),
            VELOCITY_PRESS,
        ))
    })
    .expect(|e| {
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_A),
            (Some(edo_12_pitch(DEGREE_EDO12_E4)), VELOCITY_PRESS),
        );
        e.pressed_keys
            .insert((BACKGROUND_LEGATO_BACKEND, SRC_A), (None, VELOCITY_PRESS));
        e.keys_version = 2;
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_EDO12_E4,
                pitch: edo_12_pitch(DEGREE_EDO12_E4),
                velocity: VELOCITY_PRESS,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_EDO12_E4,
                pitch: edo_12_pitch(DEGREE_EDO12_E4),
                velocity: VELOCITY_PRESS,
            },
        ));
    });

    // Switch layout to Meantone[5] (ps=2, ss=3): Isomorphic(0, 1) = degree 3 instead of degree 1
    f.when(|e| e.switch_layout(Direction::Forward))
        .expect(|_e| {});

    f.when(|e| {
        e.handle_input(InputEvent::Pressed(
            SRC_B,
            InputLocation::Isomorphic(0, 1),
            VELOCITY_LOW,
        ))
    })
    .expect(|e| {
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_A),
            (Some(edo_12_pitch(DEGREE_EDO12_E4)), VELOCITY_PRESS),
        );
        e.pressed_keys
            .insert((BACKGROUND_LEGATO_BACKEND, SRC_A), (None, VELOCITY_PRESS));
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_B),
            (Some(edo_12_pitch(DEGREE_EDO12_F4)), VELOCITY_LOW),
        );
        e.pressed_keys
            .insert((BACKGROUND_LEGATO_BACKEND, SRC_B), (None, VELOCITY_LOW));
        e.keys_version = 4;
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_B,
                degree: DEGREE_EDO12_F4,
                pitch: edo_12_pitch(DEGREE_EDO12_F4),
                velocity: VELOCITY_LOW,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_B,
                degree: DEGREE_EDO12_F4,
                pitch: edo_12_pitch(DEGREE_EDO12_F4),
                velocity: VELOCITY_LOW,
            },
        ));
    });

    // Switch tuning to Harmonics — same isomorphic layout, but KBM has unmapped keys
    // Isomorphic mapping should not be affected by KBM key table
    f.when(|e| e.switch_tuning(Direction::Forward)).expect(|e| {
        e.expected_calls
            .push((FOREGROUND_LEGATO_BACKEND, RecordedCall::SetTuning));
        e.expected_calls
            .push((BACKGROUND_LEGATO_BACKEND, RecordedCall::SetTuning));
        e.expected_calls
            .push((FOREGROUND_NO_LEGATO_BACKEND, RecordedCall::SetTuning));
        e.expected_calls
            .push((FOREGROUND_LEGATO_BACKEND, RecordedCall::RequestStatus));
    });

    // Release previous keys
    f.when(|e| {
        e.handle_input(InputEvent::Released(SRC_A, 0));
        e.handle_input(InputEvent::Released(SRC_B, 0));
    })
    .expect(|e| {
        e.pressed_keys.clear();
        e.keys_version = 8;
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::Stop {
                key_id: SRC_A,
                velocity: 0,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::Stop {
                key_id: SRC_A,
                velocity: 0,
            },
        ));
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::Stop {
                key_id: SRC_B,
                velocity: 0,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::Stop {
                key_id: SRC_B,
                velocity: 0,
            },
        ));
    });

    // On Harmonics with Meantone[5] layout still active: Isomorphic(1, 0) = degree 2
    // The KBM key table (with unmapped keys) doesn't affect isomorphic mapping
    f.when(|e| {
        e.handle_input(InputEvent::Pressed(
            SRC_A,
            InputLocation::Isomorphic(1, 0),
            VELOCITY_PRESS,
        ))
    })
    .expect(|e| {
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_A),
            (Some(harmonics_pitch(DEGREE_HARMONICS_10_8)), VELOCITY_PRESS),
        );
        e.pressed_keys
            .insert((BACKGROUND_LEGATO_BACKEND, SRC_A), (None, VELOCITY_PRESS));
        e.keys_version = 10;
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_HARMONICS_10_8,
                pitch: harmonics_pitch(DEGREE_HARMONICS_10_8),
                velocity: VELOCITY_PRESS,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_HARMONICS_10_8,
                pitch: harmonics_pitch(DEGREE_HARMONICS_10_8),
                velocity: VELOCITY_PRESS,
            },
        ));
    });

    // Switch layout back to Meantone[7] on Harmonics: Isomorphic(0, 1) = degree 1
    // KBM unmapped keys don't affect isomorphic mapping
    f.when(|e| e.switch_layout(Direction::Backward))
        .expect(|_e| {});

    f.when(|e| {
        e.handle_input(InputEvent::Released(SRC_A, 0));
        e.handle_input(InputEvent::Pressed(
            SRC_B,
            InputLocation::Isomorphic(0, 1),
            VELOCITY_LOW,
        ))
    })
    .expect(|e| {
        e.pressed_keys.remove(&(FOREGROUND_LEGATO_BACKEND, SRC_A));
        e.pressed_keys.remove(&(BACKGROUND_LEGATO_BACKEND, SRC_A));
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_B),
            (Some(harmonics_pitch(DEGREE_HARMONICS_9_8)), VELOCITY_LOW),
        );
        e.pressed_keys
            .insert((BACKGROUND_LEGATO_BACKEND, SRC_B), (None, VELOCITY_LOW));
        e.keys_version = 14;
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::Stop {
                key_id: SRC_A,
                velocity: 0,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::Stop {
                key_id: SRC_A,
                velocity: 0,
            },
        ));
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_B,
                degree: DEGREE_HARMONICS_9_8,
                pitch: harmonics_pitch(DEGREE_HARMONICS_9_8),
                velocity: VELOCITY_LOW,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_B,
                degree: DEGREE_HARMONICS_9_8,
                pitch: harmonics_pitch(DEGREE_HARMONICS_9_8),
                velocity: VELOCITY_LOW,
            },
        ));
    });
}

// Piano input test cases

#[test]
fn piano_key_mapped_to_scale_degree() {
    let mut f = PianoEngineFixture::new();

    // On 12-EDO with D4 as reference: C4 maps to degree -2
    f.when(|e| {
        e.handle_input(InputEvent::Pressed(
            SRC_A,
            InputLocation::Piano(key_c4()),
            VELOCITY_PRESS,
        ))
    })
    .expect(|e| {
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_A),
            (Some(edo_12_pitch(DEGREE_EDO12_C4)), VELOCITY_PRESS),
        );
        e.pressed_keys
            .insert((BACKGROUND_LEGATO_BACKEND, SRC_A), (None, VELOCITY_PRESS));
        e.keys_version = 2;
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_EDO12_C4,
                pitch: edo_12_pitch(DEGREE_EDO12_C4),
                velocity: VELOCITY_PRESS,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_EDO12_C4,
                pitch: edo_12_pitch(DEGREE_EDO12_C4),
                velocity: VELOCITY_PRESS,
            },
        ));
    });

    // D4 maps to degree 0
    f.when(|e| {
        e.handle_input(InputEvent::Pressed(
            SRC_B,
            InputLocation::Piano(key_d4()),
            VELOCITY_LOW,
        ))
    })
    .expect(|e| {
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_A),
            (Some(edo_12_pitch(DEGREE_EDO12_C4)), VELOCITY_PRESS),
        );
        e.pressed_keys
            .insert((BACKGROUND_LEGATO_BACKEND, SRC_A), (None, VELOCITY_PRESS));
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_B),
            (Some(edo_12_pitch(DEGREE_EDO12_D4)), VELOCITY_LOW),
        );
        e.pressed_keys
            .insert((BACKGROUND_LEGATO_BACKEND, SRC_B), (None, VELOCITY_LOW));
        e.keys_version = 4;
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_B,
                degree: DEGREE_EDO12_D4,
                pitch: edo_12_pitch(DEGREE_EDO12_D4),
                velocity: VELOCITY_LOW,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_B,
                degree: DEGREE_EDO12_D4,
                pitch: edo_12_pitch(DEGREE_EDO12_D4),
                velocity: VELOCITY_LOW,
            },
        ));
    });
}

#[test]
fn piano_location_depends_on_tuning_and_kbm_not_layout() {
    let mut f = PianoEngineFixture::new();

    // On 12-EDO with D4 as ref: E4 maps to degree 2
    f.when(|e| {
        e.handle_input(InputEvent::Pressed(
            SRC_A,
            InputLocation::Piano(key_e4()),
            VELOCITY_PRESS,
        ))
    })
    .expect(|e| {
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_A),
            (Some(edo_12_pitch(DEGREE_EDO12_E4)), VELOCITY_PRESS),
        );
        e.pressed_keys
            .insert((BACKGROUND_LEGATO_BACKEND, SRC_A), (None, VELOCITY_PRESS));
        e.keys_version = 2;
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_EDO12_E4,
                pitch: edo_12_pitch(DEGREE_EDO12_E4),
                velocity: VELOCITY_PRESS,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_EDO12_E4,
                pitch: edo_12_pitch(DEGREE_EDO12_E4),
                velocity: VELOCITY_PRESS,
            },
        ));
    });

    // Switch layout — piano key mapping should not change
    f.when(|e| e.switch_layout(Direction::Forward))
        .expect(|_e| {});

    f.when(|e| {
        e.handle_input(InputEvent::Released(SRC_A, 0));
        e.handle_input(InputEvent::Pressed(
            SRC_B,
            InputLocation::Piano(key_e4()),
            VELOCITY_LOW,
        ))
    })
    .expect(|e| {
        e.pressed_keys.remove(&(FOREGROUND_LEGATO_BACKEND, SRC_A));
        e.pressed_keys.remove(&(BACKGROUND_LEGATO_BACKEND, SRC_A));
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_B),
            (Some(edo_12_pitch(DEGREE_EDO12_E4)), VELOCITY_LOW),
        );
        e.pressed_keys
            .insert((BACKGROUND_LEGATO_BACKEND, SRC_B), (None, VELOCITY_LOW));
        e.keys_version = 6;
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::Stop {
                key_id: SRC_A,
                velocity: 0,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::Stop {
                key_id: SRC_A,
                velocity: 0,
            },
        ));
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_B,
                degree: DEGREE_EDO12_E4,
                pitch: edo_12_pitch(DEGREE_EDO12_E4),
                velocity: VELOCITY_LOW,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_B,
                degree: DEGREE_EDO12_E4,
                pitch: edo_12_pitch(DEGREE_EDO12_E4),
                velocity: VELOCITY_LOW,
            },
        ));
    });

    // Switch tuning to Harmonics with C4 as ref and key table: E4 maps to degree 2
    f.when(|e| e.switch_tuning(Direction::Forward)).expect(|e| {
        e.expected_calls
            .push((FOREGROUND_LEGATO_BACKEND, RecordedCall::SetTuning));
        e.expected_calls
            .push((BACKGROUND_LEGATO_BACKEND, RecordedCall::SetTuning));
        e.expected_calls
            .push((FOREGROUND_NO_LEGATO_BACKEND, RecordedCall::SetTuning));
        e.expected_calls
            .push((FOREGROUND_LEGATO_BACKEND, RecordedCall::RequestStatus));
    });

    // On Harmonics with C4 ref and key table: C4=degree 0, release E4 then press C4
    f.when(|e| {
        e.handle_input(InputEvent::Released(SRC_B, 0));
        e.handle_input(InputEvent::Pressed(
            SRC_A,
            InputLocation::Piano(key_c4()),
            VELOCITY_PRESS,
        ))
    })
    .expect(|e| {
        e.pressed_keys.remove(&(FOREGROUND_LEGATO_BACKEND, SRC_B));
        e.pressed_keys.remove(&(BACKGROUND_LEGATO_BACKEND, SRC_B));
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_A),
            (Some(harmonics_pitch(DEGREE_HARMONICS_8_8)), VELOCITY_PRESS),
        );
        e.pressed_keys
            .insert((BACKGROUND_LEGATO_BACKEND, SRC_A), (None, VELOCITY_PRESS));
        e.keys_version = 10;
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::Stop {
                key_id: SRC_B,
                velocity: 0,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::Stop {
                key_id: SRC_B,
                velocity: 0,
            },
        ));
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_HARMONICS_8_8,
                pitch: harmonics_pitch(DEGREE_HARMONICS_8_8),
                velocity: VELOCITY_PRESS,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_HARMONICS_8_8,
                pitch: harmonics_pitch(DEGREE_HARMONICS_8_8),
                velocity: VELOCITY_PRESS,
            },
        ));
    });
}

#[test]
fn piano_key_unmapped_via_kbm_table() {
    let mut f = PianoEngineFixture::new();

    // Switch to Harmonics tuning which has unmapped keys in its KBM table
    f.when(|e| e.switch_tuning(Direction::Forward)).expect(|e| {
        e.expected_calls
            .push((FOREGROUND_LEGATO_BACKEND, RecordedCall::SetTuning));
        e.expected_calls
            .push((BACKGROUND_LEGATO_BACKEND, RecordedCall::SetTuning));
        e.expected_calls
            .push((FOREGROUND_NO_LEGATO_BACKEND, RecordedCall::SetTuning));
        e.expected_calls
            .push((FOREGROUND_LEGATO_BACKEND, RecordedCall::RequestStatus));
    });

    // C4 maps to degree 0 (mapped key in Harmonics KBM)
    f.when(|e| {
        e.handle_input(InputEvent::Pressed(
            SRC_A,
            InputLocation::Piano(key_c4()),
            VELOCITY_PRESS,
        ))
    })
    .expect(|e| {
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_A),
            (Some(harmonics_pitch(DEGREE_HARMONICS_8_8)), VELOCITY_PRESS),
        );
        e.pressed_keys
            .insert((BACKGROUND_LEGATO_BACKEND, SRC_A), (None, VELOCITY_PRESS));
        e.keys_version = 2;
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_HARMONICS_8_8,
                pitch: harmonics_pitch(DEGREE_HARMONICS_8_8),
                velocity: VELOCITY_PRESS,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_A,
                degree: DEGREE_HARMONICS_8_8,
                pitch: harmonics_pitch(DEGREE_HARMONICS_8_8),
                velocity: VELOCITY_PRESS,
            },
        ));
    });

    // C#4 is unmapped in the Harmonics KBM table — pressing it produces no sound
    f.when(|e| {
        e.handle_input(InputEvent::Pressed(
            SRC_B,
            InputLocation::Piano(key_cs4()),
            VELOCITY_LOW,
        ))
    })
    .expect(|_e| {
        // No change — the unmapped key press is ignored
    });

    // Releasing the unmapped (never-pressed) key is suppressed — no stop calls, no version bump
    f.when(|e| e.handle_input(InputEvent::Released(SRC_B, 0)))
        .expect(|_e| {});

    // D4 is mapped (degree 1) — should work normally
    f.when(|e| {
        e.handle_input(InputEvent::Pressed(
            SRC_B,
            InputLocation::Piano(key_d4()),
            VELOCITY_LOW,
        ))
    })
    .expect(|e| {
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_A),
            (Some(harmonics_pitch(DEGREE_HARMONICS_8_8)), VELOCITY_PRESS),
        );
        e.pressed_keys
            .insert((BACKGROUND_LEGATO_BACKEND, SRC_A), (None, VELOCITY_PRESS));
        e.pressed_keys.insert(
            (FOREGROUND_LEGATO_BACKEND, SRC_B),
            (Some(harmonics_pitch(DEGREE_HARMONICS_9_8)), VELOCITY_LOW),
        );
        e.pressed_keys
            .insert((BACKGROUND_LEGATO_BACKEND, SRC_B), (None, VELOCITY_LOW));
        e.keys_version = 4;
        e.expected_calls.push((
            FOREGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_B,
                degree: DEGREE_HARMONICS_9_8,
                pitch: harmonics_pitch(DEGREE_HARMONICS_9_8),
                velocity: VELOCITY_LOW,
            },
        ));
        e.expected_calls.push((
            BACKGROUND_LEGATO_BACKEND,
            RecordedCall::Start {
                key_id: SRC_B,
                degree: DEGREE_HARMONICS_9_8,
                pitch: harmonics_pitch(DEGREE_HARMONICS_9_8),
                velocity: VELOCITY_LOW,
            },
        ));
    });
}
