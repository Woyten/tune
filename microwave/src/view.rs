use std::{
    fmt::{self, Write},
    ops::Range,
};

use geom::Range as NannouRange;
use nannou::prelude::*;
use tune::{
    note::{Note, NoteLetter},
    pitch::{Pitch, Pitched, Ratio},
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
    let draw: Draw = app.draw();

    draw.background().color(DIMGRAY);

    let window_rect = app.window_rect();
    let (w, h) = window_rect.w_h();

    render_quantization_grid(model, &draw, window_rect);

    render_recording_indicator(model, &draw, window_rect);

    render_hud(model, &draw, window_rect);

    let note_at_left_border = (model.pitch_at_left_border.as_hz() / 440.0).log2() * 12.0;
    let note_at_right_border = (model.pitch_at_right_border.as_hz() / 440.0).log2() * 12.0;

    let lowest_note_to_draw = note_at_left_border.floor();
    let highest_note_to_draw = note_at_right_border.ceil();

    let geometric_number_of_visible_notes = note_at_right_border - note_at_left_border;
    let lowest_key_position =
        (lowest_note_to_draw - note_at_left_border) / geometric_number_of_visible_notes;
    let key_stride = 1.0 / geometric_number_of_visible_notes;
    let key_width = key_stride * 0.9;

    for (stride_index, key_number) in
        (lowest_note_to_draw as i32..=highest_note_to_draw as i32).enumerate()
    {
        let note_to_draw = NoteLetter::A.in_octave(4).plus_semitones(key_number);

        let key_color = if note_to_draw.as_piano_key() == model.kbm.kbm_root().origin {
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
        let normalized_position = Ratio::between_pitches(model.pitch_at_left_border, *second)
            .as_octaves()
            / Ratio::between_pitches(model.pitch_at_left_border, model.pitch_at_right_border)
                .as_octaves();

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
            x: NannouRange::new(screen_position, screen_position + 1000.0),
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
                Ratio::between_pitches(*first, *second).nearest_fraction(model.limit);

            let width = approximation.deviation.as_semitones() * key_stride * w as f64;
            let deviation_bar_rect = Rect {
                x: NannouRange::new(screen_position - width as f32, screen_position),
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

    draw.to_frame(app, &frame).unwrap();
}

fn render_quantization_grid(model: &Model, draw: &Draw, window_rect: Rect) {
    let tuning = model.tuning();

    let lowest_rendered_key = tuning
        .find_by_pitch_sorted(model.pitch_at_left_border)
        .approx_value;
    let highest_rendered_key = tuning
        .find_by_pitch_sorted(model.pitch_at_right_border)
        .approx_value;

    let pitch_range = model.view_model.as_ref().and_then(|m| m.pitch_range());

    for degree in lowest_rendered_key..=highest_rendered_key {
        let pitch = tuning.sorted_pitch_of(degree);
        let normalized_position = Ratio::between_pitches(model.pitch_at_left_border, pitch)
            .as_octaves()
            / Ratio::between_pitches(model.pitch_at_left_border, model.pitch_at_right_border)
                .as_octaves();

        let screen_position = (normalized_position as f32 - 0.5) * window_rect.w();

        let line_color = match pitch_range.as_ref().filter(|r| !r.contains(&pitch)) {
            None => GRAY,
            Some(_) => INDIANRED,
        };

        let line_color = match degree {
            0 => LIGHTSKYBLUE,
            _ => line_color,
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
         Reference Note [Alt+Left/Right]: {ref_note}\n\
         Scale Offset [Left/Right]: {offset:+}",
        scale = model.scl.description(),
        ref_note = model.kbm.kbm_root().origin.midi_number(),
        offset = model.kbm.kbm_root().ref_degree
    )
    .unwrap();

    if let Some(view_data) = &model.view_model {
        view_data.write_info(&mut hud_text).unwrap();
    }

    writeln!(
        hud_text,
        "Continuous [Alt+C]: {continuous}\n\
         Legato [Alt+L]: {legato}\n\
         Reverb [Crtl+F8]: {reverb}\n\
         Delay [Crtl+F9]: {delay}\n\
         Rotary Speaker [Crtl+/F10]: {rotary}\n\
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

impl ViewModel for WaveformInfo {
    fn pitch_range(&self) -> Option<Range<Pitch>> {
        None
    }

    fn write_info(&self, target: &mut String) -> fmt::Result {
        writeln!(
            target,
            "Output [Alt+O]: Waveform\n\
             Waveform [Up/Down]: {waveform_number} - {waveform}\n\
             Envelope [Alt+E]: {envelope}",
            waveform_number = self.waveform_number,
            waveform = self.waveform_name,
            envelope = match self.preferred_envelope {
                Some(envelope_type) => format!("{:?}", envelope_type),
                None => format!("Default ({:?})", self.waveform_envelope),
            },
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
            "Output [Alt+O]: Fluidlite\n\
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
