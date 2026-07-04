//! Durable run queue: persists runs through a [`RunStore`], executes them on
//! worker tasks with per-step checkpoints, and serves live [`RunStatus`]
//! snapshots to long-polling progress UIs. After a restart, interrupted runs
//! resume from their checkpoint — completed steps are never re-executed.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Map, Value};
use tokio::sync::Notify;
use uuid::Uuid;

use crate::engine::{
    ActionDispatcher, BuiltinRegistry, EngineError, RunEvent, execute_from,
};
use crate::report::{RunLogEvent, RunReport, RunStatus, StepReport};
use crate::spec::TemplateSpec;

/// How long a finished run stays available for live polling before eviction
/// (history persists in the store regardless).
const EVICT_AFTER: Duration = Duration::from_secs(600);
/// Idle poll interval of the worker loop (a `Notify` wakes it immediately).
const WORKER_IDLE: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreStatus {
    Queued,
    Running,
    Ok,
    Failed,
}

impl StoreStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            StoreStatus::Queued => "queued",
            StoreStatus::Running => "running",
            StoreStatus::Ok => "ok",
            StoreStatus::Failed => "failed",
        }
    }
}

/// A run to enqueue.
pub struct NewRun {
    pub id: Uuid,
    pub template_name: String,
    /// Audit identifier of the caller.
    pub subject: String,
    /// Serialized authz snapshot, fed back to the dispatcher factory on
    /// (re-)claim so the run keeps executing as the original caller.
    pub principal: Value,
    pub params: Map<String, Value>,
}

/// A persisted run as loaded from the store.
pub struct StoredRun {
    pub id: Uuid,
    pub template_name: String,
    pub subject: String,
    pub principal: Value,
    pub params: Map<String, Value>,
    pub status: StoreStatus,
    /// Checkpoint: index of the next step to execute.
    pub next_step: u32,
    /// Checkpoint: variable state entering `next_step`.
    pub variables: Map<String, Value>,
    /// Checkpoint: reports of the steps executed so far.
    pub steps: Vec<StepReport>,
    pub log: Vec<RunLogEvent>,
    pub report: Option<RunReport>,
}

/// Persistence backend of the queue (e.g. a Postgres table).
#[async_trait::async_trait]
pub trait RunStore: Send + Sync {
    /// Insert with status `queued`.
    async fn enqueue(&self, run: &NewRun) -> Result<(), EngineError>;
    /// Atomically claim one queued run (oldest first) and mark it `running`.
    async fn claim(&self) -> Result<Option<StoredRun>, EngineError>;
    /// Persist the checkpoint after a completed step.
    async fn checkpoint(
        &self,
        id: Uuid,
        next_step: u32,
        variables: &Map<String, Value>,
        steps: &[StepReport],
        log: &[RunLogEvent],
    ) -> Result<(), EngineError>;
    /// Mark the run `ok`/`failed` with its final report and log.
    async fn finish(
        &self,
        id: Uuid,
        ok: bool,
        report: &RunReport,
        log: &[RunLogEvent],
    ) -> Result<(), EngineError>;
    /// Load one run (live-status fallback after eviction or restart).
    async fn load(&self, id: Uuid) -> Result<Option<StoredRun>, EngineError>;
}

/// Builds the dispatcher for one run from its persisted principal snapshot.
pub type DispatcherFactory =
    Arc<dyn Fn(&Value) -> Result<Arc<dyn ActionDispatcher>, EngineError> + Send + Sync>;
/// Resolves a template name to its spec (templates live outside the queue,
/// e.g. baked into the binary).
pub type TemplateLookup = Arc<dyn Fn(&str) -> Option<TemplateSpec> + Send + Sync>;

struct LiveRun {
    status: Mutex<RunStatus>,
    changed: Notify,
}

pub struct RunManager {
    store: Arc<dyn RunStore>,
    dispatcher_factory: DispatcherFactory,
    template_lookup: TemplateLookup,
    builtins: Arc<BuiltinRegistry>,
    live: Mutex<HashMap<Uuid, Arc<LiveRun>>>,
    work: Notify,
}

impl RunManager {
    pub fn new(
        store: Arc<dyn RunStore>,
        dispatcher_factory: DispatcherFactory,
        template_lookup: TemplateLookup,
        builtins: BuiltinRegistry,
    ) -> Arc<Self> {
        Arc::new(Self {
            store,
            dispatcher_factory,
            template_lookup,
            builtins: Arc::new(builtins),
            live: Mutex::new(HashMap::new()),
            work: Notify::new(),
        })
    }

