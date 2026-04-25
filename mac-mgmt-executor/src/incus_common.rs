use crate::types::OsImage;

/// Parse the JSON array returned by `incus image list images: --format json`
/// or the equivalent REST endpoint. Filters to container-type images
/// matching the current architecture.
pub fn parse_image_list(entries: &[serde_json::Value]) -> Vec<OsImage> {
    let current_arch = std::env::consts::ARCH;
    let incus_arch = match current_arch {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        other => other,
    };

    let mut images = Vec::new();
    for entry in entries {
        let props = match entry.get("properties") {
            Some(p) => p,
            None => continue,
        };

        let arch = props
            .get("architecture")
            .and_then(|a| a.as_str())
            .unwrap_or("");
        if arch != incus_arch {
            continue;
        }

        let image_type = entry.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if image_type != "container" {
            continue;
        }

        let aliases = entry.get("aliases").and_then(|a| a.as_array());
        let alias = aliases
            .and_then(|a| {
                a.iter()
                    .filter_map(|e| e.get("name").and_then(|n| n.as_str()))
                    .min_by_key(|n| n.len())
            })
            .unwrap_or("");

        if alias.is_empty() {
            continue;
        }

        let description = props
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("")
            .to_string();
        let os = props
            .get("os")
            .and_then(|o| o.as_str())
            .unwrap_or("")
            .to_string();
        let release = props
            .get("release")
            .and_then(|r| r.as_str())
            .unwrap_or("")
            .to_string();
        let variant = props
            .get("variant")
            .and_then(|v| v.as_str())
            .unwrap_or("default")
            .to_string();

        images.push(OsImage {
            alias: alias.to_string(),
            description,
            os,
            release,
            variant,
            image_type: image_type.to_string(),
        });
    }

    images.sort_by(|a, b| a.alias.cmp(&b.alias));
    images.dedup_by(|a, b| a.alias == b.alias);
    images
}
