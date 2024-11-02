use crate::error::WasmError;
use crate::Host;
use anyhow::Context;
use async_trait::async_trait;
use core::net::SocketAddr;
use std::fmt;
use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::sync::Arc;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use wrpc_transport::frame::tcp::Client;
use wrpc_transport::frame::{Incoming, Outgoing, Server};

struct Inner {
    address: String,
    host: Host<Client<String>, Server<SocketAddr, OwnedReadHalf, OwnedWriteHalf>>,
    handle: tokio::task::JoinHandle<()>,
}

#[derive(Clone)]
pub struct WasmRuntime {
    inner: Arc<Inner>,
}

impl WasmRuntime {
    pub async fn new(address: Option<&str>) -> Result<Self, WasmError> {
        let address = address.unwrap_or("0.0.0.0:0");
        let lis = tokio::net::TcpListener::bind(address)
            .await
            .with_context(|| format!("failed to bind TCP listener on `{address}`"))?;
        let address = lis.local_addr()?.to_string();
        tracing::info!("Wasm runtime listening on: {}", address);
        let client = wrpc_transport::tcp::Client::from(address.clone());
        let srv = Arc::new(wrpc_transport::Server::default());
        let clt = Arc::new(client);
        let accept = tokio::spawn({
            let srv = Arc::clone(&srv);
            async move {
                loop {
                    if let Err(err) = srv.accept(&lis).await {
                        tracing::error!(?err, "failed to accept TCP connection")
                    }
                }
            }
        });
        let host = Host::new(clt.clone(), srv.clone());
        let inner = Inner {
            address,
            host,
            handle: accept,
        };
        Ok(Self {
            inner: Arc::new(inner),
        })
    }
    pub async fn launch_component(
        &self,
        component_id: &str,
        wasm: Vec<u8>,
    ) -> Result<(), WasmError> {
        self.inner
            .host
            .launch_component(component_id, wasm, None)
            .await
    }

    pub async fn remove_component(&self, component_id: &str) -> Result<(), WasmError> {
        self.inner.host.remove_component(component_id).await
    }

    pub fn address(&self) -> &str {
        &self.inner.address
    }
}

impl Debug for WasmRuntime {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("WasmRuntime").finish()
    }
}
