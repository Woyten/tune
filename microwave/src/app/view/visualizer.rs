use bevy::asset::RenderAssetUsages;
use bevy::color::palettes::css;
use bevy::prelude::*;
use bevy::render::render_resource::Extent3d;
use bevy::render::render_resource::TextureDimension;
use bevy::render::render_resource::TextureFormat;
use bevy::ui;

use crate::app::state::VisualizerMode;
use crate::app::state::VisualizerState;
use crate::app::view::CameraViewport;
use crate::visualizer::AudioVisualizer;
use crate::visualizer::CorrelationTrigger;
use crate::visualizer::level_color;

const IMAGE_WIDTH: u32 = 640;
const IMAGE_HEIGHT: u32 = 192;
const MIN_FREQUENCY_HZ: f64 = 20.0;
const MAX_FREQUENCY_HZ: f64 = 20000.0;
const SPECTROGRAPH_SECONDS: f64 = 2.0;

#[derive(Component)]
pub(super) struct VisualizerPanel(VisualizerPanelKind);

#[derive(Clone, Copy, PartialEq, Eq)]
enum VisualizerPanelKind {
    Oscilloscope,
    Spectrograph,
}

#[derive(Resource)]
pub(super) struct VisualizerImages {
    oscilloscope: Handle<Image>,
    spectrograph: Handle<Image>,
}

pub(super) fn init_visualizer(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    assets: Res<AssetServer>,
) {
    let oscilloscope = images.add(create_image());
    let spectrograph = images.add(create_image());
    let font = assets.load("FiraMono-Regular.ttf");

    let camera = commands
        .spawn((
            CameraViewport::Visualizer,
            Camera2d,
            Camera {
                order: 0,
                clear_color: ClearColorConfig::None,
                ..default()
            },
        ))
        .id();

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: ui::px(0),
                top: ui::px(0),
                width: ui::percent(100),
                height: ui::percent(100),
                flex_direction: FlexDirection::Row,
                ..default()
            },
            UiTargetCamera(camera),
        ))
        .with_children(|parent| {
            for (kind, label, image) in [
                (
                    VisualizerPanelKind::Oscilloscope,
                    "Oscilloscope",
                    oscilloscope.clone(),
                ),
                (
                    VisualizerPanelKind::Spectrograph,
                    "Spectrograph",
                    spectrograph.clone(),
                ),
            ] {
                parent
                    .spawn((
                        VisualizerPanel(kind),
                        Node {
                            width: ui::percent(50),
                            height: ui::percent(100),
                            ..default()
                        },
                    ))
                    .with_children(|panel| {
                        panel.spawn((
                            ImageNode::new(image).with_mode(NodeImageMode::Stretch),
                            Node {
                                width: ui::percent(100),
                                height: ui::percent(100),
                                ..default()
                            },
                        ));

                        panel.spawn((
                            Text::new(label),
                            TextFont::from_font_size(12.0).with_font(font.clone()),
                            TextColor(css::WHITE.with_alpha(0.7).into()),
                            TextLayout::no_wrap(),
                            Node {
                                position_type: PositionType::Absolute,
                                left: ui::px(8),
                                top: ui::px(4),
                                ..default()
                            },
                        ));
                    });
            }
        });

    commands.insert_resource(VisualizerImages {
        oscilloscope,
        spectrograph,
    });
}

