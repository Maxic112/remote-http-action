//! Flow execution: concurrency, cooldown, start delay, timeouts, cancellation and history.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use crate::actions::{self, Outcome};
use crate::db::Db;
use crate::error::{AppError, AppResult, ErrorCode};
use crate::events::{AppEvent, EventSink};
use crate::logs::{LogCategory, Logger, clip};
use crate::model::*;
use crate::platform::Platform;

pub struct RunRequest {
    pub flow: Flow,
    pub source: RunSource,
    pub api_key: Option<(Id, String)>,
    pub client_ip: Option<String>,
}

/// Handle to a started run; `done` resolves with the finished run.
pub struct RunHandle {
    pub run: Run,
    pub done: watch::Receiver<Option<Run>>,
}

impl RunHandle {
    /// Waits for the run to finish. Returns `None` on timeout.
    pub async fn wait(mut self, timeout: Duration) -> Option<Run> {
        let fut = self.done.wait_for(|r| r.is_some());
        match tokio::time::timeout(timeout, fut).await {
            Ok(Ok(r)) => r.clone(),
            _ => None,
        }
    }
}

struct ActiveRun {
    flow_id: Id,
    cancel: CancellationToken,
    cancelable: bool,
}

#[derive(Default)]
struct State {
    active: HashMap<String, ActiveRun>,
    running: HashMap<Id, usize>,
    last_start: HashMap<Id, Instant>,
    queues: HashMap<Id, Arc<tokio::sync::Mutex<()>>>,
}

pub struct Runner {
    db: Arc<Db>,
    platform: Arc<dyn Platform>,
    events: Arc<dyn EventSink>,
    logs: Arc<Logger>,
    history_limit: Mutex<u32>,
    state: Mutex<State>,
}

impl Runner {
    pub fn new(
        db: Arc<Db>,
        platform: Arc<dyn Platform>,
        events: Arc<dyn EventSink>,
        logs: Arc<Logger>,
        history_limit: u32,
    ) -> Self {
        Self {
            db,
            platform,
            events,
            logs,
            history_limit: Mutex::new(history_limit),
            state: Mutex::default(),
        }
    }

