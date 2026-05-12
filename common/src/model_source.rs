use serde::{Deserialize, Serialize};

/// A single selectable model entry (leaf node in the tree).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelEntry {
    /// Short model ID (e.g. "claude-sonnet-4-6", "phi4-mini").
    pub model_id: String,
    /// Fully qualified ID including provider prefix (e.g. "anthropic/claude-sonnet-4-6").
    /// For Ollama models this equals `model_id`.
    pub full_model_id: String,
    /// Human-readable display name (e.g. "Claude Sonnet 4.6").
    pub display_name: String,
}

/// A node in the model tree — either a named group of children or a leaf model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ModelNode {
    Group {
        name: String,
        display_name: String,
        children: Vec<ModelNode>,
    },
    Model(ModelEntry),
}

/// A source of models with a hierarchical tree structure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelSource {
    /// Source identifier (e.g. "ollama", "openrouter", "openclaw").
    pub id: String,
    /// Human-readable name (e.g. "Ollama", "OpenRouter").
    pub display_name: String,
    /// Root-level nodes (the tree).
    pub groups: Vec<ModelNode>,
}

impl ModelSource {
    /// Flat list of all model entries in this source.
    pub fn list_all(&self) -> Vec<ModelEntry> {
        let mut out = Vec::new();
        for node in &self.groups {
            node.collect_entries(&mut out);
        }
        out
    }

    /// Search models by query, returning a pruned tree containing only
    /// matching leaves and their ancestor groups.
    /// Match is case-insensitive against model_id, full_model_id, and display_name.
    pub fn search(&self, query: &str) -> Vec<ModelNode> {
        let q = query.to_lowercase();
        self.groups.iter().filter_map(|n| n.filter(&q)).collect()
    }
}

impl ModelNode {
    /// Recursively collect all [`ModelEntry`] leaves.
    pub fn collect_entries(&self, out: &mut Vec<ModelEntry>) {
        match self {
            Self::Model(entry) => out.push(entry.clone()),
            Self::Group { children, .. } => {
                for child in children {
                    child.collect_entries(out);
                }
            }
        }
    }

