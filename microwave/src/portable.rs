use std::fmt::Display;

use tune_cli::shared;

pub use platform_specific::*;
pub use shared::portable::*;

pub fn println(message: impl Display) {
    print(format_args!("{message}\n"))
}

pub fn eprintln(message: impl Display) {
    eprint(format_args!("{message}\n"))
}

#[cfg(not(target_arch = "wasm32"))]
mod platform_specific {
    use std::{env, fmt::Display, fs::File, path::Path};

    use log::LevelFilter;
    use tune_cli::shared::error::ResultExt;

    pub fn init_environment() {
        env_logger::builder()
            .filter_level(LevelFilter::Info)
            .filter_module("wgpu", LevelFilter::Warn)
            .try_init()
            .unwrap();
    }

    pub fn get_args() -> Vec<String> {
        env::args().collect()
    }

    pub fn print(message: impl Display) {
        print!("{message}")
    }

    pub fn eprint(message: impl Display) {
        eprint!("{message}")
    }

    pub type FileRead = File;

    pub async fn read_file(file_name: &str) -> Result<Option<FileRead>, String> {
        let location = Path::new(file_name);

        location
            .exists()
            .then(|| File::open(location))
            .transpose()
            .handle_error("Could not read file")
    }

    pub type FileWrite = File;

    pub async fn write_file(file_name: &str) -> Result<FileWrite, String> {
        let location = Path::new(file_name);

        File::create(location).handle_error("Could not create file")
    }
}

#[cfg(target_arch = "wasm32")]
mod platform_specific {
    use std::{
        fmt::Display,
        io::{self, Cursor, Seek, SeekFrom, Write},
        mem, panic,
    };

    use indexed_db_futures::{
        database::Database, error::OpenDbError, prelude::*, transaction::TransactionMode,
    };
    use log::{Level, LevelFilter, Log, Metadata, Record};
    use tune_cli::shared::error::ResultExt;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{
        js_sys::{Array, Uint8Array},
        wasm_bindgen::{closure::Closure, JsCast, JsValue},
        File, UrlSearchParams,
    };

    pub fn init_environment() {
        panic::set_hook(Box::new(|panic_info| {
            log(
                &Record::builder()
                    .args(format_args!("{panic_info}"))
                    .level(Level::Error)
                    .target("panic")
                    .build(),
                None,
            );
        }));
        log::set_logger(&DivLogger)
            .map(|_| log::set_max_level(LevelFilter::Info))
            .unwrap();
    }

    pub fn get_args() -> Vec<String> {
        let window = web_sys::window().unwrap();
        let url_parameter_string = window.location().search().unwrap();
        let url_parameters = UrlSearchParams::new_with_str(&url_parameter_string).unwrap();

        let url_args: Vec<_> = url_parameters
            .get_all("a")
            .iter()
            .filter_map(|js_value| js_value.as_string())
            .collect();

        ["microwave".to_owned()]
            .into_iter()
            .chain(url_args)
            .collect()
    }

    pub fn print(message: impl Display) {
        log(
            &Record::builder()
                .args(format_args!("{message}"))
                .level(Level::Info)
                .target("stdout")
                .build(),
            None,
        );
    }

    pub fn eprint(message: impl Display) {
        log(
            &Record::builder()
                .args(format_args!("{message}"))
                .level(Level::Error)
                .target("stderr")
                .build(),
            None,
        );
    }

    struct DivLogger;

    impl Log for DivLogger {
        fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
            true
        }

        fn log(&self, record: &Record<'_>) {
            log(
                record,
                (record.metadata().level() >= Level::Error).then_some(15000),
            );
        }

        fn flush(&self) {}
    }

    fn log(record: &Record<'_>, timeout_ms: Option<i32>) {
        let message = format!("[{}]\n{}", record.target(), record.args());

        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();
        let log_element = document.get_element_by_id("log").unwrap();
        let log_entry_element = document.create_element("div").unwrap();
        log_entry_element.set_class_name(&format!("log_message {}", record.metadata().level()));
        log_entry_element.set_text_content(Some(message.trim()));
        log_element.append_child(&log_entry_element).unwrap();

        if let Some(timeout_ms) = timeout_ms {
            window
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    &Closure::<dyn FnMut()>::new(move || log_entry_element.remove())
                        .into_js_value()
                        .dyn_into()
                        .unwrap(),
                    timeout_ms,
                )
                .unwrap();
        }
    }

    const DB_NAME: &str = "microwave";
    const STORE_NAME: &str = "files";

    pub type FileRead = Cursor<Vec<u8>>;

    pub async fn read_file(file_name: &str) -> Result<Option<FileRead>, String> {
        read_file_using_indexed_db_api(file_name)
            .await
            .handle_error("Could not read file")
    }

    async fn read_file_using_indexed_db_api(
        file_name: &str,
    ) -> Result<Option<FileRead>, OpenDbError> {
        let db = open_db().await?;

        let tx = db
            .transaction(STORE_NAME)
            .with_mode(TransactionMode::Readonly)
            .build()?;

        let store = tx.object_store(STORE_NAME)?;

        Ok(
            match store.get::<JsValue, _, _>(file_name).primitive()?.await? {
                Some(file) => Some(Cursor::new(
                    Uint8Array::new(
                        &JsFuture::from(web_sys::File::from(file).array_buffer()).await?,
                    )
                    .to_vec(),
                )),
                None => None,
            },
        )
    }

    pub type FileWrite = IndexedDbWrite;

    pub async fn write_file(file_name: &str) -> Result<FileWrite, String> {
        Ok(IndexedDbWrite {
            file_name: file_name.to_owned(),
            data: Cursor::new(Vec::new()),
        })
    }

    pub struct IndexedDbWrite {
        file_name: String,
        data: Cursor<Vec<u8>>,
    }

    impl Write for IndexedDbWrite {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.data.write(buf)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Seek for IndexedDbWrite {
        fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
            self.data.seek(pos)
        }
    }

    impl Drop for IndexedDbWrite {
        fn drop(&mut self) {
            let file_name = mem::take(&mut self.file_name);
            let data = mem::take(self.data.get_mut());

            super::spawn_task(async move {
                let _ = write_file_using_indexed_db_api(&file_name, &data).await;
            });
        }
    }

    async fn write_file_using_indexed_db_api(
        file_name: &str,
        data: &[u8],
    ) -> Result<(), OpenDbError> {
        let db = open_db().await?;

        let tx = db
            .transaction(STORE_NAME)
            .with_mode(TransactionMode::Readwrite)
            .build()?;

        let store = tx.object_store(STORE_NAME)?;

        let file: &JsValue =
            &File::new_with_u8_array_sequence(&Array::of1(&Uint8Array::from(data)), file_name)
                .unwrap();

        store.add(file).with_key(file_name).build()?.await?;

        tx.commit().await?;

        Ok(())
    }

    async fn open_db() -> Result<Database, OpenDbError> {
        Database::open(DB_NAME)
            .with_on_upgrade_needed(|_, db| {
                if !db.object_store_names().any(|n| n == STORE_NAME) {
                    db.create_object_store(STORE_NAME).build()?;
                }
                Ok(())
            })
            .await
    }
}
