use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

mod config;
mod docker;
mod env;
mod local;
mod manager;

pub(crate) use env::ChildProcess;
pub(crate) use manager::WorkerEnvManager;

pub(crate) trait EnvironmentID {
    fn id(&self) -> String;
}

#[derive(Debug, Default, Clone)]
pub struct LocalEnvironmentConfig {
    pub custom_id: Option<String>,
    pub working_dir: Option<String>,
    pub envs: Option<HashMap<String, String>>,
}

#[derive(Debug, Default, Clone)]
pub struct DockerEnvironmentConfig {
    pub custom_id: Option<String>,
    pub image: String,
    pub working_dir: Option<String>,
    pub mounts: Vec<(String, String)>,
    pub envs: Option<HashMap<String, String>>,
}

#[derive(Debug, Hash, Clone)]
pub enum EnvironmentConfigMessage {
    Local(LocalEnvironmentConfig),
    Docker(DockerEnvironmentConfig),
}

impl EnvironmentConfigMessage {
    pub fn is_docker(&self) -> bool {
        matches!(self, EnvironmentConfigMessage::Docker(_))
    }
}

impl Default for EnvironmentConfigMessage {
    fn default() -> Self {
        EnvironmentConfigMessage::Local(LocalEnvironmentConfig::default())
    }
}

impl EnvironmentID for LocalEnvironmentConfig {
    fn id(&self) -> String {
        if self.custom_id.is_none() && self.working_dir.is_none() {
            return "local_default".to_string();
        }
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        format!("local_{:016x}", hasher.finish())
    }
}

impl Hash for LocalEnvironmentConfig {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.custom_id.hash(state);
        self.working_dir.hash(state);
        if let Some(envs) = &self.envs {
            for (key, val) in envs {
                key.hash(state);
                val.hash(state);
            }
        }
    }
}

impl EnvironmentID for DockerEnvironmentConfig {
    fn id(&self) -> String {
        if self.custom_id.is_none()
            && self.image.is_empty()
            && self.working_dir.is_none()
            && self.mounts.is_empty()
        {
            return "docker_default".to_string();
        }
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        format!("docker_{:016x}", hasher.finish())
    }
}

impl EnvironmentID for EnvironmentConfigMessage {
    fn id(&self) -> String {
        match self {
            EnvironmentConfigMessage::Local(config) => config.id(),
            EnvironmentConfigMessage::Docker(config) => config.id(),
        }
    }
}

impl Hash for DockerEnvironmentConfig {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.custom_id.hash(state);
        self.image.hash(state);
        self.working_dir.hash(state);
        self.mounts.hash(state);
        if let Some(envs) = &self.envs {
            for (key, val) in envs {
                key.hash(state);
                val.hash(state);
            }
        }
    }
}
