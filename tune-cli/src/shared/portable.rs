pub use platform_specific::*;

#[cfg(not(target_arch = "wasm32"))]
mod platform_specific {
    pub use async_std::task::spawn as spawn_task;
}

#[cfg(target_arch = "wasm32")]
mod platform_specific {
    pub use async_std::task::spawn_local as spawn_task;
}
