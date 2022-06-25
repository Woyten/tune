mod assets;
mod audio;
mod bench;
mod fluid;
mod keyboard;
mod keypress;
mod magnetron;
mod midi;
mod model;
mod piano;
mod synth;
mod task;
mod tunable;
mod view;

use std::{env, io, path::PathBuf, process, sync::mpsc};

use crate::magnetron::effects::{DelayOptions, ReverbOptions, RotaryOptions};
use audio::{AudioModel, AudioOptions};
use clap::Parser;
use keyboard::KeyboardLayout;
use model::{Model, SourceId};
use nannou::{app::App, wgpu::Backends};
use piano::{Backend, NoAudio, PianoEngine};
use synth::ControlChangeNumbers;
use tune::{
    key::{Keyboard, PianoKey},
    note::NoteLetter,
    pitch::Ratio,
    scala::{Kbm, Scl},
    temperament::{EqualTemperament, TemperamentPreference},
};
use tune_cli::{
    shared::{
        self,
        midi::{MidiInArgs, MidiOutArgs, TuningMethod},
        KbmOptions, SclCommand,
    },
    CliError, CliResult,
};
use view::DynViewModel;

#[derive(Parser)]
enum MainOptions {
    /// Start the microwave GUI
    #[clap(name = "run")]
    Run(RunOptions),

    /// Use a keyboard mapping with the given reference note
    #[clap(name = "ref-note")]
    WithRefNote {
        #[clap(flatten)]
        kbm: KbmOptions,

        #[clap(flatten)]
        options: RunOptions,
    },

    /// Use a kbm file
    #[clap(name = "kbm-file")]
    UseKbmFile {
        /// The location of the kbm file to import
        kbm_file_location: PathBuf,

        #[clap(flatten)]
        options: RunOptions,
    },

    /// List MIDI devices
    #[clap(name = "devices")]
    Devices,

    /// Run benchmark
    #[clap(name = "bench")]
    Bench {
        /// Analyze benchmark
        #[clap(long = "analyze")]
        analyze: bool,
    },
}

const TUN_METHOD_ARG: &str = "tun-method";
#[derive(Parser)]
struct RunOptions {
    /// MIDI input device
    #[clap(long = "midi-in")]
    midi_in_device: Option<String>,

    #[clap(flatten)]
    midi_in_args: MidiInArgs,

    /// MIDI output device
    #[clap(long = "midi-out")]
    midi_out_device: Option<String>,

    #[clap(flatten)]
    midi_out_args: MidiOutArgs,

    /// MIDI-out tuning method
    #[clap(arg_enum, long = TUN_METHOD_ARG)]
    midi_tuning_method: Option<TuningMethod>,

    /// Waveforms file location (waveform synth)
    #[clap(
        long = "wv-loc",
        env = "MICROWAVE_WV_LOC",
        default_value = "waveforms.yml"
    )]
    waveforms_file_location: PathBuf,

    /// Number of waveform buffers to allocate
    #[clap(long = "wv-bufs", default_value = "8")]
    num_waveform_buffers: usize,

    #[clap(flatten)]
    control_change: ControlChangeParameters,

    /// Pitch wheel sensitivity (waveform synth)
    #[clap(long = "pwsens", default_value = "200c")]
    pitch_wheel_sensitivity: Ratio,

    /// Enable logging
    #[clap(long = "log")]
    logging: bool,

    /// Enable fluidlite using the soundfont file at the given location
    #[clap(long = "sf-loc", env = "MICROWAVE_SF_LOC")]
    soundfont_file_location: Option<PathBuf>,

    #[clap(flatten)]
    audio: AudioParameters,

    #[clap(flatten)]
    reverb: ReverbParameters,

    #[clap(flatten)]
    delay: DelayParameters,

    #[clap(flatten)]
    rotary: RotaryParameters,

    /// Program number that should be selected at startup
    #[clap(long = "pg", default_value = "0")]
    program_number: u8,

    /// Use porcupine layout when possible
    #[clap(long = "porcupine")]
    use_porcupine: bool,

    /// Primary step width (right direction) when playing on the computer keyboard
    #[clap(long = "p-step")]
    primary_step: Option<i16>,

    /// Secondary step width (down/right direction) when playing on the computer keyboard
    #[clap(long = "s-step")]
    secondary_step: Option<i16>,

    /// Physical keyboard layout.
    /// [ansi] Large backspace key, horizontal enter key, large left shift key.
    /// [var] Subdivided backspace key, large enter key, large left shift key.
    /// [iso] Large backspace key, vertical enter key, subdivided left shift key.
    #[clap(long = "keyb", default_value = "iso")]
    keyboard_layout: KeyboardLayout,

    /// Odd limit for frequency ratio indicators
    #[clap(long = "lim", default_value = "11")]
    odd_limit: u16,

    /// Render a second scale-specific keyboard using the given color pattern (e.g. wgrwwgrwgrwgrwwgr for 17-EDO)
    #[clap(long = "kb2", parse(try_from_str=parse_keyboard_colors))]
    second_keyboard_colors: Option<KeyColors>,

    #[clap(subcommand)]
    scl: Option<SclCommand>,
}

