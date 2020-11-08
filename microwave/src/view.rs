use geom::Range;
use microwave::{model::Model, piano::SynthMode};
use nannou::prelude::*;
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
    let scale = (&*model.scale, Kbm::root_at(model.root_note));

    let lowest_key = scale.find_by_pitch_sorted(model.lowest_note).approx_value;
    let highest_key = scale.find_by_pitch_sorted(model.highest_note).approx_value;

    let lowest_fluid_pitch = Note::from_midi_number(0).pitch() / Ratio::from_semitones(0.5);
    let highest_fluid_pitch = Note::from_midi_number(127).pitch() * Ratio::from_semitones(0.5);

    for degree in lowest_key..=highest_key {
        let pitch = scale.sorted_pitch_of(degree);
        let normalized_position = Ratio::between_pitches(model.lowest_note, pitch).as_octaves()
            / Ratio::between_pitches(model.lowest_note, model.highest_note).as_octaves();

        let screen_position = (normalized_position as f32 - 0.5) * window_rect.w();

        let line_color = match model.synth_mode {
            SynthMode::OnlyWaveform | SynthMode::Waveform => GRAY,
            SynthMode::Fluid => {
                if pitch < lowest_fluid_pitch || pitch > highest_fluid_pitch {
                    INDIANRED
                } else {
                    GRAY
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
    if model.recording_active() {
        draw.ellipse().xy(rect.xy()).wh(rect.wh()).color(FIREBRICK);
    }
}

fn render_hud(model: &Model, draw: &Draw, window_rect: Rect) {
    let hud_rect = Rect::from_w_h(window_rect.w(), 12.0 * 24.0)
        .bottom_left_of(window_rect)
        .shift_y(window_rect.h() / 2.0);

    let waveform = &model.waveforms[model.waveform_number];

    let current_sound = match model.synth_mode {
        SynthMode::OnlyWaveform | SynthMode::Waveform => {
            format!("Waveform: {} - {}", model.waveform_number, waveform.name(),)
        }
        SynthMode::Fluid => format!(
            "Program: {} - {}",
            model.selected_program.program_number,
            model
                .selected_program
                .program_name
                .as_deref()
                .unwrap_or("Unknown"),
        ),
    };

    let envelope = match model.envelope_type {
        Some(envelope_type) => format!("{:?}", envelope_type),
        None => format!("Default ({:?})", waveform.envelope_type()),
    };

    let legato_text = if model.legato { "ON" } else { "OFF" };
    let continuous_text = if model.continuous { "ON" } else { "OFF" };

    let hud_text = format!(
        "Scale: {scale}\n\
         {current_sound}\n\
         <up>/<down>/+<Alt> to change\n\
         Envelope: {envelope}\n\
         <Alt+E> to change\n\
         Root Note: {root_note}\n\
         <left>/<right> to change\n\
         Range: {from:.0}..{to:.0} Hz\n\
         <scroll>/+<Alt> to change\n\
         Legato: {legato} / Continuous: {continuous}\n\
         <Alt+L>/<Alt+C> to change",
        scale = model.scale.description(),
        current_sound = current_sound,
        envelope = envelope,
        root_note = model.root_note,
        from = model.lowest_note.as_hz(),
        to = model.highest_note.as_hz(),
        legato = legato_text,
        continuous = continuous_text,
    );

    draw.text(&hud_text)
        .xy(hud_rect.xy())
        .wh(hud_rect.wh())
        .left_justify()
        .color(LIGHTGREEN)
        .font_size(24);
}