pub(super) fn render_visualizer(
    mut visualizer: NonSendMut<AudioVisualizer>,
    visualizer_state: Res<VisualizerState>,
    images: Res<VisualizerImages>,
    mut image_assets: ResMut<Assets<Image>>,
    mut panels: Query<(&VisualizerPanel, &mut Node)>,
    mut trigger: Local<CorrelationTrigger>,
    mut frames_since_column: Local<usize>,
    mut last_mode: Local<Option<VisualizerMode>>,
    mut last_source: Local<Option<Option<usize>>>,
) {
    let mode = visualizer_state.mode();
    let source = visualizer_state.source_index();
    let show_oscilloscope = matches!(
        mode,
        VisualizerMode::Oscilloscope | VisualizerMode::OscilloscopeAndSpectrograph
    );
    let show_spectrograph = matches!(
        mode,
        VisualizerMode::Spectrograph | VisualizerMode::OscilloscopeAndSpectrograph
    );
    let show_both = show_oscilloscope && show_spectrograph;
    let mode_changed = *last_mode != Some(mode);
    let source_changed = *last_source != Some(source);
    *last_mode = Some(mode);
    *last_source = Some(source);

    for (panel, mut node) in &mut panels {
        let visible = match panel.0 {
            VisualizerPanelKind::Oscilloscope => show_oscilloscope,
            VisualizerPanelKind::Spectrograph => show_spectrograph,
        };

        let display = match visible {
            true => Display::Flex,
            false => Display::None,
        };
        if node.display != display {
            node.display = display;
        }

        let width = match show_both {
            true => ui::percent(50),
            false => ui::percent(100),
        };
        if node.width != width {
            node.width = width;
        }
    }

    if mode_changed || source_changed {
        visualizer.clear_history();
        trigger.reset();
        *frames_since_column = 0;
    }

    if !show_oscilloscope && !show_spectrograph {
        return;
    }

    let update = visualizer.update();
    if update.gap {
        trigger.reset();
        *frames_since_column = 0;
    }
    let new_frames = update.new_frames;

    if show_oscilloscope && let Some(mut image) = image_assets.get_mut(&images.oscilloscope) {
        draw_oscilloscope(
            image.data.as_mut().unwrap(),
            visualizer.samples(),
            visualizer.start_frame(),
            visualizer.gain(),
            visualizer.window_frames(),
            source.is_none(),
            &mut trigger,
        );
    }

    if show_spectrograph && let Some(mut image) = image_assets.get_mut(&images.spectrograph) {
        let data = image.data.as_mut().unwrap();

        if mode_changed || source_changed || update.gap {
            clear_image(data, [8, 10, 14, 255]);
        }

        let sample_rate = visualizer.sample_rate();
        let frames_per_column = (sample_rate * SPECTROGRAPH_SECONDS / f64::from(IMAGE_WIDTH))
            .round()
            .max(1.0) as usize;
        *frames_since_column += new_frames;
        let columns = (*frames_since_column / frames_per_column).min(IMAGE_WIDTH as usize);
        *frames_since_column %= frames_per_column;

        if columns > 0 {
            let magnitudes = visualizer.spectrum();
            draw_spectrograph_column(data, magnitudes, sample_rate, columns);
        }
    }
}

