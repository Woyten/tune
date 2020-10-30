use crate::{piano::SynthMode, Model};
use geom::Range;
use nannou::prelude::*;
use std::fmt::Write;
use tune::{
    note::{Note, NoteLetter},
    pitch::Pitched,
    ratio::Ratio,
    scala::Kbm,
    tuning::Scale,
};

pub fn view(app: &App, model: &Model, frame: Frame) {
    let draw: Draw = app.draw();

    draw.background().color(DIMGRAY);

    let window_rect = app.window_rect();
    let (w, h) = window_rect.w_h();

    let note_at_left_border = (model.lowest_note.as_hz() / 440.0).log2() * 12.0;
    let note_at_right_border = (model.highest_note.as_hz() / 440.0).log2() * 12.0;

    let lowest_note_to_draw = note_at_left_border.floor();
    let highest_note_to_draw = note_at_right_border.ceil();

    let geometric_number_of_visible_notes = note_at_right_border - note_at_left_border;
    let lowest_key_position =
        (lowest_note_to_draw - note_at_left_border) / geometric_number_of_visible_notes;
    let key_stride = 1.0 / geometric_number_of_visible_notes;
    let key_width = key_stride * 0.9;

    render_quantization_grid(model, &draw, window_rect);

    render_recording_indicator(model, &draw, window_rect);

    render_hud(model, &draw, window_rect);

    for (stride_index, key_number) in
        (lowest_note_to_draw as i32..=highest_note_to_draw as i32).enumerate()
    {
        let note_to_draw = NoteLetter::A.in_octave(4).plus_semitones(key_number);

        let key_color = if note_to_draw == model.root_note {
            LIGHTSTEELBLUE
        } else {
            match note_to_draw.letter_and_octave().0 {
                NoteLetter::Csh
                | NoteLetter::Dsh
                | NoteLetter::Fsh
                | NoteLetter::Gsh
                | NoteLetter::Ash => BLACK,
                _ => LIGHTGRAY,
            }
        };

        let key_position = lowest_key_position + stride_index as f64 * key_stride;
        draw.rect()
            .color(key_color)
            .w(key_width as f32 * w)
            .h(h / 2.0)
            .x((key_position as f32 - 0.5) * w)
            .y(-h / 4.0);
    }

    let mut freqs_hz = model
        .pressed_keys
        .iter()
        .map(|(_, pressed_key)| pressed_key.pitch)
        .collect::<Vec<_>>();
    freqs_hz.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut curr_slice_window = freqs_hz.as_slice();

    while let Some((second, others)) = curr_slice_window.split_last() {
        let normalized_position = Ratio::between_pitches(model.lowest_note, *second).as_octaves()
            / Ratio::between_pitches(model.lowest_note, model.highest_note).as_octaves();

        let screen_position = (normalized_position as f32 - 0.5) * w;

        draw.line()
            .start(Point2 {
                x: screen_position,
                y: -app.window_rect().h() / 2.0,
            })
            .end(Point2 {
                x: screen_position,
                y: app.window_rect().h() / 2.0,
            })
            .color(WHITE)
            .weight(2.0);

        let mut curr_rect = Rect {
            x: Range::new(screen_position, screen_position + 1000.0),
            y: Range::from_pos_and_len(0.0, 24.0),
        }
        .align_top_of(window_rect);

        draw.text(&format!("{:.0} Hz", second.as_hz()))
            .xy(curr_rect.xy())
            .wh(curr_rect.wh())
            .left_justify()
            .color(RED)
            .font_size(24);

        for first in others.iter() {
            let approximation =
                Ratio::between_pitches(*first, *second).nearest_fraction(model.limit);

            let width = approximation.deviation.as_semitones() * key_stride * w as f64;
            let deviation_bar_rect = Rect {
                x: Range::new(screen_position - width as f32, screen_position),
                y: Range::from_pos_and_len(0.0, 24.0),
            }
            .below(curr_rect);

            draw.rect()
                .xy(deviation_bar_rect.xy())
                .wh(deviation_bar_rect.wh())
                .color(DEEPSKYBLUE);

            let deviation_text_rect = curr_rect.below(curr_rect);

            draw.text(&format!(
                "{}/{} [{:.0}c]",
                approximation.numer,
                approximation.denom,
                approximation.deviation.as_cents().abs()
            ))
            .xy(deviation_text_rect.xy())
            .wh(deviation_text_rect.wh())
            .left_justify()
            .color(BLACK)
            .font_size(24);

            curr_rect = deviation_text_rect;
        }
        curr_slice_window = others;
    }

    draw.to_frame(app, &frame).unwrap();
}

