use std::f64::consts::TAU;

use assert_approx_eq::assert_approx_eq;

use crate::tests::visualizer::fixture::TriggerFixture;
use crate::tests::visualizer::fixture::sine;
use crate::visualizer::HISTORY_FRAMES;
use crate::visualizer::correlation;

#[test]
fn trigger_selects_expected_offset() {
    const SAMPLE_RATE: f64 = 48_000.0;
    const FREQUENCY: f64 = 52.0;

    let mut fixture = TriggerFixture::new(SAMPLE_RATE as u32);
    let window = fixture.visualizer.window_frames();

    let samples = sine(SAMPLE_RATE, FREQUENCY, window);
    fixture.push_mono(&samples);
    assert_eq!(fixture.next(), 0);

    let samples = sine(SAMPLE_RATE, FREQUENCY, 3 * window);
    fixture.push_mono(&samples[window..]);
    let trigger = fixture.next();

    let expected = 2.0 * SAMPLE_RATE / FREQUENCY;
    assert_approx_eq!(trigger as f64, expected, 1.0);
}

#[test]
fn trigger_keeps_expected_offset_for_silence() {
    let mut fixture = TriggerFixture::new(48_000);
    let window = fixture.visualizer.window_frames();

    let silence = vec![0.0; window];
    fixture.push_mono(&silence);
    assert_eq!(fixture.next(), 0);

    let silence = vec![0.0; 2 * window];
    fixture.push_mono(&silence);
    assert_eq!(fixture.next(), window as i64);
}

#[test]
fn trigger_stays_within_retained_history() {
    let mut fixture = TriggerFixture::new(48_000);
    let window = fixture.visualizer.window_frames();
    let samples = sine(48_000.0, 50.0, HISTORY_FRAMES + window);

    fixture.push_mono(&samples);
    fixture.next();

    let samples = fixture.visualizer.samples();
    let frames = samples.len() / 2;
    let start_frame = fixture.visualizer.start_frame();
    let trigger = fixture.next();

    assert!(trigger >= start_frame as i64);
    assert!(trigger + window as i64 <= start_frame as i64 + frames as i64);
}

#[test]
fn correlation_weights_all_samples_equally() {
    const N: usize = 256;

    let template = (0..N)
        .map(|frame| (TAU * frame as f64 / (N - 1) as f64).cos() as f32)
        .collect::<Vec<_>>();
    let stereo = |samples: &[f32]| {
        samples
            .iter()
            .flat_map(|&sample| [sample, sample])
            .collect::<Vec<_>>()
    };

    let mut early_change = template.clone();
    early_change[0] += 0.5;
    let mut late_change = template.clone();
    late_change[N - 1] += 0.5;

    let early_score = correlation(&stereo(&early_change), 0, &template);
    let late_score = correlation(&stereo(&late_change), 0, &template);

    assert_approx_eq!(early_score, late_score, 1e-9);
}
