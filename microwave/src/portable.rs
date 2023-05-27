use std::{
    fs::File,
    io::{Read, Seek, Write},
    path::Path,
};

#[cfg(not(target_arch = "wasm32"))]
pub use async_std::task::spawn as spawn_task;

#[cfg(target_arch = "wasm32")]
pub use async_std::task::spawn_local as spawn_task;

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

pub trait ReadAndSeek: Read + Seek + Send {}

impl<T> ReadAndSeek for T where T: Read + Seek + Send {}

pub trait WriteAndSeek: Write + Seek + Send {}

impl<T> WriteAndSeek for T where T: Write + Seek + Send {}