#[derive(Parser)]
struct ControlChangeParameters {
    /// Modulation control number (MIDI -> waveform synth)
    #[clap(long = "modulation-ccn", default_value = "1")]
    modulation_ccn: u8,

    /// Breath control number (MIDI -> waveform synth)
    #[clap(long = "breath-ccn", default_value = "2")]
    breath_ccn: u8,

    /// Foot control number (MIDI -> waveform synth)
    #[clap(long = "foot-ccn", default_value = "4")]
    foot_ccn: u8,

    /// Expression control number (MIDI -> waveform synth)
    #[clap(long = "expression-ccn", default_value = "11")]
    expression_ccn: u8,

    /// Damper pedal control number (MIDI -> waveform synth)
    #[clap(long = "damper-ccn", default_value = "64")]
    damper_ccn: u8,

    /// Sostenuto pedal control number (MIDI -> waveform synth)
    #[clap(long = "sostenuto-ccn", default_value = "66")]
    sostenuto_ccn: u8,

    /// Soft pedal control number (MIDI -> waveform synth)
    #[clap(long = "soft-ccn", default_value = "67")]
    soft_ccn: u8,

    /// Mouse Y control number (microwave GUI -> MIDI)
    #[clap(long = "mouse-ccn", default_value = "2")]
    mouse_y_ccn: u8,
}

#[derive(Parser)]
struct AudioParameters {
    /// Enable audio-in
    #[clap(long = "audio-in")]
    audio_in_enabled: bool,

    /// Audio-out buffer size in frames
    #[clap(long = "out-buf", default_value = "1024")]
    out_buffer_size: u32,

    /// Audio-in buffer size in frames
    #[clap(long = "in-buf", default_value = "1024")]
    in_buffer_size: u32,

    /// Size of the ring buffer piping data from audio-in to audio-out in frames
    #[clap(long = "exc-buf", default_value = "8192")]
    exchange_buffer_size: usize,

    /// Sample rate [Hz]. If no value is specified the audio device's preferred value will be used
    #[clap(long = "s-rate")]
    sample_rate: Option<u32>,

    /// Prefix for wav file recordings
    #[clap(long = "wav-prefix", default_value = "microwave")]
    wav_file_prefix: String,
}

