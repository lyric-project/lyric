use lyric_rpc::task::driver_rpc_client::DriverRpcClient;
use lyric_rpc::task::worker_rpc_client::WorkerRpcClient;

use lyric_rpc::task::WorkerInfo;
use tonic::transport::Channel;

use lyric_utils::prelude::*;
#[derive(Debug)]
pub(crate) enum WorkerStatus {
    Idle,
    Busy,
}

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub(crate) struct WorkerID {
    pub(crate) env_id: String,
    pub(crate) id: String,
}

impl WorkerID {
    pub(crate) fn new<T>(env_id: T, id: T) -> Self
    where
        T: Into<String>,
    {
        Self {
            env_id: env_id.into(),
            id: id.into(),
        }
    }

    pub(crate) fn full_id(&self) -> String {
        format!("{}@{}", self.env_id, self.id)
    }

    pub(crate) fn from_full_id(full_id: &str) -> Self {
        let parts: Vec<&str> = full_id.split('@').collect();
        if parts.len() != 2 {
            panic!("Invalid worker id: {}", full_id);
        }
        Self::new(parts[0], parts[1])
    }
}

#[derive(Debug)]
pub(crate) struct EnvWorkerInfo {
    pub(crate) worker_info: WorkerInfo,
    pub(crate) status: WorkerStatus,
    pub(crate) worker_client: WorkerRpcClient<Channel>,
    pub(crate) is_health: bool,
    pub(crate) last_heartbeat: i64,
}

pub(crate) async fn connect_to_worker(address: &str) -> Result<WorkerRpcClient<Channel>, Error> {
    let address = if !address.starts_with("http://") {
        format!("http://{}", address)
    } else {
        address.to_string()
    };
    WorkerRpcClient::connect(address)
        .await
        .map_err(|e| Error::ConnectWorker(e.to_string()))
}

pub(crate) async fn connect_to_driver(address: &str) -> Result<DriverRpcClient<Channel>, Error> {
    DriverRpcClient::connect(address.to_string())
        .await
        .map_err(|e| Error::ConnectDriver(e.to_string()))
}
