use crate::visualizer::level_color;

#[test]
fn level_color_endpoints() {
    assert_eq!(level_color(0.0), [7, 10, 12, 255]);
    assert_eq!(level_color(1.0), [255, 63, 25, 255]);
}

#[test]
fn level_color_is_continuous_at_stop_boundaries() {
    for boundary in [0.25, 0.5, 0.75] {
        let before = level_color(boundary - 0.001);
        let after = level_color(boundary + 0.001);

        for channel in 0..3 {
            let difference = i16::from(before[channel]) - i16::from(after[channel]);
            assert!(
                difference.abs() <= 2,
                "discontinuity at level {boundary}, channel {channel}: {before:?} vs {after:?}"
            );
        }
    }
}
