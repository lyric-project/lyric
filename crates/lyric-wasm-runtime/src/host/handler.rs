use crate::capability::logging::logging;
use crate::component::Logging;
use anyhow::Context as _;
use async_trait::async_trait;
use bytes::Bytes;
use core::net::SocketAddr;
use futures::{pin_mut, Stream, StreamExt};
use std::sync::{Arc, LazyLock};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::{instrument, trace, Instrument};
use wrpc_transport::{Invoke, Serve};

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
