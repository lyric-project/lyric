mod binary;
mod interpreter;
mod logging;
mod serialization;

use crate::capability;
use crate::capability::wrpc::lyric::task;
pub use crate::component::logging::Logging;
use crate::error::WasmError;
use anyhow::{anyhow, Context as _};
use lyric_utils::resource::ResourceConfig;
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::Instrument;
use wasi_preview1_component_adapter_provider::{
    WASI_SNAPSHOT_PREVIEW1_ADAPTER_NAME, WASI_SNAPSHOT_PREVIEW1_REACTOR_ADAPTER,
};
use wasmtime::component::{types, Linker, ResourceTable};
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtx, WasiCtxBuilder, WasiView};
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpView};
use wit_bindgen_wrpc::futures::{Stream, TryStreamExt};
use wrpc_runtime_wasmtime::{
    collect_component_resources, link_item, ServeExt, SharedResourceTable, WrpcView,
};
use wrpc_transport::Invoke;

/// skips instance names, for which static (builtin) bindings exist
macro_rules! skip_static_instances {
    ($instance:expr) => {
        match ($instance) {
            "lyric:serialization/msgpack@0.2.0"
            | "wasi:blobstore/container@0.2.0-draft"
            | "wasi:blobstore/types@0.2.0-draft"
            | "wasi:cli/environment@0.2.0"
            | "wasi:cli/environment@0.2.1"
            | "wasi:cli/exit@0.2.0"
            | "wasi:cli/exit@0.2.1"
            | "wasi:cli/stderr@0.2.0"
            | "wasi:cli/stderr@0.2.1"
            | "wasi:cli/stdin@0.2.0"
            | "wasi:cli/stdin@0.2.1"
            | "wasi:cli/stdout@0.2.0"
            | "wasi:cli/stdout@0.2.1"
            | "wasi:cli/terminal-input@0.2.0"
            | "wasi:cli/terminal-input@0.2.1"
            | "wasi:cli/terminal-output@0.2.0"
            | "wasi:cli/terminal-output@0.2.1"
            | "wasi:cli/terminal-stderr@0.2.0"
            | "wasi:cli/terminal-stderr@0.2.1"
            | "wasi:cli/terminal-stdin@0.2.0"
            | "wasi:cli/terminal-stdin@0.2.1"
            | "wasi:cli/terminal-stdout@0.2.0"
            | "wasi:cli/terminal-stdout@0.2.1"
            | "wasi:clocks/monotonic-clock@0.2.0"
            | "wasi:clocks/monotonic-clock@0.2.1"
            | "wasi:clocks/timezone@0.2.1"
            | "wasi:clocks/wall-clock@0.2.0"
            | "wasi:clocks/wall-clock@0.2.1"
            | "wasi:config/runtime@0.2.0-draft"
            | "wasi:filesystem/preopens@0.2.0"
            | "wasi:filesystem/preopens@0.2.1"
            | "wasi:filesystem/types@0.2.0"
            | "wasi:filesystem/types@0.2.1"
            | "wasi:http/incoming-handler@0.2.0"
            | "wasi:http/incoming-handler@0.2.1"
            | "wasi:http/outgoing-handler@0.2.0"
            | "wasi:http/outgoing-handler@0.2.1"
            | "wasi:http/types@0.2.0"
            | "wasi:http/types@0.2.1"
            | "wasi:http/types@0.2.2"
            | "wasi:io/error@0.2.0"
            | "wasi:io/error@0.2.1"
            | "wasi:io/poll@0.2.0"
            | "wasi:io/poll@0.2.1"
            | "wasi:io/streams@0.2.0"
            | "wasi:io/streams@0.2.1"
            | "wasi:keyvalue/atomics@0.2.0-draft"
            | "wasi:keyvalue/batch@0.2.0-draft"
            | "wasi:keyvalue/store@0.2.0-draft"
            | "wasi:logging/logging"
            | "wasi:random/insecure-seed@0.2.0"
            | "wasi:random/insecure-seed@0.2.1"
            | "wasi:random/insecure@0.2.0"
            | "wasi:random/insecure@0.2.1"
            | "wasi:random/random@0.2.0"
            | "wasi:random/random@0.2.1"
            | "wasi:sockets/instance-network@0.2.0"
            | "wasi:sockets/instance-network@0.2.1"
            | "wasi:sockets/ip-name-lookup@0.2.0"
            | "wasi:sockets/ip-name-lookup@0.2.1"
            | "wasi:sockets/network@0.2.0"
            | "wasi:sockets/network@0.2.1"
            | "wasi:sockets/tcp-create-socket@0.2.0"
            | "wasi:sockets/tcp-create-socket@0.2.1"
            | "wasi:sockets/tcp@0.2.0"
            | "wasi:sockets/tcp@0.2.1"
            | "wasi:sockets/udp-create-socket@0.2.0"
            | "wasi:sockets/udp-create-socket@0.2.1"
            | "wasi:sockets/udp@0.2.0"
            | "wasi:sockets/udp@0.2.1" => continue,
            _ => {}
        }
    };
}

