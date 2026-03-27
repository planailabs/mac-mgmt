use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XzarPin {
    pub name: String,
    pub abandoned: bool,
    pub roots: Vec<XzarPinRoot>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XzarPinRoot {
    pub drv_full: String,
}

/// Fetch all pins from xzar.
pub async fn fetch_pins(url: &str, token: &str) -> Result<Vec<XzarPin>, String> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{url}/pins"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("xzar request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("xzar returned {}", resp.status()));
    }

    resp.json::<Vec<XzarPin>>()
        .await
        .map_err(|e| format!("failed to parse xzar response: {e}"))
}

/// Resolve skill store paths from a list of pins for a given architecture.
/// For each (slug, channel), looks for pin named `skill/{slug}/{channel}/{arch}`.
/// Returns a map of slug → store path. Skills without a matching pin are skipped.
pub fn resolve_store_paths(
    pins: &[XzarPin],
    skills: &[(String, String)],
    arch: &str,
) -> HashMap<String, String> {
    let mut result = HashMap::new();

    for (slug, channel) in skills {
        let pin_name = format!("skill/{slug}/{channel}/{arch}");
        if let Some(pin) = pins.iter().find(|p| p.name == pin_name && !p.abandoned) {
            if let Some(root) = pin.roots.first() {
                let path = if root.drv_full.starts_with("/nix/store/") {
                    root.drv_full.clone()
                } else {
                    format!("/nix/store/{}", root.drv_full)
                };
                result.insert(slug.clone(), path);
            }
        }
    }

    result
}

/// Extract the store path for a specific pin name from a list of pins.
pub fn store_path_for_pin(pins: &[XzarPin], pin_name: &str) -> Option<String> {
    pins.iter()
        .find(|p| p.name == pin_name && !p.abandoned)
        .and_then(|p| p.roots.first())
        .map(|root| {
            if root.drv_full.starts_with("/nix/store/") {
                root.drv_full.clone()
            } else {
                format!("/nix/store/{}", root.drv_full)
            }
        })
}
