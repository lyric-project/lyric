#[allow(clippy::doc_markdown)]
#[allow(missing_docs)]
mod wasmtime_bindings {

    wasmtime::component::bindgen!({
        world: "interfaces",
        async: true,
        tracing: true,
        trappable_imports: true,
    });
}

#[allow(clippy::doc_markdown)]
#[allow(missing_docs)]
/// wRPC interface bindings
pub mod wrpc {
    wit_bindgen_wrpc::generate!({
        world: "wrpc-interfaces",
        generate_all,
    });
}
pub use wasmtime_bindings::lyric::{serialization, task};
pub use wasmtime_bindings::wasi::logging;
pub use wasmtime_bindings::Interfaces;
pub use wrpc::lyric::task as rpc_task;