    /// Start `concurrency` worker tasks (call once, from an async context).
    pub fn spawn_workers(self: &Arc<Self>, concurrency: usize) {
        for _ in 0..concurrency.max(1) {
            let mgr = self.clone();
            tokio::spawn(async move {
                loop {
                    match mgr.store.claim().await {
                        Ok(Some(run)) => mgr.run_one(run).await,
                        Ok(None) => {
                            tokio::select! {
                                _ = mgr.work.notified() => {}
                                _ = tokio::time::sleep(WORKER_IDLE) => {}
                            }
                        }
                        Err(e) => {
                            tracing::error!("action-template queue claim failed: {e}");
                            tokio::time::sleep(WORKER_IDLE).await;
                        }
                    }
                }
            });
        }
    }

    /// Persist and queue a new run; workers pick it up immediately.
    pub async fn enqueue(
        &self,
        template_name: &str,
        params: Map<String, Value>,
        principal: Value,
        subject: &str,
        total_steps: u32,
    ) -> Result<Uuid, EngineError> {
        let run = NewRun {
            id: Uuid::new_v4(),
            template_name: template_name.to_string(),
            subject: subject.to_string(),
            principal,
            params,
        };
        self.store.enqueue(&run).await?;
        let live = Arc::new(LiveRun {
            status: Mutex::new(RunStatus {
                run_id: run.id,
                template: run.template_name.clone(),
                current_step: 0,
                total_steps,
                current_step_name: String::new(),
                done: false,
                ok: None,
                events: Vec::new(),
                report: None,
            }),
            changed: Notify::new(),
        });
        self.live.lock().unwrap().insert(run.id, live);
        self.work.notify_waiters();
        Ok(run.id)
    }

    /// Current snapshot, waiting up to `max_wait` for events past `after_seq`
    /// (long-poll). `None` when the run id is unknown to store and memory.
    pub async fn wait_status(
        &self,
        run_id: Uuid,
        after_seq: Option<u64>,
        max_wait: Duration,
    ) -> Result<Option<RunStatus>, EngineError> {
        let deadline = tokio::time::Instant::now() + max_wait;
        loop {
            let live = self.live.lock().unwrap().get(&run_id).cloned();
            let Some(live) = live else {
                // Evicted, pre-restart, or foreign id: serve from the store.
                let stored = self.store.load(run_id).await?;
                return Ok(stored.map(|r| stored_status(&r, after_seq)));
            };

            let snapshot = live.status.lock().unwrap().clone();
            let has_news = snapshot.done
                || snapshot
                    .events
                    .iter()
                    .any(|e| after_seq.is_none_or(|s| e.seq > s));
            if has_news || tokio::time::Instant::now() >= deadline {
                return Ok(Some(trim_events(snapshot, after_seq)));
            }
            tokio::select! {
                _ = live.changed.notified() => {}
                _ = tokio::time::sleep_until(deadline) => {}
            }
        }
    }

