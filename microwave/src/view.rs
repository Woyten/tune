use std::{
    collections::HashSet,
    convert::TryFrom,
    fmt::{self, Write},
    ops::Range,
};

use geom::Range as NannouRange;
use nannou::prelude::*;
use tune::{
    math,
    note::Note,
    pitch::{Pitch, Pitched, Ratio},
    scala::KbmRoot,
    tuning::Scale,
};

use crate::{fluid::FluidInfo, midi::MidiInfo, synth::WaveformInfo, Model};

pub trait ViewModel: Send + 'static {
    fn pitch_range(&self) -> Option<Range<Pitch>>;

    fn write_info(&self, target: &mut String) -> fmt::Result;
}

pub type DynViewModel = Box<dyn ViewModel>;

impl<T: ViewModel> From<T> for DynViewModel {
    fn from(data: T) -> Self {
        Box::new(data)
    }
}

pub fn view(app: &App, model: &Model, frame: Frame) {
    let draw = app.draw();
    let window_rect = app.window_rect();
    let total_range =
        Ratio::between_pitches(model.pitch_at_left_border, model.pitch_at_right_border);
    let octave_width = Ratio::octave().num_equal_steps_of_size(total_range) as f32;

    let kbm_root = model.kbm.kbm_root();
    let selected_tuning = (&model.scl, kbm_root);
    let reference_tuning = (
        &model.reference_scl,
        KbmRoot::from(Note::from_piano_key(kbm_root.origin)),
    );

    let render_second_keyboard = !model.scl_key_colors.is_empty();
    let keyboard_rect = if render_second_keyboard {
        Rect::from_w_h(window_rect.w(), window_rect.h() / 4.0)
    } else {
        Rect::from_w_h(window_rect.w(), window_rect.h() / 2.0)
    };
    let lower_keyboard_rect = keyboard_rect.align_bottom_of(window_rect);

    draw.background().color(DIMGRAY);
    render_scale_lines(model, &draw, window_rect, octave_width, selected_tuning);
    render_keyboard(
        model,
        &draw,
        lower_keyboard_rect,
        octave_width,
        reference_tuning,
        |key| is_black_key(key + kbm_root.origin.midi_number()),
    );

    if render_second_keyboard {
        let upper_keyboard_rect = keyboard_rect.above(lower_keyboard_rect);
        render_keyboard(
            model,
            &draw,
            upper_keyboard_rect,
            octave_width,
            selected_tuning,
            |key| {
                model.scl_key_colors[Into::<usize>::into(math::i32_rem_u(
                    key,
                    u16::try_from(model.scl_key_colors.len()).unwrap(),
                ))]
            },
        );
    }

    render_just_ratios_with_deviations(model, &draw, window_rect, octave_width);
    render_recording_indicator(model, &draw, window_rect);
    render_hud(model, &draw, window_rect);
    draw.to_frame(app, &frame).unwrap();
}

fn render_scale_lines(
    model: &Model,
    draw: &Draw,
    window_rect: Rect,
    octave_width: f32,
    tuning: impl Scale,
) {
    let leftmost_degree = tuning
        .find_by_pitch_sorted(model.pitch_at_left_border)
        .approx_value;
    let rightmost_degree = tuning
        .find_by_pitch_sorted(model.pitch_at_right_border)
        .approx_value;

    let pitch_range = model.view_model.as_ref().and_then(|m| m.pitch_range());

    for degree in leftmost_degree..=rightmost_degree {
        let pitch = tuning.sorted_pitch_of(degree);

        let pitch_position = Ratio::between_pitches(model.pitch_at_left_border, pitch).as_octaves()
            as f32
            * octave_width;

        let pitch_position_on_screen = (pitch_position - 0.5) * window_rect.w();

        let line_color = match pitch_range.as_ref().filter(|r| !r.contains(&pitch)) {
            None => GRAY,
            Some(_) => INDIANRED,
        };

        let line_color = match degree {
            0 => SALMON,
            _ => line_color,
        };

        draw.line()
            .start(Point2 {
                x: pitch_position_on_screen,
                y: window_rect.top(),
            })
            .end(Point2 {
                x: pitch_position_on_screen,
                y: window_rect.bottom(),
            })
            .color(line_color)
            .weight(2.0);
    }
}

