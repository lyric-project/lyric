use clap::Parser;
use lyric_utils::net_utils::{listen_available_port, local_ip};
use lyric_utils::prelude::Error;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Deserialize)]
pub struct Config {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub public_host: Option<String>,
    pub is_driver: bool,
    /// The port range of start and end for the worker.
    pub worker_port_start: u16,
    pub worker_port_end: u16,
    pub maximum_workers: u32,
    pub minimum_workers: u32,
    pub worker_start_commands: HashMap<String, String>,
    pub node_id: Option<String>,
    pub log_level: Option<String>,
}

impl Config {
    pub fn parse_address(&self) -> Result<String, Error> {
        let host = self
            .host
            .as_ref()
            .ok_or(Error::InternalError("Host is not set".into()))?;
        let port = self
            .port
            .ok_or(Error::InternalError("Port is not set".into()))?;
        Ok(format!("{}:{}", host, port))
    }

    pub fn parse_public_address(&self, schema: &str) -> Result<String, Error> {
        let public_host = self
            .public_host
            .as_ref()
            .ok_or(Error::InternalError("Public Host is not set".into()))?;
        let port = self
            .port
            .ok_or(Error::InternalError("Port is not set".into()))?;
        Ok(format!("{}://{}:{}", schema, public_host, port))
    }

    pub fn parse_node_id(&self) -> String {
        if let Some(node_id) = &self.node_id {
            node_id.clone()
        } else {
            uuid::Uuid::new_v4().to_string()
        }
    }

    pub fn to_default(self, default_host: &str) -> Result<Self, Error> {
        let host = if let Some(host) = self.host {
            host
        } else {
            default_host.to_string()
        };

        let port = if let Some(port) = self.port {
            port
        } else {
            listen_available_port(self.worker_port_start, self.worker_port_end, HashSet::new())
                .ok_or(Error::InternalError("No available port".into()))?
                .0
        };
        let public_host = if let Some(public_host) = self.public_host {
            public_host
        } else {
            local_ip()?
        };
        Ok(Self {
            host: Some(host),
            port: Some(port),
            public_host: Some(public_host),
            is_driver: self.is_driver,
            worker_port_start: self.worker_port_start,
            worker_port_end: self.worker_port_end,
            maximum_workers: self.maximum_workers,
            minimum_workers: self.minimum_workers,
            worker_start_commands: self.worker_start_commands,
            node_id: self.node_id,
            log_level: self.log_level,
        })
    }
}

#[derive(Clone, Debug, Parser)]
pub struct DriverConfig {}

#[derive(Clone, Debug, Parser)]
pub struct WorkerConfig {
    /// Driver Address To Connect
    #[clap(long)]
    pub driver_address: String,

    /// Network Mode
    #[clap(long)]
    pub network_mode: Option<String>,
}

#[derive(Clone, Debug, Parser)]
pub(crate) struct InnerWorkerConfig {
    #[clap(long, default_value = None)]
    pub node_id: Option<String>,

    #[clap(long, default_value = None)]
    pub host: Option<String>,

    #[clap(long, default_value = None)]
    pub port: Option<u16>,

    #[clap(long, default_value = None)]
    pub public_host: Option<String>,

    /// Driver Address To Connect
    #[clap(long)]
    pub driver_address: String,

    /// Network Mode
    #[clap(long)]
    pub network_mode: Option<String>,
}

pub(super) struct WorkerStartCommand {
    pub executable: String,
    pub entrypoint: String,
    pub worker_config: InnerWorkerConfig,
    pub extra_args: Vec<String>,
}

impl From<&str> for WorkerConfig {
    fn from(s: &str) -> Self {
        let args = s.split_whitespace().collect::<Vec<&str>>();
        <Self as Parser>::parse_from(args)
    }
}

impl From<Vec<&str>> for WorkerConfig {
    fn from(args: Vec<&str>) -> Self {
        let args = args.iter().map(|s| s.to_string()).collect::<Vec<String>>();
        <Self as Parser>::parse_from(args)
    }
}

