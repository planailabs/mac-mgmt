//! Template execution: value resolution ($refs + jinja), conditions, loops,
//! retries, builtin functions, and dispatch through [`ActionDispatcher`].

use std::collections::HashMap;
use std::sync::Arc;

use minijinja::{Environment, UndefinedBehavior};
use serde_json::{Map, Value, json};

use crate::report::{RunReport, StepReport, StepStatus};
use crate::spec::{Condition, InputType, StepSpec, TemplateSpec};

/// Upper bound on steps per template (checked at validation and execution).
pub const MAX_STEPS: usize = 100;
/// Upper bound on loop iterations per step.
pub const MAX_LOOP_ITEMS: usize = 1000;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("template parse error: {0}")]
    Parse(String),
    #[error("invalid template: {0}")]
    Invalid(String),
    #[error("unknown variable path '{0}'")]
    UnknownVariable(String),
    #[error("render error: {0}")]
    Render(String),
    #[error("dispatch error: {0}")]
    Dispatch(String),
    #[error("store error: {0}")]
    Store(String),
    #[error("{0}")]
    Other(String),
}

/// Dispatches a non-builtin action (a registry tool name) with resolved args.
#[async_trait::async_trait]
pub trait ActionDispatcher: Send + Sync {
    async fn call(&self, action: &str, args: Value) -> Result<Value, EngineError>;
}

/// A `_builtin` step function: resolved inputs in, JSON result out.
pub type BuiltinFn = Arc<dyn Fn(&Map<String, Value>) -> Result<Value, EngineError> + Send + Sync>;

/// Builtin (`_`-prefixed) step functions. [`BuiltinRegistry::standard`] ships
/// `_filter`; consumers add extra functions with [`BuiltinRegistry::register_function`].
#[derive(Clone, Default)]
pub struct BuiltinRegistry {
    fns: HashMap<String, BuiltinFn>,
}

impl BuiltinRegistry {
    pub fn standard() -> Self {
        let mut reg = Self::default();
        reg.register_function("_filter", Arc::new(builtin_filter));
        reg
    }

    pub fn register_function(&mut self, name: impl Into<String>, f: BuiltinFn) {
        self.fns.insert(name.into(), f);
    }

    pub fn get(&self, name: &str) -> Option<&BuiltinFn> {
        self.fns.get(name)
    }

    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.fns.keys().cloned().collect();
        names.sort();
        names
    }
}

/// `_filter`: `{ list, contains, field? }` -> `{ found: bool, matches: [...] }`.
/// Matches items where `item[field]` (or the item itself) deep-equals `contains`.
fn builtin_filter(inputs: &Map<String, Value>) -> Result<Value, EngineError> {
    let list = inputs
        .get("list")
        .and_then(Value::as_array)
        .ok_or_else(|| EngineError::Invalid("_filter: 'list' must be an array".into()))?;
    let needle = inputs
        .get("contains")
        .ok_or_else(|| EngineError::Invalid("_filter: 'contains' is required".into()))?;
    let field = inputs.get("field").and_then(Value::as_str);

    let matches: Vec<Value> = list
        .iter()
        .filter(|item| match field {
            Some(path) => lookup_value_path(item, path).is_ok_and(|v| &v == needle),
            None => *item == needle,
        })
        .cloned()
        .collect();
    Ok(json!({ "found": !matches.is_empty(), "matches": matches }))
}

/// Progress events emitted by [`execute_from`] as the run advances.
#[derive(Debug, Clone)]
pub enum RunEvent {
    StepStarted {
        index: u32,
        name: String,
        action: String,
    },
    Log {
        step_index: u32,
        message: String,
    },
    /// Checkpoint hook: `variables` is the full state after the step.
    StepFinished {
        index: u32,
        report: StepReport,
        variables: Map<String, Value>,
    },
    RunFinished {
        report: RunReport,
    },
}