/// Instance target, which is replaced in wRPC
///
/// This enum represents the original instance import invoked by the component
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ReplacedInstanceTarget {
    /// `wasi:http/incoming-handler` instance replacement
    HttpIncomingHandler,
    /// `wasi:http/outgoing-handler` instance replacement
    HttpOutgoingHandler,
}

#[derive(Clone, Debug)]
pub enum WrpcServeEvent<C> {
    /// `wasi:http/incoming-handler.handle` return event
    HttpIncomingHandlerHandleReturned {
        /// Invocation context
        context: C,
        /// Whether the invocation was successfully handled
        success: bool,
    },
    /// `wasmcloud:messaging/handler.handle-message` return event
    MessagingHandlerHandleMessageReturned {
        /// Invocation context
        context: C,
        /// Whether the invocation was successfully handled
        success: bool,
    },
    /// dynamic export return event
    DynamicExportReturned {
        /// Invocation context
        context: C,
        /// Whether the invocation was successfully handled
        success: bool,
    },

    InterpreterTaskRunReturned {
        context: C,
        success: bool,
    },
}

pub trait Handler: Invoke<Context = ()> + Logging + Send + Sync + Clone + 'static {}

impl<T> Handler for T where T: Invoke<Context = ()> + Logging + Send + Sync + Clone + 'static {}

pub(crate) struct Ctx<H>
where
    H: Handler,
{
    handler: H,
    wasi: WasiCtx,
    http: WasiHttpCtx,
    table: ResourceTable,
    shared_resources: SharedResourceTable,
    timeout: Duration,
    resource: Option<ResourceConfig>,
    limits: wasmtime::StoreLimits,
}

impl<H: Handler> WasiView for Ctx<H> {
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }

    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.wasi
    }
}

impl<H: Handler> WrpcView for Ctx<H> {
    type Invoke = H;

    fn client(&self) -> &H {
        &self.handler
    }

    fn shared_resources(&mut self) -> &mut SharedResourceTable {
        &mut self.shared_resources
    }

    fn timeout(&self) -> Option<Duration> {
        Some(self.timeout)
    }
}

impl<H: Handler> WasiHttpView for Ctx<H> {
    fn ctx(&mut self) -> &mut WasiHttpCtx {
        &mut self.http
    }
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
}

#[derive(Clone)]
pub struct Component<H>
where
    H: Handler,
{
    engine: wasmtime::Engine,
    instance_pre: wasmtime::component::InstancePre<Ctx<H>>,
}

