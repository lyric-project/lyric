use crate::config::{Config, DriverConfig};
use crate::core_lyric::CoreLyric;
use crate::env::{EnvironmentConfigMessage, WorkerEnvManager};
use crate::message::{RpcMessage, TaskStateResult};
use crate::rpc::RPC_PROTOCOL_VERSION;
use crate::worker::connect_to_driver;
use crate::{LangWorkerMessage, TaskDescription, TokioRuntime, WorkerConfig};
use lyric_rpc::task::{RegisterWorkerRequest, TaskStateInfo, WorkerInfo};
use lyric_utils::log::init_tracing_subscriber;
use lyric_utils::prelude::*;
use lyric_wasm_runtime::WasmRuntime;
use std::collections::HashMap;
use std::fmt::Debug;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot, Mutex};
use tracing;
use uuid::Uuid;

type AsyncCallback = Box<
    dyn Fn(Result<TaskStateResult, TaskError>) -> Pin<Box<dyn Future<Output = ()> + Send>>
        + Send
        + Sync,
>;
struct TaskCallback(pub AsyncCallback);

impl Debug for TaskCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskCallback").finish()
    }
}

#[derive(Debug)]
struct LyricInner {
    tx_to_core: mpsc::UnboundedSender<RpcMessage>,
    tx_shutdown: Mutex<Option<oneshot::Sender<()>>>,
    core_state: Mutex<Option<tokio::task::JoinHandle<Result<(), Error>>>>,
    runtime: TokioRuntime,
    rpc_shutdown: Mutex<Option<oneshot::Sender<()>>>,
    config: Arc<Config>,
    callbacks: Mutex<HashMap<String, TaskCallback>>,
    wasm_runtime: Mutex<Option<WasmRuntime>>,
}
#[derive(Clone, Debug)]
pub struct Lyric {
    inner: Arc<LyricInner>,
}