    /// Block until the run finishes and return its report (synchronous
    /// `execute` endpoints). `max_wait` bounds queue + execution time.
    pub async fn wait_finished(
        &self,
        run_id: Uuid,
        max_wait: Duration,
    ) -> Result<RunReport, EngineError> {
        let deadline = tokio::time::Instant::now() + max_wait;
        let mut after_seq = None;
        loop {
            let remaining = deadline
                .checked_duration_since(tokio::time::Instant::now())
                .ok_or_else(|| EngineError::Other("timed out waiting for run".into()))?;
            let status = self
                .wait_status(run_id, after_seq, remaining.min(Duration::from_secs(25)))
                .await?
                .ok_or_else(|| EngineError::Other(format!("unknown run {run_id}")))?;
            if status.done {
                return status
                    .report
                    .ok_or_else(|| EngineError::Other("run finished without report".into()));
            }
            after_seq = status.events.last().map(|e| e.seq).or(after_seq);
            // Not-yet-live runs return immediately from the store fallback;
            // pace the loop so it never busy-spins.
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    async fn run_one(self: &Arc<Self>, run: StoredRun) {
        let Some(spec) = (self.template_lookup)(&run.template_name) else {
            self.fail_immediately(&run, format!("unknown template '{}'", run.template_name))
                .await;
            return;
        };
        let dispatcher = match (self.dispatcher_factory)(&run.principal) {
            Ok(d) => d,
            Err(e) => {
                self.fail_immediately(&run, format!("cannot restore principal: {e}")).await;
                return;
            }
        };

        let live = self.live_entry(&run, &spec);

        // The engine's sink is sync; forward events through a channel so
        // checkpoints (async store writes) keep step order without blocking
        // execution on the DB.
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<RunEvent>();
        let sink = move |event: RunEvent| {
            let _ = tx.send(event);
        };

        let store = self.store.clone();
        let run_id = run.id;
        let live_for_task = live.clone();
        let mut log: Vec<RunLogEvent> = run.log.clone();
        let folder = tokio::spawn(async move {
            let mut next_seq = log.last().map(|e| e.seq + 1).unwrap_or(0);
            let mut steps: Vec<StepReport> = Vec::new();
            while let Some(event) = rx.recv().await {
                match event {
                    RunEvent::StepStarted { index, name, .. } => {
                        let mut status = live_for_task.status.lock().unwrap();
                        status.current_step = index;
                        status.current_step_name = name;
                        drop(status);
                        live_for_task.changed.notify_waiters();
                    }
                    RunEvent::Log { step_index, message } => {
                        let event = RunLogEvent { seq: next_seq, step_index, message };
                        next_seq += 1;
                        log.push(event.clone());
                        live_for_task.status.lock().unwrap().events.push(event);
                        live_for_task.changed.notify_waiters();
                    }
                    RunEvent::StepFinished { index, report, variables } => {
                        steps.push(report);
                        if let Err(e) = store
                            .checkpoint(run_id, index + 1, &variables, &steps, &log)
                            .await
                        {
                            tracing::error!("run {run_id}: checkpoint failed: {e}");
                        }
                    }
                    RunEvent::RunFinished { .. } => {}
                }
            }
            log
        });

        let report = execute_from(
            &spec,
            run.next_step as usize,
            run.variables,
            run.steps,
            dispatcher.as_ref(),
            &self.builtins,
            &sink,
        )
        .await;

        drop(sink); // close the channel so the folder drains and exits
        let log = folder.await.unwrap_or_default();

        if let Err(e) = self.store.finish(run.id, report.ok, &report, &log).await {
            tracing::error!("run {}: persisting result failed: {e}", run.id);
        }

        {
            let mut status = live.status.lock().unwrap();
            status.done = true;
            status.ok = Some(report.ok);
            status.current_step = report.steps.len() as u32;
            status.report = Some(report);
        }
        live.changed.notify_waiters();
        self.evict_later(run.id);
    }

    fn live_entry(&self, run: &StoredRun, spec: &TemplateSpec) -> Arc<LiveRun> {
        let mut live = self.live.lock().unwrap();
        live.entry(run.id)
            .or_insert_with(|| {
                // Resumed after a restart: rebuild the snapshot from the checkpoint.
                Arc::new(LiveRun {
                    status: Mutex::new(RunStatus {
                        run_id: run.id,
                        template: run.template_name.clone(),
                        current_step: run.next_step,
                        total_steps: spec.actions.len() as u32,
                        current_step_name: String::new(),
                        done: false,
                        ok: None,
                        events: run.log.clone(),
                        report: None,
                    }),
                    changed: Notify::new(),
                })
            })
            .clone()
    }

    async fn fail_immediately(self: &Arc<Self>, run: &StoredRun, error: String) {
        tracing::error!("run {}: {error}", run.id);
        let report = RunReport { ok: false, steps: run.steps.clone(), variables: run.variables.clone() };
        let mut log = run.log.clone();
        log.push(RunLogEvent {
            seq: log.last().map(|e| e.seq + 1).unwrap_or(0),
            step_index: run.next_step,
            message: error,
        });
        if let Err(e) = self.store.finish(run.id, false, &report, &log).await {
            tracing::error!("run {}: persisting failure failed: {e}", run.id);
        }
        if let Some(live) = self.live.lock().unwrap().get(&run.id) {
            let mut status = live.status.lock().unwrap();
            status.done = true;
            status.ok = Some(false);
            status.events = log;
            status.report = Some(report);
            drop(status);
            live.changed.notify_waiters();
        }
        self.evict_later(run.id);
    }

    fn evict_later(self: &Arc<Self>, run_id: Uuid) {
        let mgr = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(EVICT_AFTER).await;
            mgr.live.lock().unwrap().remove(&run_id);
        });
    }
}

/// Build a status snapshot for a run that is no longer (or not yet) live.
fn stored_status(run: &StoredRun, after_seq: Option<u64>) -> RunStatus {
    let done = matches!(run.status, StoreStatus::Ok | StoreStatus::Failed);
    trim_events(
        RunStatus {
            run_id: run.id,
            template: run.template_name.clone(),
            current_step: run.next_step,
            total_steps: run.steps.len().max(run.next_step as usize) as u32,
            current_step_name: String::new(),
            done,
            ok: match run.status {
                StoreStatus::Ok => Some(true),
                StoreStatus::Failed => Some(false),
                _ => None,
            },
            events: run.log.clone(),
            report: run.report.clone(),
        },
        after_seq,
    )
}

