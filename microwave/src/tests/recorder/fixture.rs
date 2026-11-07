use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

use flume::Receiver;
use magnetron::Magnetron;
use magnetron::automation::AutomationFactory;
use magnetron::buffer::BufferIndex;
use magnetron::stage::Stage;

use crate::control::LiveParameterStorage;
use crate::magnetron::Factory;
use crate::magnetron::source::LfSourceExpr;
use crate::pipeline::PipelineEvent;
use crate::profile::PipelineParam;
use crate::recorder::RecorderWorkers;
use crate::recorder::WavRecorderSpec;

pub const SAMPLE_RATE: u32 = 48_000;
pub const BUFFER_SIZE: usize = 512;

static NEXT_DIRECTORY: AtomicU32 = AtomicU32::new(0);

pub struct RecorderFixture {
    stages: Vec<Stage<PipelineParam>>,
    workers: RecorderWorkers,
    events: Receiver<PipelineEvent>,
    storage: LiveParameterStorage,
    globals: HashMap<String, f64>,
    magnetron: Magnetron,
    left: BufferIndex,
    right: BufferIndex,
    dir: PathBuf,
}

impl RecorderFixture {
    pub fn new() -> Self {
        Self::new_with_skip_back(0.0)
    }

    pub fn new_with_skip_back(skip_back_secs: f64) -> Self {
        let dir = create_temp_dir();
        let file_prefix = dir.join("take").to_string_lossy().into_owned();
        Self::with_directory_and_prefix(dir, file_prefix, skip_back_secs)
    }

    pub fn new_with_failing_prefix() -> Self {
        let dir = create_temp_dir();
        let file_prefix = dir
            .join("missing")
            .join("take")
            .to_string_lossy()
            .into_owned();
        Self::with_directory_and_prefix(dir, file_prefix, 0.0)
    }

    fn with_directory_and_prefix(dir: PathBuf, file_prefix: String, skip_back_secs: f64) -> Self {
        let mut factory = Factory::new(AutomationFactory::new(HashMap::new()));
        let (events_send, events) = flume::unbounded();
        let mut stages = Vec::new();
        let mut workers = Vec::new();

        WavRecorderSpec {
            in_buffers: ("rec_left".into(), "rec_right".into()),
            skip_back_secs,
            file_prefix,
            recording_active: LfSourceExpr::Global("rec".to_owned()).wrap(),
        }
        .create(
            &mut factory,
            SAMPLE_RATE,
            &mut stages,
            &mut workers,
            &events_send,
        );

        let num_buffers = factory.num_internal_buffers();
        let magnetron = Magnetron::new(f64::from(SAMPLE_RATE).recip(), num_buffers, BUFFER_SIZE);

        Self {
            stages,
            workers: RecorderWorkers::new(workers),
            events,
            storage: LiveParameterStorage::default(),
            globals: HashMap::from([("rec".to_owned(), 0.0)]),
            magnetron,
            left: BufferIndex::Internal(0),
            right: BufferIndex::Internal(1),
            dir,
        }
    }

    pub fn set_active(&mut self, active: bool) {
        self.globals
            .insert("rec".to_owned(), if active { 1.0 } else { 0.0 });
    }

    pub fn render(&mut self, num_frames: usize, left: f64, right: f64) {
        let mut buffers = self.magnetron.prepare(num_frames);
        buffers.read_0_write_1(self.left, None, || left);
        buffers.read_0_write_1(self.right, None, || right);
        buffers.process((&(), &self.storage, &self.globals), self.stages.iter_mut());
    }

    pub fn poll(&mut self) {
        self.workers.poll();
    }

    pub fn drain_events(&self) -> Vec<PipelineEvent> {
        self.events.try_iter().collect()
    }

    pub fn files(&self) -> Vec<PathBuf> {
        let mut files = std::fs::read_dir(&self.dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "wav"))
            .collect::<Vec<_>>();
        files.sort();
        files
    }

    pub fn read_wav(path: &PathBuf) -> Vec<f32> {
        let mut reader = hound::WavReader::open(path).unwrap();
        reader
            .samples::<f32>()
            .map(|sample| sample.unwrap())
            .collect()
    }
}

impl Drop for RecorderFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn create_temp_dir() -> PathBuf {
    let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "microwave-recorder-test-{}-{id}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
