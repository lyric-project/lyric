use crate::env::config::{EnvironmentBuilder, InnerEnvironment};
use crate::env::env::{ChildProcess, EnvStatusCode, EventStream, ExecutionEnvironment};
use async_trait::async_trait;
use bollard::container::{
    Config, CreateContainerOptions, KillContainerOptions, LogOutput, RemoveContainerOptions,
    StartContainerOptions, WaitContainerOptions,
};
use bollard::exec::{CreateExecOptions, StartExecResults};
use bollard::models::{
    ContainerWaitResponse, EndpointSettings, HostConfig, Mount, MountTypeEnum, PortBinding,
};
use bollard::network::{CreateNetworkOptions, ListNetworksOptions};
use bollard::Docker;
use futures_util::{StreamExt, TryFutureExt};
use lyric_utils::err::EnvError;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::path::{Path, PathBuf};
use tokio_stream::wrappers::UnboundedReceiverStream;

pub(crate) struct DockerChildProcess {
    docker: Docker,
    container_id: String,
    stdout: Option<EventStream>,
    stderr: Option<EventStream>,
    exit_code: Option<EnvStatusCode>,
    bridge_name: Option<String>,
    remove_bridge: bool,
}

impl Debug for DockerChildProcess {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Dcontainer_id: {}", self.container_id)
    }
}
impl DockerChildProcess {
    async fn cleanup_bridge(&self) -> Result<(), EnvError> {
        if let Some(bridge_name) = &self.bridge_name {
            if self.remove_bridge {
                self.docker
                    .remove_network(bridge_name)
                    .await
                    .map_err(|e| EnvError::LaunchEnvError(e.to_string()))?;
                println!("Removed bridge network: {}", bridge_name);
            }
        }
        Ok(())
    }
}

#[async_trait]
impl ChildProcess for DockerChildProcess {
    fn stdout(&mut self) -> Result<EventStream, EnvError> {
        self.stdout.take().ok_or(EnvError::LaunchEnvError(
            "stdout stream already taken".into(),
        ))
    }

    fn stderr(&mut self) -> Result<EventStream, EnvError> {
        self.stderr.take().ok_or(EnvError::LaunchEnvError(
            "stderr stream already taken".into(),
        ))
    }

    async fn wait(&mut self) -> Result<EnvStatusCode, EnvError> {
        if let Some(exit_code) = self.exit_code {
            return Ok(exit_code);
        }

        let mut wait_stream = self
            .docker
            .wait_container(&self.container_id, None::<WaitContainerOptions<String>>);

        while let Some(wait_result) = wait_stream.next().await {
            match wait_result {
                Ok(ContainerWaitResponse { status_code, error }) => {
                    if let Some(error) = error {
                        return Err(EnvError::LaunchEnvError(error.message.unwrap_or_else(
                            || "Unknown error during container wait".to_string(),
                        )));
                    }
                    let status_code = status_code as i32;
                    self.exit_code = Some(status_code);
                    return Ok(status_code);
                }
                Err(e) => return Err(EnvError::LaunchEnvError(e.to_string())),
            }
        }

        Err(EnvError::LaunchEnvError(
            "Wait stream ended unexpectedly".into(),
        ))
    }

    async fn try_wait(&mut self) -> Result<Option<EnvStatusCode>, EnvError> {
        if let Some(exit_code) = self.exit_code {
            return Ok(Some(exit_code));
        }

        let container_info = self
            .docker
            .inspect_container(&self.container_id, None)
            .await
            .map_err(|e| EnvError::LaunchEnvError(e.to_string()))?;

        if let Some(state) = container_info.state {
            if let Some(exit_code) = state.exit_code {
                let status_code = exit_code as i32;
                self.exit_code = Some(status_code);
                Ok(Some(status_code))
            } else if state.running.unwrap_or(false) {
                Ok(None)
            } else {
                Err(EnvError::LaunchEnvError(
                    "Container is not running and exit code is not available".into(),
                ))
            }
        } else {
            Err(EnvError::LaunchEnvError(
                "Unable to get container state".into(),
            ))
        }
    }

