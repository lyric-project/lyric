/// QUIC transport module
///
/// This module provides a QUIC transport implementation for wRPC.
///
/// TODO: Not working yet
///
use crate::error::WasmError;
use crate::Host;
use anyhow::{anyhow, bail, Context};
use once_cell::sync::OnceCell;
use quinn::crypto::rustls::{QuicClientConfig, QuicServerConfig};
use quinn::{ClientConfig, Endpoint, ServerConfig};
use rcgen::{generate_simple_self_signed, CertifiedKey};
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::version::TLS13;
use rustls::{
    ClientConfig as RustlsClientConfig, RootCertStore, ServerConfig as RustlsServerConfig,
};
use std::fmt::{Debug, Formatter};
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::sync::Arc;
use std::{fmt, fs, io};
use tokio::select;
use tokio::sync::Mutex;
use tokio::task::JoinSet;
use tracing::{error, info, warn};
use wrpc_transport_quic::{Client, Server};

/// Cert Manager, used to manage certificates
pub struct CertManager;

impl CertManager {
    // Get the certificate directory
    pub fn cert_dir() -> PathBuf {
        directories_next::ProjectDirs::from("org", "your-org", "wrpc")
            .expect("failed to find project directory")
            .data_local_dir()
            .to_path_buf()
    }

    // Certificate file path
    pub fn cert_path() -> PathBuf {
        Self::cert_dir().join("cert.der")
    }

    // Private key file path
    pub fn key_path() -> PathBuf {
        Self::cert_dir().join("key.der")
    }

    // Read or generate server certificate configuration
    pub fn server_crypto_config() -> anyhow::Result<Arc<QuicServerConfig>> {
        let cert_path = Self::cert_path();
        let key_path = Self::key_path();

        // Try to read existing certificates
        let (cert, key) = match fs::read(&cert_path).and_then(|x| Ok((x, fs::read(&key_path)?))) {
            Ok((cert, key)) => {
                info!("using existing certificates");
                (
                    CertificateDer::from(cert),
                    PrivateKeyDer::try_from(key)
                        .map_err(|e| anyhow!("invalid key format: {}", e))?,
                )
            }
            Err(ref e) if e.kind() == io::ErrorKind::NotFound => {
                info!("generating self-signed certificate");
                let cert = rcgen::generate_simple_self_signed(vec![
                    "localhost".into(),
                    "127.0.0.1".into(),
                    "::1".into(),
                ])?;

                let key = PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der());
                let cert = cert.cert.into();

                // Ensure the certificate directory exists
                fs::create_dir_all(Self::cert_dir())?;

                // Save certificate and private key
                fs::write(&cert_path, &cert).context("failed to write certificate")?;
                fs::write(&key_path, key.secret_pkcs8_der())
                    .context("failed to write private key")?;

                (cert, key.into())
            }
            Err(e) => {
                bail!("failed to read certificate: {}", e);
            }
        };

        let server_crypto = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert], key)?;

        let server_config = QuicServerConfig::try_from(server_crypto)?;
        Ok(Arc::new(server_config))
    }

    // Read client certificate configuration
    pub fn client_crypto_config() -> anyhow::Result<Arc<QuicClientConfig>> {
        let mut roots = rustls::RootCertStore::empty();

        // Try to read the server certificate as the root certificate
        match fs::read(Self::cert_path()) {
            Ok(cert) => {
                roots.add(CertificateDer::from(cert))?;
                info!("using existing server certificate");
            }
            Err(ref e) if e.kind() == io::ErrorKind::NotFound => {
                info!("server certificate not found, please ensure the server is running first");
            }
            Err(e) => {
                bail!("failed to read server certificate: {}", e);
            }
        }

        let client_crypto = rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();

        let client_config = QuicClientConfig::try_from(client_crypto)?;
        Ok(Arc::new(client_config))
    }
}

struct CertConfig {
    server_config: Arc<QuicServerConfig>,
    client_config: Arc<QuicClientConfig>,
}
struct Inner {
    address: String,
    host: Host<Client, Server>,
    // endpoint: Endpoint,
    handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    srv_config: Arc<QuicServerConfig>,
    clt_config: Arc<QuicClientConfig>,
}

static CRYPTO_PROVIDER: OnceCell<()> = OnceCell::new();

static CERT_CONFIG: OnceCell<CertConfig> = OnceCell::new();

// Initialize and get certificate configuration functions
pub(crate) fn get_or_init_cert_config(
) -> anyhow::Result<(Arc<QuicServerConfig>, Arc<QuicClientConfig>)> {
    let config = CERT_CONFIG.get_or_try_init(|| {
        let (srv_config, clt_config) = cert_pair()?;

        // Try to convert the Rustls configuration to QUIC configuration
        let srv_crypto_config: QuicServerConfig = srv_config
            .try_into()
            .context("failed to convert server config")?;
        let clt_crypto_config: QuicClientConfig = clt_config
            .try_into()
            .context("failed to convert client config")?;

        Ok::<CertConfig, WasmError>(CertConfig {
            server_config: Arc::new(srv_crypto_config),
            client_config: Arc::new(clt_crypto_config),
        })
    })?;

    Ok((config.server_config.clone(), config.client_config.clone()))
}
#[derive(Clone)]
pub struct WasmRuntime {
    inner: Arc<Inner>,
}

pub(crate) fn ensure_crypto_provider() -> anyhow::Result<()> {
    CRYPTO_PROVIDER.get_or_try_init(|| {
        rustls::crypto::ring::default_provider()
            .install_default()
            .map_err(|e| anyhow::Error::msg("failed to install default crypto provider"))
    })?;
    Ok(())
}