fn render_quantization_grid(model: &Model, draw: &Draw, window_rect: Rect) {
    let tuning = (&*model.scale, Kbm::root_at(model.root_note));

    let lowest_rendered_key = tuning.find_by_pitch_sorted(model.lowest_note).approx_value;
    let highest_rendered_key = tuning.find_by_pitch_sorted(model.highest_note).approx_value;

    let lowest_fluid_pitch = Note::from_midi_number(0).pitch();
    let highest_fluid_pitch = Note::from_midi_number(127).pitch();

    for degree in lowest_rendered_key..=highest_rendered_key {
        let pitch = tuning.sorted_pitch_of(degree);
        let normalized_position = Ratio::between_pitches(model.lowest_note, pitch).as_octaves()
            / Ratio::between_pitches(model.lowest_note, model.highest_note).as_octaves();

        let screen_position = (normalized_position as f32 - 0.5) * window_rect.w();

        let line_color = match model.synth_mode() {
            SynthMode::Waveform { .. } => GRAY,
            SynthMode::Fluid { .. } | SynthMode::MidiOut { .. } => {
                if (lowest_fluid_pitch..highest_fluid_pitch).contains(&pitch) {
                    GRAY
                } else {
                    INDIANRED
                }
            }
        };

        draw.line()
            .start(Point2 {
                x: screen_position,
                y: -window_rect.h() / 2.0,
            })
            .end(Point2 {
                x: screen_position,
                y: window_rect.h() / 2.0,
            })
            .color(line_color)
            .weight(2.0);
    }
}

fn render_recording_indicator(model: &Model, draw: &Draw, window_rect: Rect) {
    let rect = Rect::from_w_h(100.0, 100.0)
        .top_right_of(window_rect)
        .pad(10.0);
    if model.recording_active {
        draw.ellipse().xy(rect.xy()).wh(rect.wh()).color(FIREBRICK);
    }
}

fn render_hud(model: &Model, draw: &Draw, window_rect: Rect) {
    let mut hud_text = String::new();

    writeln!(
        hud_text,
        "Scale: {scale}\n\
         Root Note [Left/Right]: {root_note}",
        scale = model.scale.description(),
        root_note = model.root_note,
    )
    .unwrap();

    match model.synth_mode() {
        SynthMode::Waveform {
            curr_waveform,
            waveforms,
            envelope_type,
            continuous,
        } => {
            let waveform = &waveforms[*curr_waveform];

            writeln!(
                hud_text,
                "Output [Alt+O]: Waveform\n\
                 Waveform [Up/Down]: {waveform_number} - {waveform}\n\
                 Envelope [Alt+E]: {envelope}\n\
                 Continuous [Alt+C]: {continuous}",
                waveform_number = *curr_waveform,
                waveform = waveform.name(),
                envelope = match envelope_type {
                    Some(envelope_type) => format!("{:?}", envelope_type),
                    None => format!("Default ({:?})", waveform.envelope_type()),
                },
                continuous = if *continuous { "ON" } else { "OFF" },
            )
        }
        SynthMode::Fluid {
            soundfont_file_location,
        } => writeln!(
            hud_text,
            "Output [Alt+O]: Fluidlite\n\
             Soundfont File: {soundfont_file}\n\
             Program [Up/Down]: {program_number} - {program}",
            soundfont_file = soundfont_file_location
                .as_os_str()
                .to_str()
                .unwrap_or("Unknown"),
            program_number = model.selected_program.program_number,
            program = model
                .selected_program
                .program_name
                .as_deref()
                .unwrap_or("Unknown"),
        ),
        SynthMode::MidiOut {
            device,
            curr_program,
            ..
        } => writeln!(
            hud_text,
            "Output [Alt+O]: MIDI\n\
             Device: {device}\n\
             Program [Up/Down]: {program_number}",
            device = device,
            program_number = curr_program,
        ),
    }
    .unwrap();

    writeln!(
        hud_text,
        "Legato [Alt+L]: {legato}\n\
         Delay [Crtl+F9]: {delay}\n\
         Rotary Speaker [Crtl+/F10]: {rotary}\n\
         Recording [Space]: {recording}\n\
         Range [Alt+/Scroll]: {from:.0}..{to:.0} Hz",
        legato = if model.legato { "ON" } else { "OFF" },
        delay = if model.delay_active { "ON" } else { "OFF" },
        rotary = if model.rotary_active {
            format!("ON ({:.0}%)", 100.0 * model.rotary_motor_voltage)
        } else {
            "OFF".to_owned()
        },
        recording = if model.recording_active { "ON" } else { "OFF" },
        from = model.lowest_note.as_hz(),
        to = model.highest_note.as_hz(),
    )
    .unwrap();

    let hud_rect = window_rect.shift_y(window_rect.h() / 2.0);

    draw.text(&hud_text)
        .xy(hud_rect.xy())
        .wh(hud_rect.wh())
        .align_text_bottom()
        .left_justify()
        .color(LIGHTGREEN)
        .font_size(24);
}
