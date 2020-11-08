mod view;

use microwave::model::{EventId, EventPhase, Model};
use nannou::{
    event::{ElementState, KeyboardInput},
    prelude::*,
    winit::event::WindowEvent,
};
use std::{convert::TryFrom, env, process};
use tune::ratio::Ratio;

fn main() {
    nannou::app(model).update(update).run();
}

fn model(app: &App) -> Model {
    let model = microwave::create_model_from_args(env::args()).unwrap_or_else(|err| {
        eprintln!("Could not start application / {}", err);
        process::exit(1);
    });

    create_window(&app);
    model
}

fn create_window(app: &App) {
    app.new_window()
        .maximized(true)
        .title("Microwave - Microtonal Waveform Synthesizer by Woyten")
        .raw_event(raw_event)
        .key_pressed(key_pressed)
        .mouse_pressed(mouse_pressed)
        .mouse_moved(mouse_moved)
        .mouse_released(mouse_released)
        .mouse_wheel(mouse_wheel)
        .touch(touch)
        .view(view::view)
        .build()
        .unwrap();
}

fn raw_event(app: &App, model: &mut Model, event: &WindowEvent) {
    if app.keys.mods.alt() {
        return;
    }

    if let WindowEvent::KeyboardInput {
        input: KeyboardInput {
            scancode, state, ..
        },
        ..
    } = event
    {
        if let Some(key_coord) = hex_location_for_iso_keyboard(*scancode) {
            let pressed = match state {
                ElementState::Pressed => true,
                ElementState::Released => false,
            };

            model.keyboard_event(key_coord, pressed);
        }
    }
}

fn hex_location_for_iso_keyboard(keycode: u32) -> Option<(i8, i8)> {
    let keycode = match i8::try_from(keycode) {
        Ok(keycode) => keycode,
        Err(_) => return None,
    };
    let key_coord = match keycode {
        41 => (keycode - 47, 1),       // Key before <1>
        2..=14 => (keycode - 7, 1),    // <1>..<BSP>
        15..=28 => (keycode - 21, 0),  // <TAB>..<RETURN>
        58 => (keycode - 64, -1),      // <CAPS>
        30..=40 => (keycode - 35, -1), // <A>..Second key after <L>
        43 => (keycode - 37, -1),      // Third key after <L>
        42 => (keycode - 49, -2),      // Left <SHIFT>
        86 => (keycode - 92, -2),      // Key before <Z>
        44..=54 => (keycode - 49, -2), // Z..Right <SHIFT>
        _ => return None,
    };
    Some(key_coord)
}

fn key_pressed(app: &App, model: &mut Model, key: Key) {
    let alt_pressed = app.keys.mods.alt();
    let engine = &model.engine;
    match key {
        Key::L if alt_pressed => engine.toggle_legato(),
        Key::C if alt_pressed => engine.toggle_continuous(),
        Key::E if app.keys.mods.alt() => engine.toggle_envelope_type(),
        Key::Space => model.toggle_recording(),
        Key::Up if !alt_pressed => engine.dec_program(&mut model.selected_program.program_number),
        Key::Down if !alt_pressed => engine.inc_program(&mut model.selected_program.program_number),
        Key::Up if alt_pressed => engine.toggle_synth_mode(),
        Key::Down if alt_pressed => engine.toggle_synth_mode(),
        Key::Left => engine.dec_root_note(),
        Key::Right => engine.inc_root_note(),
        _ => {}
    }
}

fn mouse_pressed(app: &App, model: &mut Model, button: MouseButton) {
    if button == MouseButton::Left {
        mouse_event(app, model, EventPhase::Pressed(100), app.mouse.position());
    }
}

fn mouse_moved(app: &App, model: &mut Model, position: Point2) {
    mouse_event(app, model, EventPhase::Moved, position);
}

fn mouse_released(app: &App, model: &mut Model, button: MouseButton) {
    if button == MouseButton::Left {
        mouse_event(app, model, EventPhase::Released, app.mouse.position());
    }
}

fn mouse_wheel(app: &App, model: &mut Model, mouse_scroll_delta: MouseScrollDelta, _: TouchPhase) {
    let (mut x_delta, mut y_delta) = match mouse_scroll_delta {
        MouseScrollDelta::LineDelta(x, y) => (10.0 * x as f64, 10.0 * y as f64),
        MouseScrollDelta::PixelDelta(pos) => (pos.x, pos.y),
    };

    if app.keys.mods.alt() {
        let tmp = x_delta;
        x_delta = -y_delta;
        y_delta = tmp;
    }

    if x_delta.abs() > y_delta.abs() {
        let ratio =
            Ratio::between_pitches(model.lowest_note, model.highest_note).repeated(x_delta / 500.0);
        model.lowest_note = model.lowest_note * ratio;
        model.highest_note = model.highest_note * ratio;
    } else {
        let ratio = Ratio::from_semitones(y_delta / 10.0);
        let lowest = model.lowest_note * ratio;
        let highest = model.highest_note / ratio;
        if lowest < highest {
            model.lowest_note = lowest;
            model.highest_note = highest;
        }
    }
}

fn mouse_event(app: &App, model: &mut Model, phase: EventPhase, mut position: Point2) {
    position.x = position.x / app.window_rect().w() + 0.5;
    position.y = position.y / app.window_rect().h() + 0.5;
    position_event(model, EventId::Mouse, position, phase);
}

fn touch(app: &App, model: &mut Model, event: TouchEvent) {
    let mut position = event.position;
    position.x = position.x / app.window_rect().w() + 0.5;
    position.y = position.y / app.window_rect().h() + 0.5;
    let phase = match event.phase {
        TouchPhase::Started => EventPhase::Pressed(100),
        TouchPhase::Moved => EventPhase::Moved,
        TouchPhase::Ended | TouchPhase::Cancelled => EventPhase::Released,
    };
    position_event(model, EventId::Touchpad(event.id), position, phase);
}

fn position_event(model: &Model, id: EventId, position: Vector2, phase: EventPhase) {
    let keyboard_range = Ratio::between_pitches(model.lowest_note, model.highest_note);
    let pitch = model.lowest_note * keyboard_range.repeated(position.x);
    model.engine.handle_pitch_event(id, pitch, phase);
}

pub fn update(_: &App, model: &mut Model, _: Update) {
    model.update()
}