fn create_image() -> Image {
    Image::new_fill(
        Extent3d {
            width: IMAGE_WIDTH,
            height: IMAGE_HEIGHT,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[8, 10, 14, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
}

fn clear_image(data: &mut [u8], color: [u8; 4]) {
    for pixel in data.as_chunks_mut::<4>().0 {
        *pixel = color;
    }
}

fn draw_oscilloscope(
    data: &mut [u8],
    samples: &[f32],
    start_frame: u64,
    gain: f32,
    window_frames: usize,
    stereo: bool,
    trigger: &mut CorrelationTrigger,
) {
    clear_image(data, [8, 10, 14, 255]);

    let width = IMAGE_WIDTH as usize;
    let height = IMAGE_HEIGHT as usize;

    for x in (0..width).step_by(width / 8) {
        for y in 0..height {
            set_pixel(data, width, height, x, y, [28, 32, 40, 255]);
        }
    }

    for y in [height / 4, height / 2, 3 * height / 4] {
        for x in 0..width {
            set_pixel(data, width, height, x, y, [36, 42, 52, 255]);
        }
    }

    if samples.len() < 4 {
        return;
    }

    let frames = samples.len() / 2;
    let (trigger_frame, window) = match frames >= window_frames {
        true => (
            trigger.next(samples, start_frame, frames, window_frames),
            window_frames,
        ),
        false => (start_frame as i64, frames),
    };

    draw_channel(
        data,
        samples,
        trigger_frame,
        start_frame,
        window,
        gain,
        0,
        [80, 220, 120, 255],
    );

    if stereo {
        draw_channel(
            data,
            samples,
            trigger_frame,
            start_frame,
            window,
            gain,
            1,
            [80, 180, 255, 255],
        );
    }
}

fn draw_channel(
    data: &mut [u8],
    samples: &[f32],
    trigger: i64,
    start_frame: u64,
    window: usize,
    gain: f32,
    channel: usize,
    color: [u8; 4],
) {
    let width = IMAGE_WIDTH as usize;
    let height = IMAGE_HEIGHT as usize;
    let center = (height as f32 - 1.0) * 0.5;
    let amplitude = center * 0.9;
    let Ok(start_frame) = i64::try_from(start_frame) else {
        return;
    };
    let Some(local_trigger) = trigger
        .checked_sub(start_frame)
        .and_then(|local| usize::try_from(local).ok())
    else {
        return;
    };
    let frames = samples.len() / 2;
    if local_trigger
        .checked_add(window)
        .is_none_or(|end| end > frames)
    {
        return;
    }

    let mut last = None;
    for x in 0..width {
        let offset = x * window / width;
        let sample = samples[2 * (local_trigger + offset) + channel];
        let y = (center - sample * gain * amplitude).clamp(0.0, height as f32 - 1.0) as usize;

        if let Some((last_x, last_y)) = last {
            draw_line(data, last_x, last_y, x, y, color);
        }

        last = Some((x, y));
    }
}

fn draw_line(
    data: &mut [u8],
    start_x: usize,
    start_y: usize,
    end_x: usize,
    end_y: usize,
    color: [u8; 4],
) {
    let width = IMAGE_WIDTH as usize;
    let height = IMAGE_HEIGHT as usize;

    let (mut x, mut y) = (start_x as isize, start_y as isize);
    let (end_x, end_y) = (end_x as isize, end_y as isize);
    let dx = (end_x - x).abs();
    let dy = -(end_y - y).abs();
    let step_x = if x < end_x { 1 } else { -1 };
    let step_y = if y < end_y { 1 } else { -1 };
    let mut error = dx + dy;

    loop {
        set_pixel(data, width, height, x as usize, y as usize, color);
        if y > 0 {
            set_pixel(data, width, height, x as usize, y as usize - 1, color);
        }

        if x == end_x && y == end_y {
            break;
        }

        let doubled_error = 2 * error;
        if doubled_error >= dy {
            error += dy;
            x += step_x;
        }
        if doubled_error <= dx {
            error += dx;
            y += step_y;
        }
    }
}

fn draw_spectrograph_column(data: &mut [u8], magnitudes: &[f64], sample_rate: f64, columns: usize) {
    let width = IMAGE_WIDTH as usize;
    let height = IMAGE_HEIGHT as usize;
    let row_bytes = width * 4;
    let columns = columns.clamp(1, width);
    let shift = columns * 4;

    let max_frequency = (0.5 * sample_rate).min(MAX_FREQUENCY_HZ);
    let frequency_range = (max_frequency / MIN_FREQUENCY_HZ).ln();
    let bin_width = sample_rate / (2.0 * magnitudes.len() as f64);

    for y in 0..height {
        let row = y * row_bytes;
        data.copy_within(row + shift..row + row_bytes, row);

        let fraction = 1.0 - y as f64 / (height - 1) as f64;
        let next_fraction = 1.0 - (y + 1) as f64 / (height - 1) as f64;
        let frequency = MIN_FREQUENCY_HZ * (fraction * frequency_range).exp();
        let next_frequency = MIN_FREQUENCY_HZ * (next_fraction * frequency_range).exp();

        let low_bin = (next_frequency / bin_width).floor() as usize;
        let high_bin = (frequency / bin_width).ceil() as usize;
        let low_bin = low_bin.min(magnitudes.len() - 1);
        let high_bin = high_bin.clamp(low_bin, magnitudes.len() - 1);
        let magnitude = magnitudes[low_bin..=high_bin]
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        let level = ((magnitude + 100.0) / 100.0).clamp(0.0, 1.0) as f32;
        let color = level_color(level);

        for column in 0..columns {
            let offset = row + (width - columns + column) * 4;
            data[offset..offset + 4].copy_from_slice(&color);
        }
    }
}

fn set_pixel(data: &mut [u8], width: usize, height: usize, x: usize, y: usize, color: [u8; 4]) {
    if x >= width || y >= height {
        return;
    }

    let offset = 4 * (y * width + x);
    data[offset..offset + 4].copy_from_slice(&color);
}
