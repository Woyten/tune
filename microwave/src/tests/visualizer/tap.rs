use std::sync::Arc;

use magnetron::Magnetron;
use magnetron::buffer::BufferIndex;

use crate::tap::Tap;
use crate::tap::TapContext;
use crate::visualizer::AudioVisualizer;
use crate::visualizer::VisualizerControl;
use crate::visualizer::create_audio_capture;
use crate::visualizer::create_visualizer_tap;

const SAMPLE_RATE: u32 = 48_000;
const FRAMES: usize = 4;

fn render_tap(
    control: Arc<VisualizerControl>,
    buffer_count: usize,
    left: f64,
    right: f64,
) -> AudioVisualizer {
    let mut magnetron = Magnetron::new(f64::from(SAMPLE_RATE).recip(), 2, 1024);
    let (writer, reader) = create_audio_capture();
    let mut tap = create_visualizer_tap(
        (BufferIndex::Internal(0), BufferIndex::Internal(1)),
        buffer_count,
        writer,
        control,
    );

    let mut buffers = magnetron.prepare(FRAMES);
    buffers.read_0_write_1(BufferIndex::Internal(0), None, || left);
    buffers.read_0_write_1(BufferIndex::Internal(1), None, || right);
    let context = TapContext::new(buffers);
    tap.process(&context);

    let mut visualizer = AudioVisualizer::new(reader, SAMPLE_RATE);
    visualizer.update();
    visualizer
}

#[test]
fn tap_reads_selected_buffer() {
    let control = Arc::new(VisualizerControl::new());
    control.select(Some(1));

    let visualizer = render_tap(control, 2, 0.25, 0.75);

    assert!(visualizer.samples().iter().all(|&sample| sample == 0.75f32));
}

#[test]
fn tap_invalid_index_falls_back_to_stereo_output() {
    let control = Arc::new(VisualizerControl::new());
    control.select(Some(99));

    let visualizer = render_tap(control, 2, 0.25, 0.75);

    let expected = (0..FRAMES)
        .flat_map(|_| [0.25f32, 0.75f32])
        .collect::<Vec<_>>();
    assert_eq!(visualizer.samples(), expected);
}

#[test]
fn tap_disabled_captures_nothing() {
    let control = Arc::new(VisualizerControl::new());
    control.set_enabled(false);

    let visualizer = render_tap(control, 2, 0.25, 0.75);

    assert!(visualizer.samples().is_empty());
}
