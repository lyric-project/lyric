use crate::capability::logging::logging;
use crate::component::{DependencyTypes, Logging};
use anyhow::{bail, Context as _};
use async_trait::async_trait;
use bytes::Bytes;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, LazyLock};
use tokio::sync::Semaphore;
use tracing::{instrument, trace, Instrument};
use wrpc_transport::Invoke;

static CONNECTION_SEMAPHORE: LazyLock<Semaphore> = LazyLock::new(|| Semaphore::new(100));

pub(crate) fn compute_dependencies<'a>(
    depends_on_fns: &[String],
    depends_on: &'a HashSet<String>,
    export_index: &'a HashMap<String, Vec<(String, DependencyTypes)>>,
) -> anyhow::Result<HashMap<(Arc<str>, Arc<str>), Arc<str>>> {
    let mut depends_to_component = HashMap::new();

    if !depends_on_fns.is_empty() {
        // Collect instance-level and function-level dependencies
        let mut depends_exports_fn = Vec::new();
        let mut depends_exports_instance = Vec::new();

        // For each dependency component, collect its exported functions and instances
        for dep_component_id in depends_on {
            if let Some(exports) = export_index.get(dep_component_id) {
                for (_, dep_type) in exports {
                    match dep_type {
                        DependencyTypes::ComponentFunc { instance, func } => {
                            if instance.is_empty() {
                                depends_exports_fn.push((dep_component_id, func));
                            } else {
                                depends_exports_instance.push((dep_component_id, instance, func));
                            }
                        }
                    }
                }
            }
        }

        // For each dependency function, check if it is exported by any component
        for dep_fn in depends_on_fns {
            // First check instance-level dependencies
            let mut depends: HashMap<String, (String, String)> = depends_exports_instance
                .iter()
                .filter(|(_, instance, _)| *instance == dep_fn)
                .map(|(component_id, instance, func)| {
                    (
                        (*component_id).to_string(),
                        ((*instance).to_string(), (*func).to_string()),
                    )
                })
                .collect();

            // If no instance-level dependencies are found, check function-level dependencies
            if depends.is_empty() {
                depends = depends_exports_fn
                    .iter()
                    .filter(|(_, func)| *func == dep_fn)
                    .map(|(component_id, func)| {
                        (
                            (*component_id).to_string(),
                            (String::new(), (*func).to_string()),
                        )
                    })
                    .collect();
            }

            if !depends.is_empty() {
                for (component_id, (instance, func)) in depends {
                    let key = (Arc::from(instance.as_str()), Arc::from(func.as_str()));
                    if let Some(dep_component_id) = depends_to_component.get(&key) {
                        bail!(
                            "{}:{} is already depended on by {}",
                            key.0,
                            key.1,
                            dep_component_id
                        );
                    } else {
                        tracing::debug!("{}:{} is depended on by {}", key.0, key.1, component_id);
                        depends_to_component.insert(key, Arc::from(component_id.as_str()));
                    }
                }
            }
        }
    }

    tracing::debug!("Computed dependencies: {:?}", depends_to_component);

    Ok(depends_to_component)
}

#[derive(Clone)]
pub struct Handler<C>
where
    C: Invoke + Clone + 'static,
{
    pub component_id: Arc<str>,
    pub client: Arc<C>,
    pub dependencies: Arc<HashMap<(Arc<str>, Arc<str>), Arc<str>>>,
}
impl<C> Handler<C>
where
    C: Invoke + Clone + 'static,
{
    pub fn new(
        component_id: Arc<str>,
        client: Arc<C>,
        depends_to_component: HashMap<(Arc<str>, Arc<str>), Arc<str>>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            component_id,
            client,
            dependencies: Arc::new(depends_to_component),
        })
    }

    fn resolve_dependency(&self, instance: &str, func: &str) -> Option<&str> {
        self.dependencies
            .get(&(Arc::from(instance), Arc::from(func)))
            .map(|component_id| component_id.as_ref())
    }
}

impl Debug for Handler<wrpc_transport::tcp::Client<String>> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Handler")
            .field("component_id", &self.component_id)
            .field("client", &self.client)
            .field("dependencies", &self.dependencies)
            .finish()
    }
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
        let formatted_instance =
            if let Some(dep_component_id) = self.resolve_dependency(instance, func) {
                tracing::debug!("Handler::invokes, invoke dependency: {}", dep_component_id);
                format!("{}@{}", dep_component_id, instance)
            } else {
                format!("{}@{}", self.component_id, instance)
            };
        tracing::debug!(
            "Handler::invoke: instance={}, func={}",
            formatted_instance,
            func
        );
        self.client
            .invoke(cx, formatted_instance.as_str(), func, params, paths)
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
            dependencies: Default::default(),
        })
    }

    pub fn from_client(
        component_id: String,
        client: Arc<wrpc_transport::tcp::Client<String>>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            component_id: Arc::from(component_id),
            client,
            dependencies: Default::default(),
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
            dependencies: Default::default(),
        };

        Ok(handler)
    }

    pub fn from_client(
        component_id: String,
        client: Arc<wrpc_transport_quic::Client>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            component_id: Arc::from(component_id),
            client,
            dependencies: Default::default(),
        })
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
