use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub(crate) struct InnerEnvironment {
    pub(crate) env: HashMap<String, String>,
    pub(crate) args: Vec<String>,
    pub(crate) working_dir: Option<PathBuf>,
    pub(crate) timeout: Option<Duration>,
}

pub(crate) trait EnvironmentBuilder: Sized {
    fn env<K, V>(mut self, key: K, val: V) -> Self
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.inner().env(key, val);
        self
    }

    fn envs<I, K, V>(mut self, vars: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.inner().envs(vars);
        self
    }

    fn arg<S: AsRef<OsStr>>(mut self, arg: S) -> Self {
        self.inner().arg(arg);
        self
    }

    fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.inner().args(args);
        self
    }

    fn working_dir<P: AsRef<Path>>(mut self, dir: P) -> Self {
        self.inner().working_dir(dir);
        self
    }

    fn timeout(mut self, duration: Duration) -> Self {
        self.inner().timeout(duration);
        self
    }

    fn inner(&mut self) -> &mut InnerEnvironment;
}

impl InnerEnvironment {
    pub fn new() -> Self {
        Self {
            env: HashMap::new(),
            args: Vec::new(),
            working_dir: None,
            timeout: None,
        }
    }
    pub fn env<K, V>(&mut self, key: K, val: V) -> &mut Self
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.env.insert(
            key.as_ref().to_string_lossy().into_owned(),
            val.as_ref().to_string_lossy().into_owned(),
        );
        self
    }

    pub fn envs<I, K, V>(&mut self, vars: I) -> &mut Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        for (key, val) in vars {
            self.env(key, val);
        }
        self
    }

    pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Self {
        self.args.push(arg.as_ref().to_string_lossy().into_owned());
        self
    }

    pub fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        for arg in args {
            self.arg(arg);
        }
        self
    }
    pub fn working_dir<P: AsRef<Path>>(&mut self, dir: P) -> &mut Self {
        self.working_dir = Some(dir.as_ref().to_path_buf());
        self
    }

    pub fn timeout(&mut self, duration: Duration) -> &mut Self {
        self.timeout = Some(duration);
        self
    }
}
