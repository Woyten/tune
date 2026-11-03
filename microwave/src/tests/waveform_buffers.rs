use std::collections::HashMap;

use crate::magnetron::BufferNameMap;
use crate::synth::MagnetronSpec;

#[test]
fn buffer_names_are_local_to_waveforms() {
    let spec: MagnetronSpec = serde_yaml::from_str(
        r#"
note_input: Foreground
waveforms:
  - name: first
    envelope: ""
    stages:
      - stage_type: Generator
        out_buffer: foo
        generator_type: Oscillator
        oscillator_type: Sin
        frequency: 440.0
      - stage_type: Processor
        in_buffer: foo
        out_buffer: bar
        processor_type: Pass
  - name: second
    envelope: ""
    stages:
      - stage_type: Generator
        out_buffer: baz
        generator_type: Oscillator
        oscillator_type: Sin
        frequency: 440.0
      - stage_type: Processor
        in_buffer: baz
        out_buffer: qux
        processor_type: Pass
"#,
    )
    .unwrap();

    let (_, num_buffers) = spec.allocate_buffers(
        &HashMap::new(),
        &HashMap::new(),
        &mut BufferNameMap::default(),
    );

    assert_eq!(num_buffers, 2);
}
