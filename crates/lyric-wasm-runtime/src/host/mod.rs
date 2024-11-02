use crate::error::WasmError;
use anyhow::bail;
use async_stream::try_stream;
use futures::{join, pin_mut, stream, try_join, Stream, StreamExt, TryFutureExt, TryStreamExt};
use std::borrow::Cow;
use std::collections::{hash_map, HashMap};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::select;
use tokio::sync::{mpsc, Mutex, RwLock, Semaphore};
use tokio::task::{JoinHandle, JoinSet};
use tracing::Instrument;
use wrpc_interface_http::InvokeOutgoingHandler;
use wrpc_transport::{Invoke, Serve};

use super::component::{Component, WrpcServeEvent};
pub use handler::Handler;

mod handler;

struct WrapperServer<S>
where
    S: Serve + Sync + Send + 'static,
{
    pub component_id: Arc<str>,
    pub server: Arc<S>,
    pub paths: Mutex<HashMap<String, String>>,
}

impl<S> Serve for WrapperServer<S>
where
    S: Serve + Sync + Send + 'static,
{
    type Context = S::Context;
    type Outgoing = S::Outgoing;
    type Incoming = S::Incoming;

    async fn serve(
        &self,
        instance: &str,
        func: &str,
        paths: impl Into<Arc<[Box<[Option<usize>]>]>> + Send,
    ) -> anyhow::Result<
        impl Stream<Item = anyhow::Result<(Self::Context, Self::Outgoing, Self::Incoming)>>
            + Send
            + 'static,
    > {
        let instance = format!("{}@{}", self.component_id, instance);
        let func = func.to_string();
        let paths = paths.into();
        let server = self.server.clone();

        let stream = async_stream::try_stream! {
            let inner_stream = server.serve(&instance, &func, paths).await?;
            pin_mut!(inner_stream);
            while let Some(item) = inner_stream.next().await {
                yield item?;
            }
        };

        Ok(Box::pin(stream))
    }
}

struct HostComponent<C>
where
    C: Invoke<Context = ()> + Clone + 'static,
{
    component: Component<Handler<C>>,
    id: Arc<str>,
    handler: Handler<C>,
    exports: JoinHandle<()>,
}

pub struct Host<C, S>
where
    C: Invoke<Context = ()> + Clone + 'static,
    S: Serve + Sync + Send + 'static,
{
    client: Arc<C>,
    server: Arc<S>,
    engine: wasmtime::Engine,
    components: RwLock<HashMap<String, Arc<HostComponent<C>>>>,
}

impl<C, S> Host<C, S>
where
    C: Invoke<Context = ()> + Clone + 'static,
    S: Serve + Sync + Send + 'static,
{
    pub fn new(client: Arc<C>, server: Arc<S>) -> Self {
        let mut config = wasmtime::Config::new();
        config.wasm_component_model(true);
        config.async_support(true);
        // config.consume_fuel(true);
        Self {
            client,
            server,
            engine: wasmtime::Engine::new(&config).unwrap(),
            components: RwLock::new(HashMap::new()),
        }
    }

    pub async fn launch_component(
        &self,
        component_id: &str,
        wasm: Vec<u8>,
        adapter: Option<(&str, &[u8])>,
    ) -> Result<(), WasmError> {
        match self
            .components
            .write()
            .await
            .entry(component_id.to_string())
        {
            hash_map::Entry::Occupied(_) => {}
            hash_map::Entry::Vacant(entry) => {
                self.start_component(component_id, entry, wasm, adapter)
                    .await?;
            }
        };

        Ok(())
    }

    pub async fn remove_component(&self, component_id: &str) -> Result<(), WasmError> {
        let mut components = self.components.write().await;
        if let Some(host_component) = components.remove(component_id) {
            host_component.exports.abort();
        }
        Ok(())
    }

    async fn start_component<'a>(
        &self,
        component_id: &str,
        entry: hash_map::VacantEntry<'a, String, Arc<HostComponent<C>>>,
        wasm: Vec<u8>,
        adapter: Option<(&str, &[u8])>,
    ) -> Result<(), WasmError> {
        let handler = Handler {
            component_id: Arc::from(component_id),
            client: self.client.clone(),
        };
        let component = Component::new(self.engine.clone(), &wasm, adapter)?;
        let host_component = self
            .instantiate_component(component_id, component, handler)
            .await?;
        entry.insert(host_component);
        Ok(())
    }

    async fn instantiate_component(
        &self,
        component_id: &str,
        mut component: Component<Handler<C>>,
        handler: Handler<C>,
    ) -> Result<Arc<HostComponent<C>>, WasmError> {
        let (events_tx, mut events_rx) = mpsc::channel(10);
        let server = self.server.clone();
        let ws = WrapperServer {
            component_id: Arc::from(component_id),
            server: server.clone(),
            paths: Mutex::new(HashMap::new()),
        };
        let exports = component
            .serve_wrpc(&ws, handler.clone(), events_tx)
            .await?;
        tracing::info!("The size of exports is: {}", exports.len());
        let max_instances = 10_usize;
        let permits = Arc::new(Semaphore::new(
            usize::from(max_instances).min(Semaphore::MAX_PERMITS),
        ));
        Ok(Arc::new(HostComponent {
            component,
            id: Arc::from(""),
            handler,
            exports: tokio::task::spawn(
                async move {
                    join!(
                        async move {
                            let mut tasks = JoinSet::new();
                            let mut exports = stream::select_all(exports);
                            loop {
                                let permits = Arc::clone(&permits);
                                select! {
                                    Some(fut) = exports.next() => {
                                        match fut {
                                            Ok(fut) => {
                                                tracing::debug!("accepted invocation, acquiring permit");
                                                let permit = permits.acquire_owned().await;
                                                tasks.spawn(async move {
                                                    let _permit = permit;
                                                    tracing::debug!("handling invocation");
                                                    match fut.await {
                                                        Ok(()) => {
                                                            tracing::debug!("successfully handled invocation");
                                                            Ok(())
                                                        },
                                                        Err(err) => {
                                                            tracing::warn!(?err, "failed to handle invocation");
                                                            Err(err)
                                                        },
                                                    }
                                                });
                                            }
                                            Err(err) => {
                                                tracing::warn!(?err, "failed to accept invocation")
                                            }
                                        }
                                    }
                                    Some(res) = tasks.join_next() => {
                                        if let Err(err) = res {
                                            tracing::error!(?err, "export serving task failed");
                                        }
                                    }
                                    else => {
                                        tracing::debug!("export serving task is done");
                                        break;
                                    }
                                }
                            }
                        },
                        async move {
                            while let Some(evt) = events_rx.recv().await {
                                match evt {
                                    WrpcServeEvent::HttpIncomingHandlerHandleReturned {
                                        context: _,
                                        success,
                                    }
                                    | WrpcServeEvent::MessagingHandlerHandleMessageReturned {
                                        context: _,
                                        success,
                                    } => {
                                    }
                                _ => {}
                                };
                            }
                            tracing::debug!("serving event stream is done");
                        },
                    );
                    tracing::debug!("export serving task done");
                }
                    .in_current_span(),
            ),
        }))
    }
}