#[derive(Parser)]
struct ReverbParameters {
    /// Short-response diffusing delay lines (ms)
    #[clap(
        long = "rev-aps",
        use_value_delimiter = true,
        default_value = "5.10,7.73,10.00,12.61"
    )]
    reverb_allpasses: Vec<f64>,

    /// Short-response diffuse feedback
    #[clap(long = "rev-ap-fb", default_value = "0.5")]
    reverb_allpass_feedback: f64,

    /// Long-response resonating delay lines (ms)
    #[clap(
        long = "rev-combs",
        use_value_delimiter = true,
        default_value = "25.31,26.94,28.96,30.75,32.24,33.81,35.31,36.67"
    )]
    reverb_combs: Vec<f64>,

    /// Long-response resonant feedback
    #[clap(long = "rev-comb-fb", default_value = "0.95")]
    reverb_comb_feedback: f64,

    /// Long-response damping cutoff (Hz)
    #[clap(long = "rev-cutoff", default_value = "5600.0")]
    reverb_cutoff: f64,

    /// Additional delay (ms) on right channel for an enhanced stereo effect
    #[clap(long = "rev-stereo", default_value = "0.52")]
    reverb_stereo: f64,

    /// Balance between original and reverbed signal (0.0 = original signal only, 1.0 = reverbed signal only)
    #[clap(long = "rev-wet", default_value = "0.5")]
    reverb_wetness: f64,
}

#[derive(Parser)]
struct DelayParameters {
    /// Delay time (s)
    #[clap(long = "del-tm", default_value = "0.5")]
    delay_time: f64,

    /// Delay feedback
    #[clap(long = "del-fb", default_value = "0.6")]
    delay_feedback: f64,

    /// Delay feedback rotation angle (degrees clock-wise)
    #[clap(long = "del-rot", default_value = "135")]
    delay_feedback_rotation: f64,
}

#[derive(Parser)]
struct RotaryParameters {
    /// Rotary speaker radius (cm)
    #[clap(long = "rot-rad", default_value = "20")]
    rotation_radius: f64,

    /// Rotary speaker minimum speed (revolutions per s)
    #[clap(long = "rot-min", default_value = "1")]
    rotation_min_frequency: f64,

    /// Rotary speaker maximum speed (revolutions per s)
    #[clap(long = "rot-max", default_value = "7")]
    rotation_max_frequency: f64,

    /// Rotary speaker acceleration time (s)
    #[clap(long = "rot-acc", default_value = "1")]
    rotation_acceleration: f64,

    /// Rotary speaker deceleration time (s)
    #[clap(long = "rot-dec", default_value = "0.5")]
    rotation_deceleration: f64,
}

struct KeyColors(Vec<KeyColor>);

#[derive(Clone, Copy)]
pub enum KeyColor {
    White,
    Red,
    Green,
    Blue,
    Cyan,
    Magenta,
    Yellow,
    Black,
}

fn parse_keyboard_colors(src: &str) -> Result<KeyColors, String> {
    src.chars()
        .map(|c| match c {
            'w' => Ok(KeyColor::White),
            'r' => Ok(KeyColor::Red),
            'g' => Ok(KeyColor::Green),
            'b' => Ok(KeyColor::Blue),
            'c' => Ok(KeyColor::Cyan),
            'm' => Ok(KeyColor::Magenta),
            'y' => Ok(KeyColor::Yellow),
            'k' => Ok(KeyColor::Black),
            c => Err(c),
        })
        .collect::<Result<Vec<_>, char>>()
        .map(KeyColors)
        .map_err(|c| {
            format!(
                "Received an invalid character '{}'. Only wrgbcmyk are allowed.",
                c
            )
        })
}

fn main() {
    let model_fn = match env::args().collect::<Vec<_>>().as_slice() {
        [_] => {
            println!(
                "[WARNING] Use a subcommand, e.g. `microwave run` to start microwave properly"
            );
            create_app_from_empty_args
        }
        [_, arg, ..] if arg == "bench" => {
            create_model_from_main_options(MainOptions::parse()).unwrap();
            return;
        }
        [..] => create_app_from_given_args,
    };

    nannou::app(model_fn)
        .backends(Backends::PRIMARY | Backends::GL)
        .update(model::update)
        .run();
}

fn create_app_from_empty_args(app: &App) -> Model {
    create_app_from_main_options(app, MainOptions::parse_from(["xx", "run"]))
}

fn create_app_from_given_args(app: &App) -> Model {
    create_app_from_main_options(app, MainOptions::parse())
}

