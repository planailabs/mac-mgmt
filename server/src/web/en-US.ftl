## ── Common ──────────────────────────────────────────────────────

loading = Loading…
error-message = Error: { $message }
save = Save
cancel = Cancel
edit = Edit
delete = Delete
create = Create
add = Add
remove = Remove
close = Close
yes = Yes
no = No
name = Name
email = Email
description = Description
slug = Slug
created = Created
version = Version
status = Status
admin = Admin
member = Member
no-label = (no label)
revoked = revoked
active = active
none = None
confirm = Confirm
back = Back
download = Download
clear = Clear
hidden = Hidden

## ── Navigation ──────────────────────────────────────────────────

nav-logo = mac-mgmt
nav-clusters = Clusters
nav-command-center = Command Center
nav-fleet = Fleet
nav-easy-access = Easy Access
nav-overview = Overview
nav-mcp-skills = MCP + Skills
nav-skills = Skills
nav-mcp-servers = MCP Servers
nav-mcp-bundles = MCP Bundles
nav-bundles = Bundles
nav-import = Import
nav-import-sources = Import Sources
nav-admin = Admin
nav-admin-tokens = Admin Tokens
nav-staff-pings = Staff Pings
nav-organizations = Organizations
nav-users = Users
nav-skill-centers = Skill Centers
nav-version = Version
nav-rollouts = Rollouts
nav-rollout-groups = Rollout Groups
nav-daemon-versions = Daemon Versions
nav-resources = Resources
nav-docs = Docs
nav-api-docs = API Docs
nav-sign-out = Sign out
nav-view-profile = View Profile
nav-open-main-menu = Open main menu
nav-close-menu = Close menu
nav-toggle-sidebar = Toggle sidebar
breadcrumb-aria = Breadcrumb
breadcrumb-new = New
breadcrumb-edit = Edit
breadcrumb-config = Config
breadcrumb-packages = Packages
breadcrumb-search = Search
breadcrumb-profile = Profile

## ── Language Picker ─────────────────────────────────────────────

language-picker-label = Language

## ── Theme Toggle ────────────────────────────────────────────────

theme-system = Using system theme. Click for light mode
theme-light = Using light mode. Click for dark mode
theme-dark = Using dark mode. Click for system theme

## ── Layout ──────────────────────────────────────────────────────

impersonating = Impersonating { $email }
impersonate-stop = Stop

## ── Profile ─────────────────────────────────────────────────────

profile-role = Role
profile-organizations = Organizations
profile-no-orgs = Not a member of any organization.

## ── Cluster List ────────────────────────────────────────────────

cluster-list-title = Clusters
cluster-list-new = New Cluster
cluster-list-col-org = Organization
cluster-list-col-nixpkgs = Nixpkgs

## ── Cluster Form ────────────────────────────────────────────────

cluster-form-title = New Cluster

## ── Cluster Detail ──────────────────────────────────────────────

cluster-detail-delete-confirm = Delete this cluster?
cluster-detail-confirm-delete = Confirm
cluster-detail-created = Created: { $date }
cluster-detail-cloud-init = Cloud-init…
cluster-detail-group-access = Access Tokens
cluster-detail-group-config = Configuration
cluster-detail-group-skills = Skills & MCP
cluster-detail-group-operations = Packages & Operations
cluster-detail-tab-sync-tokens = Sync Tokens
cluster-detail-tab-setting-tokens = Setting / Cluster Tokens
cluster-detail-tab-config = Config
cluster-detail-tab-config-history = Config History
cluster-detail-tab-skills = Skills
cluster-detail-tab-mcp-servers = MCP Servers
cluster-detail-tab-ssh-keys = SSH Keys
cluster-detail-tab-packages = Packages
cluster-detail-open-packages = Manage Packages
cluster-detail-tab-healer = Healer
cluster-detail-version-label = Version:
cluster-detail-version-value = v{ $version }
cluster-detail-version-rollout = (rollout)
cluster-detail-no-version = No version pinned
cluster-detail-set = Set
cluster-detail-nixpkgs-label = Nixpkgs:
cluster-detail-nixpkgs-placeholder = commit sha
cluster-detail-no-nixpkgs = No nixpkgs pin
cluster-detail-cloud-init-title = Cloud-init for { $cluster }
cluster-detail-cloud-init-desc = Generates a fresh sync token and a ready-to-use cloud-config. Paste into any VM's user-data.
cluster-detail-generating = Generating…
cluster-detail-copied = Copied!
cluster-detail-copy = Copy
cluster-detail-download = Download

## ── Cluster Healer Settings ─────────────────────────────────────

healer-settings-override = Override server defaults
healer-settings-using-defaults = (using server defaults)
healer-settings-auto-trigger = Auto-trigger
healer-settings-auto-trigger-model = Auto-trigger model
healer-settings-server-default = Server default
healer-settings-auto-approve = Auto-approve remediation
healer-settings-fix-model = Fix model (remediation)
healer-settings-same-as-diagnosis = Same as diagnosis
healer-settings-saving = Saving...
healer-settings-ollama = Ollama (local)
healer-settings-anthropic = Anthropic
healer-settings-openrouter = OpenRouter

## ── Cluster MCP Servers ─────────────────────────────────────────

