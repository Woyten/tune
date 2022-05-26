use std::{
    collections::{BTreeMap, HashMap},
    env,
    fs::File,
    io::Write,
    path::Path,
    thread,
    time::Instant,
};

use rand::prelude::SliceRandom;
use serde::{Deserialize, Serialize};
use tune::pitch::Pitch;
use tune_cli::{CliError, CliResult};

use crate::{
    assets,
    magnetron::{
        spec::{EnvelopeSpec, WaveformSpec},
        Magnetron,
    },
    synth::{ControlStorage, SynthControl},
};

const BUFFER_SIZE: usize = 1024;

pub fn run_benchmark() -> CliResult<()> {
    let mut report = load_performance_report()?;

    let full_spec = assets::get_builtin_waveforms();

    let mut waveform_specs = full_spec.waveforms;
    waveform_specs.shuffle(&mut rand::thread_rng());

    let mut envelope_specs = full_spec
        .envelopes
        .into_iter()
        .map(|spec| (spec.name.clone(), spec))
        .collect();

    for waveform_spec in waveform_specs {
        envelope_specs = run_benchmark_for_waveform(&mut report, waveform_spec, envelope_specs);
    }

    save_performance_report(&report)
}

fn run_benchmark_for_waveform(
    report: &mut PerformanceReport,
    waveform_spec: WaveformSpec<SynthControl>,
    mut envelope_specs: HashMap<String, EnvelopeSpec>,
) -> HashMap<String, EnvelopeSpec> {
    let mut magnetron = Magnetron::new(1.0 / 44100.0, 3, BUFFER_SIZE);
    let storage = ControlStorage::default();
    let mut waveform = waveform_spec.create_waveform(
        Pitch::from_hz(440.0),
        1.0,
        envelope_specs[&waveform_spec.envelope].create_envelope(),
    );

    let thread = thread::spawn(move || {
        let start = Instant::now();
        // 50 buffer render cycles = 1.16 seconds of audio at 44.1kHz
        for _ in 0..50 {
            // 25 simultaneous waveforms
            magnetron.clear(BUFFER_SIZE);
            for _ in 0..25 {
                magnetron.write(&mut waveform, &envelope_specs, &storage, 1.0);
            }
        }
        (magnetron, envelope_specs, start.elapsed())
    });

    let elapsed;
    (magnetron, envelope_specs, elapsed) = thread.join().unwrap();

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

    envelope_specs
}

pub fn analyze_benchmark() -> CliResult<()> {
    let mut csv_columns = Vec::new();
    let mut csv_data = BTreeMap::new();

    let mut report = load_performance_report()?;

    for (waveform_name, results) in &mut report.results {
        println!("{waveform_name}:");
        csv_columns.push(waveform_name);

        for (version, results) in results.iter_mut().rev() {
            results.sort_by(|a, b| a.partial_cmp(b).unwrap());

            let median = if results.is_empty() {
                continue;
            } else if results.len() % 2 == 1 {
                results[results.len() / 2]
            } else {
                (results[results.len() / 2 - 1] + results[results.len() / 2]) / 2.0
            };

            println!("  {version}: {median:.3}ms");
            csv_data
                .entry(version)
                .or_insert_with(BTreeMap::new)
                .insert(waveform_name, median);
        }
    }

    let analysis_location = Path::new("perf-analysis.csv");
    let mut file = File::create(analysis_location)?;

    for csv_column in &csv_columns {
        write!(file, ", {csv_column}")?;
    }
    writeln!(file)?;
    for (version, results) in csv_data.iter().rev() {
        write!(file, "{version}")?;
        for csv_column in &csv_columns {
            match results.get(csv_column) {
                Some(median) => write!(file, ", {median:.6}")?,
                None => write!(file, ",")?,
            }
        }
        writeln!(file)?;
    }
    Ok(())
}

fn load_performance_report() -> CliResult<PerformanceReport> {
    let report_location = Path::new("perf-report.yml");
    if report_location.exists() {
        let file = File::open(report_location)?;
        serde_yaml::from_reader(file)
            .map_err(|err| CliError::CommandError(format!("Could not deserialize file: {}", err)))
    } else {
        Ok(PerformanceReport::default())
    }
}

fn save_performance_report(report: &PerformanceReport) -> CliResult<()> {
    let report_location = Path::new("perf-report.yml");
    let file = File::create(report_location)?;
    serde_yaml::to_writer(file, report)
        .map_err(|err| CliError::CommandError(format!("Could not serialize file: {}", err)))
}

#[derive(Deserialize, Serialize, Default)]
struct PerformanceReport {
    results: BTreeMap<String, BTreeMap<String, Vec<f64>>>,
    control: f64,
}