fn create_app_from_main_options(app: &App, options: MainOptions) -> Model {
    match create_model_from_main_options(options) {
        Ok(model) => {
            create_window(app);
            model
        }
        Err(err) => {
            eprintln!("[FAIL] {:?}", err);
            process::exit(1);
        }
    }
}

fn create_model_from_main_options(options: MainOptions) -> CliResult<Model> {
    match options {
        MainOptions::Run(options) => Kbm::builder(NoteLetter::D.in_octave(4))
            .build()
            .map_err(CliError::from)
            .and_then(|kbm| create_model_from_run_options(kbm, options)),
        MainOptions::WithRefNote { kbm, options } => kbm
            .to_kbm()
            .map_err(CliError::from)
            .and_then(|kbm| create_model_from_run_options(kbm, options)),
        MainOptions::UseKbmFile {
            kbm_file_location,
            options,
        } => shared::import_kbm_file(&kbm_file_location)
            .map_err(CliError::from)
            .and_then(|kbm| create_model_from_run_options(kbm, options)),
        MainOptions::Devices => {
            let stdout = io::stdout();
            shared::midi::print_midi_devices(stdout.lock(), "microwave").unwrap();
            process::exit(0);
        }
        MainOptions::Bench { analyze } => {
            if analyze {
                bench::analyze_benchmark()?;
            } else {
                bench::run_benchmark()?;
            }
            process::exit(0);
        }
    }
}

fn create_model_from_run_options(kbm: Kbm, options: RunOptions) -> CliResult<Model> {
    let scl = options
        .scl
        .as_ref()
        .map(|command| command.to_scl(None))
        .transpose()
        .map_err(|x| format!("error ({:?})", x))?
        .unwrap_or_else(|| {
            Scl::builder()
                .push_ratio(Ratio::from_semitones(1))
                .build()
                .unwrap()
        });

    let keyboard = create_keyboard(&scl, &options);

    let (send, recv) = mpsc::channel::<DynViewModel>();

    let mut backends = Vec::<Box<dyn Backend<SourceId>>>::new();

    if let Some(target_port) = options.midi_out_device {
        let midi_backend = midi::create(
            send.clone(),
            &target_port,
            options.midi_out_args,
            options
                .midi_tuning_method
                .ok_or_else(|| format!("MIDI out requires --{} argument", TUN_METHOD_ARG))?,
        )?;
        backends.push(Box::new(midi_backend));
    }

    let output_stream_params =
        audio::get_output_stream_params(options.audio.out_buffer_size, options.audio.sample_rate);
    let sample_rate = output_stream_params.1.sample_rate;
    let sample_rate_hz_u32 = sample_rate.0;
    let sample_rate_hz_f64 = f64::from(sample_rate_hz_u32);

    let (fluid_backend, fluid_synth) = fluid::create(
        send.clone(),
        options.soundfont_file_location.as_deref(),
        sample_rate_hz_f64,
    );
    if options.soundfont_file_location.is_some() {
        backends.push(Box::new(fluid_backend));
    }

    let (waveform_backend, waveform_synth) = synth::create(
        send.clone(),
        &options.waveforms_file_location,
        options.pitch_wheel_sensitivity,
        options.control_change.to_cc_numbers(),
        options.num_waveform_buffers,
        options.audio.out_buffer_size,
        sample_rate_hz_f64,
    )?;
    backends.push(Box::new(waveform_backend));
    backends.push(Box::new(NoAudio::new(send)));

    let (engine, engine_snapshot) =
        PianoEngine::new(scl.clone(), kbm, backends, options.program_number);

    let audio = AudioModel::new(
        fluid_synth,
        waveform_synth,
        output_stream_params,
        options.audio.into_options(),
        options.reverb.into_options(),
        options.delay.to_options(),
        options.rotary.to_options(),
    );

    let midi_in = options
        .midi_in_device
        .map(|midi_in_device| {
            midi::connect_to_midi_device(
                engine.clone(),
                &midi_in_device,
                options.midi_in_args,
                options.logging,
            )
        })
        .transpose()?
        .map(|(_, connection)| connection);

    let mut model = Model::new(
        audio,
        engine,
        engine_snapshot,
        scl,
        options
            .second_keyboard_colors
            .map(|colors| colors.0)
            .unwrap_or_else(Vec::new),
        keyboard,
        options.keyboard_layout,
        options.odd_limit,
        midi_in,
        options.control_change.mouse_y_ccn,
        recv,
    );
    model.toggle_reverb();
    Ok(model)
}

