use crate::message::{RpcMessage, TaskStateResult};
use crate::{Lyric, TaskDescription, WrappedData};
use lyric_rpc::task::driver_rpc_server::DriverRpc;
use lyric_rpc::task::worker_rpc_server::WorkerRpc;
use lyric_rpc::task::{
    RegisterWorkerReply, RegisterWorkerRequest, StopWorkerReply, StopWorkerRequest, TaskInfo,
    TaskStateInfo, TaskStateReply, TaskStateRequest, TaskStopReply, TaskStopRequest,
    TaskStreamSubmitRequest, TaskSubmitReply, TaskSubmitRequest,
};
use lyric_utils::err::TaskError;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_stream::{wrappers::ReceiverStream, Stream, StreamExt};
use tonic::{transport::Server, Request, Response, Status, Streaming};

pub(crate) const RPC_PROTOCOL_VERSION: i32 = 1;

type RpcResult<T> = Result<Response<T>, Status>;
type ResponseStream = Pin<Box<dyn Stream<Item = Result<TaskSubmitReply, Status>> + Send>>;

#[derive(Debug)]
pub struct DriverService {
    tx_to_core: mpsc::UnboundedSender<RpcMessage>,
}

#[derive(Debug)]
pub struct WorkerService {
    tx_to_core: mpsc::UnboundedSender<RpcMessage>,
    lyric: Lyric,
}

impl DriverService {
    pub fn new(tx_to_core: mpsc::UnboundedSender<RpcMessage>) -> Self {
        Self { tx_to_core }
    }
}

#[tonic::async_trait]
impl DriverRpc for DriverService {
    async fn register_worker(
        &self,
        request: Request<RegisterWorkerRequest>,
    ) -> RpcResult<RegisterWorkerReply> {
        let msg = request.into_inner();
        let version = msg.version;
        match self.tx_to_core.send(RpcMessage::RegisterWorker(msg)) {
            Ok(_) => Ok(Response::new(RegisterWorkerReply {
                version,
                status: "Register Worker OK!!".to_string(),
            })),
            Err(e) => {
                let error_msg = format!(
                    "Some error when send message to core, detail: {}",
                    e.to_string()
                );
                Err(Status::internal(error_msg))
            }
        }
    }

    async fn task_state_change(
        &self,
        request: Request<TaskStateRequest>,
    ) -> RpcResult<TaskStateReply> {
        let msg = request.into_inner();
        let version = msg.version;
        match self.tx_to_core.send(RpcMessage::TaskStateChange(msg)) {
            Ok(_) => Ok(Response::new(TaskStateReply { version })),
            Err(e) => {
                let error_msg = format!(
                    "Some error when send message to core, detail: {}",
                    e.to_string()
                );
                Err(Status::internal(error_msg))
            }
        }
    }
}

impl WorkerService {
    pub fn new(tx_to_core: mpsc::UnboundedSender<RpcMessage>, lyric: Lyric) -> Self {
        Self { tx_to_core, lyric }
    }

    #[tracing::instrument(level = "debug", skip_all, fields(task_info = ?task_info.task.as_ref().unwrap().task_id))]
    async fn _parse_task_desc(
        &self,
        task_info: TaskSubmitRequest,
        streaming_result: bool,
    ) -> Result<TaskStateResult, Status> {
        let task_info = task_info
            .task
            .ok_or(Status::invalid_argument("Task info is required"))?;
        let mut task_desc =
            TaskDescription::try_from(task_info).map_err(|e| Status::internal(e.to_string()))?;
        task_desc.streaming_result = streaming_result;
        let res = self
            .lyric
            .submit_task(task_desc, None)
            .await
            .map_err(|e| Status::internal(e.to_string()));
        res
    }
}

#[tonic::async_trait]
impl WorkerRpc for WorkerService {
    async fn stop_worker(
        &self,
        request: Request<StopWorkerRequest>,
    ) -> Result<Response<StopWorkerReply>, Status> {
        todo!()
    }

    /// Receive a task from the driver and submit it to the core.
    /// The core will execute the task and return the task state to the worker.
    async fn submit_task(&self, request: Request<TaskSubmitRequest>) -> RpcResult<TaskSubmitReply> {
        let task_info = request.into_inner();
        let version = task_info.version;
        let mut res = self._parse_task_desc(task_info, false).await;
        match res {
            Ok(TaskStateResult::TaskState(s)) => {
                let task_state_info = TaskStateInfo::from(s);
                Ok(Response::new(TaskSubmitReply {
                    version,
                    task: Some(task_state_info),
                }))
            }
            Err(e) => Err(e),
            _ => Err(Status::internal("Some error when submit task")),
        }
    }

    async fn stop_task(&self, request: Request<TaskStopRequest>) -> RpcResult<TaskStopReply> {
        let request = request.into_inner();
        let version = request.version;
        self.lyric
            .stop_task(request.task_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))
            .map(|_| Response::new(TaskStopReply { version }))
    }

    type ToStreamSubmitTaskStream = ResponseStream;

    async fn to_stream_submit_task(
        &self,
        request: Request<TaskSubmitRequest>,
    ) -> RpcResult<Self::ToStreamSubmitTaskStream> {
        let task_info = request.into_inner();
        let version = task_info.version;
        let mut res = self._parse_task_desc(task_info, true).await;
        match res {
            Ok(TaskStateResult::StreamTaskState(mut s)) => {
                let (tx, rx) = mpsc::channel(64);
                tokio::spawn(async move {
                    while let Some(s) = s.next().await {
                        tracing::info!("to_stream_submit_task: Receive task state: {:?}", s);
                        match s {
                            Ok(s) => {
                                let _ = tx
                                    .send(Ok(TaskSubmitReply {
                                        version,
                                        task: Some(s),
                                    }))
                                    .await
                                    .unwrap();
                            }
                            Err(TaskError::StreamStopped) => {
                                // Stream stopped, break the loop
                                break;
                            }
                            Err(e) => {
                                let _ =
                                    tx.send(Err(Status::internal(e.to_string()))).await.unwrap();
                                break;
                            }
                        }
                    }
                });
                let out_stream = ReceiverStream::new(rx);
                Ok(Response::new(
                    Box::pin(out_stream) as Self::ToStreamSubmitTaskStream
                ))
            }
            Err(e) => Err(e),
            _ => Err(Status::internal("Some error when submit task")),
        }
    }

    type StreamTransformSubmitTaskStream = ResponseStream;

    async fn stream_transform_submit_task(
        &self,
        request: Request<Streaming<TaskStreamSubmitRequest>>,
    ) -> RpcResult<Self::StreamTransformSubmitTaskStream> {
        todo!()
    }

    async fn un_stream_transform_submit_task(
        &self,
        request: Request<Streaming<TaskStreamSubmitRequest>>,
    ) -> Result<Response<TaskSubmitReply>, Status> {
        todo!()
    }
}