cluster-mcp-direct = Direct MCP Servers
cluster-mcp-select = Select MCP server...
cluster-mcp-local = Local
cluster-mcp-from-sc = From { $name }
cluster-mcp-no-direct = No direct MCP server assignments.
cluster-mcp-via = via { $source } MCP Center
cluster-mcp-overwritten = overwritten
cluster-mcp-from-bundles = From Bundles
cluster-mcp-no-bundle-mcp = No MCP servers from bundles.
cluster-mcp-from-skills = From Skills (transitive)
cluster-mcp-no-transitive = No transitive MCP dependencies.
cluster-mcp-bundles-title = MCP Bundles
cluster-mcp-select-bundle = Select MCP bundle...
cluster-mcp-no-bundle-assign = No MCP bundle assignments.

## ── Cluster Skills ──────────────────────────────────────────────

cluster-skills-direct = Direct Skills
cluster-skills-select = Select skill/channel...
cluster-skills-local = Local
cluster-skills-from-sc = From { $name }
cluster-skills-no-direct = No direct skill assignments.
cluster-skills-via = via { $source } Skill Center
cluster-skills-from-bundles = From Bundles
cluster-skills-no-bundle-skills = No skills from bundles.
cluster-skills-overwritten = overwritten
cluster-skills-bundles-title = Bundles
cluster-skills-select-bundle = Select bundle...
cluster-skills-no-bundle-assign = No bundle assignments.

## ── Cluster SSH Keys ────────────────────────────────────────────

ssh-keys-placeholder = ssh-ed25519 AAAA... user@host
ssh-keys-no-keys = No SSH keys.

## ── Cluster Client Certificates ────────────────────────────────

cluster-detail-tab-client-certs = Client Certificates
cluster-detail-tab-client-cas = Client CAs
admin-client-certs-title = Admin Client Certificates
admin-client-certs-description = Client certificates with admin-level access. These certificates can access all clusters via the relay.
nav-admin-client-certs = Client Certificates
nav-admin-client-cas = Client CAs
admin-client-cas-title = Admin Client CAs
admin-client-cas-description = CA certificates for admin-level trust. Client certificates signed by these CAs get admin access to all clusters.
client-certs-fingerprint-placeholder = sha256:ab12cd34...
client-certs-label-placeholder = Label (optional)
client-certs-no-certs = No client certificates.
client-certs-pem-placeholder = Paste PEM-encoded certificate (optional)...
client-cas-pem-placeholder = Paste PEM-encoded CA certificate...
client-cas-no-cas = No CA certificates.
org-detail-client-certs = Client Certificates
org-detail-client-cas = Client CAs
org-client-certs-description = Client certificates with organization-level access. These certificates can access all clusters belonging to this organization.
org-client-cas-description = CA certificates for organization-level trust. Client certificates signed by these CAs get access to this organization's clusters.

## ── Fleet Dashboard ─────────────────────────────────────────────

fleet-title = Fleet Dashboard
fleet-last-refreshed = Last refreshed: { $time }
fleet-filtered-by = Filtered by rollout:
fleet-clear-filter = Clear filter
fleet-no-daemons = No daemons have reported in yet.
fleet-unhealthy-only = Unhealthy only
fleet-col-cluster = Cluster
fleet-col-hostname = Hostname
fleet-col-env = Env
fleet-col-status = Status
fleet-col-load = Load
fleet-col-services = Services
fleet-col-probes = Probes
fleet-col-tunnels = Tunnels
fleet-col-last-seen = Last Seen
fleet-online = online
fleet-offline = offline
fleet-kpi-online = Online instances
fleet-kpi-services-healthy = Healthy services
fleet-kpi-clusters = Clusters
fleet-kpi-failing-probes = Failing probes
fleet-just-now = just now
fleet-healthy = healthy
fleet-unhealthy = unhealthy
fleet-ok = ok
fleet-fail = fail
fleet-pending = pending
fleet-no-result = no result yet
fleet-idle = idle

## ── Fleet Detail ────────────────────────────────────────────────