    pub fn set_history_limit(&self, limit: u32) {
        *self.history_limit.lock().unwrap() = limit;
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Validates and starts a run in the background.
    pub fn start(self: &Arc<Self>, req: RunRequest) -> AppResult<RunHandle> {
        let flow = req.flow;
        if !flow.input.enabled {
            return Err(AppError::new(
                ErrorCode::Forbidden,
                format!("flow \"{}\" is disabled", flow.input.path),
            ));
        }
        let parsed: Vec<_> = flow
            .input
            .actions
            .iter()
            .map(actions::parse)
            .collect::<AppResult<_>>()?;

        let queue = {
            let mut st = self.state();
            let now = Instant::now();
            if flow.input.cooldown_sec > 0
                && let Some(last) = st.last_start.get(&flow.id)
            {
                let cd = Duration::from_secs(flow.input.cooldown_sec);
                let elapsed = now.duration_since(*last);
                if elapsed < cd {
                    let left = (cd - elapsed).as_secs().max(1);
                    return Err(AppError::new(
                        ErrorCode::TooManyRequests,
                        format!("flow is cooling down, retry in {left}s"),
                    ));
                }
            }
            let running = st.running.get(&flow.id).copied().unwrap_or(0);
            if flow.input.concurrency == Concurrency::RejectIfRunning && running > 0 {
                return Err(AppError::conflict("flow is already running"));
            }
            st.last_start.insert(flow.id, now);
            *st.running.entry(flow.id).or_default() += 1;
            (flow.input.concurrency == Concurrency::Queue)
                .then(|| st.queues.entry(flow.id).or_default().clone())
        };

        let (api_key_id, api_key_name) = req
            .api_key
            .map(|(i, n)| (Some(i), Some(n)))
            .unwrap_or((None, None));
        let run = Run {
            id: uuid::Uuid::new_v4().to_string(),
            flow_id: Some(flow.id),
            flow_name: flow.input.name.clone(),
            flow_path: flow.input.path.clone(),
            source: req.source,
            api_key_id,
            api_key_name,
            client_ip: req.client_ip,
            status: RunStatus::Pending,
            created_at: now(),
            started_at: None,
            finished_at: None,
            error: None,
            steps: Vec::new(),
        };
        if let Err(e) = self.db.save_run(&run) {
            self.release(flow.id);
            return Err(e);
        }

        let cancel = CancellationToken::new();
        self.state().active.insert(
            run.id.clone(),
            ActiveRun {
                flow_id: flow.id,
                cancel: cancel.clone(),
                cancelable: flow.input.cancelable,
            },
        );
        let (tx, rx) = watch::channel(None);
        let me = self.clone();
        let initial = run.clone();
        tokio::spawn(async move {
            let _guard = match &queue {
                Some(q) => Some(q.clone().lock_owned().await),
                None => None,
            };
            let finished = me.execute(run, &flow, parsed, cancel).await;
            me.log_finished(&finished, &flow);
            me.state().active.remove(&finished.id);
            me.release(flow.id);
            let _ = me.db.prune_runs(*me.history_limit.lock().unwrap());
            let _ = tx.send(Some(finished));
        });
        self.logs.write(
            LogCategory::Flow,
            serde_json::json!({
                "event": "started",
                "run_id": initial.id,
                "flow": initial.flow_path,
                "source": initial.source,
                "key": initial.api_key_name,
                "ip": initial.client_ip,
            }),
        );
        self.events.emit(AppEvent::RunUpdated {
            run: Box::new(initial.clone()),
        });
        Ok(RunHandle {
            run: initial,
            done: rx,
        })
    }

    /// Flow log entry with every step: parameters, timing, result and (clipped) output.
    fn log_finished(&self, run: &Run, flow: &Flow) {
        let limit = self.logs.output_limit();
        let steps: Vec<serde_json::Value> = run
            .steps
            .iter()
            .map(|st| {
                let def = flow.input.actions.get(st.index);
                serde_json::json!({
                    "index": st.index,
                    "type": st.action_type,
                    "params": def.map(|d| d.params.clone()),
                    "run_as_admin": def.is_some_and(|d| d.run_as_admin),
                    "status": st.status,
                    "started_at": st.started_at,
                    "finished_at": st.finished_at,
                    "exit_code": st.exit_code,
                    "error": st.error,
                    "stdout": st.stdout.as_deref().map(|o| clip(o, limit)),
                    "stderr": st.stderr.as_deref().map(|o| clip(o, limit)),
                })
            })
            .collect();
        self.logs.write(
            LogCategory::Flow,
            serde_json::json!({
                "event": "finished",
                "run_id": run.id,
                "flow": run.flow_path,
                "source": run.source,
                "key": run.api_key_name,
                "ip": run.client_ip,
                "status": run.status,
                "error": run.error,
                "started_at": run.started_at,
                "finished_at": run.finished_at,
                "steps": steps,
            }),
        );
    }

    fn release(&self, flow_id: Id) {
        let mut st = self.state();
        if let Some(n) = st.running.get_mut(&flow_id) {
            *n = n.saturating_sub(1);
        }
    }

    pub fn cancel(&self, run_id: &str, force: bool) -> AppResult<()> {
        let st = self.state();
        let active = st
            .active
            .get(run_id)
            .ok_or_else(|| AppError::not_found(format!("run {run_id} is not active")))?;
        if !active.cancelable && !force {
            return Err(AppError::new(
                ErrorCode::Forbidden,
                "this flow cannot be canceled",
            ));
        }
        active.cancel.cancel();
        Ok(())
    }

    pub fn active_runs(&self, flow_id: Option<Id>) -> Vec<String> {
        self.state()
            .active
            .iter()
            .filter(|(_, a)| flow_id.is_none_or(|f| a.flow_id == f))
            .map(|(id, _)| id.clone())
            .collect()
    }

    fn update(&self, run: &Run) {
        let _ = self.db.save_run(run);
        self.events.emit(AppEvent::RunUpdated {
            run: Box::new(run.clone()),
        });
    }

    async fn execute(
        &self,
        mut run: Run,
        flow: &Flow,
        parsed: Vec<actions::ActionParams>,
        cancel: CancellationToken,
    ) -> Run {
        let finish = |mut run: Run, status: RunStatus, error: Option<String>| {
            run.status = status;
            run.error = error;
            run.finished_at = Some(now());
            run
        };

        if flow.input.start_delay_sec > 0 {
            run.status = RunStatus::Delayed;
            self.update(&run);
            self.events.emit(AppEvent::RunDelayed {
                run_id: run.id.clone(),
                flow_name: flow.input.name.clone(),
                seconds: flow.input.start_delay_sec,
                cancelable: flow.input.cancelable,
            });
            let canceled = tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(flow.input.start_delay_sec)) => false,
                _ = cancel.cancelled() => true,
            };
            if canceled {
                let run = finish(
                    run,
                    RunStatus::Canceled,
                    Some("canceled during start delay".into()),
                );
                self.update(&run);
                return run;
            }
        } else if cancel.is_cancelled() {
            let run = finish(run, RunStatus::Canceled, Some("canceled".into()));
            self.update(&run);
            return run;
        }