pub fn cert_pair() -> anyhow::Result<(RustlsServerConfig, RustlsClientConfig)> {
    let CertifiedKey {
        cert: srv_crt,
        key_pair: srv_key,
    } = generate_simple_self_signed([
        "127.0.0.1".to_string(),
        "::1".to_string(),
        "localhost".to_string(),
    ])
    .context("failed to generate server certificate")?;

    let CertifiedKey {
        cert: clt_crt,
        key_pair: clt_key,
    } = generate_simple_self_signed(["client.wrpc".to_string()])
        .context("failed to generate client certificate")?;

    let srv_crt = CertificateDer::from(srv_crt);

    let mut ca = RootCertStore::empty();
    ca.add(srv_crt.clone())?;

    let clt_cnf = RustlsClientConfig::builder_with_protocol_versions(&[&TLS13])
        .with_root_certificates(ca)
        .with_client_auth_cert(
            vec![clt_crt.into()],
            PrivatePkcs8KeyDer::from(clt_key.serialize_der()).into(),
        )
        .context("failed to create client config")?;

    let srv_cnf = RustlsServerConfig::builder_with_protocol_versions(&[&TLS13])
        .with_no_client_auth()
        .with_single_cert(
            vec![srv_crt],
            PrivatePkcs8KeyDer::from(srv_key.serialize_der()).into(),
        )
        .context("failed to create server config")?;

    Ok((srv_cnf, clt_cnf))
}

impl WasmRuntime {
    pub async fn new(address: Option<&str>) -> Result<Self, WasmError> {
        // Install the default crypto provider
        ensure_crypto_provider().map_err(WasmError::from)?;

        // Get certificate configuration
        let srv_crypto_config = CertManager::server_crypto_config()
            .context("failed to get server certificate configuration")
            .map_err(WasmError::from)?;

        let clt_crypto_config = CertManager::client_crypto_config()
            .context("failed to get client certificate configuration")
            .map_err(WasmError::from)?;

        // Create a client endpoint
        let mut client_endpoint = Endpoint::client((Ipv4Addr::LOCALHOST, 0).into())
            .context("failed to create client endpoint")
            .map_err(WasmError::from)?;

        client_endpoint.set_default_client_config(ClientConfig::new(clt_crypto_config.clone()));

        // Create a server endpoint
        let server_endpoint = Endpoint::server(
            ServerConfig::with_crypto(srv_crypto_config.clone()),
            match address {
                Some(addr) => addr
                    .parse()
                    .context("failed to parse address")
                    .map_err(WasmError::from)?,
                None => (Ipv4Addr::LOCALHOST, 0).into(),
            },
        )
        .context("failed to create server endpoint")
        .map_err(WasmError::from)?;

        let server_addr = server_endpoint
            .local_addr()
            .context("failed to get local address")
            .map_err(WasmError::from)?;

        tracing::info!("Wasm runtime listening on: {}", server_addr);

        // Create a QUIC server
        let srv = Arc::new(Server::new());

        let accept = tokio::spawn({
            let mut tasks = JoinSet::<anyhow::Result<()>>::new();
            let srv = Arc::clone(&srv);
            let ep = server_endpoint;
            async move {
                loop {
                    select! {
                        Some(conn) = ep.accept() => {
                            let srv = Arc::clone(&srv);
                            tasks.spawn(async move {
                                let conn = conn
                                    .accept()
                                    .context("failed to accept QUIC connection")?;
                                let conn = conn.await.context("failed to establish QUIC connection")?;
                                tracing::info!("new connection established");
                                let wrpc = wrpc_transport_quic::Client::from(conn);
                                loop {
                                    srv.accept(&wrpc)
                                        .await
                                        .context("failed to accept wRPC connection")?;
                                }
                            });
                        }
                        Some(res) = tasks.join_next() => {
                            match res {
                                Ok(Ok(())) => {}
                                Ok(Err(err)) => {
                                    warn!(?err, "failed to serve connection")
                                }
                                Err(err) => {
                                    error!(?err, "failed to join task")
                                }
                            }
                        }
                        else => {
                            return;
                        }
                    }
                }
            }
        });

        // Create a connection
        let connection = client_endpoint
            .connect(server_addr, "::1")
            .context("failed to initiate connection")
            .map_err(WasmError::from)?
            .await
            .context("failed to establish connection")
            .map_err(WasmError::from)?;

        let clt = Arc::new(Client::from(connection));

        let host = Host::new(clt.clone(), srv.clone());

        let inner = Inner {
            address: server_addr.to_string(),
            host,
            handle: Mutex::new(Some(accept)),
            srv_config: srv_crypto_config,
            clt_config: clt_crypto_config,
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
        tracing::info!("launching component: {}", component_id);
        self.inner
            .host
            .launch_component(component_id, wasm, None, None)
            .await
    }

    pub async fn remove_component(&self, component_id: &str) -> Result<(), WasmError> {
        self.inner.host.remove_component(component_id).await
    }

    pub fn address(&self) -> &str {
        &self.inner.address
    }

    pub fn server_config(&self) -> Arc<QuicServerConfig> {
        self.inner.srv_config.clone()
    }

    pub fn client_config(&self) -> Arc<QuicClientConfig> {
        self.inner.clt_config.clone()
    }

    pub async fn join(&self) -> Result<(), WasmError> {
        self.inner
            .handle
            .lock()
            .await
            .take()
            .unwrap()
            .await
            .unwrap();
        Ok(())
    }
}

impl Debug for WasmRuntime {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("WasmRuntime").finish()
    }
}

impl Drop for WasmRuntime {
    fn drop(&mut self) {}
}
