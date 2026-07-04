use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    println!("cargo:rustc-env=BUILD_TIMESTAMP={ts}");

    bake_action_templates();
}

/// Bake `actions/*.yaml` into the binary, failing the build on any template
/// that doesn't parse or fails structural validation. Tool names can't be
/// checked here (the registry exists at runtime only); that half runs at
/// startup and in the `baked_templates_validate` test.
fn bake_action_templates() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let actions_dir = Path::new(&manifest_dir).join("actions");
    println!("cargo:rerun-if-changed={}", actions_dir.display());

    let mut templates: Vec<(String, std::path::PathBuf)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&actions_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let is_yaml = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e == "yaml" || e == "yml");
            if !is_yaml {
                continue;
            }
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_string();
            if name.is_empty()
                || !name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
            {
                panic!(
                    "action template file '{}': name must be a lowercase slug ([a-z0-9-_])",
                    path.display()
                );
            }
            templates.push((name, path));
        }
    }
    templates.sort();

    let builtins = plan_ai_actions::engine::BuiltinRegistry::standard();
    let mut entries = String::new();
    for (name, path) in &templates {
        println!("cargo:rerun-if-changed={}", path.display());
        let yaml = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("action template '{}': {e}", path.display()));
        let spec = plan_ai_actions::engine::parse_template(&yaml)
            .unwrap_or_else(|e| panic!("action template '{}': {e}", path.display()));
        if let Err(errors) = plan_ai_actions::engine::validate_template(&spec, None, &builtins) {
            panic!(
                "action template '{}' is invalid:\n  {}",
                path.display(),
                errors.join("\n  ")
            );
        }
        entries.push_str(&format!(
            "    ({name:?}, include_str!({:?})),\n",
            path.display()
        ));
    }

    let out = format!(
        "/// (name, yaml) of every template in actions/, validated at build time.\n\
         pub static ACTION_TEMPLATES: &[(&str, &str)] = &[\n{entries}];\n"
    );
    let out_path = Path::new(&std::env::var("OUT_DIR").unwrap()).join("actions_gen.rs");
    std::fs::write(&out_path, out).expect("write actions_gen.rs");
}
