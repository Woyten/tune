use assert_approx_eq::assert_approx_eq;

use crate::tests::visualizer::fixture::VisualizerFixture;
use crate::tests::visualizer::fixture::sine;

#[test]
fn spectrum_peak_matches_sine_frequency() {
    const SAMPLE_RATE: f64 = 48_000.0;
    const BIN: usize = 128;

    let mut fixture = VisualizerFixture::new(SAMPLE_RATE as u32);
    let window = fixture.window_frames();
    let frequency = BIN as f64 * SAMPLE_RATE / window as f64;
    let samples = sine(SAMPLE_RATE, frequency, window);

    fixture.push_mono(&samples);
    fixture.update();
    let magnitudes = fixture.spectrum();

    let peak_bin = magnitudes
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.total_cmp(b))
        .map(|(index, _)| index)
        .unwrap();

    assert_eq!(peak_bin, BIN);
    assert_approx_eq!(magnitudes[BIN], 0.0, 0.1);
}

#[test]
fn spectrum_does_not_cancel_antiphase_stereo() {
    const SAMPLE_RATE: f64 = 48_000.0;
    const BIN: usize = 128;

    let mut fixture = VisualizerFixture::new(SAMPLE_RATE as u32);
    let window = fixture.window_frames();
    let frequency = BIN as f64 * SAMPLE_RATE / window as f64;
    let left = sine(SAMPLE_RATE, frequency, window);
    let right = left.iter().map(|sample| -sample).collect::<Vec<_>>();

    fixture.push_stereo(&left, &right);
    fixture.update();
    let magnitudes = fixture.spectrum();

    assert_approx_eq!(magnitudes[BIN], 0.0, 0.1);
}

#[test]
fn silence_produces_minimum_magnitude() {
    let mut fixture = VisualizerFixture::new(48_000);
    let window = fixture.window_frames();
    let silence = vec![0.0; window];

    fixture.push_mono(&silence);
    fixture.update();

    assert!(
        fixture
            .spectrum()
            .iter()
            .all(|&magnitude| magnitude <= -119.0)
    );
}