    /// Recursively prune: keep only nodes whose display_name or model_id
    /// contain `query` (already lowercased). Returns `None` if nothing matches.
    pub fn filter(&self, query: &str) -> Option<ModelNode> {
        match self {
            Self::Model(entry) => {
                if entry.model_id.to_lowercase().contains(query)
                    || entry.full_model_id.to_lowercase().contains(query)
                    || entry.display_name.to_lowercase().contains(query)
                {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::Group {
                name,
                display_name,
                children,
            } => {
                // If the group name itself matches, return the whole subtree.
                if name.to_lowercase().contains(query)
                    || display_name.to_lowercase().contains(query)
                {
                    return Some(self.clone());
                }
                // Otherwise prune children.
                let filtered: Vec<_> = children.iter().filter_map(|c| c.filter(query)).collect();
                if filtered.is_empty() {
                    None
                } else {
                    Some(Self::Group {
                        name: name.clone(),
                        display_name: display_name.clone(),
                        children: filtered,
                    })
                }
            }
        }
    }

    /// Count total leaf model entries under this node.
    pub fn count_models(&self) -> usize {
        match self {
            Self::Model(_) => 1,
            Self::Group { children, .. } => children.iter().map(|c| c.count_models()).sum(),
        }
    }
}

// ── Grouping helpers ─────────────────────────────────────────────────

/// Build a recursive tree from a flat list of models, grouping by the
/// provided `group_fn`. `group_fn` returns a list of group segments
/// (e.g. `["Anthropic", "Claude 4.6"]`).
pub fn group_models(
    models: Vec<ModelEntry>,
    group_fn: impl Fn(&ModelEntry) -> Vec<String>,
) -> Vec<ModelNode> {
    let mut root_children: Vec<ModelNode> = Vec::new();
    for entry in models {
        let segments = group_fn(&entry);
        insert_into_tree(&mut root_children, &segments, entry);
    }
    // Sort groups alphabetically, models by id.
    sort_tree(&mut root_children);
    root_children
}

fn insert_into_tree(nodes: &mut Vec<ModelNode>, segments: &[String], entry: ModelEntry) {
    if segments.is_empty() {
        nodes.push(ModelNode::Model(entry));
        return;
    }
    let seg = &segments[0];
    // Find or create the group for this segment.
    let pos = nodes.iter().position(|n| matches!(n, ModelNode::Group { name, .. } if name == seg));
    if let Some(idx) = pos {
        if let ModelNode::Group { children, .. } = &mut nodes[idx] {
            insert_into_tree(children, &segments[1..], entry);
        }
    } else {
        let mut children = Vec::new();
        insert_into_tree(&mut children, &segments[1..], entry);
        nodes.push(ModelNode::Group {
            name: seg.clone(),
            display_name: seg.clone(),
            children,
        });
    }
}

fn sort_tree(nodes: &mut [ModelNode]) {
    nodes.sort_by(|a, b| {
        let key = |n: &ModelNode| match n {
            ModelNode::Group { name, .. } => (0, name.to_lowercase()),
            ModelNode::Model(e) => (1, e.model_id.to_lowercase()),
        };
        key(a).cmp(&key(b))
    });
    for node in nodes.iter_mut() {
        if let ModelNode::Group { children, .. } = node {
            sort_tree(children);
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_source() -> ModelSource {
        ModelSource {
            id: "test".into(),
            display_name: "Test".into(),
            groups: vec![
                ModelNode::Group {
                    name: "anthropic".into(),
                    display_name: "Anthropic".into(),
                    children: vec![
                        ModelNode::Group {
                            name: "claude-4.6".into(),
                            display_name: "Claude 4.6".into(),
                            children: vec![
                                ModelNode::Model(ModelEntry {
                                    model_id: "claude-opus-4-6".into(),
                                    full_model_id: "anthropic/claude-opus-4-6".into(),
                                    display_name: "Claude Opus 4.6".into(),
                                }),
                                ModelNode::Model(ModelEntry {
                                    model_id: "claude-sonnet-4-6".into(),
                                    full_model_id: "anthropic/claude-sonnet-4-6".into(),
                                    display_name: "Claude Sonnet 4.6".into(),
                                }),
                            ],
                        },
                        ModelNode::Model(ModelEntry {
                            model_id: "claude-haiku-4-5".into(),
                            full_model_id: "anthropic/claude-haiku-4-5".into(),
                            display_name: "Claude Haiku 4.5".into(),
                        }),
                    ],
                },
                ModelNode::Group {
                    name: "deepseek".into(),
                    display_name: "DeepSeek".into(),
                    children: vec![ModelNode::Model(ModelEntry {
                        model_id: "deepseek-chat".into(),
                        full_model_id: "deepseek/deepseek-chat".into(),
                        display_name: "DeepSeek Chat".into(),
                    })],
                },
            ],
        }
    }

    #[test]
    fn list_all_returns_all_leaves() {
        let src = sample_source();
        let all = src.list_all();
        assert_eq!(all.len(), 4);
        let ids: Vec<_> = all.iter().map(|e| e.model_id.as_str()).collect();
        assert!(ids.contains(&"claude-opus-4-6"));
        assert!(ids.contains(&"claude-sonnet-4-6"));
        assert!(ids.contains(&"claude-haiku-4-5"));
        assert!(ids.contains(&"deepseek-chat"));
    }

    #[test]
    fn search_filters_by_model_id() {
        let src = sample_source();
        let results = src.search("opus");
        // Should keep anthropic > claude-4.6 > claude-opus-4-6
        assert_eq!(results.len(), 1); // only anthropic group
        let all = ModelSource {
            id: "r".into(),
            display_name: "r".into(),
            groups: results,
        }
        .list_all();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].model_id, "claude-opus-4-6");
    }

    #[test]
    fn search_by_group_name_returns_full_subtree() {
        let src = sample_source();
        let results = src.search("deepseek");
        assert_eq!(results.len(), 1);
        let all = ModelSource {
            id: "r".into(),
            display_name: "r".into(),
            groups: results,
        }
        .list_all();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].model_id, "deepseek-chat");
    }

    #[test]
    fn search_case_insensitive() {
        let src = sample_source();
        let results = src.search("CLAUDE");
        let all = ModelSource {
            id: "r".into(),
            display_name: "r".into(),
            groups: results,
        }
        .list_all();
        // All claude models should match
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn search_no_match_returns_empty() {
        let src = sample_source();
        let results = src.search("nonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn count_models() {
        let src = sample_source();
        let total: usize = src.groups.iter().map(|n| n.count_models()).sum();
        assert_eq!(total, 4);
    }

    #[test]
    fn group_models_builds_tree() {
        let entries = vec![
            ModelEntry {
                model_id: "a".into(),
                full_model_id: "x/a".into(),
                display_name: "A".into(),
            },
            ModelEntry {
                model_id: "b".into(),
                full_model_id: "x/b".into(),
                display_name: "B".into(),
            },
            ModelEntry {
                model_id: "c".into(),
                full_model_id: "y/c".into(),
                display_name: "C".into(),
            },
        ];
        let tree = group_models(entries, |e| {
            let prefix = e.full_model_id.split('/').next().unwrap();
            vec![prefix.to_string()]
        });
        assert_eq!(tree.len(), 2); // groups x and y
        let src = ModelSource {
            id: "t".into(),
            display_name: "t".into(),
            groups: tree,
        };
        assert_eq!(src.list_all().len(), 3);
    }
}