fleet-detail-subtitle = { $cluster } · { $env } · v{ $version }
fleet-detail-instance-id = instance_id:
fleet-detail-last-heartbeat = last heartbeat: { $time }
fleet-detail-services = Services
fleet-detail-no-services = No services reported.
fleet-detail-col-service = Service
fleet-detail-col-upgrade = Upgrade
fleet-detail-col-busy = Busy
fleet-detail-tunnels = Tunnels
fleet-detail-col-port = Port
fleet-detail-config-files = Configuration Files
fleet-detail-shell-commands = Shell Commands
fleet-detail-logs = Logs
fleet-detail-healer-agent = Healer Agent
fleet-detail-probes = Probes
fleet-detail-no-probes = No probe results yet — the first run can take up to 15 minutes.
fleet-detail-col-kind = Kind
fleet-detail-col-result = Result
fleet-detail-col-duration = Duration
fleet-detail-col-tokens = Tokens
fleet-detail-col-model = Model
fleet-detail-col-collected = Collected
fleet-detail-col-detail = Detail
fleet-detail-dynamic-sample = Dynamic sample
fleet-detail-stat-cpu = CPU load
fleet-detail-stat-cpu-sub = 1m avg
fleet-detail-stat-memory = Memory
fleet-detail-stat-net-rx = Net RX
fleet-detail-stat-net-tx = Net TX
fleet-detail-stat-net-sub = since boot
fleet-detail-no-sample = No sample on the most recent heartbeat.
fleet-detail-disks = Disks
fleet-detail-col-mount = Mount
fleet-detail-col-free = Free
fleet-detail-col-total = Total
fleet-detail-col-used = Used
fleet-detail-inventory = Inventory
fleet-detail-inventory-collected = collected { $time }
fleet-detail-no-inventory = No inventory snapshot yet.
fleet-detail-nixpkgs-pin = Nixpkgs pin
fleet-detail-network = Network interfaces
fleet-detail-gpus = GPUs
fleet-detail-col-num = #
fleet-detail-col-vendor = Vendor
fleet-detail-col-driver = Driver
fleet-detail-col-vram = VRAM
fleet-detail-col-util = Util
fleet-detail-col-temp = Temp
fleet-detail-col-power = Power
fleet-detail-col-pci = PCI
fleet-detail-col-gpu-index = #
fleet-detail-col-gpu-vendor = Vendor
fleet-detail-col-gpu-driver = Driver
fleet-detail-col-gpu-vram = VRAM
fleet-detail-col-gpu-util = Util
fleet-detail-col-gpu-temp = Temp
fleet-detail-col-gpu-power = Power
fleet-detail-col-gpu-pci = PCI
fleet-detail-nixpkgs-commit = Nixpkgs commit
fleet-detail-open = Open
fleet-detail-security = Security posture
fleet-detail-no-posture = No posture data yet.
fleet-detail-per-service = Per-service details
fleet-detail-tab-inventory = Inventory
fleet-detail-tab-live = Live status
fleet-detail-tab-live-status = Live status
fleet-detail-tab-security = Security

## ── File Editor ─────────────────────────────────────────────────

file-editor-unavailable = File editor unavailable: { $error }
file-editor-loading = Loading file editor...
file-editor-saved = Saved
file-editor-conflict = Conflict: file changed on disk. Reload and retry.
file-editor-back = Back to instance
file-editor-title = Configuration Files
file-editor-disclaimer = Changes here apply to this instance only and are not synced across the cluster. Use the cluster configuration for settings that should be consistent across all instances.
file-editor-readonly = read-only
file-editor-unsaved = unsaved changes
file-editor-saving = Saving...
file-editor-binary = Binary file — download to view
file-editor-select = Select a file to view or edit
file-editor-no-files = No configuration files available

## ── Shell Commands ──────────────────────────────────────────────

shell-title = Shell Commands
shell-running = Running...
shell-run = Run
shell-arg-label = { $label }:

## ── Log Viewer ──────────────────────────────────────────────────

log-title = Logs
log-all-services = All services
log-stop = Stop
log-start-tailing = Start Tailing
log-fetch-latest = Fetch Latest
log-empty-hint = Click 'Fetch Latest' or 'Start Tailing' to view logs.

## ── Skill List ──────────────────────────────────────────────────

skill-list-title = Skills
skill-list-syncing = Syncing...
skill-list-sync = Sync from xzar

## ── Skill Detail ────────────────────────────────────────────────

skill-detail-hide = Hide from public catalog
skill-detail-channels = Channels
skill-detail-channels-synced = Channels are synced from xzar.
skill-detail-no-channels = No channels synced yet.
skill-detail-nix-deps = Nix Dependencies
skill-detail-nix-placeholder = package-name
skill-detail-no-nix-deps = No nix dependencies.
skill-detail-mcp-deps = MCP Dependencies
skill-detail-select-mcp = Select MCP server...
skill-detail-no-mcp-deps = No MCP dependencies.

## ── Bundle List ─────────────────────────────────────────────────

bundle-list-title = Bundles
bundle-list-new = New Bundle

## ── Bundle Form ─────────────────────────────────────────────────

bundle-form-title = New Bundle
bundle-form-slug-placeholder = my-bundle

## ── Bundle Detail ───────────────────────────────────────────────

bundle-detail-hide = Hide from public catalog
bundle-detail-skill-channels = Skill Channels
bundle-detail-select-skill = Select skill/channel...

## ── MCP Server List ─────────────────────────────────────────────

mcp-server-list-title = MCP Servers
mcp-server-list-new = New MCP Server

## ── MCP Server Detail ───────────────────────────────────────────

mcp-server-slug-placeholder = my-server
mcp-server-config-json = Config JSON
mcp-server-hide = Hide from public catalog
mcp-server-nix-deps = Nix Dependencies
mcp-server-nix-placeholder = package-name
mcp-server-no-nix-deps = No nix dependencies.
mcp-server-required-by = Required by Skills
mcp-server-no-skills-depend = No skills depend on this MCP server.
mcp-server-new-title = New MCP Server
mcp-server-edit-title = Edit MCP Server

## ── MCP Bundle List ─────────────────────────────────────────────

mcp-bundle-list-title = MCP Bundles
mcp-bundle-list-new = New MCP Bundle

## ── MCP Bundle Form ─────────────────────────────────────────────

mcp-bundle-form-title = New MCP Bundle
mcp-bundle-slug-placeholder = my-mcp-bundle

## ── MCP Bundle Detail ───────────────────────────────────────────

mcp-bundle-detail-hide = Hide from public catalog
mcp-bundle-detail-mcp-servers = MCP Servers
mcp-bundle-detail-select-mcp = Select MCP server...

## ── Rollout List ────────────────────────────────────────────────