impl WorkerStartCommand {
    pub fn new(
        raw_command: &str,
        default_driver_address: &str,
        default_node_id: &str,
        public_host: Option<String>,
    ) -> Result<Self, Error> {
        let args: Vec<&str> = raw_command.split_whitespace().collect();

        if args.len() < 2 {
            return Err(Error::InternalError(
                "Command must have at least executable and entrypoint".into(),
            ));
        }

        let executable = args[0].to_string();
        let entrypoint = args[1].to_string();

        // Separate known args and extra args
        let mut known_args = vec!["worker".to_string()];
        let mut extra_args = Vec::new();
        let known_flags = vec![
            "--node_id",
            "--host",
            "--port",
            "--public-host",
            "--driver-address",
            "--network-mode",
        ];

        let mut i = 2;
        let mut has_driver_address = false;
        let mut has_node_id = false;
        let mut has_public_host = false;
        while i < args.len() {
            if known_flags.contains(&args[i]) {
                known_args.push(args[i].to_string());
                match args[i] {
                    "--driver-address" => {
                        has_driver_address = true;
                    }
                    "--node_id" => {
                        has_node_id = true;
                    }
                    "--public-host" => {
                        has_public_host = true;
                    }
                    _ => {}
                }
                if i + 1 < args.len() {
                    known_args.push(args[i + 1].to_string());
                    i += 2;
                } else {
                    i += 1;
                }
            } else {
                extra_args.push(args[i].to_string());
                i += 1;
            }
        }
        if !has_driver_address {
            known_args.push("--driver-address".to_string());
            known_args.push(default_driver_address.to_string());
        }
        if !has_node_id {
            known_args.push("--node-id".to_string());
            known_args.push(default_node_id.to_string());
        }

        if !has_public_host {
            if let Some(public_host) = public_host {
                known_args.push("--public-host".to_string());
                known_args.push(public_host);
            }
        }

        // Parse WorkerConfig with only known args
        let worker_config = match InnerWorkerConfig::try_parse_from(&known_args) {
            Ok(config) => config,
            Err(e) => {
                return Err(Error::InternalError(format!(
                    "Failed to parse WorkerConfig: {}",
                    e
                )))
            }
        };

        Ok(Self {
            executable,
            entrypoint,
            worker_config,
            extra_args,
        })
    }