fn render_just_ratios_with_deviations(
    model: &Model,
    draw: &Draw,
    window_rect: Rect,
    octave_width: f32,
) {
    let mut freqs_hz = model
        .pressed_keys
        .iter()
        .map(|(_, pressed_key)| pressed_key.pitch)
        .collect::<Vec<_>>();
    freqs_hz.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let mut curr_slice_window = freqs_hz.as_slice();
    while let Some((second, others)) = curr_slice_window.split_last() {
        let pitch_position = Ratio::between_pitches(model.pitch_at_left_border, *second)
            .as_octaves() as f32
            * octave_width;

        let pitch_position_on_screen = (pitch_position - 0.5) * window_rect.w();

        draw.line()
            .start(Point2 {
                x: pitch_position_on_screen,
                y: window_rect.top(),
            })
            .end(Point2 {
                x: pitch_position_on_screen,
                y: window_rect.bottom(),
            })
            .color(WHITE)
            .weight(2.0);

        let mut curr_rect = Rect {
            x: NannouRange::new(pitch_position_on_screen, pitch_position_on_screen + 1000.0),
            y: NannouRange::from_pos_and_len(0.0, 24.0),
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
                Ratio::between_pitches(*first, *second).nearest_fraction(model.odd_limit);

            let width =
                approximation.deviation.as_octaves() as f32 * octave_width * window_rect.w();
            let deviation_bar_rect = Rect {
                x: NannouRange::new(pitch_position_on_screen - width, pitch_position_on_screen),
                y: NannouRange::from_pos_and_len(0.0, 24.0),
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
}

fn render_keyboard(
    model: &Model,
    draw: &Draw,
    rect: Rect,
    octave_width: f32,
    tuning: impl Scale,
    is_black_key: impl Fn(i32) -> bool,
) {
    let highlighted_keys: HashSet<_> = model
        .pressed_keys
        .values()
        .map(|pressed_key| tuning.find_by_pitch_sorted(pressed_key.pitch).approx_value)
        .collect();

    let leftmost_key = tuning
        .find_by_pitch_sorted(model.pitch_at_left_border)
        .approx_value;
    let rightmost_key = tuning
        .find_by_pitch_sorted(model.pitch_at_right_border)
        .approx_value;

    let (mut mid, mut right) = Default::default();

    for iterated_key in (leftmost_key - 1)..=(rightmost_key + 1) {
        let pitch = tuning.sorted_pitch_of(iterated_key);
        let coord = Ratio::between_pitches(model.pitch_at_left_border, pitch).as_octaves() as f32
            * octave_width;

        let left = mid;
        mid = right;
        right = Some(coord);

        if let (Some(left), Some(mid), Some(right)) = (left, mid, right) {
            let drawn_key = iterated_key - 1;

            let key_color = if highlighted_keys.contains(&drawn_key) {
                LIGHTSTEELBLUE
            } else if is_black_key(drawn_key) {
                BLACK
            } else {
                LIGHTGRAY
            };

            let pos = (left + right) / 4.0 + mid / 2.0;
            let width = (left - right) / 2.0;

            let key_rect = Rect::from_x_y_w_h(
                rect.left() + pos * rect.w(),
                rect.y(),
                width * rect.w(),
                rect.h(),
            );

            if drawn_key == 0 {
                draw.rect().color(RED).xy(key_rect.xy()).wh(key_rect.wh());
            }

            draw.rect()
                .color(key_color)
                .xy(key_rect.xy())
                .w(0.9 * key_rect.w())
                .h(0.98 * key_rect.h());
        }
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
         Reference Note [Alt+Left/Right]: {ref_note}\n\
         Scale Offset [Left/Right]: {offset:+}",
        scale = model.scl.description(),
        ref_note = model.kbm.kbm_root().origin.midi_number(),
        offset = -model.kbm.kbm_root().ref_degree
    )
    .unwrap();

    if let Some(view_data) = &model.view_model {
        view_data.write_info(&mut hud_text).unwrap();
    }

    writeln!(
        hud_text,
        "Continuous [Alt+C]: {continuous}\n\
         Legato [Alt+L]: {legato}\n\
         Reverb [Ctrl+F8]: {reverb}\n\
         Delay [Ctrl+F9]: {delay}\n\
         Rotary Speaker [Ctrl+/F10]: {rotary}\n\
         Recording [Space]: {recording}\n\
         Range [Alt+/Scroll]: {from:.0}..{to:.0} Hz",
        continuous = if model.continuous { "ON" } else { "OFF" },
        legato = if model.legato { "ON" } else { "OFF" },
        reverb = if model.reverb_active { "ON" } else { "OFF" },
        delay = if model.delay_active { "ON" } else { "OFF" },
        rotary = if model.rotary_active {
            format!("ON ({:.0}%)", 100.0 * model.rotary_motor_voltage)
        } else {
            "OFF".to_owned()
        },
        recording = if model.recording_active { "ON" } else { "OFF" },
        from = model.pitch_at_left_border.as_hz(),
        to = model.pitch_at_right_border.as_hz(),
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

fn is_black_key(key: i32) -> bool {
    [1, 3, 6, 8, 10].contains(&key.rem_euclid(12))
}

impl ViewModel for WaveformInfo {
    fn pitch_range(&self) -> Option<Range<Pitch>> {
        None
    }

    fn write_info(&self, target: &mut String) -> fmt::Result {
        writeln!(
            target,
            "Output [Alt+O]: Waveform\n\
             Waveform [Up/Down]: {waveform_number} - {waveform_name}\n\
             Envelope [Alt+E]: {envelope_name}{is_default_indicator}",
            waveform_number = self.waveform_number,
            waveform_name = self.waveform_name,
            envelope_name = self.envelope_name,
            is_default_indicator = if self.is_default_envelope {
                ""
            } else {
                " (default) "
            }
        )
    }
}

impl ViewModel for FluidInfo {
    fn pitch_range(&self) -> Option<Range<Pitch>> {
        Some(Note::from_midi_number(0).pitch()..Note::from_midi_number(127).pitch())
    }

    fn write_info(&self, target: &mut String) -> fmt::Result {
        writeln!(
            target,
            "Output [Alt+O]: FluidLite\n\
             Soundfont File: {soundfont_file}\n\
             Program [Up/Down]: {program_number} - {program_name}",
            soundfont_file = self.soundfont_file_location.as_deref().unwrap_or("Unknown"),
            program_number = self
                .program
                .map(|p| p.to_string())
                .as_deref()
                .unwrap_or("Unknown"),
            program_name = self.program_name.as_deref().unwrap_or("Unknown"),
        )
    }
}

impl ViewModel for MidiInfo {
    fn pitch_range(&self) -> Option<Range<Pitch>> {
        Some(Note::from_midi_number(0).pitch()..Note::from_midi_number(127).pitch())
    }

    fn write_info(&self, target: &mut String) -> fmt::Result {
        writeln!(
            target,
            "Output [Alt+O]: MIDI\n\
             Device: {device}\n\
             Program [Up/Down]: {program_number}",
            device = self.device,
            program_number = self.program_number,
        )
    }
}

impl ViewModel for () {
    fn pitch_range(&self) -> Option<Range<Pitch>> {
        None
    }

    fn write_info(&self, target: &mut String) -> fmt::Result {
        writeln!(target, "Output [Alt+O]: No Audio")
    }
}