    async fn cleanup(&mut self) -> Result<(), EnvError> {
        tracing::debug!("Cleaning up container {}", self.container_id);
        kill_container(&self.docker, &self.container_id).await?;
        tracing::debug!("Killed container {}", self.container_id);
        remove_container(&self.docker, &self.container_id).await?;
        tracing::debug!("Trying to cleanup bridge network if necessary");
        self.cleanup_bridge().await?;
        Ok(())
    }
}

async fn kill_container(docker: &Docker, container_id: &str) -> Result<(), EnvError> {
    let kill_options = Some(KillContainerOptions { signal: "SIGKILL" });
    docker
        .kill_container(container_id, kill_options)
        .await
        .map_err(|e| EnvError::LaunchEnvError(e.to_string()))
}

async fn remove_container(docker: &Docker, container_id: &str) -> Result<(), EnvError> {
    docker
        .remove_container(
            container_id,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
            // None::<RemoveContainerOptions>,
        )
        .await
        .map_err(|e| EnvError::LaunchEnvError(e.to_string()))
}

pub(crate) struct DockerEnvironment {
    docker: Docker,
    inner_env: InnerEnvironment,
    image: String,
    container_name: Option<String>,
    mounts: Vec<(PathBuf, PathBuf)>,
    network: Option<String>,
    port_bindings: HashMap<String, String>,
    bridge_name: Option<String>,
    remove_bridge: bool,
}

impl DockerEnvironment {
    async fn ensure_bridge_network(&self, bridge_name: &str) -> Result<(), EnvError> {
        let networks = self
            .docker
            .list_networks(Some(ListNetworksOptions::<String> {
                filters: {
                    let mut filters = HashMap::new();
                    filters.insert("name".to_string(), vec![bridge_name.to_string()]);
                    filters
                },
                ..Default::default()
            }))
            .await
            .map_err(|e| EnvError::LaunchEnvError(e.to_string()))?;

        if networks.is_empty() {
            // Create the bridge network
            let options = CreateNetworkOptions {
                name: bridge_name.to_string(),
                driver: "bridge".to_string(),
                ..Default::default()
            };

            self.docker
                .create_network(options)
                .await
                .map_err(|e| EnvError::LaunchEnvError(e.to_string()))?;

            println!("Created bridge network: {}", bridge_name);
        } else {
            println!("Bridge network {} already exists", bridge_name);
        }

        Ok(())
    }
}

