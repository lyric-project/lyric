use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use futures::Stream;
use futures_util::StreamExt;
use lyric_utils::err::EnvError;
use std::fmt::Debug;
use std::pin::Pin;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_util::io::StreamReader;

pub type StreamItem = Pin<Box<dyn Stream<Item = Result<Bytes, EnvError>> + Send + Sync>>;
pub struct EventStream(pub StreamItem);

impl Debug for EventStream {
    fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}

pub type EnvStatusCode = i32;

// #[async_trait]
pub trait ExecutionEnvironment {
    async fn execute(&mut self) -> Result<Box<dyn ChildProcess>, EnvError>;
}

#[async_trait]
pub trait ChildProcess: Debug + Sync + Send {
    fn stdout(&mut self) -> Result<EventStream, EnvError>;
    fn stderr(&mut self) -> Result<EventStream, EnvError>;
    async fn wait(&mut self) -> Result<EnvStatusCode, EnvError>;
    async fn try_wait(&mut self) -> Result<Option<EnvStatusCode>, EnvError>;
    async fn cleanup(&mut self) -> Result<(), EnvError>;
}

impl EventStream {
    pub async fn lines(self) -> impl Stream<Item = Result<String, EnvError>> {
        let stream = self
            .0
            .map(|s| s.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)));
        let reader = StreamReader::new(stream);
        let mut lines = BufReader::new(reader).lines();
        let (stdout_tx, stdout_rx) = mpsc::unbounded_channel();
        let stdout_stream = UnboundedReceiverStream::new(stdout_rx);

        tokio::spawn(async move {
            while let msg = lines.next_line().await {
                match msg {
                    Ok(Some(line)) => {
                        let _ = stdout_tx.send(Ok(line));
                    }
                    Err(e) => {
                        let _ = stdout_tx.send(Err(EnvError::IoError(e)));
                    }
                    _ => break,
                }
            }
        });
        stdout_stream
    }

    pub async fn read(self) -> Result<Bytes, EnvError> {
        let mut result = BytesMut::new();
        let mut stream = self.0;
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(bytes) => {
                    result.extend_from_slice(&bytes);
                }
                Err(e) => return Err(e),
            }
        }
        Ok(result.freeze())
    }
}