rollout-list-title = Rollouts
rollout-list-manage-groups = Manage Groups
rollout-list-new = New Rollout
rollout-list-no-rollouts = No rollouts yet.
rollout-list-col-stages = Stages
rollout-list-col-health = Health
rollout-status-rolling = rolling
rollout-status-completed = completed
rollout-status-paused = paused
rollout-status-failed = failed
rollout-status-pending = pending
rollout-health-pass = pass
rollout-health-fail = fail
rollout-health-grace = grace
rollout-health-no-data = no data
rollout-health-tooltip = { $evaluated } stage(s) evaluated, { $failing } failing
rollout-health-tooltip-summary = { $evaluated } stage(s) evaluated, { $failing } failing — { $summary }

## ── Rollout Form ────────────────────────────────────────────────

rollout-form-title = New Version Rollout
rollout-form-name-label = Name (optional)
rollout-form-name-placeholder = e.g. v0.2.0 rollout, nixpkgs security update
rollout-form-name-help = A short label to identify this rollout. Shown in the list and detail views.
rollout-form-target-version = Target Version
rollout-form-none-nixpkgs = — none (nixpkgs only) —
rollout-form-no-versions = No daemon versions available. Sync them on the Daemon Versions page.
rollout-form-version-help = Pick a version uploaded via xzar. Downgrades are blocked.
rollout-form-nixpkgs-label = Nixpkgs Commit (optional)
rollout-form-nixpkgs-placeholder = e.g. 170a4b510ad7ee95dde01adf2fe21704498dbb5c
rollout-form-nixpkgs-help = Pin the nixpkgs source to this commit. Leave blank to leave each cluster's existing pin untouched.
rollout-form-ramp-label = Ramp duration (minutes)
rollout-form-ramp-help = Release each stage gradually: over this many minutes from stage start, a growing share of the group's clusters becomes eligible for the target. Leave blank to release each stage all at once.
rollout-form-ramp-placeholder = e.g. 60
rollout-form-stages-label = Stages (select in order)
rollout-form-all-clusters = All Clusters
rollout-form-stage-num = (stage { $num })
rollout-form-create-groups-prefix = Create groups
rollout-form-create-groups-suffix = { " " }to roll out in stages.
rollout-form-health-gate = Health gate
rollout-form-auto-pause = Auto-pause stages when assessment data falls below thresholds
rollout-form-heartbeat-pct = Heartbeat fresh % (min)
rollout-form-heartbeat-window = Heartbeat freshness window (sec)
rollout-form-grace-period = Grace period after start (sec)
rollout-form-probe-thresholds = Probe success thresholds
rollout-form-service-placeholder = service (e.g. ollama)
rollout-form-pct-symbol = %
rollout-form-remove-service = remove
rollout-form-add-service = + add service
rollout-form-gate-help = A stage fails its gate if any listed service drops below the threshold over the last 30 min.
rollout-form-create = Create Rollout

## ── Rollout Detail ──────────────────────────────────────────────

rollout-detail-health = Rollout health
rollout-detail-stages-failing = { $failing }/{ $total } stages failing
rollout-detail-stages-passing = { $evaluated }/{ $total } stages passing
rollout-detail-cohort = cohort:{ " " }
rollout-detail-heartbeats = heartbeats fresh:{ " " }
rollout-detail-service-probe = { $service }:{ " " }
rollout-detail-top-reason = Top reason: { $reason }
rollout-detail-rollout-prefix = Rollout { $id }
rollout-detail-created-label = Created: { $date }
rollout-detail-rollback = Rollback
rollout-detail-rollback-confirm = Roll back this rollout? Every cohort cluster's pinned_version and nixpkgs_commit will be reset to the baseline captured at start time. Daemons that already took the new version will downgrade on the next tick.
rollout-detail-delete-confirm = Delete this rollout? The rollout row and its stage history will be removed. Cluster pins stay where they are — this is only a cleanup of the rollout record.
rollout-detail-start = Start Rollout
rollout-detail-advance = Advance to Next Stage
rollout-detail-pause = Pause
rollout-detail-resume = Resume
rollout-detail-complete-all = Complete All
rollout-detail-complete-confirm = Complete this rollout now? Every stage is marked completed and the target version is written to the pinned_version of every cohort cluster, including stages that never rolled. Skips the health gate.
rollout-detail-stages = Stages
rollout-detail-stage-num = Stage { $num }
rollout-detail-started = Started: { $date }
rollout-detail-completed = Completed: { $date }
rollout-detail-online = { $healthy }/{ $total } online
rollout-detail-ramp = Ramp: { $pct }% eligible ({ $mins } min window)
rollout-detail-no-heartbeats = no heartbeats
rollout-detail-version-progress = { $upgraded }/{ $total } version
rollout-detail-nixpkgs-progress = { $upgraded }/{ $total } nixpkgs
rollout-detail-no-gate = No health gate configured.
rollout-detail-view-fleet = View fleet
rollout-detail-add-gate = Add gate
rollout-detail-assessment-gate = Assessment gate
rollout-detail-gate-pass = gate: pass
rollout-detail-gate-fail = gate: fail
rollout-detail-gate-grace = gate: grace period
rollout-detail-gate-no-data = gate: no data
rollout-detail-evaluated = evaluated { $ts }
rollout-detail-edit-gate = Edit gate
rollout-detail-reevaluating = Reevaluating…
rollout-detail-reevaluate-now = Reevaluate now
rollout-detail-request-assessment = Request fresh assessment
rollout-detail-requesting = requesting…
rollout-detail-no-clusters = no clusters in cohort
rollout-detail-no-daemons = 0 of { $cohort } daemons reachable — none have an active SSE connection right now
rollout-detail-pushed = pushed to { $dispatched } of { $cohort } daemons (results land in ~30s)
rollout-detail-trigger-self-update = Trigger self-update
rollout-detail-triggering = triggering…
rollout-detail-push-result = pushed to { $dispatched } of { $cohort } daemons
rollout-detail-push-none = 0 of { $cohort } daemons reachable
rollout-detail-trigger-sync-nixpkgs = Trigger sync nixpkgs
rollout-detail-gate-config = Gate configuration
rollout-detail-gate-enabled = Gate enabled (unchecked = no gate, always passes)
rollout-detail-freshness-window = Freshness window (sec)
rollout-detail-grace-period = Grace period (sec)
rollout-detail-apply-all = Apply this gate to all stages of this rollout
rollout-detail-target = Target
rollout-detail-version-label = Version:{ " " }
rollout-detail-nixpkgs-label = Nixpkgs commit:{ " " }
rollout-detail-rollback-baseline = Rollback baseline
rollout-detail-delivered = Delivered clusters ({ $count })
rollout-detail-delivered-help = These clusters fetched this rollout's target and keep receiving it for the rollout's lifetime, even while paused or gated.
rollout-detail-samples = samples:{ " " }
rollout-detail-cpu-load = cpu load:{ " " }
rollout-detail-mem = mem:{ " " }
rollout-detail-max-disk = max disk:{ " " }
rollout-detail-gpu-util = gpu util:{ " " }
rollout-detail-thermal-alerts = thermal alerts: { $count }
rollout-detail-col-service = Service
rollout-detail-col-ok-total = OK / total
rollout-detail-col-avg-ms = avg ms
rollout-detail-col-ttft = TTFT
rollout-detail-col-tokens-out = tokens out
rollout-detail-col-last-failure = Last failure
rollout-status-rolled-back = rolled back