fn create_keyboard(scl: &Scl, config: &RunOptions) -> Keyboard {
    let preference = if config.use_porcupine {
        TemperamentPreference::Porcupine
    } else {
        TemperamentPreference::PorcupineWhenMeantoneIsBad
    };

    let average_step_size = scl.period().divided_into_equal_steps(scl.num_items());

    let temperament = EqualTemperament::find()
        .with_preference(preference)
        .by_step_size(average_step_size);

    let keyboard = Keyboard::root_at(PianoKey::from_midi_number(0))
        .with_steps_of(&temperament)
        .coprime();

    let primary_step = config
        .primary_step
        .unwrap_or_else(|| keyboard.primary_step());
    let secondary_step = config
        .secondary_step
        .unwrap_or_else(|| keyboard.secondary_step());

    keyboard.with_steps(primary_step, secondary_step)
}

fn create_window(app: &App) {
    app.new_window()
        .maximized(true)
        .title("Microwave - Microtonal Waveform Synthesizer by Woyten")
        .raw_event(model::raw_event)
        .key_pressed(model::key_pressed)
        .mouse_pressed(model::mouse_pressed)
        .mouse_moved(model::mouse_moved)
        .mouse_released(model::mouse_released)
        .mouse_wheel(model::mouse_wheel)
        .touch(model::touch)
        .view(view::view)
        .build()
        .unwrap();
}

impl ControlChangeParameters {
    fn to_cc_numbers(&self) -> ControlChangeNumbers {
        ControlChangeNumbers {
            modulation: self.modulation_ccn,
            breath: self.breath_ccn,
            foot: self.foot_ccn,
            expression: self.expression_ccn,
            damper: self.damper_ccn,
            sostenuto: self.sostenuto_ccn,
            soft: self.soft_ccn,
        }
    }
}

impl AudioParameters {
    fn into_options(self) -> AudioOptions {
        AudioOptions {
            audio_in_enabled: self.audio_in_enabled,
            output_buffer_size: self.out_buffer_size,
            input_buffer_size: self.in_buffer_size,
            exchange_buffer_size: self.exchange_buffer_size,
            wav_file_prefix: self.wav_file_prefix,
        }
    }
}

impl ReverbParameters {
    fn into_options(self) -> ReverbOptions {
        ReverbOptions {
            allpasses_ms: self.reverb_allpasses,
            allpass_feedback: self.reverb_allpass_feedback,
            combs_ms: self.reverb_combs,
            comb_feedback: self.reverb_comb_feedback,
            stereo_ms: self.reverb_stereo,
            cutoff_hz: self.reverb_cutoff,
            wetness: self.reverb_wetness,
        }
    }
}

impl DelayParameters {
    fn to_options(&self) -> DelayOptions {
        DelayOptions {
            delay_time_in_s: self.delay_time,
            feedback_intensity: self.delay_feedback,
            feedback_rotation: self.delay_feedback_rotation.to_radians(),
        }
    }
}

impl RotaryParameters {
    fn to_options(&self) -> RotaryOptions {
        RotaryOptions {
            rotation_radius_in_cm: self.rotation_radius,
            min_frequency_in_hz: self.rotation_min_frequency,
            max_frequency_in_hz: self.rotation_max_frequency,
            acceleration_time_in_s: self.rotation_acceleration,
            deceleration_time_in_s: self.rotation_deceleration,
        }
    }
}
