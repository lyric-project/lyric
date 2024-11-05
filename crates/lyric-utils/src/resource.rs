/// Cpu configuration
///
#[derive(Debug, Default, Clone)]
pub struct CpuConfig {
    /// CPU core limit (e.g. 1.0 represents a full core, 0.5 represents half a core)
    pub cpu_cores: Option<f32>,
    pub cpu_shares: Option<u32>,
    /// CPU period configuration (microseconds, at most use quota microseconds within period)
    pub cpu_quota: Option<u32>,
    /// CPU period time (microseconds, default 100000, i.e. 100ms)
    pub cpu_period: Option<u32>,
}

#[derive(Debug, Default, Clone)]
pub struct MemoryConfig {
    /// Memory limit in bytes
    pub memory_limit: Option<u64>,
}

#[derive(Debug, Default, Clone)]
pub struct NetworkConfig {
    /// Whether to enable network access
    pub enable_network: bool,
    /// Inbound bandwidth limit (KB/s)
    pub ingress_limit_kbps: Option<u32>,
    /// Outbound bandwidth limit (KB/s)
    pub egress_limit_kbps: Option<u32>,
    /// Allowed host list
    pub allowed_hosts: Option<Vec<String>>,
    /// Allowed port range list (start_port, end_port)
    pub allowed_ports: Option<Vec<(u16, u16)>>,
}

bitflags::bitflags! {
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub struct FilePerms: usize {
        const READ = 0b1;
        const WRITE = 0b10;
    }
}

#[derive(Debug, Default, Clone)]
pub struct FsConfig {
    /// Pre-mapped directory list (host path, container path, directory permissions, file permissions)
    pub preopens: Vec<(String, String, FilePerms, FilePerms)>,
    /// File system size limit in bytes
    pub fs_size_limit: Option<u64>,
    /// Temporary directory for wasi
    pub temp_dir: Option<String>,
}

/// Instance limits
#[derive(Debug, Default, Clone)]
pub struct InstanceLimits {
    // Max number of instances
    pub max_instances: Option<u32>,
    /// Max number of tables
    pub max_tables: Option<u32>,
    /// Max number of elements in each table
    pub max_table_elements: Option<u32>,
}

/// Full resource configuration
#[derive(Debug, Default, Clone)]
pub struct ResourceConfig {
    /// CPU configuration
    pub cpu: Option<CpuConfig>,
    /// Memory configuration
    pub memory: Option<MemoryConfig>,
    /// Network configuration
    pub network: Option<NetworkConfig>,
    /// File system configuration
    pub fs: Option<FsConfig>,
    // Instance limits
    pub instance: Option<InstanceLimits>,
    /// Task timeout in milliseconds
    pub timeout_ms: Option<u32>,
    /// List of environment variables
    pub env_vars: Vec<(String, String)>,
}