impl Lyric {
    pub fn new(
        runtime: TokioRuntime,
        tx_lang_worker: mpsc::UnboundedSender<LangWorkerMessage>,
        config: Config,
    ) -> Result<Self, Error> {
        let default_log_level = config.log_level.clone().unwrap_or("INFO".to_string());
        init_tracing_subscriber("lyric", default_log_level);
        let (tx_shutdown, rx_shutdown) = oneshot::channel();
        let (tx_api, rx_api) = mpsc::unbounded_channel();
        let (tx_notify, rx_notify) = mpsc::unbounded_channel();

        let config = Arc::new(config.to_default("0.0.0.0")?);
        let worker_manager =
            WorkerEnvManager::new(config.clone(), tx_notify.clone(), runtime.clone());
        let core = CoreLyric {
            tx_api: tx_api.clone(),
            rx_api,
            tx_notify,
            rx_notify,
            tx_lang_worker: Some(tx_lang_worker),
            worker_manager,
            pending_tasks: Vec::new(),
            config: Arc::clone(&config),
        };
        let core_handle = runtime.runtime.clone().spawn(core.main(rx_shutdown));

        let inner = LyricInner {
            tx_to_core: tx_api,
            tx_shutdown: Mutex::new(Some(tx_shutdown)),
            // core_state: Mutex::new(CoreState::Running(core_handle)),
            core_state: Mutex::new(Some(core_handle)),
            runtime,
            rpc_shutdown: Mutex::new(None),
            config,
            callbacks: Mutex::new(HashMap::new()),
            wasm_runtime: Mutex::new(None),
        };
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    pub fn start_driver(&self, _: DriverConfig) -> Result<(), Error> {
        let (tx, rx) = oneshot::channel();

        let mut rpc_shutdown = self
            .inner
            .runtime
            .runtime
            .block_on(self.inner.rpc_shutdown.lock());
        *rpc_shutdown = Some(tx);
        let addr = self.inner.config.parse_address()?;
        self._start_in_driver(addr, rx)
    }

    pub fn start_worker(&self, config: WorkerConfig) -> Result<(), Error> {
        let (tx, rx) = oneshot::channel();
        let mut rpc_shutdown = self
            .inner
            .runtime
            .runtime
            .block_on(self.inner.rpc_shutdown.lock());
        *rpc_shutdown = Some(tx);
        let addr = self.inner.config.parse_address()?;
        self._start_in_worker(addr, config.driver_address, rx)
    }

    fn _start_in_driver(&self, addr: String, rx: oneshot::Receiver<()>) -> Result<(), Error> {
        use crate::rpc::DriverService;
        use lyric_rpc::task::driver_rpc_server::DriverRpcServer;
        use tonic::transport::Server;

        let tx_to_core = self.inner.tx_to_core.clone();

        tracing::info!("Starting driver server on {}", addr);
        let socket_addr = addr
            .parse()
            .map_err(|e| Error::InternalError(format!("Failed to parse address: {}", e)))?;

        self.inner.runtime.runtime.block_on(async {
            check_address_availability(&addr, Duration::from_secs(5), 3, Duration::from_secs(1))
                .await
                .map_err(|e| Error::InternalError(format!("Address check failed: {}", e)))
        })?;
        tracing::info!("Server listening on {}", addr);

        self.inner.runtime.runtime.spawn(async move {
            let driver_service = DriverService::new(tx_to_core);
            let _ = Server::builder()
                .add_service(DriverRpcServer::new(driver_service))
                .serve_with_shutdown(socket_addr, async {
                    rx.await.ok();
                })
                .await;
        });

        Ok(())
    }

    fn _start_in_worker(
        &self,
        addr: String,
        driver_addr: String,
        rx: oneshot::Receiver<()>,
    ) -> Result<(), Error> {
        use crate::rpc::WorkerService;
        use lyric_rpc::task::worker_rpc_server::WorkerRpcServer;
        use tonic::transport::Server;
        let tx_to_core = self.inner.tx_to_core.clone();

        let rt = self.inner.runtime.clone();
        let pg = self.clone();

        let public_addr = self.inner.config.parse_public_address("http")?;
        let worker_id = self.inner.config.parse_node_id();

        let inner = self.inner.clone();

        let socket_addr = addr
            .parse()
            .map_err(|e| Error::InternalError(format!("Failed to parse address: {}", e)))?;

        self.inner.runtime.runtime.block_on(async {
            check_address_availability(&addr, Duration::from_secs(5), 3, Duration::from_secs(1))
                .await
                .map_err(|e| Error::InternalError(format!("Address check failed: {}", e)))
        })?;

        tracing::info!("LyricServer {} listening on {}", worker_id, addr);
        tracing::info!("Connect to driver: {}", driver_addr);

        self.inner.runtime.runtime.spawn(async move {
            let worker_service = WorkerService::new(tx_to_core, pg);

            let (tx_server, rx_server) = oneshot::channel();
            // Start wasm runtime
            let wasm_rt = WasmRuntime::new(None).await.unwrap();
            let wasm_handler_address = wasm_rt.address().to_string();
            rt.runtime.spawn(async move {
                let _ = rx_server.await;
                let mut driver_client = connect_to_driver(&driver_addr).await.unwrap();
                let req = RegisterWorkerRequest {
                    version: RPC_PROTOCOL_VERSION,
                    worker: Some(WorkerInfo {
                        worker_id,
                        address: public_addr,
                        handler_address: wasm_handler_address,
                        total_memory: 0,
                        used_memory: 0,
                        total_cpu: 0,
                        used_cpu: 0,
                    }),
                };
                driver_client.register_worker(req).await.unwrap();
                tracing::info!("Worker registered to driver");
            });
            inner.wasm_runtime.lock().await.replace(wasm_rt);
            let _ = Server::builder()
                .add_service(WorkerRpcServer::new(worker_service))
                .serve_with_shutdown(socket_addr, async {
                    let _ = tx_server.send(());
                    rx.await.ok();
                })
                .await;
        });

        Ok(())
    }

    pub fn stop(&mut self) {
        self._stop_core();
        self._stop_rpc();
    }

    fn _stop_core(&mut self) {
        let mut core_tx_shutdown = self
            .inner
            .runtime
            .runtime
            .block_on(self.inner.tx_shutdown.lock());
        // Send shutdown signal to core
        if let Some(tx) = core_tx_shutdown.take() {
            let _ = tx.send(());
        }
        drop(core_tx_shutdown);

        let mut core_state_handle = self
            .inner
            .runtime
            .runtime
            .block_on(self.inner.core_state.lock());
        if let Some(handle) = core_state_handle.take() {
            let _ = self.inner.runtime.runtime.block_on(handle);
        }
    }
    fn _stop_rpc(&mut self) {
        let mut rpc_shutdown = self
            .inner
            .runtime
            .runtime
            .block_on(self.inner.rpc_shutdown.lock());

        // Send shutdown signal to rpc server
        if let Some(tx) = rpc_shutdown.take() {
            let _ = tx.send(());
        }
    }

    /// Submit a task to the core
    ///
    /// When the task is submitted, it will be added to the task queue and executed by the core.
    pub async fn submit_task(
        &self,
        task_info: TaskDescription,
        env: Option<EnvironmentConfigMessage>,
    ) -> Result<TaskStateResult, Error> {
        let (tx, rx) = oneshot::channel();
        self.call_core(
            RpcMessage::SubmitTask {
                rpc: task_info,
                tx,
                env,
            },
            rx,
        )
        .await
    }

    #[tracing::instrument(level = "debug", skip_all, fields(task_info = ?task_info.trace_info()))]
    pub async fn submit_task_with_callback<F, U>(
        &self,
        task_info: TaskDescription,
        env: Option<EnvironmentConfigMessage>,
        callback: F,
    ) -> Result<String, Error>
    where
        F: Fn(Result<TaskStateResult, TaskError>) -> U + Send + Sync + 'static,
        U: Future<Output = ()> + Send + 'static,
    {
        let task_id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        {
            let mut callbacks = self.inner.callbacks.lock().await;
            // Check if the task_id already exists
            if callbacks.contains_key(&task_id) {
                return Err(Error::InternalError("Task ID already exists".to_string()));
            }
            callbacks.insert(
                task_id.clone(),
                TaskCallback(Box::new(move |result| Box::pin(callback(result)))),
            );
        }
        match self.inner.tx_to_core.send(RpcMessage::SubmitTask {
            rpc: task_info,
            tx,
            env,
        }) {
            Err(e) => {
                // tracing::error!("core_call fatal error");
                return Err(Error::InternalError(
                    format!("Failed to send message to core: {}", e).to_string(),
                ));
            }
            _ => {}
        }

        // Spawn a task to wait for the result and call the callback
        let inner = self.inner.clone();
        let copy_task_id = task_id.clone();
        self.inner.runtime.runtime.spawn(async move {
            let task_res = match rx.await {
                Ok(Ok(result)) => Ok(result),
                Ok(Err(e)) => {
                    tracing::error!("Failed to get result for task, error: {:?}", e);
                    Err(TaskError::TaskExecuteError(e.to_string()))
                }
                Err(e) => {
                    tracing::error!("Failed to get result for task, error: {:?}", e);
                    Err(TaskError::TaskExecuteError(e.to_string()))
                }
            };
            tracing::debug!("Task finished, task_id: {}", copy_task_id);
            let mut callbacks = inner.callbacks.lock().await;
            if let Some(callback) = callbacks.remove(&copy_task_id) {
                // Call the callback with the result
                tracing::debug!("Calling callback for task_id: {}", copy_task_id);
                drop(callbacks);
                callback.0(task_res).await;
                tracing::debug!("Callback finished for task_id: {}", copy_task_id);
            } else {
                tracing::error!("Callback not found for task_id: {}", copy_task_id);
            }
        });

        Ok(task_id)
    }

    pub async fn stop_task(&self, task_id: String) -> Result<(), Error> {
        let (tx, rx) = oneshot::channel();
        self.call_core(
            RpcMessage::StopTask {
                task_id: task_id.into(),
                tx,
            },
            rx,
        )
        .await
    }

    /// Call core with a message and wait for the response
    pub(crate) async fn call_core<T>(
        &self,
        msg: RpcMessage,
        res_rx: oneshot::Receiver<Result<T, Error>>,
    ) -> Result<T, Error> {
        match self.inner.tx_to_core.send(msg) {
            Err(e) => {
                // tracing::error!("core_call fatal error");
                return Err(Error::InternalError(
                    format!("Failed to send message to core: {}", e).to_string(),
                ));
            }
            _ => {}
        }
        match res_rx.await {
            Ok(r) => r.map_err(|e| Error::APIError(e.into())),
            Err(e) => {
                // tracing::error!("core_call fatal error");
                Err(Error::InternalError(
                    format!("Failed to receive response from core: {}", e).to_string(),
                ))
            }
        }
    }
    pub async fn with_wasm_runtime<F, U, T>(&self, f: F) -> Result<T, Error>
    where
        F: FnOnce(WasmRuntime) -> U + Send + Sync + 'static,
        U: Future<Output = Result<T, Error>> + Send + 'static,
    {
        let wasm_rt = self.inner.wasm_runtime.lock().await;
        let rt = wasm_rt
            .as_ref()
            .ok_or(Error::InternalError("Wasm runtime not found".to_string()))?
            .clone();
        drop(wasm_rt);
        let res = f(rt).await;
        res
    }
}

pub(crate) async fn check_address_availability(
    addr: &str,
    timeout: Duration,
    retry_times: usize,
    retry_interval: Duration,
) -> Result<(), Error> {
    let socket_addr: SocketAddr = addr
        .parse()
        .map_err(|e| Error::InternalError(format!("Failed to parse address {}: {}", addr, e)))?;
    let mut current_try = 0;
    loop {
        tracing::info!(
            "Checking address availability: {} (attempt {}/{})",
            addr,
            current_try + 1,
            retry_times
        );

        match tokio::time::timeout(timeout, TcpListener::bind(&socket_addr)).await {
            Ok(result) => match result {
                Ok(_) => {
                    tracing::info!("Address {} is available", addr);
                    return Ok(());
                }
                Err(e) => {
                    tracing::warn!("Failed to bind to {}: {}", addr, e);
                }
            },
            Err(_) => {
                tracing::warn!("Timeout while checking address: {}", addr);
                return Err(Error::InternalError(format!(
                    "Timeout while checking address: {}",
                    addr
                )));
            }
        }

        current_try += 1;
        if current_try >= retry_times {
            return Err(Error::InternalError(format!(
                "Failed to bind to address: {}",
                addr
            )));
        }

        tracing::info!("Retrying in {} seconds...", retry_interval.as_secs());
        tokio::time::sleep(retry_interval).await;
    }
}
