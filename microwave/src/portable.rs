use std::io::{Read, Seek, Write};

pub trait ReadAndSeek: Read + Seek + Send {}

impl<T> ReadAndSeek for T where T: Read + Seek + Send {}

pub trait WriteAndSeek: Write + Seek + Send {}

impl<T> WriteAndSeek for T where T: Write + Seek + Send {}

pub use platform_specific::*;

#[cfg(not(target_arch = "wasm32"))]
mod platform_specific {
    use std::{fs::File, path::Path};

    use super::{ReadAndSeek, WriteAndSeek};

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
        io::{self, Cursor, Seek, SeekFrom, Write},
        mem,
    };

    use indexed_db_futures::{
        js_sys::{Array, Uint8Array},
        request::IdbOpenDbRequestLike,
        web_sys::{File, IdbTransactionMode},
        IdbDatabase, IdbQuerySource, IdbVersionChangeEvent,
    };
    use wasm_bindgen::JsValue;
    use wasm_bindgen_futures::JsFuture;

    use super::{ReadAndSeek, WriteAndSeek};

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
        Ok(db_req.into_future().await?)
    }
}