impl ExecutionEnvironment for DockerEnvironment {
    async fn execute(&mut self) -> Result<Box<dyn ChildProcess>, EnvError> {
        // Check and create bridge network if necessary
        if let Some(bridge_name) = &self.bridge_name {
            self.ensure_bridge_network(bridge_name).await?;
        }

        // Prepare port bindings
        let mut port_bindings = HashMap::new();
        for (container_port, host_port) in &self.port_bindings {
            port_bindings.insert(
                container_port.clone(),
                Some(vec![PortBinding {
                    host_ip: Some(String::from("0.0.0.0")),
                    host_port: Some(host_port.clone()),
                }]),
            );
        }
        // Modify the container config to use the bridge network
        let mut network_config = HashMap::new();
        let mut network_mode = None;
        if let Some(bridge_name) = &self.bridge_name {
            let net = EndpointSettings {
                network_id: Some(bridge_name.clone()),
                ..Default::default()
            };
            network_config.insert(bridge_name.clone(), net);
            network_mode = Some(String::from("bridge"));
        }

        let mounts = self
            .mounts
            .iter()
            .map(|(host_path, container_path)| Mount {
                target: Some(container_path.to_string_lossy().to_string()),
                source: Some(host_path.to_string_lossy().to_string()),
                typ: Some(MountTypeEnum::BIND),
                ..Default::default()
            })
            .collect::<Vec<Mount>>();

        // Create a container
        let container_config = Config {
            image: Some(self.image.clone()),
            cmd: Some(vec!["/bin/sh".to_string()]),
            tty: Some(true),
            exposed_ports: Some(
                self.port_bindings
                    .keys()
                    .map(|k| (k.clone(), HashMap::new()))
                    .collect(),
            ),
            host_config: Some(HostConfig {
                mounts: Some(mounts),
                port_bindings: Some(port_bindings),
                network_mode,
                ..Default::default()
            }),
            networking_config: Some(bollard::container::NetworkingConfig {
                endpoints_config: network_config,
            }),
            ..Default::default()
        };
        let container_name = self
            .container_name
            .as_ref()
            .map(|s| s.clone())
            .unwrap_or("".to_string());
        let container_options: CreateContainerOptions<String> = CreateContainerOptions {
            name: container_name,
            ..Default::default()
        };

        let container_info = self
            .docker
            .create_container(Some(container_options), container_config)
            .map_err(|e| EnvError::LaunchEnvError(e.to_string()))
            .await?;
        let container_id = container_info.id;
        let container_name = self
            .container_name
            .clone()
            .unwrap_or_else(|| container_id.clone());

        // Start the container
        self.docker
            .start_container(
                container_name.as_str(),
                None::<StartContainerOptions<String>>,
            )
            .map_err(|e| EnvError::LaunchEnvError(e.to_string()))
            .await?;

        // Prepare the command
        let exec_options = CreateExecOptions {
            cmd: Some(self.inner_env.args.clone()),
            env: Some(
                self.inner_env
                    .env
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect(),
            ),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            // working_dir: self.inner_env.working_dir,
            ..Default::default()
        };
        // Create the exec instance
        let exec_info = self
            .docker
            .create_exec(&container_id, exec_options)
            .map_err(|e| EnvError::LaunchEnvError(e.to_string()))
            .await?;
        // Start the exec instance
        let exec_result = self
            .docker
            .start_exec(&exec_info.id, None)
            .map_err(|e| EnvError::LaunchEnvError(e.to_string()))
            .await?;

        let (stdout, stderr) = match exec_result {
            StartExecResults::Attached { mut output, .. } => {
                let (stdout_tx, stdout_rx) = tokio::sync::mpsc::unbounded_channel();
                let (stderr_tx, stderr_rx) = tokio::sync::mpsc::unbounded_channel();
                let stdout_stream = UnboundedReceiverStream::new(stdout_rx);
                let stderr_stream = UnboundedReceiverStream::new(stderr_rx);
                tokio::spawn(async move {
                    while let Some(msg) = output.next().await {
                        let now = chrono::Local::now();
                        tracing::debug!("{:?} Receive docker msg: {:?}", now, msg);
                        match msg {
                            Ok(LogOutput::StdOut { message }) => {
                                let _ = stdout_tx.send(Ok(message));
                            }
                            Ok(LogOutput::StdErr { message }) => {
                                let _ = stderr_tx.send(Ok(message));
                            }
                            Err(e) => {
                                let _ =
                                    stderr_tx.send(Err(EnvError::LaunchEnvError(e.to_string())));
                            }
                            _ => {}
                        }
                    }
                });

                (stdout_stream, stderr_stream)
            }
            _ => {
                return Err(EnvError::LaunchEnvError(
                    "Failed to attach to exec instance".into(),
                ))
            }
        };

        println!("Container {} started", container_id);
        Ok(Box::new(DockerChildProcess {
            docker: self.docker.clone(),
            container_id,
            stdout: Some(EventStream(Box::pin(stdout))),
            stderr: Some(EventStream(Box::pin(stderr))),
            exit_code: None,
            bridge_name: self.bridge_name.clone(),
            remove_bridge: self.remove_bridge,
        }))
    }
}

pub(crate) struct DockerEnvironmentBuilder {
    inner_env: InnerEnvironment,
    image: String,
    container_name: Option<String>,
    mounts: Vec<(PathBuf, PathBuf)>,
    network: Option<String>,
    port_bindings: HashMap<String, String>,
    bridge_name: Option<String>,
    remove_bridge: bool,
}

impl DockerEnvironmentBuilder {
    pub fn new<S: Into<String>>(image: S) -> Self {
        Self {
            inner_env: InnerEnvironment::new(),
            image: image.into(),
            container_name: None,
            mounts: Vec::new(),
            network: None,
            port_bindings: HashMap::new(),
            bridge_name: None,
            remove_bridge: false,
        }
    }

    pub fn container_name<S: Into<String>>(mut self, name: S) -> Self {
        self.container_name = Some(name.into());
        self
    }

    pub fn mount<P: AsRef<Path>, Q: AsRef<Path>>(
        mut self,
        host_path: P,
        container_path: Q,
    ) -> Self {
        self.mounts.push((
            host_path.as_ref().to_path_buf(),
            container_path.as_ref().to_path_buf(),
        ));
        self
    }