## ── Rollout Group List ──────────────────────────────────────────

rollout-group-list-title = Rollout Groups
rollout-group-list-create = Create Group
rollout-group-list-name-placeholder = Group name
rollout-group-list-desc-placeholder = Description
rollout-group-list-col-members = Members

## ── Rollout Group Detail ────────────────────────────────────────

rollout-group-cannot-delete = the All Clusters group cannot be deleted
rollout-group-delete = Delete Group
rollout-group-members = Members
rollout-group-select-cluster = Select cluster to add...
rollout-group-add-all = Add All Clusters
rollout-group-no-members = No members yet.
rollout-group-col-cluster = Cluster

## ── Admin Tokens ────────────────────────────────────────────────

admin-tokens-title = Admin Tokens
admin-tokens-description = Admin tokens are not scoped to any cluster. They can list all clusters and create sync/setting tokens for any cluster.
admin-token-new = New token (copy now, shown once):
admin-token-label-placeholder = Admin token label
admin-token-create = Create Admin Token
admin-token-revoke = Revoke

## ── Federation Tokens ───────────────────────────────────────────

federation-tokens-title = Federation Tokens
federation-tokens-description = Federation tokens allow remote management servers to access this instance's skill center catalog and resolve skills. Share these with management servers that pull from this skill center.
all-tokens-title = All Tokens
all-tokens-description = Every token across the system, with its scope. Read-only — create tokens from their respective sections. Revoke any token here.
federation-token-new = New federation token (copy now, shown once):
federation-token-label = Federation token label
federation-token-create = Create Federation Token
federation-token-none = No federation tokens yet.
federation-token-revoke = Revoke

## ── Sync Tokens ─────────────────────────────────────────────────

sync-token-new = New token (copy now, shown once):
sync-token-label = Sync token label
sync-token-create = Create Sync Token
sync-token-expired = expired { $date }
sync-token-expires = expires { $date }
sync-token-revoke = Revoke

## ── Setting Tokens ──────────────────────────────────────────────

setting-token-new = New token (copy now, shown once):
setting-token-label = Setting token label
setting-token-create = Create Setting Token
setting-token-expired = expired { $date }
setting-token-expires = expires { $date }
setting-token-revoke = Revoke

## ── Organization List ───────────────────────────────────────────

org-list-title = Organizations
org-list-new = New Organization
org-list-col-members = Members
org-list-col-clusters = Clusters

## ── Organization Form ───────────────────────────────────────────

org-form-title = New Organization
org-form-name-required = Name is required
org-form-name-placeholder = Organization name

## ── Organization Detail ─────────────────────────────────────────

org-detail-created = Created { $date }
org-detail-confirm = Are you sure?
org-detail-confirm-delete = Confirm Delete
org-detail-delete = Delete Organization
org-detail-members = Members
org-detail-select-user = Select user to add...
org-detail-role-read = Read
org-detail-role-write = Write
org-detail-role-admin = Admin
org-detail-no-members = No members yet.
org-detail-col-email = email
org-detail-col-name = name
org-detail-col-role = role
org-detail-clusters = Clusters
org-detail-select-cluster = Select cluster to add...
org-detail-no-clusters = No clusters yet.
org-detail-tokens = Tokens
org-detail-token-label-placeholder = Token label...
org-detail-create-token = Create Token
org-detail-token-created = Token created! Copy it now — it won't be shown again.
org-detail-no-tokens = No tokens yet.

