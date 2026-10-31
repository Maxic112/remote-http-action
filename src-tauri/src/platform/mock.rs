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
    pub launched: Mutex<Vec<(ProcessSpec, LaunchWait)>>,
    /// Successive answers of `process_count`; the last one repeats.
    pub process_counts: Mutex<VecDeque<usize>>,
    pub autostart: Mutex<bool>,
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

    async fn launch(
        &self,
        spec: ProcessSpec,
        wait: LaunchWait,
        _cancel: CancellationToken,
    ) -> AppResult<Option<i32>> {
        self.launched.lock().unwrap().push((spec, wait));
        Ok((wait != LaunchWait::None).then_some(0))
    }

    fn process_count(&self, _name: &str) -> usize {
        let mut counts = self.process_counts.lock().unwrap();
        if counts.len() > 1 {
            counts.pop_front().unwrap()
        } else {
            counts.front().copied().unwrap_or(0)
        }
    }

    fn open_folder(&self, _path: &std::path::Path) -> AppResult<()> {
        Ok(())
    }

    fn autostart_status(&self) -> AppResult<AutostartStatus> {
        let installed = *self.autostart.lock().unwrap();
        Ok(AutostartStatus {
            supported: true,
            installed,
            command: installed.then(|| "app.exe".into()),
            matches_current: installed,
        })
    }

    fn autostart_install(&self) -> AppResult<()> {
        if !self.elevated {
            return Err(AppError::new(
                crate::error::ErrorCode::AdminRequired,
                "administrator rights required",
            ));
        }
        *self.autostart.lock().unwrap() = true;
        Ok(())
    }

    fn autostart_remove(&self) -> AppResult<()> {
        *self.autostart.lock().unwrap() = false;
        Ok(())
    }
}
