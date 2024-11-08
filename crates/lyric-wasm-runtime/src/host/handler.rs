use crate::capability::logging::logging;
use crate::component::Logging;
use crate::error::WasmError;
use anyhow::Context as _;
use async_trait::async_trait;
use bytes::Bytes;
use core::net::SocketAddr;
use futures::{pin_mut, Stream, StreamExt};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::{instrument, trace, Instrument};
use wrpc_transport::{Accept, Invoke, Serve};

static CONNECTION_SEMAPHORE: LazyLock<Semaphore> = LazyLock::new(|| Semaphore::new(100));

#[derive(Clone)]
pub struct Handler<C>
where
    C: Invoke + Clone + 'static,
{
    pub component_id: Arc<str>,
    pub client: Arc<C>,
}

impl<C> Invoke for Handler<C>
where
    C: Invoke + Clone + 'static,
{
    type Context = C::Context;
    type Outgoing = C::Outgoing;
    type Incoming = C::Incoming;

    #[instrument(level = "trace", skip(self, cx, paths, params), fields(params = format!("{params:02x?}")))]
    async fn invoke<P>(
        &self,
        cx: Self::Context,
        instance: &str,
        func: &str,
        params: Bytes,
        paths: impl AsRef<[P]> + Send,
    ) -> anyhow::Result<(Self::Outgoing, Self::Incoming)>
    where
        P: AsRef<[Option<usize>]> + Send + Sync,
    {
        let _permit = CONNECTION_SEMAPHORE.acquire().await?;
        let instance = format!("{}@{}", self.component_id, instance);
        self.client
            .invoke(cx, instance.as_str(), func, params, paths)
            .await
    }
}

// Implement the `Handler` trait for the `wrpc_transport::tcp::Client<String>` type.
#[cfg(feature = "tcp")]
impl Handler<wrpc_transport::tcp::Client<String>> {
    pub async fn from_address(component_id: String, address: String) -> anyhow::Result<Self> {
        let client = wrpc_transport::tcp::Client::from(address);
        Ok(Self {
            component_id: Arc::from(component_id),
            client: Arc::new(client),
        })
    }
}

// Implement the `Handler` trait for the `wrpc_transport_quic::Client` type.
#[cfg(feature = "quic")]
impl Handler<wrpc_transport_quic::Client> {
    pub async fn from_address(component_id: String, address: String) -> anyhow::Result<Self> {
        use crate::quic::CertManager;
        use quinn::ClientConfig;

        use crate::quic::{ensure_crypto_provider, get_or_init_cert_config};

        ensure_crypto_provider()?;

        tracing::debug!("Creating QUIC client for address: {}", address);

        let server_addr = address.parse()?;

        let client_crypto_config = CertManager::client_crypto_config()
            .context("failed to get client certificate configuration")?;

        // 创建客户端 endpoint
        let mut endpoint = quinn::Endpoint::client((Ipv4Addr::LOCALHOST, 0).into())?;

        endpoint.set_default_client_config(ClientConfig::new(client_crypto_config));

        tracing::debug!("Connecting to QUIC server...");

        // 建立连接
        let connecting = endpoint.connect(server_addr, "localhost")?;

        // 仅等待很短时间
        let connection = connecting
            .await
            .context("failed to establish QUIC connection")?;

        tracing::debug!("QUIC connection established");

        let client = wrpc_transport_quic::Client::from(connection);

        let handler = Self {
            component_id: Arc::from(component_id),
            client: Arc::new(client),
        };

        Ok(handler)
    }
}

#[async_trait]
impl<C> Logging for Handler<C>
where
    C: Invoke + Clone + 'static,
{
    #[tracing::instrument(level = "trace", skip(self))]
    async fn log(
        &self,
        level: logging::Level,
        context: String,
        message: String,
    ) -> anyhow::Result<()> {
        match level {
            logging::Level::Trace => {
                tracing::event!(
                    tracing::Level::TRACE,
                    component_id = ?self.component_id,
                    ?level,
                    context,
                    "{message}"
                );
            }
            logging::Level::Debug => {
                tracing::event!(
                    tracing::Level::DEBUG,
                    component_id = ?self.component_id,
                    ?level,
                    context,
                    "{message}"
                );
            }
            logging::Level::Info => {
                tracing::event!(
                    tracing::Level::INFO,
                    component_id = ?self.component_id,
                    ?level,
                    context,
                    "{message}"
                );
            }
            logging::Level::Warn => {
                tracing::event!(
                    tracing::Level::WARN,
                    component_id = ?self.component_id,
                    ?level,
                    context,
                    "{message}"
                );
            }
            logging::Level::Error => {
                tracing::event!(
                    tracing::Level::ERROR,
                    component_id = ?self.component_id,
                    ?level,
                    context,
                    "{message}"
                );
            }
            logging::Level::Critical => {
                tracing::event!(
                    tracing::Level::ERROR,
                    component_id = ?self.component_id,
                    ?level,
                    context,
                    "{message}"
                );
            }
        };
        Ok(())
    }
}
