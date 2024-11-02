use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use std::future::Future;
use std::net::Ipv4Addr;
use std::net::Ipv6Addr;
use std::path::Path;
use std::pin::pin;

use crate::error::WasmError;
use crate::Host;
use anyhow::Context as _;
use std::sync::Arc;
use tokio::select;
use tracing::error;

pub trait Hashable {
    fn hash_val(&self) -> u64;
}

impl<T> Hashable for T
where
    T: Hash,
{
    fn hash_val(&self) -> u64 {
        let mut s = DefaultHasher::new();
        self.hash(&mut s);
        s.finish()
    }
}

pub async fn create_host() -> Result<(), WasmError> {
    let import = "127.0.0.1:15688";
    let client = wrpc_transport::tcp::Client::from(String::from(import));

    let lis = tokio::net::TcpListener::bind(import)
        .await
        .with_context(|| format!("failed to bind TCP listener on `{import}`"))?;
    let srv = Arc::new(wrpc_transport::Server::default());
    let clt = Arc::new(client);
    let accept = tokio::spawn({
        let srv = Arc::clone(&srv);
        async move {
            loop {
                if let Err(err) = srv.accept(&lis).await {
                    error!(?err, "failed to accept TCP connection")
                }
            }
        }
    });
    let host = Host::new(clt.clone(), srv.clone());
    accept.await.unwrap();
    Ok(())
}
