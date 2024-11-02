pub mod capability;
mod component;
pub mod error;
mod host;
mod tcp;
mod utils;

pub use component::{new_store, Component};
pub use host::{Handler, Host};

pub enum WasmMessage {
    LaunchComponent { id: String, wasm: Vec<u8> },
}
pub use tcp::WasmRuntime;