pub type EventSink<'a> = &'a (dyn Fn(RunEvent) + Send + Sync);

/// No-op sink for callers that don't track progress.
pub fn no_events(_: RunEvent) {}

// ── Parsing & validation ─────────────────────────────────────────────

pub fn parse_template(yaml: &str) -> Result<TemplateSpec, EngineError> {
    yaml_serde::from_str(yaml).map_err(|e| EngineError::Parse(e.to_string()))
}

/// Names a step's variables may not shadow.
const RESERVED_VARS: &[&str] = &["item", "item_index"];

fn valid_var_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Structural validation. `known_tools` is checked when given (build scripts
/// don't have the runtime registry, servers do). Returns all problems at once.
pub fn validate_template(
    spec: &TemplateSpec,
    known_tools: Option<&[String]>,
    builtins: &BuiltinRegistry,
) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if spec.actions.is_empty() {
        errors.push("template has no actions".into());
    }
    if spec.actions.len() > MAX_STEPS {
        errors.push(format!("template has {} steps (max {MAX_STEPS})", spec.actions.len()));
    }

    for (name, input) in &spec.inputs {
        if !valid_var_name(name) || RESERVED_VARS.contains(&name.as_str()) {
            errors.push(format!("input '{name}': not a usable variable name"));
        }
        match input.ty {
            InputType::List if input.options.as_ref().is_none_or(|o| o.is_empty()) => {
                errors.push(format!("input '{name}': list type requires non-empty 'options'"));
            }
            InputType::Id if input.reference.is_none() => {
                errors.push(format!("input '{name}': id type requires 'ref'"));
            }
            _ => {}
        }
    }

    for (i, step) in spec.actions.iter().enumerate() {
        let at = format!("step {} ('{}')", i + 1, step.name);
        if step.name.trim().is_empty() {
            errors.push(format!("step {}: empty name", i + 1));
        }
        if step.action.starts_with('_') {
            if builtins.get(&step.action).is_none() {
                errors.push(format!("{at}: unknown builtin '{}'", step.action));
            }
        } else if let Some(tools) = known_tools {
            if !tools.iter().any(|t| t == &step.action) {
                errors.push(format!("{at}: unknown action '{}'", step.action));
            }
        }
        for var in step.outputs.values() {
            if !valid_var_name(var) || RESERVED_VARS.contains(&var.as_str()) {
                errors.push(format!("{at}: output variable '{var}' is not a usable name"));
            }
        }
    }

    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

/// Check the caller-supplied params against the declared inputs, applying
/// defaults and light coercion. Returns the initial variables map.
pub fn validate_params(
    spec: &TemplateSpec,
    params: Map<String, Value>,
) -> Result<Map<String, Value>, EngineError> {
    for key in params.keys() {
        if !spec.inputs.contains_key(key) {
            return Err(EngineError::Invalid(format!("unknown parameter '{key}'")));
        }
    }

    let mut vars = Map::new();
    for (name, input) in &spec.inputs {
        let value = params.get(name).cloned().or_else(|| input.default.clone());
        let Some(value) = value else {
            if input.required {
                return Err(EngineError::Invalid(format!("missing required parameter '{name}'")));
            }
            continue;
        };
        let coerced = match input.ty {
            InputType::String => match value {
                Value::String(_) => value,
                Value::Number(n) => Value::String(n.to_string()),
                _ => return Err(EngineError::Invalid(format!("parameter '{name}' must be a string"))),
            },
            InputType::Int => match &value {
                Value::Number(n) if n.is_i64() || n.is_u64() => value,
                Value::String(s) => s
                    .parse::<i64>()
                    .map(|n| json!(n))
                    .map_err(|_| EngineError::Invalid(format!("parameter '{name}' must be an integer")))?,
                _ => return Err(EngineError::Invalid(format!("parameter '{name}' must be an integer"))),
            },
            InputType::Bool => match value {
                Value::Bool(_) => value,
                _ => return Err(EngineError::Invalid(format!("parameter '{name}' must be a boolean"))),
            },
            InputType::Id => match &value {
                Value::String(s) if !s.is_empty() => value,
                _ => return Err(EngineError::Invalid(format!("parameter '{name}' must be a non-empty id"))),
            },
            InputType::List => match &value {
                Value::String(s)
                    if input.options.as_ref().is_some_and(|opts| opts.iter().any(|o| o == s)) =>
                {
                    value
                }
                _ => {
                    return Err(EngineError::Invalid(format!(
                        "parameter '{name}' must be one of {:?}",
                        input.options.as_deref().unwrap_or_default()
                    )));
                }
            },
        };
        vars.insert(name.clone(), coerced);
    }
    Ok(vars)
}

// ── Value resolution ─────────────────────────────────────────────────

/// Walk a dotted path (`results.0.id`) into a JSON value.
fn lookup_value_path(root: &Value, path: &str) -> Result<Value, EngineError> {
    let mut cur = root;
    if path != "." {
        for seg in path.split('.') {
            cur = match cur {
                Value::Object(m) => m
                    .get(seg)
                    .ok_or_else(|| EngineError::UnknownVariable(path.to_string()))?,
                Value::Array(a) => seg
                    .parse::<usize>()
                    .ok()
                    .and_then(|i| a.get(i))
                    .ok_or_else(|| EngineError::UnknownVariable(path.to_string()))?,
                _ => return Err(EngineError::UnknownVariable(path.to_string())),
            };
        }
    }
    Ok(cur.clone())
}

fn lookup_scope_path(scope: &Map<String, Value>, path: &str) -> Result<Value, EngineError> {
    let (first, rest) = match path.split_once('.') {
        Some((f, r)) => (f, Some(r)),
        None => (path, None),
    };
    let root = scope
        .get(first)
        .ok_or_else(|| EngineError::UnknownVariable(path.to_string()))?;
    match rest {
        Some(rest) => lookup_value_path(root, rest),
        None => Ok(root.clone()),
    }
}

/// Resolve one spec value against the current variables:
/// `"$$..."` -> literal `$...`; `"$path"` -> typed variable lookup; strings
/// containing jinja markers -> rendered string; arrays/objects -> recursive.
fn resolve_value(
    value: &Value,
    scope: &Map<String, Value>,
    env: &Environment,
) -> Result<Value, EngineError> {
    match value {
        Value::String(s) => {
            if let Some(rest) = s.strip_prefix("$$") {
                Ok(Value::String(format!("${rest}")))
            } else if let Some(path) = s.strip_prefix('$') {
                lookup_scope_path(scope, path)
            } else if s.contains("{{") || s.contains("{%") {
                let ctx = minijinja::Value::from_serialize(scope);
                env.render_str(s, ctx)
                    .map(Value::String)
                    .map_err(|e| EngineError::Render(e.to_string()))
            } else {
                Ok(value.clone())
            }
        }
        Value::Array(items) => items
            .iter()
            .map(|v| resolve_value(v, scope, env))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        Value::Object(map) => map
            .iter()
            .map(|(k, v)| Ok((k.clone(), resolve_value(v, scope, env)?)))
            .collect::<Result<Map<_, _>, EngineError>>()
            .map(Value::Object),
        _ => Ok(value.clone()),
    }
}

pub fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// Evaluate a step condition (all present keys AND'ed). Returns a short
/// human-readable explanation alongside the verdict for the debug log.
fn eval_condition(
    cond: &Condition,
    scope: &Map<String, Value>,
    env: &Environment,
) -> Result<(bool, String), EngineError> {
    let mut verdict = true;
    let mut parts = Vec::new();

    if let Some(v) = &cond.is {
        let resolved = resolve_value(v, scope, env)?;
        let ok = truthy(&resolved);
        parts.push(format!("is({resolved}) = {ok}"));
        verdict &= ok;
    }
    if let Some(v) = &cond.is_not {
        let resolved = resolve_value(v, scope, env)?;
        let ok = !truthy(&resolved);
        parts.push(format!("is_not({resolved}) = {ok}"));
        verdict &= ok;
    }
    if let Some([l, r]) = &cond.equals {
        let (l, r) = (resolve_value(l, scope, env)?, resolve_value(r, scope, env)?);
        let ok = l == r;
        parts.push(format!("equals({l}, {r}) = {ok}"));
        verdict &= ok;
    }
    if let Some([l, r]) = &cond.not_equals {
        let (l, r) = (resolve_value(l, scope, env)?, resolve_value(r, scope, env)?);
        let ok = l != r;
        parts.push(format!("not_equals({l}, {r}) = {ok}"));
        verdict &= ok;
    }

    Ok((verdict, parts.join(" && ")))
}

// ── Execution ────────────────────────────────────────────────────────

/// Run a template from `start_step` with the given variable state — the
/// resume form used by the durable queue. Fresh runs use [`execute`].
pub async fn execute_from(
    spec: &TemplateSpec,
    start_step: usize,
    mut variables: Map<String, Value>,
    prior_steps: Vec<StepReport>,
    dispatcher: &dyn ActionDispatcher,
    builtins: &BuiltinRegistry,
    on_event: EventSink<'_>,
) -> RunReport {
    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Strict);

    let mut steps = prior_steps;

    for (index, step) in spec.actions.iter().enumerate().skip(start_step).take(MAX_STEPS) {
        let idx = index as u32;
        on_event(RunEvent::StepStarted {
            index: idx,
            name: step.name.clone(),
            action: step.action.clone(),
        });
        let log = |message: String| on_event(RunEvent::Log { step_index: idx, message });

        let report = run_step(step, &mut variables, &env, dispatcher, builtins, &log).await;
        let failed = report.status == StepStatus::Failed;

        steps.push(report.clone());
        on_event(RunEvent::StepFinished {
            index: idx,
            report,
            variables: variables.clone(),
        });

        if failed {
            let report = RunReport { ok: false, steps, variables };
            on_event(RunEvent::RunFinished { report: report.clone() });
            return report;
        }
    }

    let report = RunReport { ok: true, steps, variables };
    on_event(RunEvent::RunFinished { report: report.clone() });
    report
}

