use std::{collections::BTreeMap, env, fs::File, io::Write, path::Path, thread, time::Instant};

use magnetron::{spec::Creator, waveform::WaveformProperties, Magnetron};
use rand::prelude::SliceRandom;
use serde::{Deserialize, Serialize};
use tune_cli::{CliError, CliResult};

use crate::{
    assets,
    control::{LiveParameter, LiveParameterStorage},
    magnetron::{source::LfSource, WaveformProperty, WaveformSpec},
};

const BUFFER_SIZE: u16 = 1024;
const SAMPLE_WIDTH_SECS: f64 = 1.0 / 44100.0;
const NUM_RENDER_CYCLES: u16 = 50;
const NUM_SIMULTANEOUS_WAVEFORMS: u16 = 25;

pub fn run_benchmark() -> CliResult<()> {
    let mut report = load_performance_report()?;

    let mut full_spec = assets::get_builtin_waveforms();

    full_spec.waveforms.shuffle(&mut rand::thread_rng());

    let templates = full_spec
        .waveform_templates
        .into_iter()
        .map(|spec| (spec.name, spec.value))
        .collect();

    let envelopes = full_spec
        .waveform_envelopes
        .into_iter()
        .map(|spec| (spec.name, spec.spec))
        .collect();
    let creator = Creator::new(templates, envelopes);

    for waveform_spec in full_spec.waveforms {
        run_benchmark_for_waveform(&mut report, &creator, waveform_spec);
    }

    save_performance_report(&report)
}

fn run_benchmark_for_waveform(
    report: &mut PerformanceReport,
    creator: &Creator<LfSource<WaveformProperty, LiveParameter>>,
    waveform_spec: WaveformSpec<LfSource<WaveformProperty, LiveParameter>>,
) {
    let mut magnetron = Magnetron::new(SAMPLE_WIDTH_SECS, 3, usize::from(BUFFER_SIZE));

    let mut waveform = creator.create(&waveform_spec);
    let properties = WaveformProperties::initial(440.0, 1.0);

    let payload = (properties, LiveParameterStorage::default());

    let thread = thread::spawn(move || {
        let start = Instant::now();
        for _ in 0..NUM_RENDER_CYCLES {
            magnetron.clear(usize::from(BUFFER_SIZE));
            for _ in 0..NUM_SIMULTANEOUS_WAVEFORMS {
                magnetron.write(&mut waveform, &payload);
            }
        }

        (magnetron, start.elapsed())
    });

    let elapsed;
    (magnetron, elapsed) = thread.join().unwrap();

    let rendered_time = f64::from(BUFFER_SIZE)
        * f64::from(NUM_RENDER_CYCLES)
        * f64::from(NUM_SIMULTANEOUS_WAVEFORMS)
        * SAMPLE_WIDTH_SECS;
    let time_consumption = elapsed.as_secs_f64() / rendered_time;

    let executable_name = env::args().next().unwrap();
    report
        .results
        .entry(waveform_spec.name)
        .or_insert_with(BTreeMap::new)
        .entry(executable_name)
        .or_insert_with(Vec::new)
        .push(time_consumption * 1000.0);

    // Make sure all elements are evaluated and not optimized away
    report.control = (report.control + magnetron.mix().iter().sum::<f64>()).recip();
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

            println!("  {version}: {median:.3} ‰");
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
