pub mod capability;
mod component;
pub mod error;
mod host;
#[cfg(feature = "quic")]
pub mod quic;
pub mod resource;
#[cfg(feature = "tcp")]
pub mod tcp;
mod utils;

pub use component::{new_store, Component};
pub use host::{Handler, Host};

pub enum WasmMessage {
    LaunchComponent { id: String, wasm: Vec<u8> },
}

#[cfg(feature = "tcp")]
pub type WasmRuntime = tcp::WasmRuntime;
#[cfg(feature = "quic")]
pub type WasmRuntime = quic::WasmRuntime;

#[cfg(feature = "tcp")]
pub type DefaultClient = wrpc_transport::tcp::Client<String>;
#[cfg(feature = "quic")]
pub type DefaultClient = wrpc_transport_quic::Client;
