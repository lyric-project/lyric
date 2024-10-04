use crate::env::config::{EnvironmentBuilder, InnerEnvironment};
use crate::env::env::{ChildProcess, EnvStatusCode, EventStream, ExecutionEnvironment};
use async_trait::async_trait;
use bytes::Bytes;
use lyric_utils::err::EnvError;
use std::fmt::Debug;
use std::process::Stdio;
use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};

#[derive(Debug)]
pub(crate) struct LocalChildProcess {
    child: Child,
    stdout: Option<EventStream>,
    stderr: Option<EventStream>,
}

#[async_trait]
impl ChildProcess for LocalChildProcess {
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
        self.child
            .wait()
            .await
            .map(|status| status.code().unwrap_or(1))
            .map_err(|e| EnvError::LaunchEnvError(e.to_string()))
    }

    async fn try_wait(&mut self) -> Result<Option<EnvStatusCode>, EnvError> {
        self.child
            .try_wait()
            .map(|status| status.map(|s| s.code().unwrap_or(1)))
            .map_err(|e| EnvError::LaunchEnvError(e.to_string()))
    }

    async fn cleanup(&mut self) -> Result<(), EnvError> {
        self.child
            .kill()
            .await
            .map_err(|e| EnvError::LaunchEnvError(e.to_string()))?;
        self.child
            .wait()
            .await
            .map_err(|e| EnvError::LaunchEnvError(e.to_string()))?;
        Ok(())
    }
}

impl Drop for LocalChildProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

pub(crate) struct LocalEnvironment {
    inner_env: InnerEnvironment,
}

impl ExecutionEnvironment for LocalEnvironment {
    async fn execute(&mut self) -> Result<Box<dyn ChildProcess>, EnvError> {
        let mut commands = self.inner_env.args.iter().map(|s| s.as_str());
        let mut command = Command::new(
            commands
                .next()
                .ok_or(EnvError::LaunchEnvError("No command provided".into()))?,
        );
        command.args(commands);
        command.envs(self.inner_env.env.iter());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        let mut child = command
            .spawn()
            .map_err(|e| EnvError::LaunchEnvError(e.to_string()))?;

        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();

        let stdout_stream = futures::stream::unfold(stdout, |mut stdout| async move {
            let mut buffer = [0; 1024];
            match stdout.read(&mut buffer).await {
                Ok(0) => None,
                Ok(n) => Some((Ok(Bytes::copy_from_slice(&buffer[..n])), stdout)),
                Err(e) => Some((Err(EnvError::LaunchEnvError(e.to_string())), stdout)),
            }
        });

        let stderr_stream = futures::stream::unfold(stderr, |mut stderr| async move {
            let mut buffer = [0; 1024];
            tracing::info!("Reading stderr");
            match stderr.read(&mut buffer).await {
                Ok(0) => None,
                Ok(n) => Some((Ok(Bytes::copy_from_slice(&buffer[..n])), stderr)),
                Err(e) => Some((Err(EnvError::LaunchEnvError(e.to_string())), stderr)),
            }
        });
        Ok(Box::new(LocalChildProcess {
            child,
            stdout: Some(EventStream(Box::pin(stdout_stream))),
            stderr: Some(EventStream(Box::pin(stderr_stream))),
        }))
    }
}

pub(crate) struct LocalEnvironmentBuilder {
    inner_env: InnerEnvironment,
}

impl LocalEnvironmentBuilder {
    pub fn new() -> Self {
        LocalEnvironmentBuilder {
            inner_env: InnerEnvironment::new(),
        }
    }

    pub fn build(self) -> Result<LocalEnvironment, EnvError> {
        Ok(LocalEnvironment {
            inner_env: self.inner_env,
        })
    }
}

impl EnvironmentBuilder for LocalEnvironmentBuilder {
    fn inner(&mut self) -> &mut InnerEnvironment {
        &mut self.inner_env
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn test_echo_command() {
        let mut env = LocalEnvironmentBuilder::new()
            .args(&["echo", "Hello, World!"])
            .build()
            .unwrap();
        let mut child = env.execute().await.unwrap();
        let stdout = child.stdout().unwrap();

        let output = stdout.read().await.unwrap();
        #[cfg(windows)]
        assert_eq!(output, Bytes::from("Hello, World!\r\n"));
        #[cfg(not(windows))]
        assert_eq!(output, Bytes::from("Hello, World!\n"));
    }

    #[tokio::test]
    async fn test_env_variables() {
        let args = vec![
            #[cfg(windows)]
            "cmd",
            #[cfg(windows)]
            "/C",
            #[cfg(not(windows))]
            "sh",
            #[cfg(not(windows))]
            "-c",
            "echo $TEST_VAR",
        ];
        let mut env = LocalEnvironmentBuilder::new()
            .args(args)
            .env("TEST_VAR", "test value")
            .build()
            .unwrap();
        let mut child = env.execute().await.unwrap();
        let stdout = child.stdout().unwrap();

        let output = stdout.read().await.unwrap();
        #[cfg(windows)]
        assert_eq!(output, Bytes::from("test value\r\n"));
        #[cfg(not(windows))]
        assert_eq!(output, Bytes::from("test value\n"));
    }

    #[tokio::test]
    async fn test_large_output() {
        let args = vec![
            #[cfg(windows)]
            "cmd",
            #[cfg(windows)]
            "/C",
            #[cfg(not(windows))]
            "sh",
            #[cfg(not(windows))]
            "-c",
            "for i in {1..1000}; do echo $i; done",
        ];
        let mut env = LocalEnvironmentBuilder::new().args(args).build().unwrap();
        let mut child = env.execute().await.unwrap();
        let stdout = child.stdout().unwrap();

        let output = stdout.read().await.unwrap();
        #[cfg(windows)]
        let expect_str = (1..=1000).map(|i| format!("{}\r\n", i)).collect::<String>();
        #[cfg(not(windows))]
        let expect_str = (1..=1000).map(|i| format!("{}\n", i)).collect::<String>();
        assert_eq!(output, Bytes::from(expect_str));
    }

    #[tokio::test]
    async fn test_stderr_output() {
        let args = vec![
            #[cfg(windows)]
            "cmd",
            #[cfg(windows)]
            "/C",
            #[cfg(not(windows))]
            "sh",
            #[cfg(not(windows))]
            "-c",
            "echo Error >&2",
        ];
        let mut env = LocalEnvironmentBuilder::new().args(args).build().unwrap();
        let mut child = env.execute().await.unwrap();
        let stderr = child.stderr().unwrap();

        let output = stderr.read().await.unwrap();
        #[cfg(windows)]
        assert_eq!(output[0], Bytes::from("Error \r\n"));
        #[cfg(not(windows))]
        assert_eq!(output, Bytes::from("Error\n"));
    }

    #[tokio::test]
    async fn test_command_not_found() {
        let mut env = LocalEnvironmentBuilder::new()
            .args(&["non_existent_command"])
            .build()
            .unwrap();
        let result = env.execute().await;
        assert!(result.is_err());
    }
}
