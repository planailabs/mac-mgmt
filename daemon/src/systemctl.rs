use anyhow::{Context, Result};
use mac_mgmt_services::protocol::ServiceStatus;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Run the systemctl shim.
///
/// When `passthrough` is true (argv0 mode), unrecognised verbs and non-managed
/// services are forwarded to the real systemctl.  When false (subcommand mode),
/// only managed services are handled and everything else is an error.
pub async fn run(args: Vec<String>, passthrough: bool) -> Result<i32> {
    let verb = match args.first() {
        Some(v) => v.as_str(),
        None => {
            if passthrough {
                return passthrough_exec(&args);
            }
            anyhow::bail!("usage: mac-mgmt systemctl <verb> [unit ...]");
        }
    };

    match verb {
        "start" | "stop" | "restart" | "status" | "is-active" | "kill" => {
            handle_service_verb(verb, &args[1..], passthrough).await
        }
        "list-units" => handle_list_units(&args[1..], passthrough).await,
        "cat" => handle_cat(&args[1..], passthrough).await,
        _ => {
            if passthrough {
                passthrough_exec(&args)
            } else {
                anyhow::bail!("unsupported verb '{verb}'");
            }
        }
    }
}

// ── Verb handlers ───────────────────────────────────────────────────

async fn handle_service_verb(verb: &str, rest: &[String], passthrough: bool) -> Result<i32> {
    let (flags, units) = split_flags_and_units(rest);

    if units.is_empty() {
        if passthrough {
            let mut full = vec![verb.to_string()];
            full.extend(rest.iter().cloned());
            return passthrough_exec(&full);
        }
        anyhow::bail!("unit name required");
    }

    let mut client = match connect_supervisor().await {
        Some(c) => c,
        None => {
            if passthrough {
                let mut full = vec![verb.to_string()];
                full.extend(rest.iter().cloned());
                return passthrough_exec(&full);
            }
            anyhow::bail!("supervisor not available — is the daemon running?");
        }
    };

    let services = client.list().await.unwrap_or_default();
    let managed: HashMap<&str, &ServiceStatus> =
        services.iter().map(|s| (s.name.as_str(), s)).collect();

    let mut managed_units = Vec::new();
    let mut unmanaged_units = Vec::new();

    for unit in &units {
        let name = normalize_service_name(unit);
        if managed.contains_key(name) {
            managed_units.push(name.to_string());
        } else {
            unmanaged_units.push(unit.clone());
        }
    }

    let mut exit_code = 0;

    // Handle managed services.
    for name in &managed_units {
        let result = match verb {
            "start" => client.start_service(name).await,
            "stop" => client.stop_service(name).await,
            "restart" => client.restart_service(name).await,
            "kill" => {
                let signal = parse_kill_signal(&flags);
                client.kill_service(name, signal).await
            }
            "status" => {
                let svc = managed.get(name.as_str()).unwrap();
                print_status(svc);
                if svc.stopped || svc.pid.is_none() {
                    exit_code = 3;
                }
                Ok(())
            }
            "is-active" => {
                let svc = managed.get(name.as_str()).unwrap();
                if !svc.stopped && svc.pid.is_some() {
                    println!("active");
                } else {
                    println!("inactive");
                    exit_code = 3;
                }
                Ok(())
            }
            _ => unreachable!(),
        };
        if let Err(e) = result {
            eprintln!("Failed to {verb} {name}.service: {e}");
            exit_code = 1;
        }
    }

    // Handle unmanaged services.
    if !unmanaged_units.is_empty() {
        if passthrough {
            let mut passthrough_args = vec![verb.to_string()];
            passthrough_args.extend(flags.iter().cloned());
            passthrough_args.extend(unmanaged_units);
            let code = passthrough_exec(&passthrough_args)?;
            if code != 0 {
                exit_code = code;
            }
        } else {
            for unit in &unmanaged_units {
                eprintln!("Unknown service '{unit}'");
            }
            exit_code = 1;
        }
    }

    Ok(exit_code)
}

