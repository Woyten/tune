use std::{env, fs::File};

use fluid_xenth::tune::{
    key::PianoKey,
    pitch::Pitch,
    scala::{self, KbmRoot},
};
use hound::{SampleFormat, WavSpec, WavWriter};
use oxisynth::{MidiEvent, SoundFont};
use tune::scala::SegmentType;

fn main() {
    let args: Vec<_> = env::args().collect();
    let sf_location = args
        .get(1)
        .expect("Expected soundfont file location as first argument");

    let per_semitone_polyphony = 4; // Handle up to 4 frequencies per semitone. This reduces the absolute limit for the number of xenharmonic channels to 64 = 256/4.
    let (mut xenth, mut control) =
        fluid_xenth::create_aot(Default::default(), per_semitone_polyphony).unwrap();

    xenth.synth_mut().add_font(
        SoundFont::load(&mut File::open(sf_location).unwrap()).unwrap(),
        false,
    );

    let mut audio_buffer = vec![0.0; 400000];

    let scl = scala::create_harmonics_scale(None, SegmentType::Otonal, 8, 8, None).unwrap();
    let kbm = KbmRoot {
        ref_key: PianoKey::from_midi_number(55),
        ref_pitch: Pitch::from_hz(200.0),
        root_offset: 0,
    }
    .to_kbm();
    let keys = PianoKey::from_midi_number(0).keys_before(PianoKey::from_midi_number(128));

    control.set_tuning(0, (scl, kbm), keys).unwrap();

    // Use send_channel_command to send messages to a xenharmonic channel.
    control
        .send_command(0, |s, channel| {
            s.send_event(MidiEvent::ProgramChange {
                channel,
                program_id: 50,
            })
        })
        .unwrap();

    control
        .send_command(0, |s, channel| {
            let channel_preset = s.channel_preset(channel).unwrap();
            println!("Preset on channel {}: {}", channel, channel_preset.name());
            Ok(())
        })
        .unwrap();

    // Define buffer write_callback callback function.
    let mut index = 0;
    let mut cb = |(l, r)| {
        audio_buffer[index] = l;
        index += 1;
        audio_buffer[index] = r;
        index += 1;
    };

    // Use note_{on,off} commands directly s.t. fluid-xenth can manage pressed keys.
    control
        .note_on(0, PianoKey::from_midi_number(55), 100)
        .unwrap();
    xenth.write(50000, &mut cb).unwrap();
    control
        .note_on(0, PianoKey::from_midi_number(61), 100)
        .unwrap();
    xenth.write(50000, &mut cb).unwrap();
    control
        .note_on(0, PianoKey::from_midi_number(66), 100)
        .unwrap();
    xenth.write(50000, &mut cb).unwrap();
    control.note_off(0, PianoKey::from_midi_number(55)).unwrap();
    control.note_off(0, PianoKey::from_midi_number(61)).unwrap();
    control.note_off(0, PianoKey::from_midi_number(66)).unwrap();
    xenth.write(50000, &mut cb).unwrap();

    let spec = WavSpec {
        channels: 2,
        sample_rate: 44100,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };
    let mut writer = WavWriter::create("demo_aot.wav", spec).unwrap();

    for sample in audio_buffer {
        writer.write_sample(sample).unwrap();
    }
}