## ── User List ───────────────────────────────────────────────────

user-list-title = Users
user-list-new = New User
user-list-col-orgs = Organizations

## ── User Form ───────────────────────────────────────────────────

user-form-title = New User
user-form-email-placeholder = user@example.com
user-form-name-placeholder = Display name

## ── User Detail ─────────────────────────────────────────────────

user-detail-col-email = email
user-detail-col-name = name
user-detail-created = Created { $date }
user-detail-impersonate = Impersonate
user-detail-confirm = Are you sure?
user-detail-yes-delete = Yes, delete
user-detail-delete = Delete User
user-detail-orgs = Organizations
user-detail-select-org = Select organization to add...
user-detail-no-orgs = Not a member of any organization.
user-detail-col-org = organization_name
user-detail-col-role = role

## ── Skill Center List ───────────────────────────────────────────

skill-center-list-title = Skill Centers
skill-center-list-new = New Skill Center
skill-center-list-none = No skill centers registered.
skill-center-list-col-url = URL
skill-center-list-col-priority = Priority
skill-center-list-col-enabled = Enabled

## ── Skill Center Form ───────────────────────────────────────────

skill-center-form-title = New Skill Center
skill-center-form-name-required = Name is required
skill-center-form-url-required = URL is required
skill-center-form-token-required = Federation token is required
skill-center-form-name-placeholder = Skill Center Name
skill-center-form-url-label = URL
skill-center-form-url-placeholder = https://skills.example.com:7378
skill-center-form-token-label = Federation Token
skill-center-form-token-placeholder = fed_...
skill-center-form-priority-label = Priority
skill-center-form-priority-help = Higher priority wins on slug collision between skill centers.
skill-center-form-enabled = Enabled

## ── Skill Center Detail ─────────────────────────────────────────

skill-center-detail-url = URL
skill-center-detail-priority = Priority
skill-center-detail-enabled = Enabled
skill-center-detail-created = Created
skill-center-detail-updated = Updated
skill-center-detail-token-label = Federation Token
skill-center-detail-token-hint = Leave empty to keep current token
skill-center-detail-catalog = Cached Catalog
skill-center-detail-syncing = Syncing...
skill-center-detail-sync-now = Sync Now
skill-center-detail-no-catalog = No catalog data cached yet. The catalog will be fetched automatically.
skill-center-detail-skill-channels = Skill Channels
skill-center-detail-bundles = Bundles
skill-center-detail-mcp-servers = MCP Servers
skill-center-detail-mcp-bundles = MCP Bundles
skill-center-detail-last-synced = Last synced: { $time }
skill-center-detail-catalog-error = Failed to load catalog summary: { $error }
skill-center-detail-loading-catalog = Loading catalog...
skill-center-detail-not-found = Skill center not found.

## ── Daemon Version List ─────────────────────────────────────────

daemon-version-list-title = Daemon Versions
daemon-version-list-syncing = Syncing...
daemon-version-list-sync = Sync from xzar
daemon-version-list-none = No daemon versions yet. Run xzar.sh to upload binaries, then click Sync.
daemon-version-list-col-added = Added

## ── Daemon Version Detail ───────────────────────────────────────

daemon-version-detail-title = Daemon { $version }
daemon-version-detail-all = ← All versions
daemon-version-detail-no-paths = No store paths found in xzar for this version.
daemon-version-detail-col-system = System
daemon-version-detail-col-store-path = Store Path
daemon-version-detail-clusters = Clusters on this version
daemon-version-detail-no-daemons = No daemons reporting this version.
daemon-version-detail-col-cluster = Cluster
daemon-version-detail-col-instances = Instances
daemon-version-detail-rollouts = Rollouts targeting this version
daemon-version-detail-no-rollouts = No rollouts target this version.
daemon-version-detail-col-rollout = Rollout
daemon-version-detail-pinned = Clusters pinned to this version
daemon-version-detail-no-pinned = No clusters pinned to this version.

## ── Healer Page ─────────────────────────────────────────────────