    pub fn mounts<I, P, Q>(mut self, mounts: I) -> Self
    where
        I: IntoIterator<Item = (P, Q)>,
        P: AsRef<Path>,
        Q: AsRef<Path>,
    {
        for (host_path, container_path) in mounts {
            self = self.mount(host_path, container_path);
        }
        self
    }

    pub fn network<S: Into<String>>(mut self, network: S) -> Self {
        self.network = Some(network.into());
        self
    }

    pub fn port_binding(mut self, container_port: u16, host_port: u16) -> Self {
        self.port_bindings
            .insert(format!("{}/tcp", container_port), format!("{}", host_port));
        self
    }
    pub fn bridge<S: Into<String>>(mut self, name: S, remove_on_cleanup: bool) -> Self {
        self.bridge_name = Some(name.into());
        self.remove_bridge = remove_on_cleanup;
        self
    }
    pub fn build(self) -> Result<DockerEnvironment, EnvError> {
        let docker = Docker::connect_with_local_defaults()
            .map_err(|e| EnvError::LaunchEnvError(e.to_string()))?;
        Ok(DockerEnvironment {
            docker,
            inner_env: self.inner_env,
            image: self.image,
            container_name: self.container_name,
            mounts: self.mounts,
            network: self.network,
            port_bindings: self.port_bindings,
            bridge_name: self.bridge_name,
            remove_bridge: self.remove_bridge,
        })
    }
}

impl EnvironmentBuilder for DockerEnvironmentBuilder {
    fn inner(&mut self) -> &mut InnerEnvironment {
        &mut self.inner_env
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream::StreamExt;
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio_util::io::StreamReader;

    #[tokio::test]
    async fn test_docker_env() {
        let mut env = DockerEnvironmentBuilder::new("ubuntu:latest")
            .container_name("test_container")
            .mount("/tmp/", "/tmp")
            .args(["/bin/ls", "/tmp"])
            .env("ENV1", "VAL1")
            .network("host")
            .build()
            .unwrap();
        let mut child = env.execute().await.unwrap();

        let mut stdout_lines = child.stdout().unwrap().lines().await;

        while let Some(Ok(line)) = stdout_lines.next().await {
            println!("stdout: {:?}", line);
        }

        let mut stderr_lines = child.stderr().unwrap().lines().await;

        while let Some(Ok(line)) = stderr_lines.next().await {
            println!("stderr: {:?}", line);
        }
        child.cleanup().await.unwrap();
    }

    #[tokio::test]
    async fn test_stream_shell() {
        let mut env = DockerEnvironmentBuilder::new("ubuntu:latest")
            .container_name("test_container")
            .args([
                "/bin/bash",
                "-c",
                "for i in {1..5}; do echo $i; sleep 1; done",
            ])
            .build()
            .unwrap();
        let mut child = env.execute().await.unwrap();

        let mut stdout_lines = child.stdout().unwrap().lines().await;

        while let Some(Ok(line)) = stdout_lines.next().await {
            let now = chrono::Local::now();
            println!("{:?} stdout: {:?}", now, line);
        }

        let mut stderr_lines = child.stderr().unwrap().lines().await;

        while let Some(Ok(line)) = stderr_lines.next().await {
            println!("stderr: {:?}", line);
        }
    }

    #[tokio::test]
    async fn test_docker_nginx() {
        let mut env = DockerEnvironmentBuilder::new("nginx:alpine")
            .args(["nginx", "-g", "daemon off;"])
            .port_binding(80, 8189)
            .build()
            .unwrap();

        let mut child = env.execute().await.unwrap();
        let stdout = child.stdout().unwrap();

        let stream = stdout
            .0
            .map(|s| s.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)));
        let reader = StreamReader::new(stream);
        let mut lines = BufReader::new(reader).lines();

        while let Some(line) = lines.next_line().await.unwrap() {
            println!("stdout: {:?}", line);
        }

        let stderr = child.stderr().unwrap();
        let stream = stderr
            .0
            .map(|s| s.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)));
        let reader = StreamReader::new(stream);
        let mut lines = BufReader::new(reader).lines();
        while let Some(line) = lines.next_line().await.unwrap() {
            println!("stderr: {:?}", line);
        }

        child.cleanup().await.unwrap();
    }
}
