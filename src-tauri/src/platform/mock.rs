//! Test double that records calls and returns scripted results.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use super::*;
use crate::error::AppError;

#[derive(Debug, Clone)]
pub struct MockResponse {
    pub output: ProcessOutput,
    pub delay: Duration,
}

impl MockResponse {
    pub fn ok(stdout: &str) -> Self {
        Self {
            output: ProcessOutput {
                exit_code: Some(0),
                stdout: stdout.into(),
                stderr: String::new(),
            },
            delay: Duration::ZERO,
        }
    }
    pub fn exit(code: i32) -> Self {
        Self {
            output: ProcessOutput {
                exit_code: Some(code),
                stdout: String::new(),
                stderr: "boom".into(),
            },
            delay: Duration::ZERO,
        }
    }
    pub fn slow(delay: Duration) -> Self {
        Self {
            delay,
            ..Self::ok("")
        }
    }
}

#[derive(Default)]
pub struct MockPlatform {
    pub elevated: bool,
    pub power_calls: Mutex<Vec<PowerAction>>,
    pub processes: Mutex<Vec<ProcessSpec>>,
    pub responses: Mutex<VecDeque<MockResponse>>,
}

impl MockPlatform {
    pub fn with_responses(responses: Vec<MockResponse>) -> Self {
        Self {
            responses: Mutex::new(responses.into()),
            ..Default::default()
        }
    }
}

#[async_trait]
impl Platform for MockPlatform {
    fn is_elevated(&self) -> bool {
        self.elevated
    }

    async fn power(&self, action: PowerAction) -> AppResult<()> {
        self.power_calls.lock().unwrap().push(action);
        Ok(())
    }

    async fn run_process(
        &self,
        spec: ProcessSpec,
        cancel: CancellationToken,
    ) -> AppResult<ProcessOutput> {
        self.processes.lock().unwrap().push(spec);
        let resp = self
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| MockResponse::ok(""));
        tokio::select! {
            _ = tokio::time::sleep(resp.delay) => Ok(resp.output),
            _ = cancel.cancelled() => Err(AppError::internal("process killed")),
        }
    }

    fn shells(&self) -> Vec<ShellInfo> {
        vec![ShellInfo {
            id: "cmd".into(),
            name: "Command Prompt".into(),
            path: "cmd.exe".into(),
        }]
    }

    fn relaunch_elevated(&self) -> AppResult<()> {
        Ok(())
    }
}