healer-title = Healer Agent
healer-instance = Instance: { $instance_id } ({ $hostname })
healer-unhealthy = Unhealthy services: { $services }
healer-settings = Settings
healer-new-session = New Session
healer-model = Model
healer-ollama-free = Ollama (local, free)
healer-anthropic-cloud = Anthropic (cloud)
healer-openrouter-cloud = OpenRouter (cloud)
healer-ollama-hint = Free to run, but local models are less capable than cloud models.
healer-openrouter-hint = Uses OpenRouter API credits. Subject to token budget.
healer-anthropic-hint = Uses Anthropic API credits. More capable, subject to token budget.
healer-fix-model = Fix Model (remediation)
healer-fix-model-hint = Optional: use a different model for the remediation phase after diagnosis.
healer-same-as-diagnosis = Same as diagnosis model
healer-instructions-placeholder = Optional instructions (leave empty for auto-diagnosis)...
healer-auto-approve = Auto-approve remediation
healer-skip-approval = (skip approval gate between diagnosis and fix)
healer-start = Start Healing
healer-pause = Pause
healer-cancel-session = Cancel
healer-resume = Resume
healer-back-to-sessions = Back to sessions
healer-thinking = Agent is thinking...
healer-verify-outcome = verify outcome
healer-previous-sessions = Previous Sessions
healer-approval-pending = approval pending
healer-auto = auto
healer-auto-approve-label = auto-approve
healer-view = View
healer-session-title = Healer Session
healer-session-subtitle = Instance: { $instance_id } — Session: { $session_id }
healer-auto-triggered = auto-triggered
healer-more-tokens = More Tokens (1M)
healer-paused-by-user = paused by user
healer-token-budget = token budget exceeded
healer-proxy-expiring = proxy token expiring
healer-server-shutdown = server shutdown
healer-state-initializing = Initializing
healer-state-diagnosing = Diagnosing
healer-state-remediating = Remediating
healer-state-verifying = Verifying
healer-state-done = Done
healer-state-failed = Failed
healer-state-cancelled = Cancelled
healer-state-paused = Paused
healer-state-awaiting-approval = Awaiting Approval
healer-state-awaiting-retry = Awaiting retry
healer-state-needs-human = Needs Human Attention
healer-state-unknown = Unknown
healer-event-state-change = State Change
healer-event-system = System
healer-event-agent = Agent
healer-event-user = User
healer-event-summary = Summary
healer-event-other = Other
healer-event-error = error
healer-phase-diagnosis = Diagnosis
healer-phase-diagnosis-short = D
healer-phase-d = D
healer-phase-remediation = Remediation Plan
healer-phase-remediation-short = R
healer-phase-r = R
healer-phase-final = Final Report
healer-phase-final-short = F
healer-phase-f = F
healer-phase-final-report = Final Report

## ── Config Editor ───────────────────────────────────────────────

config-editor-raw-json = Raw JSON
config-editor-paste-placeholder = Paste JSON config here...
config-editor-schema-error = Failed to load schema: { $error }
config-editor-loading-schema = Loading schema...
config-editor-save = Save Config
config-filter-all = All
config-filter-enabled = Enabled
config-filter-modified = Modified
config-filter-errors = Errors
config-save-unsaved = Unsaved changes
config-save-summary = { $fields ->
    [one] { $fields } unsaved change
   *[other] { $fields } unsaved changes
} in { $sections ->
    [one] { $sections } section
   *[other] { $sections } sections
}
config-save-discard = Discard
config-save-draft = Draft
config-save-review-diff = Review diff
config-save-review-diff-title = Review unsaved changes
config-save-review-diff-help = Diff against the last server snapshot. Click outside to close.
config-save-review-diff-empty = No textual changes detected.
config-editor-last-saved = Last saved: { $time }
config-editor-no-config = No config saved yet.
config-editor-key-hash = key_hash
config-editor-key-hash-help = Hex-encoded multihash of the API key (the raw key is only shown once on generation)
config-editor-generate = Generate
config-editor-key-warning = Save this key now — it will not be shown again:
config-editor-dismiss = Dismiss
config-editor-add-entry = + Add entry
config-add-cloud = + Add cloud LLM provider
config-add-custom-service = + Add custom service
config-editor-reset-default = reset to default
config-editor-discard-field = discard unsaved changes to this field
config-editor-add-item = Add item...
config-editor-select = -- select --
config-editor-secret-placeholder = secret:NAME or raw value
config-editor-secret-hint = Use secret:NAME to reference a vault secret, or enter a raw value
config-editor-convert-to-secret = Move to vault
config-editor-converting = Converting...
config-editor-advanced = Advanced · { $count } { $count ->
    [one] field
    *[other] fields
}
config-editor-enabled-suffix = enabled
config-editor-remove = Remove
config-editor-on-this-page = On this page
config-editor-sections-count = sections

## ── Config Categories ──────────────────────────────────────────

category-identity = Identity
category-llm-providers = LLM Providers
category-agents = Agents
category-infra = Infrastructure
category-ops = Operations
category-custom = Custom Services
category-other = Other

## ── Config History ──────────────────────────────────────────────

config-history-left = Left (older)
config-history-right = Right (newer)
config-history-select = Select version...
config-history-compare = Compare
config-history-no-history = No config history yet.
config-history-insert = insert
config-history-delete = delete
config-history-equal = equal

## ── Extra Config Modal ──────────────────────────────────────────

extra-config-label = extra_config
extra-config-help = Arbitrary openclaw.json keys merged after typed fields.
extra-config-values-set = { $count } value(s) set
extra-config-edit = Edit extra_config…
extra-config-title = Edit openclaw extra_config
extra-config-filter = Filter by path… (space-separated tokens match in order)
extra-config-legend = legend:
extra-config-sensitive = sensitive
extra-config-array-of-objects = array of objects
extra-config-string-map = string-keyed map
extra-config-type = type
extra-config-active-union = active union mode
extra-config-schema-error = Failed to load schema: { $error }
extra-config-loading-schema = Loading schema…
extra-config-key = key
extra-config-add = + add
extra-config-add-item = + add item

## ── Generate Button ─────────────────────────────────────────────

generate-generating = Generating...
generate-with-ai = Generate with AI

## ── Generate All Button ─────────────────────────────────────────

generate-all-generating = Generating...
generate-all-done = All have descriptions
generate-all-pending = Generate all ({ $count })
generate-all-progress = { $done } / { $total }

## ── Table Utils ─────────────────────────────────────────────────