impl<H> Component<H>
where
    H: Handler,
{
    pub fn new(
        engine: wasmtime::Engine,
        wasm: &[u8],
        adapter: Option<(&str, &[u8])>,
    ) -> Result<Self, WasmError> {
        if wasmparser::Parser::is_core_wasm(wasm) {
            let wasm = wit_component::ComponentEncoder::default()
                .module(wasm)
                .context("failed to set core component module")?
                .adapter(
                    WASI_SNAPSHOT_PREVIEW1_ADAPTER_NAME,
                    WASI_SNAPSHOT_PREVIEW1_REACTOR_ADAPTER,
                )
                .context("failed to add WASI preview1 adapter")?;
            let wasm = if let Some((name, adapter)) = adapter {
                wasm.adapter(name, adapter)
                    .context(format!("failed to add adapter: {}", name))?
            } else {
                wasm
            };
            let wasm = wasm
                .encode()
                .context("failed to encode a component from module")?;
            return Self::new(engine, &wasm, None);
        }
        let mut linker = Linker::new(&engine);
        wasmtime_wasi::add_to_linker_async(&mut linker)
            .context("failed to link core WASI interfaces")?;
        wasmtime_wasi_http::add_only_http_to_linker_async(&mut linker)
            .context("failed to link `wasi:http`")?;
        let component = wasmtime::component::Component::new(&engine, wasm)
            .context("failed to compile component")?;

        capability::logging::logging::add_to_linker(&mut linker, |ctx| ctx)
            .context("failed to link `wasi:logging/logging`")?;
        capability::serialization::msgpack::add_to_linker(&mut linker, |ctx| ctx)
            .context("failed to link `lyric:serialization/msgpack@0.2.0`")?;

        let ty = component.component_type();
        let mut guest_resources = Vec::new();
        collect_component_resources(&engine, &ty, &mut guest_resources);
        if !guest_resources.is_empty() {
            tracing::warn!("exported component resources are not supported in wasmCloud runtime and will be ignored, use a provider instead to enable this functionality");
        }
        for (name, ty) in ty.imports(&engine) {
            skip_static_instances!(name);
            tracing::info!("Linking import: {}", name);
            link_item(&engine, &mut linker.root(), [], ty, "", name, ())
                .context("failed to link item")?;
        }

        let instance_pre = linker.instantiate_pre(&component)?;
        Ok(Self {
            engine,
            instance_pre,
        })
    }

    pub async fn serve_wrpc<S>(
        &self,
        srv: &S,
        handler: H,
        events: mpsc::Sender<WrpcServeEvent<S::Context>>,
    ) -> anyhow::Result<Vec<InvocationStream>>
    where
        S: wrpc_transport::Serve,
    {
        let span = tracing::Span::current();
        let max_execution_time = Duration::from_secs(5);

        let instance = Instance {
            engine: self.engine.clone(),
            pre: self.instance_pre.clone(),
            handler: handler.clone(),
            events: events.clone(),
            max_execution_time: max_execution_time.clone(),
        };
        let mut invocations = vec![];

        for (name, ty) in self
            .instance_pre
            .component()
            .component_type()
            .exports(&self.engine)
        {
            match (name, ty) {
                (
                    "lyric:task/interpreter-task@0.2.0",
                    types::ComponentItem::ComponentInstance(..),
                ) => {
                    let instance = instance.clone();
                    // interpreter_task has two exports function `run` and `run1`
                    let res = interpreter::wrpc_handler_bindings::exports::lyric::task::interpreter_task::serve_interface(
                        srv,
                        instance,
                    )
                        .await
                        .context("failed to serve `lyric:task/interpreter-task@0.2.0`")?;
                    for (_, _, handle_message) in res {
                        invocations.push(handle_message);
                    }
                }
                (name, types::ComponentItem::ComponentFunc(ty)) => {
                    let engine = self.engine.clone();
                    let handler = handler.clone();
                    let pre = self.instance_pre.clone();
                    tracing::debug!(?name, "serving root function");
                    let func = srv
                        .serve_function(
                            move || {
                                new_store(&engine, handler.clone(), max_execution_time, None, None)
                                    .expect("failed to create store")
                            },
                            pre,
                            ty,
                            "",
                            name,
                        )
                        .await
                        .context("failed to serve root function")?;
                    let events = events.clone();
                    let span = span.clone();
                    invocations.push(Box::pin(func.map_ok(move |(cx, res)| {
                        let events = events.clone();
                        Box::pin(
                            async move {
                                let res = res.await;
                                let success = res.is_ok();
                                if let Err(err) =
                                    events.try_send(WrpcServeEvent::DynamicExportReturned {
                                        context: cx,
                                        success,
                                    })
                                {
                                    tracing::warn!(
                                        ?err,
                                        success,
                                        "failed to send dynamic root export return event"
                                    );
                                }
                                res
                            }
                            .instrument(span.clone()),
                        )
                            as Pin<Box<dyn Future<Output = _> + Send + 'static>>
                    })));
                }
                (_, types::ComponentItem::CoreFunc(_)) => {
                    tracing::warn!(name, "serving root core function exports not supported yet");
                }
                (_, types::ComponentItem::Module(_)) => {
                    tracing::warn!(name, "serving root module exports not supported yet");
                }
                (_, types::ComponentItem::Component(_)) => {
                    tracing::warn!(name, "serving root component exports not supported yet");
                }
                (instance_name, types::ComponentItem::ComponentInstance(ty)) => {
                    for (name, ty) in ty.exports(&self.engine) {
                        match ty {
                            types::ComponentItem::ComponentFunc(ty) => {
                                let engine = self.engine.clone();
                                let handler = handler.clone();
                                let pre = self.instance_pre.clone();
                                tracing::debug!(?instance_name, ?name, "serving instance function");
                                let func = srv
                                    .serve_function(
                                        move || {
                                            new_store(
                                                &engine,
                                                handler.clone(),
                                                max_execution_time,
                                                None,
                                                None,
                                            )
                                            .expect("failed to create store")
                                        },
                                        pre,
                                        ty,
                                        instance_name,
                                        name,
                                    )
                                    .await
                                    .context("failed to serve instance function")?;
                                let events = events.clone();
                                let span = span.clone();
                                invocations.push(Box::pin(func.map_ok(move |(cx, res)| {
                                    let events = events.clone();
                                    Box::pin(
                                        async move {
                                            let res = res.await;
                                            let success = res.is_ok();
                                            if let Err(err) = events.try_send(
                                                WrpcServeEvent::DynamicExportReturned {
                                                    context: cx,
                                                    success,
                                                },
                                            ) {
                                                tracing::warn!(
                                                    ?err,
                                                    success,
                                                    "failed to send dynamic instance export return event"
                                                );
                                            }
                                            res
                                        }
                                            .instrument(span.clone()),
                                    )
                                        as Pin<Box<dyn Future<Output = _> + Send + 'static>>
                                })));
                            }
                            types::ComponentItem::CoreFunc(_) => {
                                tracing::warn!(
                                    instance_name,
                                    name,
                                    "serving instance core function exports not supported yet"
                                );
                            }
                            types::ComponentItem::Module(_) => {
                                tracing::warn!(
                                    instance_name,
                                    name,
                                    "serving instance module exports not supported yet"
                                );
                            }
                            types::ComponentItem::Component(_) => {
                                tracing::warn!(
                                    instance_name,
                                    name,
                                    "serving instance component exports not supported yet"
                                );
                            }
                            types::ComponentItem::ComponentInstance(_) => {
                                tracing::warn!(
                                    instance_name,
                                    name,
                                    "serving nested instance exports not supported yet"
                                );
                            }
                            types::ComponentItem::Type(_) | types::ComponentItem::Resource(_) => {}
                        }
                    }
                }
                (_, types::ComponentItem::Type(_) | types::ComponentItem::Resource(_)) => {}
            }
        }
        Ok(invocations)
    }

    pub fn with_instance_pre<T>(
        &self,
        f: impl FnOnce(&wasmtime::component::InstancePre<Ctx<H>>) -> T,
    ) -> T {
        f(&self.instance_pre)
    }

    pub async fn run_command(&self, handler: H) -> anyhow::Result<()> {
        let mut store = new_store(
            &self.engine,
            handler,
            Duration::from_secs(10),
            Some("command.wasm"),
            None,
        )?;
        let cmd = wasmtime_wasi::bindings::CommandPre::new(self.instance_pre.clone())?
            .instantiate_async(&mut store)
            .await
            .context("failed to instantiate `command`")?;

        cmd.wasi_cli_run()
            .call_run(&mut store)
            .await
            .context("failed to run component")?
            .map_err(|()| anyhow!("component failed"))
    }
}

