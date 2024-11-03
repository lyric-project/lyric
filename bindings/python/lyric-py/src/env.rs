use lyric::{DockerEnvironmentConfig, EnvironmentConfigMessage, LocalEnvironmentConfig};
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

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
    pub envs: Option<HashMap<String, String>>,
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

    #[pyo3(name = "env_id")]
    fn env_id(&self) -> String {
        self.get_env_id()
    }

    #[pyo3(signature = (custom_id=None))]
    fn clone_new(&self, custom_id: Option<String>) -> Self {
        let mut new = self.clone();
        if let Some(local) = new.local.as_mut() {
            local.custom_id = custom_id.clone();
        }
        if let Some(docker) = new.docker.as_mut() {
            docker.custom_id = custom_id;
        }
        new
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
    #[pyo3(signature = (image, custom_id=None, working_dir=None, mounts=None, envs=None))]
    fn new(
        image: String,
        custom_id: Option<String>,
        working_dir: Option<String>,
        mounts: Option<Vec<(String, String)>>,
        envs: Option<HashMap<String, String>>,
    ) -> Self {
        PyDockerEnvironmentConfig {
            image,
            custom_id,
            working_dir,
            mounts: mounts.unwrap_or_default(),
            envs,
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
            envs: config.envs,
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

impl PyEnvironmentConfig {
    pub(crate) fn get_env_id(&self) -> String {
        match (self.local.as_ref(), self.docker.as_ref()) {
            (Some(local), None) => local.env_id(),
            (None, Some(docker)) => docker.env_id(),
            _ => "default_none_env_id".to_string(),
        }
    }
}

impl PyLocalEnvironmentConfig {
    pub(crate) fn env_id(&self) -> String {
        let mut hasher = DefaultHasher::new();
        if let Some(custom_id) = &self.custom_id {
            custom_id.hash(&mut hasher);
        }
        if let Some(working_dir) = &self.working_dir {
            working_dir.hash(&mut hasher);
        }
        if let Some(envs) = &self.envs {
            for (key, val) in envs {
                key.hash(&mut hasher);
                val.hash(&mut hasher);
            }
        }
        format!("local_{:016x}", hasher.finish())
    }
}

impl PyDockerEnvironmentConfig {
    pub(crate) fn env_id(&self) -> String {
        let mut hasher = DefaultHasher::new();
        if let Some(custom_id) = &self.custom_id {
            custom_id.hash(&mut hasher);
        }
        self.image.hash(&mut hasher);
        if let Some(working_dir) = &self.working_dir {
            working_dir.hash(&mut hasher);
        }
        for (key, val) in &self.mounts {
            key.hash(&mut hasher);
            val.hash(&mut hasher);
        }
        for (key, val) in self.envs.iter().flatten() {
            key.hash(&mut hasher);
            val.hash(&mut hasher);
        }
        format!("docker_{:016x}", hasher.finish())
    }
}
