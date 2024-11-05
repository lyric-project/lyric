use crate::capability::task::types as wasmtime;
use crate::capability::wrpc::lyric::task::types as wrpc;
use lyric_utils::resource as utils;

// Define macro to handle conversions between utils <-> wasmtime and utils <-> wrpc
macro_rules! implement_conversions {
    ($type_name:ident, {$($field:ident),*}) => {
        // utils -> wasmtime
        impl From<utils::$type_name> for wasmtime::$type_name {
            fn from(value: utils::$type_name) -> Self {
                Self {
                    $($field: value.$field),*
                }
            }
        }

        // wasmtime -> utils
        impl From<wasmtime::$type_name> for utils::$type_name {
            fn from(value: wasmtime::$type_name) -> Self {
                Self {
                    $($field: value.$field),*
                }
            }
        }

        // utils -> wrpc
        impl From<utils::$type_name> for wrpc::$type_name {
            fn from(value: utils::$type_name) -> Self {
                Self {
                    $($field: value.$field),*
                }
            }
        }

        // wrpc -> utils
        impl From<wrpc::$type_name> for utils::$type_name {
            fn from(value: wrpc::$type_name) -> Self {
                Self {
                    $($field: value.$field),*
                }
            }
        }
    };
}

// Implement conversions for all configuration types
implement_conversions!(CpuConfig, {
    cpu_cores,
    cpu_shares,
    cpu_quota,
    cpu_period
});

implement_conversions!(MemoryConfig, { memory_limit });

implement_conversions!(NetworkConfig, {
    enable_network,
    ingress_limit_kbps,
    egress_limit_kbps,
    allowed_hosts,
    allowed_ports
});

// Define conversions for FilePerms
macro_rules! implement_file_perms_conversions {
    () => {
        // utils <-> wasmtime
        impl From<utils::FilePerms> for wasmtime::FilePerms {
            fn from(perms: utils::FilePerms) -> Self {
                let mut result = wasmtime::FilePerms::empty();
                if perms.contains(utils::FilePerms::READ) {
                    result |= wasmtime::FilePerms::READ;
                }
                if perms.contains(utils::FilePerms::WRITE) {
                    result |= wasmtime::FilePerms::WRITE;
                }
                result
            }
        }

        impl From<wasmtime::FilePerms> for utils::FilePerms {
            fn from(perms: wasmtime::FilePerms) -> Self {
                let mut result = utils::FilePerms::empty();
                if perms.contains(wasmtime::FilePerms::READ) {
                    result |= utils::FilePerms::READ;
                }
                if perms.contains(wasmtime::FilePerms::WRITE) {
                    result |= utils::FilePerms::WRITE;
                }
                result
            }
        }

        // utils <-> wrpc
        impl From<utils::FilePerms> for wrpc::FilePerms {
            fn from(perms: utils::FilePerms) -> Self {
                let mut result = wrpc::FilePerms::empty();
                if perms.contains(utils::FilePerms::READ) {
                    result |= wrpc::FilePerms::READ;
                }
                if perms.contains(utils::FilePerms::WRITE) {
                    result |= wrpc::FilePerms::WRITE;
                }
                result
            }
        }

        impl From<wrpc::FilePerms> for utils::FilePerms {
            fn from(perms: wrpc::FilePerms) -> Self {
                let mut result = utils::FilePerms::empty();
                if perms.contains(wrpc::FilePerms::READ) {
                    result |= utils::FilePerms::READ;
                }
                if perms.contains(wrpc::FilePerms::WRITE) {
                    result |= utils::FilePerms::WRITE;
                }
                result
            }
        }
    };
}

