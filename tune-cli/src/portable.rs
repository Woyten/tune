pub use platform_specific::*;

#[cfg(not(target_arch = "wasm32"))]
mod platform_specific {
    pub trait SendTask: Send {}

    impl<T: Send> SendTask for T {}

    pub use async_std::task::spawn as spawn_task;
}

#[cfg(target_arch = "wasm32")]
mod platform_specific {
    pub trait SendTask {}

    impl<T> SendTask for T {}

    // On WASM environments, block_on doesn't block but spawns a thread-local task.
    pub use async_std::task::block_on as spawn_task;
}
