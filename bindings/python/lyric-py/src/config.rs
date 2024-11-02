use lyric::{Config, DriverConfig, WorkerConfig};
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[pyclass]
#[derive(Clone, Serialize, Deserialize)]
pub struct PyConfig {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub public_host: Option<String>,
    pub is_driver: bool,
    pub worker_port_start: u16,
    pub worker_port_end: u16,
    pub maximum_workers: u32,
    pub minimum_workers: u32,
    pub worker_start_commands: HashMap<String, String>,
    pub node_id: Option<String>,
    pub eventloop_worker_threads: Option<usize>,
    pub log_level: Option<String>,
}

#[pyclass]
#[derive(Clone, Serialize, Deserialize)]
pub struct PyWorkerConfig {
    /// Driver Address To Connect
    pub driver_address: String,

    /// Network Mode
    pub network_mode: Option<String>,
}

#[pyclass]
#[derive(Clone, Serialize, Deserialize)]
pub struct PyDriverConfig {}

#[pymethods]
impl PyConfig {
    #[new]
    #[pyo3(signature = (is_driver, host=None, port=None, public_host=None, worker_port_start=None, worker_port_end=None, maximum_workers=None, minimum_workers=None, worker_start_commands=None, node_id=None, eventloop_worker_threads=None, log_level=None))]
    fn new(
        is_driver: bool,
        host: Option<String>,
        port: Option<u16>,
        public_host: Option<String>,
        worker_port_start: Option<u16>,
        worker_port_end: Option<u16>,
        maximum_workers: Option<u32>,
        minimum_workers: Option<u32>,
        worker_start_commands: Option<HashMap<String, String>>,
        node_id: Option<String>,
        eventloop_worker_threads: Option<usize>,
        log_level: Option<String>,
    ) -> Self {
        PyConfig {
            host,
            port,
            public_host,
            is_driver,
            worker_port_start: worker_port_start.unwrap_or(15670),
            worker_port_end: worker_port_end.unwrap_or(16670),
            maximum_workers: maximum_workers.unwrap_or(10),
            minimum_workers: minimum_workers.unwrap_or(1),
            worker_start_commands: worker_start_commands.unwrap_or_default(),
            node_id,
            eventloop_worker_threads,
            log_level,
        }
    }
}

#[pymethods]
impl PyWorkerConfig {
    #[new]
    #[pyo3(signature = (driver_address, network_mode=None))]
    fn new(driver_address: String, network_mode: Option<String>) -> Self {
        PyWorkerConfig {
            driver_address,
            network_mode,
        }
    }
}

#[pymethods]
impl PyDriverConfig {
    #[new]
    fn new() -> Self {
        PyDriverConfig {}
    }
}

impl From<PyConfig> for Config {
    fn from(py_config: PyConfig) -> Self {
        Config {
            host: py_config.host,
            port: py_config.port,
            public_host: py_config.public_host,
            is_driver: py_config.is_driver,
            worker_port_start: py_config.worker_port_start,
            worker_port_end: py_config.worker_port_end,
            maximum_workers: py_config.maximum_workers,
            minimum_workers: py_config.minimum_workers,
            worker_start_commands: py_config.worker_start_commands,
            node_id: py_config.node_id,
            log_level: py_config.log_level,
        }
    }
}

impl From<PyWorkerConfig> for WorkerConfig {
    fn from(py_worker_config: PyWorkerConfig) -> Self {
        WorkerConfig {
            driver_address: py_worker_config.driver_address,
            network_mode: py_worker_config.network_mode,
        }
    }
}

impl From<PyDriverConfig> for DriverConfig {
    fn from(py_driver_config: PyDriverConfig) -> Self {
        DriverConfig {}
    }
}
