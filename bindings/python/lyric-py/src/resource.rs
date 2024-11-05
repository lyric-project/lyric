use lyric_utils::resource::*;
use lyric_wasm_runtime::capability::rpc_task;
use lyric_wasm_runtime::resource::*;
use pyo3::prelude::*;

#[pyclass]
#[derive(Clone)]
pub struct PyTaskCpuConfig {
    inner: CpuConfig,
}

#[pyclass]
#[derive(Clone)]
pub struct PyTaskMemoryConfig {
    inner: MemoryConfig,
}

#[pyclass]
#[derive(Clone)]
pub struct PyTaskNetworkConfig {
    inner: NetworkConfig,
}

#[pyclass]
#[derive(Clone)]
pub struct PyTaskFilePerms {
    inner: FilePerms,
}

#[pyclass]
#[derive(Clone)]
pub struct PyTaskFsConfig {
    inner: FsConfig,
}

#[pyclass]
#[derive(Clone)]
pub struct PyTaskInstanceLimits {
    inner: InstanceLimits,
}

#[pyclass]
#[derive(Clone)]
pub struct PyTaskResourceConfig {
    inner: ResourceConfig,
}

#[pymethods]
impl PyTaskCpuConfig {
    #[new]
    #[pyo3(signature = (cpu_cores=None, cpu_shares=None, cpu_quota=None, cpu_period=None))]
    fn new(
        cpu_cores: Option<f32>,
        cpu_shares: Option<u32>,
        cpu_quota: Option<u32>,
        cpu_period: Option<u32>,
    ) -> Self {
        PyTaskCpuConfig {
            inner: CpuConfig {
                cpu_cores,
                cpu_shares,
                cpu_quota,
                cpu_period,
            },
        }
    }
}

#[pymethods]
impl PyTaskMemoryConfig {
    #[new]
    #[pyo3(signature = (memory_limit=None))]
    fn new(memory_limit: Option<u64>) -> Self {
        PyTaskMemoryConfig {
            inner: MemoryConfig { memory_limit },
        }
    }
}

#[pymethods]
impl PyTaskNetworkConfig {
    #[new]
    #[pyo3(signature = (enable_network=None, ingress_limit_kbps=None, egress_limit_kbps=None, allowed_hosts=None, allowed_ports=None))]
    fn new(
        enable_network: Option<bool>,
        ingress_limit_kbps: Option<u32>,
        egress_limit_kbps: Option<u32>,
        allowed_hosts: Option<Vec<String>>,
        allowed_ports: Option<Vec<(u16, u16)>>,
    ) -> Self {
        PyTaskNetworkConfig {
            inner: NetworkConfig {
                enable_network: enable_network.unwrap_or_default(),
                ingress_limit_kbps,
                egress_limit_kbps,
                allowed_hosts,
                allowed_ports,
            },
        }
    }
}

#[pymethods]
impl PyTaskFilePerms {
    #[new]
    #[pyo3(signature = (inner=None))]
    fn new(inner: Option<usize>) -> Self {
        PyTaskFilePerms {
            inner: FilePerms::from_bits_truncate(inner.unwrap_or_default()),
        }
    }
}

#[pymethods]
impl PyTaskFsConfig {
    #[new]
    #[pyo3(signature = (preopens=None, fs_size_limit=None, temp_dir=None))]
    fn new(
        preopens: Option<Vec<(String, String, usize, usize)>>,
        fs_size_limit: Option<u64>,
        temp_dir: Option<String>,
    ) -> Self {
        let mut r_preopens = Vec::new();
        for (host_path, container_path, dir_perms, file_perms) in preopens.unwrap_or_default() {
            let mut r_dir_perms = FilePerms::empty();
            let mut r_file_perms = FilePerms::empty();
            if dir_perms & 0b1 != 0 {
                r_dir_perms |= FilePerms::READ;
            }
            if dir_perms & 0b10 != 0 {
                r_dir_perms |= FilePerms::WRITE;
            }

            if file_perms & 0b1 != 0 {
                r_file_perms |= FilePerms::READ;
            }
            if file_perms & 0b10 != 0 {
                r_file_perms |= FilePerms::WRITE;
            }
            r_preopens.push((host_path, container_path, r_dir_perms, r_file_perms));
        }
        PyTaskFsConfig {
            inner: FsConfig {
                preopens: r_preopens,
                fs_size_limit,
                temp_dir,
            },
        }
    }
}

#[pymethods]
impl PyTaskInstanceLimits {
    #[new]
    #[pyo3(signature = (max_instances=None, max_tables=None, max_table_elements=None))]
    fn new(
        max_instances: Option<u32>,
        max_tables: Option<u32>,
        max_table_elements: Option<u32>,
    ) -> Self {
        PyTaskInstanceLimits {
            inner: InstanceLimits {
                max_instances,
                max_tables,
                max_table_elements,
            },
        }
    }
}

#[pymethods]
impl PyTaskResourceConfig {
    #[new]
    #[pyo3(signature = (cpu=None, memory=None, network=None, fs=None, instance_limits=None, timeout_ms=None, env_vars=None))]
    fn new(
        cpu: Option<PyTaskCpuConfig>,
        memory: Option<PyTaskMemoryConfig>,
        network: Option<PyTaskNetworkConfig>,
        fs: Option<PyTaskFsConfig>,
        instance_limits: Option<PyTaskInstanceLimits>,
        timeout_ms: Option<u32>,
        env_vars: Option<Vec<(String, String)>>,
    ) -> Self {
        let env_vars = env_vars.unwrap_or_default();
        PyTaskResourceConfig {
            inner: ResourceConfig {
                cpu: cpu.map(|c| c.inner),
                memory: memory.map(|m| m.inner),
                network: network.map(|n| n.inner),
                fs: fs.map(|f| f.inner),
                instance: instance_limits.map(|i| i.inner),
                timeout_ms,
                env_vars,
            },
        }
    }
}

impl PyTaskResourceConfig {
    pub fn into_rpc(self) -> rpc_task::types::ResourceConfig {
        self.inner.into()
    }
}
