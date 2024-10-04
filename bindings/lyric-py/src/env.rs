use lyric::{DockerEnvironmentConfig, EnvironmentConfigMessage, LocalEnvironmentConfig};
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[pyclass]
#[derive(Clone, Serialize, Deserialize)]
pub struct PyEnvironmentConfig {
    pub local: Option<PyLocalEnvironmentConfig>,
    pub docker: Option<PyDockerEnvironmentConfig>,
    pub envs: Option<HashMap<String, String>>,
}

#[pyclass]
#[derive(Clone, Serialize, Deserialize)]
pub struct PyLocalEnvironmentConfig {
    pub custom_id: Option<String>,
    pub working_dir: Option<String>,
    pub envs: Option<HashMap<String, String>>,
}

#[pyclass]
#[derive(Clone, Serialize, Deserialize)]
pub struct PyDockerEnvironmentConfig {
    pub image: String,
    pub custom_id: Option<String>,
    pub working_dir: Option<String>,
    pub mounts: Vec<(String, String)>,
}

#[pymethods]
impl PyEnvironmentConfig {
    #[new]
    #[pyo3(signature = (local=None, docker=None, envs=None))]
    fn new(
        local: Option<PyLocalEnvironmentConfig>,
        docker: Option<PyDockerEnvironmentConfig>,
        envs: Option<HashMap<String, String>>,
    ) -> Self {
        PyEnvironmentConfig {
            local,
            docker,
            envs,
        }
    }
}

#[pymethods]
impl PyLocalEnvironmentConfig {
    #[new]
    #[pyo3(signature = (custom_id=None, working_dir=None, envs=None))]
    fn new(
        custom_id: Option<String>,
        working_dir: Option<String>,
        envs: Option<HashMap<String, String>>,
    ) -> Self {
        PyLocalEnvironmentConfig {
            custom_id,
            working_dir,
            envs,
        }
    }
}

#[pymethods]
impl PyDockerEnvironmentConfig {
    #[new]
    #[pyo3(signature = (image, custom_id=None, working_dir=None, mounts=None))]
    fn new(
        image: String,
        custom_id: Option<String>,
        working_dir: Option<String>,
        mounts: Option<Vec<(String, String)>>,
    ) -> Self {
        PyDockerEnvironmentConfig {
            image,
            custom_id,
            working_dir,
            mounts: mounts.unwrap_or_default(),
        }
    }
}

impl From<PyLocalEnvironmentConfig> for LocalEnvironmentConfig {
    fn from(config: PyLocalEnvironmentConfig) -> Self {
        LocalEnvironmentConfig {
            custom_id: config.custom_id,
            working_dir: config.working_dir,
            envs: config.envs,
        }
    }
}

impl From<PyDockerEnvironmentConfig> for DockerEnvironmentConfig {
    fn from(config: PyDockerEnvironmentConfig) -> Self {
        DockerEnvironmentConfig {
            custom_id: config.custom_id,
            image: config.image,
            working_dir: config.working_dir,
            mounts: config.mounts,
            envs: None,
        }
    }
}

impl From<PyEnvironmentConfig> for EnvironmentConfigMessage {
    fn from(config: PyEnvironmentConfig) -> Self {
        match (config.local, config.docker) {
            (Some(local), None) => EnvironmentConfigMessage::Local(local.into()),
            (None, Some(docker)) => EnvironmentConfigMessage::Docker(docker.into()),
            _ => EnvironmentConfigMessage::default(),
        }
    }
}
