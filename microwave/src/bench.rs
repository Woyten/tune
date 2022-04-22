use std::{
    collections::{BTreeMap, HashMap},
    env,
    fs::File,
    path::Path,
    time::Instant,
};

use serde::{Deserialize, Serialize};
use tune::pitch::Pitch;

use crate::{
    assets,
    magnetron::{
        spec::{EnvelopeSpec, WaveformSpec},
        Magnetron,
    },
    synth::{ControlStorage, SynthControl},
};

const BUFFER_SIZE: usize = 1024;

pub fn run_benchmark() {
    let mut report = load_performance_report();

    let full_spec = assets::get_builtin_waveforms();
    let waveform_specs = full_spec.waveforms;
    let envelope_specs = full_spec
        .envelopes
        .into_iter()
        .map(|spec| (spec.name.clone(), spec))
        .collect();

    for waveform_spec in &waveform_specs {
        run_benchmark_for_waveform(&mut report, waveform_spec, &envelope_specs);
    }

    save_performance_report(&report);
}

fn run_benchmark_for_waveform(
    report: &mut PerformanceReport,
    waveform_spec: &WaveformSpec<SynthControl>,
    envelope_specs: &HashMap<String, EnvelopeSpec>,
) {
    let mut magnetron = Magnetron::new(3, BUFFER_SIZE);
    let storage = ControlStorage::default();
    let mut waveform = waveform_spec.create_waveform(
        Pitch::from_hz(440.0),
        1.0,
        envelope_specs[&waveform_spec.envelope].create_envelope(),
    );

    let start = Instant::now();
    // 50 buffer render cycles = 1.16 seconds of audio at 44.1kHz
    for _ in 0..50 {
        // 25 simultaneous waveforms
        magnetron.clear(BUFFER_SIZE);
        for _ in 0..25 {
            magnetron.write(&mut waveform, envelope_specs, &storage, 1.0, 1.0 / 44100.0);
        }
    }
    let elapsed = start.elapsed();

    let executable_name = env::args().next().unwrap();
    report
        .results
        .entry(waveform_spec.name().to_owned())
        .or_insert_with(BTreeMap::new)
        .entry(executable_name)
        .or_insert_with(Vec::new)
        .push(elapsed.as_secs_f64() * 100.0);

    // Make sure all elements are evaluated and not optimized away
    report.control = (report.control + magnetron.total().iter().sum::<f64>()).recip();
}

pub fn analyze_benchmark() {
    let mut report = load_performance_report();

    for (waveform_name, results) in &mut report.results {
        println!("{waveform_name}:");

        for (version, results) in results {
            results.sort_by(|a, b| a.partial_cmp(b).unwrap());

            let median = if results.is_empty() {
                continue;
            } else if results.len() % 2 == 1 {
                results[results.len() / 2]
            } else {
                (results[results.len() / 2 - 1] + results[results.len() / 2]) / 2.0
            };

            println!("  {version}: {median:.3}ms");
        }
    }
}

fn load_performance_report() -> PerformanceReport {
    let report_location = Path::new("perf-report.yml");
    if report_location.exists() {
        let file = File::open(report_location).unwrap();
        serde_yaml::from_reader(file).unwrap()
    } else {
        PerformanceReport::default()
    }
}

fn save_performance_report(report: &PerformanceReport) {
    let report_location = Path::new("perf-report.yml");
    let file = File::create(report_location).unwrap();
    serde_yaml::to_writer(file, report).unwrap();
}

#[derive(Deserialize, Serialize, Default)]
struct PerformanceReport {
    results: BTreeMap<String, BTreeMap<String, Vec<f64>>>,
    control: f64,
}