async fn handle_list_units(rest: &[String], passthrough: bool) -> Result<i32> {
    let mut client = match connect_supervisor().await {
        Some(c) => c,
        None => {
            if passthrough {
                let mut full = vec!["list-units".to_string()];
                full.extend(rest.iter().cloned());
                return passthrough_exec(&full);
            }
            anyhow::bail!("supervisor not available — is the daemon running?");
        }
    };

    let services = client.list().await?;
    if services.is_empty() {
        println!("0 loaded units listed.");
        return Ok(0);
    }

    print_list_units(&services);
    Ok(0)
}

async fn handle_cat(rest: &[String], passthrough: bool) -> Result<i32> {
    let (_flags, units) = split_flags_and_units(rest);

    if units.is_empty() {
        if passthrough {
            let mut full = vec!["cat".to_string()];
            full.extend(rest.iter().cloned());
            return passthrough_exec(&full);
        }
        anyhow::bail!("unit name required");
    }

    let mut client = match connect_supervisor().await {
        Some(c) => c,
        None => {
            if passthrough {
                let mut full = vec!["cat".to_string()];
                full.extend(rest.iter().cloned());
                return passthrough_exec(&full);
            }
            anyhow::bail!("supervisor not available — is the daemon running?");
        }
    };

    let services = client.list().await?;
    let managed: HashMap<&str, &ServiceStatus> =
        services.iter().map(|s| (s.name.as_str(), s)).collect();

    let mut exit_code = 0;
    let mut first = true;

    for unit in &units {
        let name = normalize_service_name(unit);
        if let Some(svc) = managed.get(name) {
            if !first {
                println!();
            }
            first = false;
            print_cat(svc);
        } else if passthrough {
            let code = passthrough_exec(&["cat".to_string(), unit.clone()])?;
            if code != 0 {
                exit_code = code;
            }
        } else {
            eprintln!("Unknown service '{unit}'");
            exit_code = 1;
        }
    }

    Ok(exit_code)
}

// ── Output formatting ───────────────────────────────────────────────

fn print_status(svc: &ServiceStatus) {
    let is_active = !svc.stopped && svc.pid.is_some();
    let dot = if is_active { "\u{25cf}" } else { "\u{25cb}" };

    println!(
        "{dot} {name}.service - {name} (managed by mac-mgmt)",
        name = svc.name
    );
    println!("     Loaded: loaded (mac-mgmt-services; managed)");
    if is_active {
        let pid = svc.pid.unwrap();
        let program = svc.spec.as_ref().map(|s| s.program.as_str()).unwrap_or("?");
        println!("     Active: active (running)");
        println!("   Main PID: {pid} ({program})");
    } else {
        println!("     Active: inactive (dead)");
    }
}

pub fn print_list_units(services: &[ServiceStatus]) {
    // Column widths.
    let unit_width = services
        .iter()
        .map(|s| s.name.len() + 8) // ".service"
        .max()
        .unwrap_or(20)
        .max(20);

    println!(
        "  {:<unit_width$} {:<8} {:<10} {:<8} {}",
        "UNIT", "LOAD", "ACTIVE", "SUB", "DESCRIPTION"
    );
    for svc in services {
        let unit_name = format!("{}.service", svc.name);
        let (active, sub) = if svc.stopped || svc.pid.is_none() {
            ("inactive", "dead")
        } else {
            ("active", "running")
        };
        println!(
            "  {:<unit_width$} {:<8} {:<10} {:<8} {} (managed by mac-mgmt)",
            unit_name, "loaded", active, sub, svc.name
        );
    }
    println!();
    println!("LEGEND: LOAD   \u{2192} Reflects whether the unit definition was properly loaded.");
    println!(
        "        ACTIVE \u{2192} The high-level unit activation state, i.e. generalization of SUB."
    );
    println!(
        "        SUB    \u{2192} The low-level unit activation state, values depend on unit type."
    );
    println!();
    println!("{} loaded units listed.", services.len());
}