table-via = via { $source }
table-via-skill-center = via { $source } Skill Center
table-via-mcp-center = via { $source } MCP Center
table-version-prefix = v{ $version }
## ── Hidden Badge ────────────────────────────────────────────────

hidden-badge = Hidden
hidden-col-visibility = Visibility { $indicator }

## ── Push Menu ───────────────────────────────────────────────────

push-menu-button = Push ↓
push-sync-config = Sync Config
push-sync-config-desc = Push config to daemons
push-sync-skills = Sync Skills
push-sync-skills-desc = Push skill assignments
push-sync-mcp = Sync MCP Servers
push-sync-mcp-desc = Push MCP server assignments
push-sync-ssh = Sync SSH Keys
push-sync-ssh-desc = Push SSH key changes
push-sync-nixpkgs = Sync Nixpkgs
push-sync-nixpkgs-desc = Push nixpkgs pin
push-sync-packages = Sync Packages
push-sync-packages-desc = Push unified nix package sync
push-self-update = Self Update
push-self-update-desc = Trigger daemon binary update
push-request-assessment = Request Assessment
push-request-assessment-desc = Trigger immediate health probe

## ── Import Sources ─────────────────────────────────────────────

import-title = Import Sources
import-add-source = Add Source
import-new-source = New Import Source
import-field-name = Name
import-name-placeholder = My skill source
import-field-type = Type
import-type-git = Git Repository
import-type-clawhub = ClawHub
import-field-repo-url = Repository URL
import-field-branch = Branch (optional)
import-field-glob = Glob pattern
import-field-clawhub-slug = ClawHub Skill Slug
import-field-channel = Channel
import-auto-sync = Auto-sync periodically
import-create = Create
import-no-sources = No import sources configured.
import-badge-auto = auto
import-last-synced = Last synced: { $time }
import-never-synced = never
import-show-jobs = Jobs
import-hide-jobs = Hide Jobs
import-sync-now = Sync Now
import-syncing = Syncing...
import-source-config = Source Configuration
import-recent-jobs = Recent Jobs
import-no-jobs = No jobs yet.
import-skills-imported = { $count } skill(s) imported
import-edit-source = Edit Import Source
import-search-clawhub = Search ClawHub
import-search-placeholder = Search for skills...
import-search-button = Search
import-searching = Searching...
import-search-no-results = No results found.
import-search-import = Import

## ── Easy Access ─────────────────────────────────────────────────

easy-access-title = Easy Access
easy-access-no-nodes = No online nodes with tunnels configured.
easy-access-files = Files
easy-access-shell = Shell

## ── Docs ────────────────────────────────────────────────────────

docs-title = Documentation
docs-user-guides = User Guides
docs-administration = Administration
docs-none-prefix = No documentation pages found. Add
docs-none-md = .md
docs-none-suffix = server/docs/
docs-back = ← Back to docs
docs-badge-admin = Admin
docs-badge-user = User

## ── Staff Pings ─────────────────────────────────────────────────

staff-pings-title = Staff Pings
staff-pings-description = Actionable notifications from the healer agent.
staff-pings-open = Open ({ $count })
staff-pings-no-open = No open staff pings
staff-pings-resolved = Resolved ({ $count })
staff-pings-view-session = View session
staff-pings-resolve = Resolve
staff-pings-resolved-by = Resolved by { $by }
staff-pings-loading = Loading staff pings...

## ── Fleet Dashboard reachability ────────────────────────────────

fleet-reachable-zero = 0 of { $total } daemons reachable
fleet-reachable-zero-hint = 0 of { $total } daemons reachable — none have an active SSE connection right now

## ── Cluster Config Page ────────────────────────────────────────────

cluster-config-back = Back to cluster
cluster-detail-open-config = Open Config & Secrets

## ── Cluster Packages Page ──────────────────────────────────────────

cluster-packages-title = Packages
cluster-packages-manual = Manual Packages
cluster-packages-all = All Packages (all sources)
cluster-packages-add-placeholder = package-name
cluster-packages-no-manual = No manual packages added.

## ── Secrets ────────────────────────────────────────────────────────

secrets-title = Secrets Vault
secrets-description = Secrets are encrypted at rest and can be referenced in config fields as secret:NAME.
secrets-empty = No secrets stored yet. Add one to reference it from any config field with secret:NAME.
secrets-col-name = Name
secrets-col-reference = Config Reference
secrets-col-value = Value
secrets-col-created = Created
secrets-col-actions = Actions
secrets-add = Add Secret
secrets-add-hint = encrypted at rest
secrets-click-to-copy = Click to copy reference
secrets-update = Update
secrets-new-value = New value...
secrets-value-placeholder = Secret value...
secrets-confirm-delete = Delete?

# Overview / Command Center
overview-title = Command Center
overview-subtitle = all organizations
overview-kicker = Command Center
overview-kpi-online = Instances online
overview-kpi-services = Healthy services
overview-kpi-rollouts = Active rollouts
overview-kpi-pings = Open staff pings
overview-kpi-signals = Critical signals
overview-activity-title = Live activity

# Model select modal
model-select-title = Select Models
model-select-search = Search models…
model-select-show-selected = Show selected
model-select-custom = Custom
model-select-custom-placeholder = Enter model ID…
model-select-loading = Loading models…
model-select-error = Failed to load models: { $error }
model-select-catalog = From catalog
model-select-apply = Apply
model-select-apply-count = Apply ({ $count })
model-select-browse = Select models…
