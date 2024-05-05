use std::{
    fmt::Display,
    io::{Read, Seek, Write},
};

pub trait ReadAndSeek: Read + Seek + Send {}

impl<T> ReadAndSeek for T where T: Read + Seek + Send {}

pub trait WriteAndSeek: Write + Seek + Send {}

impl<T> WriteAndSeek for T where T: Write + Seek + Send {}

pub use platform_specific::*;

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

    use super::{ReadAndSeek, WriteAndSeek};

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

    pub use async_std::task::spawn as spawn_task;

    pub async fn read_file(file_name: &str) -> Result<Option<impl ReadAndSeek>, String> {
        let location = Path::new(file_name);

        location
            .exists()
            .then(|| File::open(location).map_err(|err| err.to_string()))
            .transpose()
    }

    pub async fn write_file(file_name: &str) -> Result<impl WriteAndSeek, String> {
        let location = Path::new(file_name);

        File::create(location).map_err(|err| err.to_string())
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
        js_sys::{Array, Uint8Array},
        request::IdbOpenDbRequestLike,
        web_sys::{File, IdbTransactionMode},
        IdbDatabase, IdbQuerySource, IdbVersionChangeEvent,
    };
    use log::{Level, LevelFilter, Log, Metadata, Record};
    use wasm_bindgen::JsValue;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::UrlSearchParams;

    use super::{ReadAndSeek, WriteAndSeek};

    pub fn init_environment() {
        panic::set_hook(Box::new(|panic_info| {
            DivLogger.log(
                &Record::builder()
                    .args(format_args!("{panic_info}"))
                    .level(Level::Error)
                    .target("panic")
                    .build(),
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
        DivLogger.log(
            &Record::builder()
                .args(format_args!("{message}"))
                .level(Level::Info)
                .target("stdout")
                .build(),
        );
    }

    pub fn eprint(message: impl Display) {
        DivLogger.log(
            &Record::builder()
                .args(format_args!("{message}"))
                .level(Level::Error)
                .target("stderr")
                .build(),
        );
    }

    struct DivLogger;

    impl Log for DivLogger {
        fn enabled(&self, metadata: &Metadata<'_>) -> bool {
            let target = metadata.target();

            if target.starts_with("wgpu_hal::gles::device")
                || target.starts_with("microwave::audio")
            {
                metadata.level() <= Level::Error
            } else if target.starts_with("microwave::profile") || target.starts_with("wgpu") {
                metadata.level() <= Level::Warn
            } else {
                metadata.level() <= Level::Info
            }
        }

        fn log(&self, record: &Record<'_>) {
            if !self.enabled(record.metadata()) {
                return;
            }

            let message = format!("[{}]\n{}", record.target(), record.args());

            let window = web_sys::window().unwrap();
            let document = window.document().unwrap();
            let log_element = document.get_element_by_id("log").unwrap();
            let log_entry_element = document.create_element("div").unwrap();
            log_entry_element.set_class_name(&format!("log_message {}", record.metadata().level()));
            log_entry_element.set_text_content(Some(message.trim()));
            log_element.append_child(&log_entry_element).unwrap();
        }

        fn flush(&self) {}
    }

    pub use async_std::task::spawn_local as spawn_task;

    const DB_NAME: &str = "microwave";
    const STORE_NAME: &str = "files";

    pub async fn read_file(file_name: &str) -> Result<Option<impl ReadAndSeek>, String> {
        read_file_using_indexed_db_api(file_name)
            .await
            .map_err(|err| err.as_string().expect("Not a string"))
    }

    async fn read_file_using_indexed_db_api(
        file_name: &str,
    ) -> Result<Option<impl ReadAndSeek>, JsValue> {
        let db = open_db().await?;
        let tx = db.transaction_on_one_with_mode(STORE_NAME, IdbTransactionMode::Readonly)?;
        let store = tx.object_store(STORE_NAME)?;

        Ok(match store.get_owned(file_name)?.await? {
            Some(file) => Some(Cursor::new(
                Uint8Array::new(&JsFuture::from(File::from(file).array_buffer()).await?).to_vec(),
            )),
            None => None,
        })
    }

    pub async fn write_file(file_name: &str) -> Result<impl WriteAndSeek, String> {
        Ok(IndexedDbWrite {
            file_name: file_name.to_owned(),
            data: Cursor::new(Vec::new()),
        })
    }

    struct IndexedDbWrite {
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

            spawn_task(async move {
                let _ = write_file_using_indexed_db_api(&file_name, &data).await;
            });
        }
    }

    async fn write_file_using_indexed_db_api(file_name: &str, data: &[u8]) -> Result<(), JsValue> {
        let db = open_db().await?;
        let tx = db.transaction_on_one_with_mode(STORE_NAME, IdbTransactionMode::Readwrite)?;
        let store = tx.object_store(STORE_NAME)?;

        store.put_key_val_owned(
            file_name,
            &File::new_with_u8_array_sequence(&Array::of1(&Uint8Array::from(data)), file_name)?,
        )?;

        Ok(())
    }

    async fn open_db() -> Result<IdbDatabase, JsValue> {
        let mut db_req = IdbDatabase::open(DB_NAME)?;
        db_req.set_on_upgrade_needed(Some(|evt: &IdbVersionChangeEvent| -> Result<(), JsValue> {
            if !evt.db().object_store_names().any(|n| n == STORE_NAME) {
                evt.db().create_object_store(STORE_NAME)?;
            }
            Ok(())
        }));
        Ok(db_req.await?)
    }
}