        run.status = RunStatus::Running;
        run.started_at = Some(now());
        self.update(&run);

        let steps: Arc<Mutex<Vec<StepResult>>> = Arc::default();
        let step_cancel = cancel.child_token();
        let body = self.run_steps(flow, parsed, steps.clone(), step_cancel.clone());
        let outcome =
            tokio::time::timeout(Duration::from_secs(flow.input.timeout_sec.max(1)), body).await;
        if outcome.is_err() {
            step_cancel.cancel();
        }
        run.steps = steps.lock().unwrap().clone();
        let run = match outcome {
            Err(_) => finish(
                run,
                RunStatus::Timeout,
                Some(format!("flow timed out after {}s", flow.input.timeout_sec)),
            ),
            Ok(_) if cancel.is_cancelled() => {
                finish(run, RunStatus::Canceled, Some("canceled".into()))
            }
            Ok(Some(err)) => finish(run, RunStatus::Failed, Some(err)),
            Ok(None) => finish(run, RunStatus::Success, None),
        };
        self.update(&run);
        run
    }

    /// Runs steps in order. Returns the error that stopped the flow, if any.
    async fn run_steps(
        &self,
        flow: &Flow,
        parsed: Vec<actions::ActionParams>,
        steps: Arc<Mutex<Vec<StepResult>>>,
        cancel: CancellationToken,
    ) -> Option<String> {
        let mut first_error = None;
        for (index, (def, params)) in flow.input.actions.iter().zip(parsed).enumerate() {
            if cancel.is_cancelled() {
                return Some("canceled".into());
            }
            let started_at = now();
            let outcome = if def.run_as_admin && !self.platform.is_elevated() {
                Outcome::failed("ADMIN_REQUIRED: the application is not running as administrator")
            } else {
                let fut = actions::execute(params, self.platform.as_ref(), cancel.clone());
                // Dropping the timed-out future kills the child process (`kill_on_drop`).
                match def.timeout_sec {
                    Some(t) => match tokio::time::timeout(Duration::from_secs(t), fut).await {
                        Ok(o) => o,
                        Err(_) => Outcome {
                            status: StepStatus::Timeout,
                            error: Some(format!("step timed out after {t}s")),
                            ..Outcome::failed("")
                        },
                    },
                    None => fut.await,
                }
            };
            let failed = matches!(outcome.status, StepStatus::Failed | StepStatus::Timeout);
            let canceled = outcome.status == StepStatus::Canceled;
            let error = outcome.error.clone();
            steps.lock().unwrap().push(StepResult {
                index,
                action_type: def.action_type.clone(),
                status: outcome.status,
                started_at,
                finished_at: now(),
                exit_code: outcome.exit_code,
                stdout: outcome.stdout,
                stderr: outcome.stderr,
                error: outcome.error,
                capture_output: def.capture_output,
            });
            if canceled {
                return Some("canceled".into());
            }
            if failed {
                let msg = format!(
                    "step {} ({}) failed: {}",
                    index + 1,
                    def.action_type,
                    error.unwrap_or_default()
                );
                if def.on_error == OnError::Stop {
                    return Some(msg);
                }
                first_error.get_or_insert(msg);
            }
        }
        first_error
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::events::NoopSink;
    use crate::platform::mock::{MockPlatform, MockResponse};

    fn action(t: &str, params: serde_json::Value) -> ActionDef {
        ActionDef {
            action_type: t.into(),
            params,
            on_error: OnError::Stop,
            timeout_sec: None,
            run_as_admin: false,
            capture_output: false,
        }
    }

    fn cmd(command: &str) -> ActionDef {
        action("cli_command", json!({"shell": "cmd", "command": command}))
    }

    fn setup(platform: MockPlatform, input: FlowInput) -> (Arc<Runner>, Arc<MockPlatform>, Flow) {
        let db = Arc::new(Db::open_in_memory().unwrap());
        let id = db.insert_flow(&input).unwrap();
        let flow = db.get_flow(id).unwrap();
        let platform = Arc::new(platform);
        let dir = tempfile::tempdir().unwrap().keep();
        let logs = Arc::new(Logger::new(dir, Default::default()));
        let runner = Arc::new(Runner::new(
            db,
            platform.clone(),
            Arc::new(NoopSink),
            logs,
            100,
        ));
        (runner, platform, flow)
    }

    fn input(actions: Vec<ActionDef>) -> FlowInput {
        FlowInput {
            name: "t".into(),
            path: "t".into(),
            actions,
            ..Default::default()
        }
    }

    fn req(flow: &Flow) -> RunRequest {
        RunRequest {
            flow: flow.clone(),
            source: RunSource::Cli,
            api_key: None,
            client_ip: None,
        }
    }

    async fn run_to_end(runner: &Arc<Runner>, flow: &Flow) -> Run {
        runner
            .start(req(flow))
            .unwrap()
            .wait(Duration::from_secs(10))
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn runs_steps_in_order_and_stores_history() {
        let (runner, platform, flow) = setup(
            MockPlatform::with_responses(vec![MockResponse::ok("one"), MockResponse::ok("two")]),
            input(vec![
                cmd("a"),
                action("delay", json!({"seconds": 0})),
                cmd("b"),
                action("sleep", json!({})),
            ]),
        );
        let run = run_to_end(&runner, &flow).await;
        assert_eq!(run.status, RunStatus::Success);
        assert_eq!(run.steps.len(), 4);
        assert_eq!(run.steps[0].stdout.as_deref(), Some("one"));
        assert_eq!(run.steps[2].stdout.as_deref(), Some("two"));
        assert_eq!(platform.processes.lock().unwrap().len(), 2);
        assert_eq!(platform.power_calls.lock().unwrap().len(), 1);
        let stored = runner.db.get_run(&run.id).unwrap();
        assert_eq!(stored.status, RunStatus::Success);
        assert_eq!(stored.steps.len(), 4);
    }

    #[tokio::test]
    async fn runs_are_written_to_the_flow_log() {
        let (runner, _, flow) = setup(
            MockPlatform::with_responses(vec![MockResponse::ok("hello")]),
            input(vec![cmd("echo hello")]),
        );
        run_to_end(&runner, &flow).await;
        let entries = runner.logs.read(LogCategory::Flow, 10, None).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["event"], "finished");
        assert_eq!(entries[0]["status"], "success");
        assert_eq!(entries[0]["steps"][0]["params"]["command"], "echo hello");
        assert_eq!(entries[0]["steps"][0]["stdout"], "hello");
        assert_eq!(entries[1]["event"], "started");
    }

    #[tokio::test]
    async fn stop_on_error_skips_remaining_steps() {
        let (runner, platform, flow) = setup(
            MockPlatform::with_responses(vec![MockResponse::exit(1)]),
            input(vec![cmd("bad"), cmd("never")]),
        );
        let run = run_to_end(&runner, &flow).await;
        assert_eq!(run.status, RunStatus::Failed);
        assert_eq!(run.steps.len(), 1);
        assert!(run.error.unwrap().contains("exit code 1"));
        assert_eq!(platform.processes.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn continue_on_error_runs_everything_but_reports_failure() {
        let mut bad = cmd("bad");
        bad.on_error = OnError::Continue;
        let (runner, _, flow) = setup(
            MockPlatform::with_responses(vec![MockResponse::exit(2), MockResponse::ok("")]),
            input(vec![bad, cmd("ok")]),
        );
        let run = run_to_end(&runner, &flow).await;
        assert_eq!(run.status, RunStatus::Failed);
        assert_eq!(run.steps.len(), 2);
        assert_eq!(run.steps[1].status, StepStatus::Success);
    }

    #[tokio::test]
    async fn admin_steps_fail_without_elevation() {
        let mut a = action("shutdown", json!({}));
        a.run_as_admin = true;
        let (runner, platform, flow) = setup(MockPlatform::default(), input(vec![a.clone()]));
        let run = run_to_end(&runner, &flow).await;
        assert_eq!(run.status, RunStatus::Failed);
        assert!(
            run.steps[0]
                .error
                .as_ref()
                .unwrap()
                .contains("ADMIN_REQUIRED")
        );
        assert!(platform.power_calls.lock().unwrap().is_empty());

        let elevated = MockPlatform {
            elevated: true,
            ..Default::default()
        };
        let (runner, platform, flow) = setup(elevated, input(vec![a]));
        assert_eq!(run_to_end(&runner, &flow).await.status, RunStatus::Success);
        assert_eq!(platform.power_calls.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn flow_timeout_kills_running_step() {
        let mut i = input(vec![cmd("slow")]);
        i.timeout_sec = 1;
        let (runner, _, flow) = setup(
            MockPlatform::with_responses(vec![MockResponse::slow(Duration::from_secs(30))]),
            i,
        );
        let run = run_to_end(&runner, &flow).await;
        assert_eq!(run.status, RunStatus::Timeout);
    }

    #[tokio::test]
    async fn step_timeout_fails_step() {
        let mut slow = cmd("slow");
        slow.timeout_sec = Some(1);
        let (runner, _, flow) = setup(
            MockPlatform::with_responses(vec![MockResponse::slow(Duration::from_secs(30))]),
            input(vec![slow]),
        );
        let run = run_to_end(&runner, &flow).await;
        assert_eq!(run.status, RunStatus::Failed);
        assert_eq!(run.steps[0].status, StepStatus::Timeout);
    }

    #[tokio::test]
    async fn reject_if_running_and_cooldown() {
        let (runner, _, flow) = setup(
            MockPlatform::with_responses(vec![MockResponse::slow(Duration::from_millis(300))]),
            input(vec![cmd("slow")]),
        );
        let h = runner.start(req(&flow)).unwrap();
        assert_eq!(
            runner.start(req(&flow)).err().unwrap().code,
            ErrorCode::Conflict
        );
        h.wait(Duration::from_secs(5)).await.unwrap();
        assert!(runner.start(req(&flow)).is_ok());

        let mut i = input(vec![action("delay", json!({"seconds": 0}))]);
        i.cooldown_sec = 60;
        i.concurrency = Concurrency::Allow;
        let (runner, _, flow) = setup(MockPlatform::default(), i);
        runner.start(req(&flow)).unwrap();
        assert_eq!(
            runner.start(req(&flow)).err().unwrap().code,
            ErrorCode::TooManyRequests
        );
    }

    #[tokio::test]
    async fn queue_runs_sequentially() {
        let mut i = input(vec![cmd("slow")]);
        i.concurrency = Concurrency::Queue;
        let (runner, _, flow) = setup(
            MockPlatform::with_responses(vec![
                MockResponse::slow(Duration::from_millis(200)),
                MockResponse::slow(Duration::from_millis(200)),
            ]),
            i,
        );
        let a = runner.start(req(&flow)).unwrap();
        let b = runner.start(req(&flow)).unwrap();
        let ra = a.wait(Duration::from_secs(5)).await.unwrap();
        let rb = b.wait(Duration::from_secs(5)).await.unwrap();
        assert!(rb.started_at.unwrap() >= ra.finished_at.unwrap());
    }

    #[tokio::test]
    async fn start_delay_can_be_canceled() {
        let mut i = input(vec![action("shutdown", json!({}))]);
        i.start_delay_sec = 30;
        let (runner, platform, flow) = setup(MockPlatform::default(), i);
        let h = runner.start(req(&flow)).unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(
            runner.db.get_run(&h.run.id).unwrap().status,
            RunStatus::Delayed
        );
        runner.cancel(&h.run.id, false).unwrap();
        let run = h.wait(Duration::from_secs(5)).await.unwrap();
        assert_eq!(run.status, RunStatus::Canceled);
        assert!(platform.power_calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn non_cancelable_flow_rejects_cancel() {
        let mut i = input(vec![action("delay", json!({"seconds": 5}))]);
        i.cancelable = false;
        let (runner, _, flow) = setup(MockPlatform::default(), i);
        let h = runner.start(req(&flow)).unwrap();
        assert_eq!(
            runner.cancel(&h.run.id, false).unwrap_err().code,
            ErrorCode::Forbidden
        );
        runner.cancel(&h.run.id, true).unwrap();
        assert_eq!(
            h.wait(Duration::from_secs(5)).await.unwrap().status,
            RunStatus::Canceled
        );
    }

    #[tokio::test]
    async fn disabled_flow_is_rejected() {
        let mut i = input(vec![]);
        i.enabled = false;
        let (runner, _, flow) = setup(MockPlatform::default(), i);
        assert_eq!(
            runner.start(req(&flow)).err().unwrap().code,
            ErrorCode::Forbidden
        );
    }
}