/// Convenience wrapper: run from the start with `params` as initial variables.
pub async fn execute(
    spec: &TemplateSpec,
    params: Map<String, Value>,
    dispatcher: &dyn ActionDispatcher,
    builtins: &BuiltinRegistry,
    on_event: EventSink<'_>,
) -> RunReport {
    execute_from(spec, 0, params, Vec::new(), dispatcher, builtins, on_event).await
}

/// Execute one step (all its iterations/retries) and fold the outcome into
/// `variables`. Never panics; failures come back as `StepStatus::Failed`.
async fn run_step(
    step: &StepSpec,
    variables: &mut Map<String, Value>,
    env: &Environment<'_>,
    dispatcher: &dyn ActionDispatcher,
    builtins: &BuiltinRegistry,
    log: &(dyn Fn(String) + Sync),
) -> StepReport {
    let base = |status: StepStatus| StepReport {
        name: step.name.clone(),
        action: step.action.clone(),
        status,
        inputs: None,
        output: None,
        error: None,
    };
    let fail = |report: StepReport, error: EngineError| {
        log(format!("failed: {error}"));
        StepReport { status: StepStatus::Failed, error: Some(error.to_string()), ..report }
    };

    // Resolve the loop items (a single implicit iteration when absent).
    let items: Option<Vec<Value>> = match &step.loop_items {
        None => None,
        Some(spec_value) => match resolve_value(spec_value, variables, env) {
            Ok(Value::Array(items)) if items.len() <= MAX_LOOP_ITEMS => Some(items),
            Ok(Value::Array(items)) => {
                return fail(
                    base(StepStatus::Ok),
                    EngineError::Invalid(format!("loop has {} items (max {MAX_LOOP_ITEMS})", items.len())),
                );
            }
            Ok(other) => {
                return fail(
                    base(StepStatus::Ok),
                    EngineError::Invalid(format!("loop value is not an array: {other}")),
                );
            }
            Err(e) => return fail(base(StepStatus::Ok), e),
        },
    };

    let mut iter_inputs: Vec<Value> = Vec::new();
    let mut iter_outputs: Vec<Value> = Vec::new();
    let mut any_ran = false;

    let count = items.as_ref().map_or(1, Vec::len);
    for i in 0..count {
        // Iteration scope: variables plus item/item_index for loops.
        let mut scope = variables.clone();
        if let Some(items) = &items {
            scope.insert("item".into(), items[i].clone());
            scope.insert("item_index".into(), json!(i));
            log(format!("iteration {}/{}", i + 1, count));
        }

        if let Some(cond) = &step.condition {
            match eval_condition(cond, &scope, env) {
                Ok((true, detail)) => log(format!("condition holds: {detail}")),
                Ok((false, detail)) => {
                    log(format!("condition does not hold: {detail} -> skipped"));
                    if items.is_some() {
                        iter_inputs.push(Value::Null);
                        iter_outputs.push(Value::Null);
                        continue;
                    }
                    return base(StepStatus::Skipped);
                }
                Err(e) => return fail(base(StepStatus::Ok), e),
            }
        }

        let resolved = match resolve_value(&Value::Object(step.inputs.clone()), &scope, env) {
            Ok(Value::Object(map)) => map,
            Ok(_) => unreachable!("resolving an object yields an object"),
            Err(e) => return fail(base(StepStatus::Ok), e),
        };
        log(format!("inputs: {}", Value::Object(resolved.clone())));

        // Dispatch with retries.
        let attempts = step.retries + 1;
        let mut output = None;
        for attempt in 1..=attempts {
            let result = if step.action.starts_with('_') {
                builtins
                    .get(&step.action)
                    .ok_or_else(|| EngineError::Invalid(format!("unknown builtin '{}'", step.action)))
                    .and_then(|f| f(&resolved))
            } else {
                dispatcher.call(&step.action, Value::Object(resolved.clone())).await
            };
            match result {
                Ok(value) => {
                    output = Some(value);
                    break;
                }
                Err(e) if attempt < attempts => {
                    log(format!("attempt {attempt}/{attempts} failed: {e}; retrying in {}s", step.delay));
                    tokio::time::sleep(std::time::Duration::from_secs(step.delay)).await;
                }
                Err(e) => {
                    log(format!("attempt {attempt}/{attempts} failed: {e}"));
                    let inputs = collect_iter(items.is_some(), iter_inputs, Value::Object(resolved));
                    return StepReport {
                        inputs: Some(inputs),
                        error: Some(e.to_string()),
                        ..base(StepStatus::Failed)
                    };
                }
            }
        }
        let output = output.expect("loop above either sets output or returns");
        log(format!("output: {output}"));

        any_ran = true;
        iter_inputs.push(Value::Object(resolved));
        iter_outputs.push(output);
    }

    // Map outputs into variables. For loops, collect per-iteration values.
    for (path, var) in &step.outputs {
        let value = if items.is_some() {
            let collected: Result<Vec<Value>, EngineError> = iter_outputs
                .iter()
                .map(|out| match out {
                    Value::Null => Ok(Value::Null), // skipped iteration
                    out => lookup_value_path(out, path),
                })
                .collect();
            match collected {
                Ok(values) => Value::Array(values),
                Err(e) => {
                    return StepReport {
                        inputs: Some(Value::Array(iter_inputs)),
                        output: Some(Value::Array(iter_outputs)),
                        error: Some(e.to_string()),
                        ..base(StepStatus::Failed)
                    };
                }
            }
        } else {
            match iter_outputs.first().map(|out| lookup_value_path(out, path)) {
                Some(Ok(value)) => value,
                Some(Err(e)) => {
                    return StepReport {
                        inputs: iter_inputs.into_iter().next(),
                        output: iter_outputs.into_iter().next(),
                        error: Some(e.to_string()),
                        ..base(StepStatus::Failed)
                    };
                }
                None => unreachable!("non-loop steps always run exactly one iteration"),
            }
        };
        log(format!("set {var}"));
        variables.insert(var.clone(), value);
    }

    let status = if any_ran { StepStatus::Ok } else { StepStatus::Skipped };
    StepReport {
        inputs: Some(collect_all(items.is_some(), iter_inputs)),
        output: Some(collect_all(items.is_some(), iter_outputs)),
        ..base(status)
    }
}