    pub fn to_full_command(self) -> Vec<String> {
        let mut command = vec![self.executable, self.entrypoint];

        if let Some(node_id) = self.worker_config.node_id {
            command.push("--node-id".to_string());
            command.push(node_id);
        }
        if let Some(host) = self.worker_config.host {
            command.push("--host".to_string());
            command.push(host);
        }

        if let Some(port) = self.worker_config.port {
            command.push("--port".to_string());
            command.push(port.to_string());
        }
        if let Some(public_host) = self.worker_config.public_host {
            command.push("--public-host".to_string());
            command.push(public_host);
        }

        command.push("--driver-address".to_string());
        command.push(self.worker_config.driver_address);

        if let Some(network_mode) = self.worker_config.network_mode {
            command.push("--network-mode".to_string());
            command.push(network_mode);
        }

        command.extend(self.extra_args);

        command
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_command() {
        let cmd = WorkerStartCommand::new(
            "cargo run --host localhost --port 8080",
            "127.0.0.1",
            "node_id",
        )
        .unwrap();
        assert_eq!(cmd.executable, "cargo");
        assert_eq!(cmd.entrypoint, "run");
        assert_eq!(cmd.worker_config.host, Some("localhost".to_string()));
        assert_eq!(cmd.worker_config.port, Some(8080));
        assert!(cmd.extra_args.is_empty());
    }

    #[test]
    fn test_command_with_extra_args() {
        let cmd = WorkerStartCommand::new(
            "./worker start --host 127.0.0.1 --port 9000 --extra-flag value",
            "127.0.0.1",
            "node_id",
        )
        .unwrap();
        assert_eq!(cmd.executable, "./worker");
        assert_eq!(cmd.entrypoint, "start");
        assert_eq!(cmd.worker_config.host, Some("127.0.0.1".to_string()));
        assert_eq!(cmd.worker_config.port, Some(9000));
        assert_eq!(
            cmd.extra_args,
            vec!["--extra-flag".to_string(), "value".to_string()]
        );
    }

    #[test]
    fn test_command_with_default_values() {
        let cmd = WorkerStartCommand::new("worker run", "127.0.0.1", "node_id").unwrap();
        assert_eq!(cmd.executable, "worker");
        assert_eq!(cmd.entrypoint, "run");
        assert_eq!(cmd.worker_config.host, None);
        assert_eq!(cmd.worker_config.port, None);
        assert_eq!(cmd.worker_config.driver_address, "150");
        assert!(cmd.extra_args.is_empty());
    }

    #[test]
    fn test_command_with_network_mode() {
        let cmd = WorkerStartCommand::new(
            "docker run --network-mode host --driver-address 192.168.1.100",
            "127.0.0.1",
            "node_id",
        )
        .unwrap();
        assert_eq!(cmd.executable, "docker");
        assert_eq!(cmd.entrypoint, "run");
        assert_eq!(cmd.worker_config.network_mode, Some("host".to_string()));
        assert_eq!(cmd.worker_config.driver_address, "192.168.1.100");
        assert!(cmd.extra_args.is_empty());
    }

    #[test]
    fn test_invalid_command() {
        let result = WorkerStartCommand::new("", "127.0.0.1", "node_id");
        assert!(result.is_err());

        let result = WorkerStartCommand::new("just_executable", "127.0.0.1", "node_id");
        assert!(result.is_err());
    }

    #[test]
    fn test_mixed_known_and_unknown_args() {
        let cmd = WorkerStartCommand::new(
            "worker start --host localhost --unknown-flag --port 8080 --another-unknown value",
            "127.0.0.1",
            "node_id",
        )
        .unwrap();
        assert_eq!(cmd.executable, "worker");
        assert_eq!(cmd.entrypoint, "start");
        assert_eq!(cmd.worker_config.host, Some("localhost".to_string()));
        assert_eq!(cmd.worker_config.port, Some(8080));
        assert_eq!(
            cmd.extra_args,
            vec![
                "--unknown-flag".to_string(),
                "--another-unknown".to_string(),
                "value".to_string()
            ]
        );
    }
    #[test]
    fn test_to_full_command() {
        let cmd = WorkerStartCommand::new(
            "cargo run --host localhost --port 8080 --extra-flag value",
            "127.0.0.1",
            "node_id",
        )
        .unwrap();
        let full_command = cmd.to_full_command();
        assert_eq!(
            full_command,
            vec![
                "cargo".to_string(),
                "run".to_string(),
                "--host".to_string(),
                "localhost".to_string(),
                "--port".to_string(),
                "8080".to_string(),
                "--driver-address".to_string(),
                "150".to_string(),
                "--extra-flag".to_string(),
                "value".to_string()
            ]
        );
    }

    #[test]
    fn test_to_full_command_with_network_mode() {
        let cmd = WorkerStartCommand::new(
            "docker run --network-mode host --driver-address 192.168.1.100",
            "127.0.0.1",
            "node_id",
        )
        .unwrap();
        let full_command = cmd.to_full_command();
        assert_eq!(
            full_command,
            vec![
                "docker".to_string(),
                "run".to_string(),
                "--driver-address".to_string(),
                "192.168.1.100".to_string(),
                "--network-mode".to_string(),
                "host".to_string()
            ]
        );
    }

    #[test]
    fn test_to_full_command_with_mixed_args() {
        let cmd = WorkerStartCommand::new(
            "worker start --host localhost --unknown-flag --port 8080 --another-unknown value",
            "127.0.0.1",
            "node_id",
        )
        .unwrap();
        let full_command = cmd.to_full_command();
        assert_eq!(
            full_command,
            vec![
                "worker".to_string(),
                "start".to_string(),
                "--host".to_string(),
                "localhost".to_string(),
                "--port".to_string(),
                "8080".to_string(),
                "--driver-address".to_string(),
                "150".to_string(),
                "--unknown-flag".to_string(),
                "--another-unknown".to_string(),
                "value".to_string()
            ]
        );
    }
}
