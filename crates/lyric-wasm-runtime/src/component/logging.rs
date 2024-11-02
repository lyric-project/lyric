use super::{Ctx, Handler};
use crate::capability::logging::logging;
use async_trait::async_trait;

/// `wasi:logging/logging` implementation
#[async_trait]
pub trait Logging {
    /// Handle `wasi:logging/logging.log`
    async fn log(
        &self,
        level: logging::Level,
        context: String,
        message: String,
    ) -> anyhow::Result<()>;
}

#[async_trait]
impl<H: Handler> logging::Host for Ctx<H> {
    #[tracing::instrument(skip_all)]
    async fn log(
        &mut self,
        level: logging::Level,
        context: String,
        message: String,
    ) -> anyhow::Result<()> {
        self.handler.log(level, context, message).await
    }
}
