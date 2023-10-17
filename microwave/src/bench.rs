use std::{
    collections::BTreeMap, env, fs::File, hint, io::Write, path::Path, thread, time::Instant,
};

use log::info;
use magnetron::{
    creator::Creator,
    stage::{Stage, StageActivity},
    Magnetron,
};
use rand::prelude::SliceRandom;
use serde::{Deserialize, Serialize};
use tune_cli::{CliError, CliResult};

use crate::{assets, control::LiveParameterStorage, magnetron::waveform::WaveformProperties};

const BUFFER_SIZE: u16 = 1024;
const SAMPLE_WIDTH_SECS: f64 = 1.0 / 44100.0;
const NUM_RENDER_CYCLES: u16 = 50;
const NUM_SIMULTANEOUS_WAVEFORMS: u16 = 25;

pub fn run_benchmark() -> CliResult {
    let mut report = load_performance_report()?;

    let profile = assets::get_default_profile();

    let mut magnetron_spec = assets::get_default_magnetron_spec();
    magnetron_spec.waveforms.shuffle(&mut rand::thread_rng());

    let templates = profile
        .waveform_templates
        .into_iter()
        .map(|spec| (spec.name, spec.value))
        .collect();

    let envelopes = profile
        .waveform_envelopes
        .into_iter()
        .map(|spec| (spec.name, spec.spec))
        .collect();

    let creator = Creator::new(templates);

    for waveform_spec in magnetron_spec.waveforms {
        let waveform = waveform_spec.use_creator(&creator, &envelopes);

        run_benchmark_for_waveform(
            &mut report,
            profile.num_buffers,
            magnetron_spec.num_buffers,
            waveform_spec.name,
            waveform,
        );
    }

    save_performance_report(&report)
}

fn run_benchmark_for_waveform(
    report: &mut PerformanceReport,
    num_microwave_buffers: usize,
    num_waveform_buffers: usize,
    waveform_name: String,
    mut waveform: Vec<Stage<(WaveformProperties, LiveParameterStorage)>>,
) {
    let mut magnetron = Magnetron::new(
        SAMPLE_WIDTH_SECS,
        num_microwave_buffers,
        usize::from(BUFFER_SIZE),
    );

    let mut waveforms_magnetron = Magnetron::new(
        SAMPLE_WIDTH_SECS,
        num_waveform_buffers,
        usize::from(BUFFER_SIZE),
    );
    let mut waveforms_stage = Stage::new(move |buffers, _| {
        for _ in 0..NUM_SIMULTANEOUS_WAVEFORMS {
            waveforms_magnetron.prepare_nested(buffers).process(
                &(WaveformProperties::initial(440.0, 1.0), Default::default()),
                &mut waveform,
            );
        }
        StageActivity::Internal
    });

    let thread = thread::spawn(move || {
        let start = Instant::now();
        for _ in 0..NUM_RENDER_CYCLES {
            magnetron
                .prepare(usize::from(BUFFER_SIZE), false)
                .process(&(), [&mut waveforms_stage]);
            magnetron = hint::black_box(magnetron);
        }

        start.elapsed()
    });

    let elapsed = thread.join().unwrap();

    let rendered_time = f64::from(BUFFER_SIZE)
        * f64::from(NUM_RENDER_CYCLES)
        * f64::from(NUM_SIMULTANEOUS_WAVEFORMS)
        * SAMPLE_WIDTH_SECS;
    let time_consumption = elapsed.as_secs_f64() / rendered_time;

    let executable_name = env::args().next().unwrap();
    report
        .results
        .entry(waveform_name)
        .or_default()
        .entry(executable_name)
        .or_default()
        .push(time_consumption * 1000.0);
}

pub fn analyze_benchmark() -> CliResult {
    let mut csv_columns = Vec::new();
    let mut csv_data = BTreeMap::<_, BTreeMap<_, _>>::new();

    let mut report = load_performance_report()?;

    for (waveform_name, results) in &mut report.results {
        info!("{waveform_name}:");
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

            info!("  {version}: {median:.3} ‰");
            csv_data
                .entry(version)
                .or_default()
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
            .map_err(|err| CliError::CommandError(format!("Could not deserialize file: {err}")))
    } else {
        Ok(PerformanceReport::default())
    }
}

fn save_performance_report(report: &PerformanceReport) -> CliResult {
    let report_location = Path::new("perf-report.yml");
    let file = File::create(report_location)?;
    serde_yaml::to_writer(file, report)
        .map_err(|err| CliError::CommandError(format!("Could not serialize file: {err}")))
}

#[derive(Deserialize, Serialize, Default)]
struct PerformanceReport {
    results: BTreeMap<String, BTreeMap<String, Vec<f64>>>,
    control: f64, // No longer in use (replaced with hint::black_box) but required for compatibility with older benchmark runs
}