fn print_cat(svc: &ServiceStatus) {
    println!(
        "# /mac-mgmt-services/{name}.service (managed by mac-mgmt)",
        name = svc.name
    );
    println!("[Service]");
    if let Some(ref spec) = svc.spec {
        let exec_start = if spec.args.is_empty() {
            spec.program.clone()
        } else {
            format!("{} {}", spec.program, spec.args.join(" "))
        };
        println!("ExecStart={exec_start}");
        let mut keys: Vec<&String> = spec.env.keys().collect();
        keys.sort();
        for key in keys {
            let val = &spec.env[key];
            println!("Environment=\"{key}={val}\"");
        }
    } else {
        println!("# (spec not available)");
    }
}

// ── Helpers ─────────────────────────────────────────────────────────

fn normalize_service_name(unit: &str) -> &str {
    unit.strip_suffix(".service").unwrap_or(unit)
}

/// Split arguments into flags (start with `-`) and positional unit names.
fn split_flags_and_units(args: &[String]) -> (Vec<String>, Vec<String>) {
    let mut flags = Vec::new();
    let mut units = Vec::new();
    let mut skip_next = false;
    for (i, arg) in args.iter().enumerate() {
        if skip_next {
            skip_next = false;
            continue;
        }
        if arg.starts_with('-') {
            flags.push(arg.clone());
            // `-s` and `--signal` take a value argument.
            if (arg == "-s" || arg == "--signal" || arg == "--kill-who") && i + 1 < args.len() {
                flags.push(args[i + 1].clone());
                skip_next = true;
            }
        } else {
            units.push(arg.clone());
        }
    }
    (flags, units)
}

/// Parse the kill signal from flags. Defaults to SIGTERM (15).
fn parse_kill_signal(flags: &[String]) -> i32 {
    for (i, flag) in flags.iter().enumerate() {
        if flag == "-s" || flag == "--signal" {
            if let Some(val) = flags.get(i + 1) {
                return parse_signal(val);
            }
        }
        if let Some(val) = flag.strip_prefix("--signal=") {
            return parse_signal(val);
        }
    }
    libc::SIGTERM
}

fn parse_signal(s: &str) -> i32 {
    // Try numeric first.
    if let Ok(n) = s.parse::<i32>() {
        return n;
    }
    // Strip optional SIG prefix.
    let name = s.strip_prefix("SIG").unwrap_or(s);
    match name.to_uppercase().as_str() {
        "HUP" => libc::SIGHUP,
        "INT" => libc::SIGINT,
        "QUIT" => libc::SIGQUIT,
        "KILL" => libc::SIGKILL,
        "TERM" => libc::SIGTERM,
        "USR1" => libc::SIGUSR1,
        "USR2" => libc::SIGUSR2,
        "CONT" => libc::SIGCONT,
        "STOP" => libc::SIGSTOP,
        _ => {
            eprintln!("Unknown signal '{s}', using SIGTERM");
            libc::SIGTERM
        }
    }
}

async fn connect_supervisor() -> Option<mac_mgmt_services::Client> {
    let socket_path = mac_mgmt_services::default_socket_path();
    mac_mgmt_services::Client::connect(&socket_path, Duration::from_secs(1))
        .await
        .ok()
}

fn find_real_systemctl() -> Option<PathBuf> {
    let self_exe = std::fs::canonicalize("/proc/self/exe").ok();

    let is_self = |candidate: &Path| -> bool {
        if let Some(ref self_exe) = self_exe {
            if let Ok(resolved) = std::fs::canonicalize(candidate) {
                return resolved == *self_exe;
            }
        }
        false
    };

    // Search PATH, skipping our own binary.
    for dir in std::env::var("PATH").unwrap_or_default().split(':') {
        let candidate = Path::new(dir).join("systemctl");
        if candidate.exists() && !is_self(&candidate) {
            return Some(candidate);
        }
    }

    // Fallback well-known paths.
    for path in &["/usr/bin/systemctl", "/bin/systemctl"] {
        let p = Path::new(path);
        if p.exists() && !is_self(p) {
            return Some(p.to_path_buf());
        }
    }

    None
}

