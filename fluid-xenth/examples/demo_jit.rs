use std::env;

use fluid_xenth::{
    fluidlite::{IsPreset, IsSettings, Settings, Synth},
    tune::pitch::Pitch,
};
use hound::{SampleFormat, WavSpec, WavWriter};

fn main() {
    let args: Vec<_> = env::args().collect();
    let sf_location = args
        .get(1)
        .expect("Expected soundfont file location as first argument");

    let settings = Settings::new().unwrap();
    settings
        .str_("synth.drums-channel.active")
        .unwrap()
        .set("no");

    let synth = Synth::new(settings).unwrap();
    synth.sfload(sf_location, true).unwrap();

    let polyphony = 4; // Handle 4 frequencies per semitone. This reduces the number of xenharmonic channels to 64 = 256/4.
    let (xenth, mut control) = fluid_xenth::create_jit(synth, polyphony);

    let mut audio_buffer = vec![0.0; 400000];

    // Use send_channel_command to send messages to a xenharmonic channel.
    control
        .send_command(0, |s, channel| s.program_change(channel, 50))
        .unwrap();

    control
        .send_command(0, |s, channel| {
            let channel_preset = s.get_channel_preset(channel).unwrap();
            let preset_name = channel_preset.get_name().unwrap();
            println!("Preset name: {}", preset_name);
            Ok(())
        })
        .unwrap();

    // Use note_{on,off} commands directly s.t. fluid-xenth can manage pressed keys.
    control.note_on(0, "A", Pitch::from_hz(200.0), 100).unwrap();
    xenth.write(&mut audio_buffer[0..100000]).unwrap();
    control.note_on(0, "B", Pitch::from_hz(350.0), 100).unwrap();
    xenth.write(&mut audio_buffer[100000..200000]).unwrap();
    control.note_on(0, "C", Pitch::from_hz(550.0), 100).unwrap();
    xenth.write(&mut audio_buffer[200000..300000]).unwrap();
    control.note_off(0, "A").unwrap();
    control.note_off(0, "B").unwrap();
    control.note_off(0, "C").unwrap();
    xenth.write(&mut audio_buffer[300000..400000]).unwrap();

    let spec = WavSpec {
        channels: 2,
        sample_rate: 44100,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };
    let mut writer = WavWriter::create("demo_jit.wav", spec).unwrap();

    for sample in audio_buffer {
        writer.write_sample(sample).unwrap();
    }
}