fn trim_events(mut status: RunStatus, after_seq: Option<u64>) -> RunStatus {
    if let Some(seq) = after_seq {
        status.events.retain(|e| e.seq > seq);
    }
    status
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::parse_template;
    use serde_json::json;

    #[derive(Default)]
    struct MemRow {
        run: Option<StoredRun>,
    }

    /// In-memory RunStore mirroring the Postgres one's semantics.
    #[derive(Default)]
    struct MemStore {
        rows: Mutex<HashMap<Uuid, MemRow>>,
        order: Mutex<Vec<Uuid>>,
    }

    fn clone_run(r: &StoredRun) -> StoredRun {
        StoredRun {
            id: r.id,
            template_name: r.template_name.clone(),
            subject: r.subject.clone(),
            principal: r.principal.clone(),
            params: r.params.clone(),
            status: r.status,
            next_step: r.next_step,
            variables: r.variables.clone(),
            steps: r.steps.clone(),
            log: r.log.clone(),
            report: r.report.clone(),
        }
    }

    #[async_trait::async_trait]
    impl RunStore for MemStore {
        async fn enqueue(&self, run: &NewRun) -> Result<(), EngineError> {
            self.rows.lock().unwrap().insert(
                run.id,
                MemRow {
                    run: Some(StoredRun {
                        id: run.id,
                        template_name: run.template_name.clone(),
                        subject: run.subject.clone(),
                        principal: run.principal.clone(),
                        params: run.params.clone(),
                        status: StoreStatus::Queued,
                        next_step: 0,
                        variables: run.params.clone(),
                        steps: Vec::new(),
                        log: Vec::new(),
                        report: None,
                    }),
                },
            );
            self.order.lock().unwrap().push(run.id);
            Ok(())
        }

        async fn claim(&self) -> Result<Option<StoredRun>, EngineError> {
            let order = self.order.lock().unwrap().clone();
            let mut rows = self.rows.lock().unwrap();
            for id in order {
                if let Some(row) = rows.get_mut(&id) {
                    if let Some(run) = &mut row.run {
                        if run.status == StoreStatus::Queued {
                            run.status = StoreStatus::Running;
                            return Ok(Some(clone_run(run)));
                        }
                    }
                }
            }
            Ok(None)
        }

        async fn checkpoint(
            &self,
            id: Uuid,
            next_step: u32,
            variables: &Map<String, Value>,
            steps: &[StepReport],
            log: &[RunLogEvent],
        ) -> Result<(), EngineError> {
            let mut rows = self.rows.lock().unwrap();
            let run = rows.get_mut(&id).and_then(|r| r.run.as_mut()).unwrap();
            run.next_step = next_step;
            run.variables = variables.clone();
            run.steps = steps.to_vec();
            run.log = log.to_vec();
            Ok(())
        }

        async fn finish(
            &self,
            id: Uuid,
            ok: bool,
            report: &RunReport,
            log: &[RunLogEvent],
        ) -> Result<(), EngineError> {
            let mut rows = self.rows.lock().unwrap();
            let run = rows.get_mut(&id).and_then(|r| r.run.as_mut()).unwrap();
            run.status = if ok { StoreStatus::Ok } else { StoreStatus::Failed };
            run.report = Some(report.clone());
            run.log = log.to_vec();
            Ok(())
        }

        async fn load(&self, id: Uuid) -> Result<Option<StoredRun>, EngineError> {
            Ok(self.rows.lock().unwrap().get(&id).and_then(|r| r.run.as_ref()).map(clone_run))
        }
    }

    struct ScriptedDispatcher {
        replies: Mutex<Vec<Result<Value, String>>>,
    }

    #[async_trait::async_trait]
    impl ActionDispatcher for ScriptedDispatcher {
        async fn call(&self, _action: &str, _args: Value) -> Result<Value, EngineError> {
            let mut replies = self.replies.lock().unwrap();
            if replies.is_empty() {
                return Err(EngineError::Dispatch("no reply scripted".into()));
            }
            replies.remove(0).map_err(EngineError::Dispatch)
        }
    }

    const TEMPLATE: &str = r#"
inputs:
  who: { type: string }
actions:
  - name: one
    action: a
    inputs: { name: "{{ who }}" }
    outputs: { ".": first }
  - name: two
    action: b
    inputs: { prev: "$first" }
"#;

    fn manager(store: Arc<MemStore>, replies: Vec<Result<Value, String>>) -> Arc<RunManager> {
        let dispatcher: Arc<dyn ActionDispatcher> =
            Arc::new(ScriptedDispatcher { replies: Mutex::new(replies) });
        RunManager::new(
            store,
            Arc::new(move |_principal| Ok(dispatcher.clone())),
            Arc::new(|name| (name == "t").then(|| parse_template(TEMPLATE).unwrap())),
            BuiltinRegistry::standard(),
        )
    }

    fn params() -> Map<String, Value> {
        json!({ "who": "w" }).as_object().cloned().unwrap()
    }

    #[tokio::test]
    async fn enqueue_run_and_poll_to_completion() {
        let store = Arc::new(MemStore::default());
        let mgr = manager(store.clone(), vec![Ok(json!("r1")), Ok(json!("r2"))]);
        mgr.spawn_workers(1);

        let id = mgr.enqueue("t", params(), json!({}), "tester", 2).await.unwrap();
        let report = mgr.wait_finished(id, Duration::from_secs(5)).await.unwrap();
        assert!(report.ok);
        assert_eq!(report.steps.len(), 2);

        // Store holds the finished run + full log.
        let stored = store.load(id).await.unwrap().unwrap();
        assert_eq!(stored.status, StoreStatus::Ok);
        assert!(!stored.log.is_empty());
        assert_eq!(stored.next_step, 2, "checkpointed through the last step");

        // Live status (or store fallback) reports done with a report.
        let status = mgr.wait_status(id, None, Duration::from_millis(10)).await.unwrap().unwrap();
        assert!(status.done);
        assert_eq!(status.ok, Some(true));
    }

    #[tokio::test]
    async fn after_seq_filters_events() {
        let store = Arc::new(MemStore::default());
        let mgr = manager(store, vec![Ok(json!("r1")), Ok(json!("r2"))]);
        mgr.spawn_workers(1);

        let id = mgr.enqueue("t", params(), json!({}), "tester", 2).await.unwrap();
        mgr.wait_finished(id, Duration::from_secs(5)).await.unwrap();

        let all = mgr.wait_status(id, None, Duration::from_millis(10)).await.unwrap().unwrap();
        assert!(all.events.len() >= 2);
        let cut = all.events[all.events.len() - 2].seq;
        let tail = mgr.wait_status(id, Some(cut), Duration::from_millis(10)).await.unwrap().unwrap();
        assert!(tail.events.iter().all(|e| e.seq > cut));
        assert!(tail.events.len() < all.events.len());
    }

    #[tokio::test]
    async fn resume_from_checkpoint_skips_completed_steps() {
        let store = Arc::new(MemStore::default());

        // Simulate a run that died after step one: enqueue + hand-craft the
        // checkpoint the way run_one would have left it.
        let run = NewRun {
            id: Uuid::new_v4(),
            template_name: "t".into(),
            subject: "tester".into(),
            principal: json!({}),
            params: params(),
        };
        store.enqueue(&run).await.unwrap();
        let steps = vec![StepReport {
            name: "one".into(),
            action: "a".into(),
            status: crate::report::StepStatus::Ok,
            inputs: Some(json!({ "name": "w" })),
            output: Some(json!("r1")),
            error: None,
        }];
        let mut vars = params();
        vars.insert("first".into(), json!("r1"));
        store.checkpoint(run.id, 1, &vars, &steps, &[]).await.unwrap();

        // Only step two's reply is scripted: re-executing step one would fail.
        let mgr = manager(store.clone(), vec![Ok(json!("r2"))]);
        mgr.spawn_workers(1);

        let report = mgr.wait_finished(run.id, Duration::from_secs(5)).await.unwrap();
        assert!(report.ok, "{:?}", report.steps);
        assert_eq!(report.steps.len(), 2);
        assert_eq!(report.steps[0].output, Some(json!("r1")), "prior step kept");
    }

    #[tokio::test]
    async fn unknown_template_fails_run() {
        let store = Arc::new(MemStore::default());
        let mgr = manager(store, vec![]);
        mgr.spawn_workers(1);
        let id = mgr.enqueue("missing", Map::new(), json!({}), "tester", 0).await.unwrap();
        let err = mgr.wait_finished(id, Duration::from_secs(5)).await;
        // Run finishes as failed (report present, ok = false).
        match err {
            Ok(report) => assert!(!report.ok),
            Err(e) => panic!("expected failed report, got error {e}"),
        }
    }
}