/// Complex type conversions for FsConfig
macro_rules! implement_fs_config_conversions {
    () => {
        // utils <-> wasmtime
        impl From<utils::FsConfig> for wasmtime::FsConfig {
            fn from(config: utils::FsConfig) -> Self {
                wasmtime::FsConfig {
                    preopens: config
                        .preopens
                        .into_iter()
                        .map(|(host_path, container_path, dir_perms, file_perms)| {
                            (
                                host_path,
                                container_path,
                                dir_perms.into(),
                                file_perms.into(),
                            )
                        })
                        .collect(),
                    fs_size_limit: config.fs_size_limit,
                    temp_dir: config.temp_dir,
                }
            }
        }

        impl From<wasmtime::FsConfig> for utils::FsConfig {
            fn from(config: wasmtime::FsConfig) -> Self {
                utils::FsConfig {
                    preopens: config
                        .preopens
                        .into_iter()
                        .map(|(host_path, container_path, dir_perms, file_perms)| {
                            (
                                host_path,
                                container_path,
                                dir_perms.into(),
                                file_perms.into(),
                            )
                        })
                        .collect(),
                    fs_size_limit: config.fs_size_limit,
                    temp_dir: config.temp_dir,
                }
            }
        }

        // utils <-> wrpc
        impl From<utils::FsConfig> for wrpc::FsConfig {
            fn from(config: utils::FsConfig) -> Self {
                wrpc::FsConfig {
                    preopens: config
                        .preopens
                        .into_iter()
                        .map(|(host_path, container_path, dir_perms, file_perms)| {
                            (
                                host_path,
                                container_path,
                                dir_perms.into(),
                                file_perms.into(),
                            )
                        })
                        .collect(),
                    fs_size_limit: config.fs_size_limit,
                    temp_dir: config.temp_dir,
                }
            }
        }

        impl From<wrpc::FsConfig> for utils::FsConfig {
            fn from(config: wrpc::FsConfig) -> Self {
                utils::FsConfig {
                    preopens: config
                        .preopens
                        .into_iter()
                        .map(|(host_path, container_path, dir_perms, file_perms)| {
                            (
                                host_path,
                                container_path,
                                dir_perms.into(),
                                file_perms.into(),
                            )
                        })
                        .collect(),
                    fs_size_limit: config.fs_size_limit,
                    temp_dir: config.temp_dir,
                }
            }
        }
    };
}

implement_conversions!(InstanceLimits, {
    max_instances,
    max_tables,
    max_table_elements
});

// Implement conversions for ResourceConfig
macro_rules! implement_resource_config_conversions {
    () => {
        // utils <-> wasmtime
        impl From<utils::ResourceConfig> for wasmtime::ResourceConfig {
            fn from(config: utils::ResourceConfig) -> Self {
                wasmtime::ResourceConfig {
                    cpu: config.cpu.map(|c| c.into()),
                    memory: config.memory.map(|m| m.into()),
                    network: config.network.map(|n| n.into()),
                    fs: config.fs.map(|f| f.into()),
                    instance: config.instance.map(|i| i.into()),
                    timeout_ms: config.timeout_ms,
                    env_vars: config.env_vars,
                }
            }
        }

        impl From<wasmtime::ResourceConfig> for utils::ResourceConfig {
            fn from(config: wasmtime::ResourceConfig) -> Self {
                utils::ResourceConfig {
                    cpu: config.cpu.map(|c| c.into()),
                    memory: config.memory.map(|m| m.into()),
                    network: config.network.map(|n| n.into()),
                    fs: config.fs.map(|f| f.into()),
                    instance: config.instance.map(|i| i.into()),
                    timeout_ms: config.timeout_ms,
                    env_vars: config.env_vars,
                }
            }
        }

        // utils <-> wrpc
        impl From<utils::ResourceConfig> for wrpc::ResourceConfig {
            fn from(config: utils::ResourceConfig) -> Self {
                wrpc::ResourceConfig {
                    cpu: config.cpu.map(|c| c.into()),
                    memory: config.memory.map(|m| m.into()),
                    network: config.network.map(|n| n.into()),
                    fs: config.fs.map(|f| f.into()),
                    instance: config.instance.map(|i| i.into()),
                    timeout_ms: config.timeout_ms,
                    env_vars: config.env_vars,
                }
            }
        }

        impl From<wrpc::ResourceConfig> for utils::ResourceConfig {
            fn from(config: wrpc::ResourceConfig) -> Self {
                utils::ResourceConfig {
                    cpu: config.cpu.map(|c| c.into()),
                    memory: config.memory.map(|m| m.into()),
                    network: config.network.map(|n| n.into()),
                    fs: config.fs.map(|f| f.into()),
                    instance: config.instance.map(|i| i.into()),
                    timeout_ms: config.timeout_ms,
                    env_vars: config.env_vars,
                }
            }
        }
    };
}

// Implement conversions for all types
implement_file_perms_conversions!();
implement_fs_config_conversions!();
implement_resource_config_conversions!();
