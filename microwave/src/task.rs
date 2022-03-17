#[cfg(not(target_arch = "wasm32"))]
pub use async_std::task::spawn;

#[cfg(target_arch = "wasm32")]
pub use async_std::task::spawn_local as spawn;