fn collect_iter(looped: bool, mut prior: Vec<Value>, current: Value) -> Value {
    if looped {
        prior.push(current);
        Value::Array(prior)
    } else {
        current
    }
}

fn collect_all(looped: bool, values: Vec<Value>) -> Value {
    if looped {
        Value::Array(values)
    } else {
        values.into_iter().next().unwrap_or(Value::Null)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::StepStatus;
    use std::sync::Mutex;

    /// Dispatcher that records calls and replies from a canned list.
    struct MockDispatcher {
        calls: Mutex<Vec<(String, Value)>>,
        replies: Mutex<Vec<Result<Value, String>>>,
    }

    impl MockDispatcher {
        fn new(replies: Vec<Result<Value, String>>) -> Self {
            Self { calls: Mutex::new(Vec::new()), replies: Mutex::new(replies) }
        }
    }

    #[async_trait::async_trait]
    impl ActionDispatcher for MockDispatcher {
        async fn call(&self, action: &str, args: Value) -> Result<Value, EngineError> {
            self.calls.lock().unwrap().push((action.to_string(), args));
            let mut replies = self.replies.lock().unwrap();
            if replies.is_empty() {
                return Err(EngineError::Dispatch("no reply scripted".into()));
            }
            replies.remove(0).map_err(EngineError::Dispatch)
        }
    }

    fn spec(yaml: &str) -> TemplateSpec {
        parse_template(yaml).expect("test template parses")
    }

    async fn run(
        yaml: &str,
        params: Value,
        dispatcher: &MockDispatcher,
    ) -> (RunReport, Vec<RunEvent>) {
        let events = Mutex::new(Vec::new());
        let sink = |e: RunEvent| events.lock().unwrap().push(e);
        let template = spec(yaml);
        let vars = validate_params(&template, params.as_object().cloned().unwrap_or_default())
            .expect("params validate");
        let report =
            execute(&template, vars, dispatcher, &BuiltinRegistry::standard(), &sink).await;
        (report, events.into_inner().unwrap())
    }

    const CREATE_IF_MISSING: &str = r#"
description: create unless present
inputs:
  bla: { type: string }
actions:
  - name: list
    action: webspace_list
    outputs: { ".": results }
  - name: check
    action: _filter
    inputs: { list: "$results", field: name, contains: "{{ bla }}" }
    outputs: { found: should_skip }
  - name: create
    action: webspace_create
    if: { is_not: "$should_skip" }
    inputs: { name: "{{ bla }}" }
    outputs: { ".": new_id }
"#;

    #[tokio::test]
    async fn creates_when_missing() {
        let dispatcher = MockDispatcher::new(vec![
            Ok(json!([{ "name": "other", "id": "1" }])),
            Ok(json!("new-uuid")),
        ]);
        let (report, _) = run(CREATE_IF_MISSING, json!({ "bla": "mine" }), &dispatcher).await;
        assert!(report.ok);
        assert_eq!(
            report.steps.iter().map(|s| s.status).collect::<Vec<_>>(),
            vec![StepStatus::Ok, StepStatus::Ok, StepStatus::Ok]
        );
        assert_eq!(report.variables["new_id"], json!("new-uuid"));
        let calls = dispatcher.calls.lock().unwrap();
        assert_eq!(calls[1].0, "webspace_create");
        assert_eq!(calls[1].1, json!({ "name": "mine" }));
    }

    #[tokio::test]
    async fn skips_create_when_present() {
        let dispatcher =
            MockDispatcher::new(vec![Ok(json!([{ "name": "mine", "id": "1" }]))]);
        let (report, _) = run(CREATE_IF_MISSING, json!({ "bla": "mine" }), &dispatcher).await;
        assert!(report.ok);
        assert_eq!(report.steps[2].status, StepStatus::Skipped);
        assert_eq!(dispatcher.calls.lock().unwrap().len(), 1, "create not dispatched");
        assert_eq!(report.variables["should_skip"], json!(true));
    }

    #[tokio::test]
    async fn dollar_refs_keep_types_and_walk_paths() {
        let yaml = r#"
inputs: {}
actions:
  - name: fetch
    action: fetch
    outputs: { ".": data }
  - name: use
    action: consume
    inputs:
      first_id: "$data.rows.0.id"
      count: "$data.count"
      literal: "$$data"
"#;
        let dispatcher = MockDispatcher::new(vec![
            Ok(json!({ "rows": [{ "id": 7 }], "count": 2 })),
            Ok(json!(null)),
        ]);
        let (report, _) = run(yaml, json!({}), &dispatcher).await;
        assert!(report.ok, "{:?}", report.steps);
        let calls = dispatcher.calls.lock().unwrap();
        assert_eq!(calls[1].1, json!({ "first_id": 7, "count": 2, "literal": "$data" }));
    }

    #[tokio::test]
    async fn unknown_variable_fails_step() {
        let yaml = r#"
inputs: {}
actions:
  - name: boom
    action: consume
    inputs: { x: "$nope" }
"#;
        let dispatcher = MockDispatcher::new(vec![]);
        let (report, _) = run(yaml, json!({}), &dispatcher).await;
        assert!(!report.ok);
        assert_eq!(report.steps[0].status, StepStatus::Failed);
        assert!(report.steps[0].error.as_deref().unwrap_or("").contains("nope"));
        assert!(dispatcher.calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn loop_collects_outputs_and_sets_item() {
        let yaml = r#"
inputs: {}
actions:
  - name: each
    action: touch
    loop: [a, b, c]
    inputs: { name: "{{ item }}-{{ item_index }}" }
    outputs: { ".": touched }
"#;
        let dispatcher = MockDispatcher::new(vec![
            Ok(json!("ra")),
            Ok(json!("rb")),
            Ok(json!("rc")),
        ]);
        let (report, _) = run(yaml, json!({}), &dispatcher).await;
        assert!(report.ok);
        assert_eq!(report.variables["touched"], json!(["ra", "rb", "rc"]));
        let calls = dispatcher.calls.lock().unwrap();
        assert_eq!(calls[0].1, json!({ "name": "a-0" }));
        assert_eq!(calls[2].1, json!({ "name": "c-2" }));
    }

    #[tokio::test]
    async fn loop_condition_skips_single_iterations() {
        let yaml = r#"
inputs: {}
actions:
  - name: each
    action: touch
    loop: [keep, drop, keep]
    if: { not_equals: ["$item", "drop"] }
    inputs: { name: "$item" }
    outputs: { ".": touched }
"#;
        let dispatcher = MockDispatcher::new(vec![Ok(json!(1)), Ok(json!(2))]);
        let (report, _) = run(yaml, json!({}), &dispatcher).await;
        assert!(report.ok);
        assert_eq!(report.variables["touched"], json!([1, null, 2]));
        assert_eq!(dispatcher.calls.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn retries_then_succeeds() {
        let yaml = r#"
inputs: {}
actions:
  - name: flaky
    action: flaky
    retries: 2
    delay: 0
    outputs: { ".": out }
"#;
        let dispatcher = MockDispatcher::new(vec![
            Err("transient".into()),
            Err("transient".into()),
            Ok(json!("worked")),
        ]);
        let (report, events) = run(yaml, json!({}), &dispatcher).await;
        assert!(report.ok);
        assert_eq!(report.variables["out"], json!("worked"));
        assert_eq!(dispatcher.calls.lock().unwrap().len(), 3);
        let logs: Vec<String> = events
            .iter()
            .filter_map(|e| match e {
                RunEvent::Log { message, .. } => Some(message.clone()),
                _ => None,
            })
            .collect();
        assert!(logs.iter().any(|m| m.contains("attempt 1/3 failed")), "{logs:?}");
    }

    #[tokio::test]
    async fn retries_exhausted_fails_run() {
        let yaml = r#"
inputs: {}
actions:
  - name: flaky
    action: flaky
    retries: 1
    delay: 0
  - name: never
    action: never
"#;
        let dispatcher =
            MockDispatcher::new(vec![Err("down".into()), Err("still down".into())]);
        let (report, _) = run(yaml, json!({}), &dispatcher).await;
        assert!(!report.ok);
        assert_eq!(report.steps.len(), 1, "run stops at the failed step");
        assert_eq!(report.steps[0].status, StepStatus::Failed);
        assert!(report.steps[0].error.as_deref().unwrap().contains("still down"));
    }

    #[test]
    fn validate_catches_problems() {
        let template = spec(
            r#"
inputs:
  pick: { type: list }
  target: { type: id }
actions:
  - name: a
    action: _nope
  - name: b
    action: unknown_tool
    outputs: { ".": "9bad" }
"#,
        );
        let errors = validate_template(
            &template,
            Some(&["known_tool".to_string()]),
            &BuiltinRegistry::standard(),
        )
        .unwrap_err();
        let joined = errors.join("\n");
        assert!(joined.contains("list type requires non-empty 'options'"));
        assert!(joined.contains("id type requires 'ref'"));
        assert!(joined.contains("unknown builtin '_nope'"));
        assert!(joined.contains("unknown action 'unknown_tool'"));
        assert!(joined.contains("output variable '9bad'"));
    }

    #[test]
    fn params_validate_and_coerce() {
        let template = spec(
            r#"
inputs:
  name: { type: string }
  count: { type: int }
  flag: { type: bool, required: false, default: false }
  mode: { type: list, options: [fast, slow] }
actions:
  - name: x
    action: _filter
    inputs: { list: [], contains: 1 }
"#,
        );
        let vars = validate_params(
            &template,
            json!({ "name": "n", "count": "42", "mode": "fast" }).as_object().cloned().unwrap(),
        )
        .unwrap();
        assert_eq!(vars["count"], json!(42));
        assert_eq!(vars["flag"], json!(false));

        let err = validate_params(
            &template,
            json!({ "name": "n", "count": 1, "mode": "warp" }).as_object().cloned().unwrap(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("mode"));

        let err = validate_params(
            &template,
            json!({ "name": "n", "count": 1, "mode": "fast", "extra": 1 })
                .as_object()
                .cloned()
                .unwrap(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("extra"));
    }

    #[test]
    fn filter_builtin_matches_by_field() {
        let out = builtin_filter(
            json!({
                "list": [{ "name": "a" }, { "name": "b" }],
                "field": "name",
                "contains": "b"
            })
            .as_object()
            .unwrap(),
        )
        .unwrap();
        assert_eq!(out, json!({ "found": true, "matches": [{ "name": "b" }] }));

        let out = builtin_filter(
            json!({ "list": ["x"], "contains": "y" }).as_object().unwrap(),
        )
        .unwrap();
        assert_eq!(out["found"], json!(false));
    }
}