fn passthrough_exec(args: &[String]) -> Result<i32> {
    let real = find_real_systemctl()
        .ok_or_else(|| anyhow::anyhow!("cannot find real systemctl in PATH"))?;
    let status = std::process::Command::new(&real)
        .args(args)
        .status()
        .with_context(|| format!("exec {}", real.display()))?;
    Ok(status.code().unwrap_or(1))
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use mac_mgmt_services::protocol::SpawnSpec;

    #[test]
    fn test_normalize_service_name() {
        assert_eq!(normalize_service_name("ollama.service"), "ollama");
        assert_eq!(normalize_service_name("ollama"), "ollama");
        assert_eq!(normalize_service_name("foo.timer"), "foo.timer");
        assert_eq!(normalize_service_name(".service"), "");
    }

    #[test]
    fn test_parse_signal_name() {
        assert_eq!(parse_signal("SIGTERM"), libc::SIGTERM);
        assert_eq!(parse_signal("TERM"), libc::SIGTERM);
        assert_eq!(parse_signal("SIGKILL"), libc::SIGKILL);
        assert_eq!(parse_signal("HUP"), libc::SIGHUP);
        assert_eq!(parse_signal("USR1"), libc::SIGUSR1);
    }

    #[test]
    fn test_parse_signal_number() {
        assert_eq!(parse_signal("15"), 15);
        assert_eq!(parse_signal("9"), 9);
        assert_eq!(parse_signal("1"), 1);
    }

    #[test]
    fn test_parse_kill_args() {
        let flags = vec!["-s".into(), "SIGKILL".into()];
        assert_eq!(parse_kill_signal(&flags), libc::SIGKILL);

        let flags = vec!["--signal=HUP".into()];
        assert_eq!(parse_kill_signal(&flags), libc::SIGHUP);

        let flags: Vec<String> = vec![];
        assert_eq!(parse_kill_signal(&flags), libc::SIGTERM);

        let flags = vec!["-s".into(), "9".into()];
        assert_eq!(parse_kill_signal(&flags), 9);
    }

    #[test]
    fn test_split_flags_and_units() {
        let args: Vec<String> = vec!["-s", "SIGKILL", "ollama", "hermes"]
            .into_iter()
            .map(String::from)
            .collect();
        let (flags, units) = split_flags_and_units(&args);
        assert_eq!(flags, vec!["-s", "SIGKILL"]);
        assert_eq!(units, vec!["ollama", "hermes"]);
    }

    #[test]
    fn test_format_status_active() {
        let svc = ServiceStatus {
            name: "ollama".into(),
            pid: Some(1234),
            exe: None,
            resolved_program: None,
            spec: Some(SpawnSpec {
                program: "ollama".into(),
                args: vec!["serve".into()],
                env: Default::default(),
            }),
            stopped: false,
        };
        // Just verify it doesn't panic; output goes to stdout.
        print_status(&svc);
    }

    #[test]
    fn test_format_status_inactive() {
        let svc = ServiceStatus {
            name: "ollama".into(),
            pid: None,
            exe: None,
            resolved_program: None,
            spec: None,
            stopped: true,
        };
        print_status(&svc);
    }

    #[test]
    fn test_format_list_units() {
        let services = vec![
            ServiceStatus {
                name: "ollama".into(),
                pid: Some(1234),
                exe: None,
                resolved_program: None,
                spec: None,
                stopped: false,
            },
            ServiceStatus {
                name: "hermes".into(),
                pid: None,
                exe: None,
                resolved_program: None,
                spec: None,
                stopped: true,
            },
        ];
        print_list_units(&services);
    }

    #[test]
    fn test_format_cat() {
        let svc = ServiceStatus {
            name: "ollama".into(),
            pid: Some(1234),
            exe: None,
            resolved_program: None,
            spec: Some(SpawnSpec {
                program: "ollama".into(),
                args: vec!["serve".into()],
                env: [("OLLAMA_HOST".into(), "127.0.0.1:11434".into())]
                    .into_iter()
                    .collect(),
            }),
            stopped: false,
        };
        print_cat(&svc);
    }
}
