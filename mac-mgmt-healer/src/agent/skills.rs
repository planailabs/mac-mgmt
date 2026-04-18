/// A built-in remediation procedure matched by error class.
pub struct RemediationSkill {
    pub error_class: &'static str,
    pub name: &'static str,
    pub steps: &'static str,
}

pub static SKILLS: &[RemediationSkill] = &[
    RemediationSkill {
        error_class: "timeout",
        name: "Service Timeout Recovery",
        steps: "\
1. Fetch recent logs for the affected service using `fetch_logs` with the service filter
2. Check the dynamic sample data for resource exhaustion (high CPU, low memory, low disk, high GPU VRAM)
3. Look for OOM kills, swap thrashing, or thermal throttling indicators in logs
4. If the service appears hung with no log activity, use the restart shell command if available
5. If resource exhaustion is evident, check the configuration for resource limits or model size issues
6. After making changes, wait briefly and re-check logs to verify recovery",
    },
    RemediationSkill {
        error_class: "connection",
        name: "Connection Failure Recovery",
        steps: "\
1. Fetch logs for the service to understand why connections are being refused
2. Check if the service process is running by looking at recent log output
3. Read the service configuration file to verify host/port settings
4. Check for port conflicts — another service may have claimed the port
5. If the service is down, use the restart shell command if available
6. If configuration is wrong (wrong port, wrong bind address), fix it via write_file
7. Verify the fix by checking logs again after changes",
    },
    RemediationSkill {
        error_class: "pull_failed",
        name: "Model Pull Failure Recovery",
        steps: "\
1. Check disk space in the dynamic sample — model pulls fail when disk is full
2. Fetch logs filtered to the failing service to see the exact pull error
3. If disk is low, look for large files or old models that can be cleaned up via shell commands
4. If it's a network issue (DNS, proxy, timeout), check the service configuration for proxy settings
5. If a specific model is failing, try pulling a smaller model first to verify connectivity
6. After freeing space or fixing network config, retry the pull via the appropriate shell command",
    },
    RemediationSkill {
        error_class: "bad_response",
        name: "Bad Response Recovery",
        steps: "\
1. Fetch logs for the service to see what responses it's producing
2. Read the configuration file to check model settings and API parameters
3. Verify the configured model is actually loaded/available by checking service status in logs
4. If the model is wrong or missing, update the configuration to use a known-good model
5. Compare configuration with other healthy instances in the cluster using `fetch_cluster_logs`
6. After config changes, verify the service produces correct responses by checking logs",
    },
    RemediationSkill {
        error_class: "error",
        name: "Generic Error Recovery",
        steps: "\
1. Start by fetching logs for ALL services to identify which ones are failing and why
2. Read configuration files for the affected services
3. Check the dynamic sample for system-level issues (disk full, memory exhaustion, GPU errors)
4. Compare with healthy instances in the cluster — fetch their logs to spot differences
5. Look for recent configuration changes that may have introduced the error
6. Apply the most targeted fix possible — prefer config changes over restarts
7. Document what you found even if you cannot fix it automatically",
    },
];

/// Find all matching remediation skills for a given error class.
pub fn find_skills(error_class: &str) -> Vec<&'static RemediationSkill> {
    let mut matched: Vec<_> = SKILLS
        .iter()
        .filter(|s| s.error_class == error_class)
        .collect();
    // Always include the generic "error" skill as a fallback
    if error_class != "error" && matched.is_empty() {
        matched.extend(SKILLS.iter().filter(|s| s.error_class == "error"));
    }
    matched
}

/// Format matched skills into a section for the system prompt.
pub fn format_skills_for_prompt(error_classes: &[String]) -> String {
    let mut seen = std::collections::HashSet::new();
    let mut output = String::new();

    for class in error_classes {
        for skill in find_skills(class) {
            if seen.insert(skill.name) {
                output.push_str(&format!("### {}\n", skill.name));
                output.push_str(skill.steps);
                output.push_str("\n\n");
            }
        }
    }

    if output.is_empty() {
        output.push_str("No specific remediation procedures matched. Use general troubleshooting.\n");
    }

    output
}
