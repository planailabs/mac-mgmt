//! Template definition types, deserialized from YAML (see the crate docs for
//! the format). Wasm-safe: the UI renders parameter forms from these.

use std::collections::BTreeMap;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// One action template: typed inputs plus an ordered list of steps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TemplateSpec {
    /// Display name; servers usually fill this from the filename stem.
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// Template parameters, in declaration order (drives the UI form).
    #[serde(default)]
    pub inputs: IndexMap<String, InputSpec>,
    pub actions: Vec<StepSpec>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InputType {
    String,
    Int,
    Bool,
    /// An entity id; `ref` names the resource whose list endpoint feeds the picker.
    Id,
    /// One of a fixed set of `options`.
    List,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InputSpec {
    #[serde(rename = "type")]
    pub ty: InputType,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub default: Option<serde_json::Value>,
    #[serde(default = "default_true")]
    pub required: bool,
    /// `list` type: the allowed values.
    #[serde(default)]
    pub options: Option<Vec<String>>,
    /// `id` type: resource (plural REST segment, e.g. "webspaces") whose list
    /// endpoint populates the picker.
    #[serde(default, rename = "ref")]
    pub reference: Option<String>,
}

/// One step: dispatch `action` (a registry tool name, or a `_builtin`) with
/// rendered `inputs`, optionally per item of `loop`, mapping `outputs` into
/// variables for later steps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StepSpec {
    pub name: String,
    pub action: String,
    #[serde(default)]
    pub inputs: serde_json::Map<String, serde_json::Value>,
    /// result-field path (`"."` = whole result) -> variable name.
    #[serde(default)]
    pub outputs: BTreeMap<String, String>,
    /// Skip the step (or the current loop iteration) unless this holds.
    #[serde(default, rename = "if")]
    pub condition: Option<Condition>,
    /// Resolves to an array; the step runs once per element with `item` /
    /// `item_index` in scope and `outputs` collected into arrays.
    #[serde(default, rename = "loop", alias = "with_items")]
    pub loop_items: Option<serde_json::Value>,
    /// Extra dispatch attempts after a failure (0 = single attempt).
    #[serde(default)]
    pub retries: u32,
    /// Seconds between retry attempts.
    #[serde(default = "default_delay")]
    pub delay: u64,
}

/// All present keys are AND'ed. Operands go through normal value resolution
/// (`$var` refs, jinja), so `is: "$found"` or `equals: ["{{ a }}", "b"]` work.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Condition {
    /// Truthy check.
    #[serde(default)]
    pub is: Option<serde_json::Value>,
    /// Falsy check.
    #[serde(default)]
    pub is_not: Option<serde_json::Value>,
    /// `[left, right]` deep equality.
    #[serde(default)]
    pub equals: Option<[serde_json::Value; 2]>,
    #[serde(default)]
    pub not_equals: Option<[serde_json::Value; 2]>,
}

/// A picker option for an `id`-typed input, resolved from the ref'd
/// resource's list endpoint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct IdOption {
    pub value: String,
    pub label: String,
}

fn default_true() -> bool {
    true
}

fn default_delay() -> u64 {
    5
}