pub(crate) struct Instance<H, C>
where
    H: Handler,
{
    engine: wasmtime::Engine,
    pre: wasmtime::component::InstancePre<Ctx<H>>,
    handler: H,
    events: mpsc::Sender<WrpcServeEvent<C>>,
    max_execution_time: Duration,
}

impl<H, C> Clone for Instance<H, C>
where
    H: Handler,
{
    fn clone(&self) -> Self {
        Self {
            engine: self.engine.clone(),
            pre: self.pre.clone(),
            handler: self.handler.clone(),
            events: self.events.clone(),
            max_execution_time: self.max_execution_time,
        }
    }
}

impl<H, C> Debug for Instance<H, C>
where
    H: Handler,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Instance")
    }
}

pub fn new_store<H: Handler>(
    engine: &wasmtime::Engine,
    handler: H,
    max_execution_time: Duration,
    arg0: Option<&str>,
    resource: Option<ResourceConfig>,
) -> anyhow::Result<wasmtime::Store<Ctx<H>>> {
    tracing::info!("Creating new store with resource: {:?}", resource);
    let resource = resource.unwrap_or_default();

    let table = ResourceTable::new();
    let arg0 = arg0.unwrap_or("main.wasm");

    let mut wasi_builder = WasiCtxBuilder::new();
    wasi_builder.args(&[arg0]).inherit_stdio().inherit_stderr();

    // Configure environment variables
    for (key, value) in &resource.env_vars {
        wasi_builder.env(key, value);
    }

    // Configure preopened directories
    if let Some(fs_config) = &resource.fs {
        // Configured pre-mapped directory list (host path, container path, directory permissions, file permissions)
        for (host_path, guest_path, dir_perms, file_perms) in &fs_config.preopens {
            let dir_perms = DirPerms::from_bits_truncate(dir_perms.bits());
            let file_perms = FilePerms::from_bits_truncate(file_perms.bits());
            tracing::info!(
                "Pre-mapped directory: host_path={}, guest_path={}, dir_perms={:?}, file_perms={:?}",
                host_path,
                guest_path,
                dir_perms,
                file_perms
            );
            wasi_builder.preopened_dir(host_path, guest_path, dir_perms, file_perms)?;
        }
    }
    let wasi = wasi_builder.build();

    let timeout = resource
        .timeout_ms
        .map(|ms| Duration::from_millis(ms as u64))
        .unwrap_or(max_execution_time);

    let mut limits_builder = wasmtime::StoreLimitsBuilder::new();
    if let Some(memory) = &resource.memory {
        if let Some(memory_size) = memory.memory_limit {
            limits_builder = limits_builder.memory_size(memory_size as usize);
        }
    }
    if let Some(instance) = &resource.instance {
        if let Some(max_instances) = instance.max_instances {
            limits_builder = limits_builder.instances(max_instances as usize);
        }
        if let Some(max_tables) = instance.max_tables {
            limits_builder = limits_builder.tables(max_tables as usize);
        }
        if let Some(max_table_elements) = instance.max_table_elements {
            limits_builder = limits_builder.table_elements(max_table_elements);
        }
    }
    let limits = limits_builder.build();

    let mut store = wasmtime::Store::new(
        engine,
        Ctx {
            handler,
            wasi,
            http: WasiHttpCtx::new(),
            table,
            shared_resources: SharedResourceTable::default(),
            timeout,
            resource: Some(resource),
            limits,
        },
    );
    /// TODO: Limit the cpu time by setting fuel
    // store.set_fuel()
    store.limiter(|state| &mut state.limits);
    store.set_epoch_deadline(timeout.as_secs());
    Ok(store)
}

/// This represents a [Stream] of incoming invocations.
/// Each item represents processing of a single invocation.
pub type InvocationStream = Pin<
    Box<
        dyn Stream<
                Item = anyhow::Result<
                    Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'static>>,
                >,
            > + Send
            + 'static,
    >,
>;
