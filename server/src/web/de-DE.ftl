## ── Allgemein ───────────────────────────────────────────────────

loading = Wird geladen…
error-message = Fehler: { $message }
save = Speichern
cancel = Abbrechen
edit = Bearbeiten
delete = Löschen
create = Erstellen
add = Hinzufügen
remove = Entfernen
close = Schließen
yes = Ja
no = Nein
name = Name
email = E-Mail
description = Beschreibung
slug = Slug
created = Erstellt
version = Version
status = Status
admin = Admin
member = Mitglied
no-label = (kein Label)
revoked = widerrufen
active = aktiv
none = Keine
confirm = Bestätigen
back = Zurück
download = Herunterladen
clear = Leeren
search-placeholder = Suchen…
hidden = Versteckt
dash = -
em-dash = —

## ── Navigation ──────────────────────────────────────────────────

nav-logo = mac-mgmt
nav-clusters = Cluster
nav-fleet = Flotte
nav-easy-access = Schnellzugriff
nav-overview = Übersicht
nav-mcp-skills = MCP + Skills
nav-skills = Skills
nav-mcp-servers = MCP Server
nav-mcp-bundles = MCP Bundles
nav-bundles = Bundles
nav-import = Import
nav-import-sources = Import-Quellen
nav-admin = Admin
nav-admin-tokens = Admin Tokens
nav-staff-pings = Mitarbeiter-Pings
nav-organizations = Organisationen
nav-users = Benutzer
nav-skill-centers = Skill Centers
nav-version = Version
nav-rollouts = Rollouts
nav-daemon-versions = Daemon-Versionen
nav-resources = Ressourcen
nav-docs = Dokumentation
nav-api-docs = API Docs
nav-sign-out = Abmelden
nav-view-profile = Profil anzeigen
nav-open-main-menu = Hauptmenü öffnen
nav-close-menu = Menü schließen

## ── Sprachauswahl ──────────────────────────────────────────────

language-picker-label = Sprache

## ── Design-Umschalter ──────────────────────────────────────────

theme-system = Systemdesign aktiv. Klicken für helles Design
theme-light = Helles Design aktiv. Klicken für dunkles Design
theme-dark = Dunkles Design aktiv. Klicken für Systemdesign

## ── Layout ──────────────────────────────────────────────────────

impersonating = Identitätswechsel: { $email }
impersonate-stop = Stopp

## ── Profil ──────────────────────────────────────────────────────

profile-role = Rolle
profile-organizations = Organisationen
profile-no-orgs = Kein Mitglied einer Organisation.

## ── Cluster-Liste ──────────────────────────────────────────────

cluster-list-title = Cluster
cluster-list-new = Neuer Cluster
cluster-list-col-org = Organisation
cluster-list-col-nixpkgs = Nixpkgs

## ── Cluster-Formular ───────────────────────────────────────────

cluster-form-title = Neuer Cluster

## ── Cluster-Details ────────────────────────────────────────────

cluster-detail-delete-confirm = Diesen Cluster löschen?
cluster-detail-confirm-delete = Bestätigen
cluster-detail-created = Erstellt: { $date }
cluster-detail-cloud-init = Cloud-init…
cluster-detail-tab-sync-tokens = Sync Tokens
cluster-detail-tab-setting-tokens = Einstellungs- / Cluster-Tokens
cluster-detail-tab-config = Konfiguration
cluster-detail-tab-config-history = Konfigurationsverlauf
cluster-detail-tab-skills = Skills
cluster-detail-tab-mcp-servers = MCP Server
cluster-detail-tab-ssh-keys = SSH Keys
cluster-detail-tab-healer = Healer
cluster-detail-version-label = Version:
cluster-detail-version-value = v{ $version }
cluster-detail-version-rollout = (Rollout)
cluster-detail-no-version = Keine Version festgelegt
cluster-detail-set = Setzen
cluster-detail-nixpkgs-label = Nixpkgs:
cluster-detail-nixpkgs-placeholder = Commit-SHA
cluster-detail-no-nixpkgs = Kein Nixpkgs-Pin
cluster-detail-cloud-init-title = Cloud-init für { $cluster }
cluster-detail-cloud-init-desc = Erzeugt einen neuen Sync-Token und eine einsatzbereite Cloud-Config. In die User-Data einer beliebigen VM einfügen.
cluster-detail-generating = Wird generiert…
cluster-detail-copied = Kopiert!
cluster-detail-copy = Kopieren
cluster-detail-download = Herunterladen

## ── Cluster Healer-Einstellungen ──────────────────────────────

healer-settings-override = Serverstandards überschreiben
healer-settings-using-defaults = (Serverstandards werden verwendet)
healer-settings-auto-trigger = Automatisch auslösen
healer-settings-auto-trigger-model = Modell für automatische Auslösung
healer-settings-server-default = Serverstandard
healer-settings-auto-approve = Behebung automatisch genehmigen
healer-settings-fix-model = Fix-Modell (Behebung)
healer-settings-same-as-diagnosis = Wie Diagnose
healer-settings-saving = Wird gespeichert...
healer-settings-ollama = Ollama (lokal)
healer-settings-anthropic = Anthropic
healer-settings-openrouter = OpenRouter

## ── Cluster MCP Server ────────────────────────────────────────

cluster-mcp-direct = Direkte MCP Server
cluster-mcp-select = MCP Server auswählen...
cluster-mcp-local = Lokal
cluster-mcp-from-sc = Von { $name }
cluster-mcp-no-direct = Keine direkten MCP-Server-Zuweisungen.
cluster-mcp-via = über { $source }
cluster-mcp-overwritten = überschrieben
cluster-mcp-from-bundles = Aus Bundles
cluster-mcp-no-bundle-mcp = Keine MCP Server aus Bundles.
cluster-mcp-from-skills = Aus Skills (transitiv)
cluster-mcp-no-transitive = Keine transitiven MCP-Abhängigkeiten.
cluster-mcp-bundles-title = MCP Bundles
cluster-mcp-select-bundle = MCP Bundle auswählen...
cluster-mcp-no-bundle-assign = Keine MCP-Bundle-Zuweisungen.

## ── Cluster Skills ─────────────────────────────────────────────

cluster-skills-direct = Direkte Skills
cluster-skills-select = Skill/Channel auswählen...
cluster-skills-local = Lokal
cluster-skills-from-sc = Von { $name }
cluster-skills-no-direct = Keine direkten Skill-Zuweisungen.
cluster-skills-via = über { $source }
cluster-skills-from-bundles = Aus Bundles
cluster-skills-no-bundle-skills = Keine Skills aus Bundles.
cluster-skills-overwritten = überschrieben
cluster-skills-bundles-title = Bundles
cluster-skills-select-bundle = Bundle auswählen...
cluster-skills-no-bundle-assign = Keine Bundle-Zuweisungen.

## ── Cluster SSH Keys ───────────────────────────────────────────

ssh-keys-placeholder = ssh-ed25519 AAAA... user@host
ssh-keys-no-keys = Keine SSH Keys.

## ── Flotten-Dashboard ──────────────────────────────────────────

fleet-title = Flotten-Dashboard
fleet-last-refreshed = Zuletzt aktualisiert: { $time }
fleet-filtered-by = Gefiltert nach Rollout:
fleet-clear-filter = Filter zurücksetzen
fleet-no-daemons = Es haben sich noch keine Daemons gemeldet.
fleet-unhealthy-only = Nur fehlerhafte
fleet-col-cluster = Cluster
fleet-col-hostname = Hostname
fleet-col-env = Umgebung
fleet-col-status = Status
fleet-col-load = Last
fleet-col-services = Services
fleet-col-probes = Probes
fleet-col-tunnels = Tunnel
fleet-col-last-seen = Zuletzt gesehen
fleet-online = online
fleet-offline = offline
fleet-just-now = gerade eben
fleet-healthy = fehlerfrei
fleet-unhealthy = fehlerhaft
fleet-ok = ok
fleet-fail = fehlgeschlagen
fleet-pending = ausstehend
fleet-no-result = noch kein Ergebnis
fleet-idle = inaktiv

## ── Flottendetails ─────────────────────────────────────────────

fleet-detail-subtitle = { $cluster } · { $env } · v{ $version }
fleet-detail-instance-id = instance_id:
fleet-detail-last-heartbeat = Letzter Heartbeat: { $time }
fleet-detail-services = Services
fleet-detail-no-services = Keine Services gemeldet.
fleet-detail-col-service = Service
fleet-detail-col-upgrade = Upgrade
fleet-detail-col-busy = Beschäftigt
fleet-detail-tunnels = Tunnel
fleet-detail-col-port = Port
fleet-detail-config-files = Konfigurationsdateien
fleet-detail-shell-commands = Shell-Befehle
fleet-detail-logs = Logs
fleet-detail-healer-agent = Healer Agent
fleet-detail-probes = Probes
fleet-detail-no-probes = Noch keine Probe-Ergebnisse — der erste Durchlauf kann bis zu 15 Minuten dauern.
fleet-detail-col-kind = Art
fleet-detail-col-result = Ergebnis
fleet-detail-col-duration = Dauer
fleet-detail-col-tokens = Tokens
fleet-detail-col-model = Modell
fleet-detail-col-collected = Erfasst
fleet-detail-col-detail = Detail
fleet-detail-dynamic-sample = Dynamische Stichprobe
fleet-detail-no-sample = Keine Stichprobe im letzten Heartbeat.
fleet-detail-disks = Festplatten
fleet-detail-col-mount = Einhängepunkt
fleet-detail-col-free = Frei
fleet-detail-col-total = Gesamt
fleet-detail-col-used = Belegt
fleet-detail-inventory = Inventar
fleet-detail-inventory-collected = erfasst { $time }
fleet-detail-no-inventory = Noch kein Inventar-Snapshot vorhanden.
fleet-detail-nixpkgs-pin = Nixpkgs-Pin
fleet-detail-network = Netzwerkschnittstellen
fleet-detail-gpus = GPUs
fleet-detail-col-num = #
fleet-detail-col-vendor = Hersteller
fleet-detail-col-driver = Treiber
fleet-detail-col-vram = VRAM
fleet-detail-col-util = Auslastung
fleet-detail-col-temp = Temp
fleet-detail-col-power = Leistung
fleet-detail-col-pci = PCI
fleet-detail-col-gpu-index = #
fleet-detail-col-gpu-vendor = Hersteller
fleet-detail-col-gpu-driver = Treiber
fleet-detail-col-gpu-vram = VRAM
fleet-detail-col-gpu-util = Auslastung
fleet-detail-col-gpu-temp = Temp
fleet-detail-col-gpu-power = Leistung
fleet-detail-col-gpu-pci = PCI
fleet-detail-nixpkgs-commit = Nixpkgs-Commit
fleet-detail-open = Öffnen
fleet-detail-security = Sicherheitsstatus
fleet-detail-no-posture = Noch keine Sicherheitsdaten vorhanden.
fleet-detail-per-service = Details pro Service
fleet-detail-tab-inventory = Inventar
fleet-detail-tab-live = Livestatus
fleet-detail-tab-live-status = Livestatus
fleet-detail-tab-security = Sicherheit

## ── Dateieditor ────────────────────────────────────────────────

file-editor-unavailable = Dateieditor nicht verfügbar: { $error }
file-editor-loading = Dateieditor wird geladen...
file-editor-saved = Gespeichert
file-editor-conflict = Konflikt: Datei wurde auf der Festplatte geändert. Neu laden und erneut versuchen.
file-editor-back = Zurück zur Instanz
file-editor-title = Konfigurationsdateien
file-editor-disclaimer = Änderungen hier gelten nur für diese Instanz und werden nicht im Cluster synchronisiert. Verwenden Sie die Cluster-Konfiguration für Einstellungen, die für alle Instanzen einheitlich sein sollen.
file-editor-readonly = schreibgeschützt
file-editor-unsaved = ungespeicherte Änderungen
file-editor-saving = Wird gespeichert...
file-editor-binary = Binärdatei — zum Ansehen herunterladen
file-editor-select = Datei zum Anzeigen oder Bearbeiten auswählen
file-editor-no-files = Keine Konfigurationsdateien verfügbar

## ── Shell-Befehle ──────────────────────────────────────────────

shell-title = Shell-Befehle
shell-running = Wird ausgeführt...
shell-run = Ausführen
shell-arg-label = { $label }:

## ── Log-Ansicht ────────────────────────────────────────────────

log-title = Logs
log-all-services = Alle Services
log-stop = Stopp
log-start-tailing = Tailing starten
log-fetch-latest = Neueste abrufen
log-empty-hint = Klicken Sie auf 'Neueste abrufen' oder 'Tailing starten', um Logs anzuzeigen.

## ── Skill-Liste ────────────────────────────────────────────────

skill-list-title = Skills
skill-list-syncing = Wird synchronisiert...
skill-list-sync = Von xzar synchronisieren

## ── Skill-Details ──────────────────────────────────────────────

skill-detail-hide = Im öffentlichen Katalog ausblenden
skill-detail-channels = Channels
skill-detail-channels-synced = Channels werden von xzar synchronisiert.
skill-detail-no-channels = Noch keine Channels synchronisiert.
skill-detail-mcp-deps = MCP-Abhängigkeiten
skill-detail-select-mcp = MCP Server auswählen...
skill-detail-no-mcp-deps = Keine MCP-Abhängigkeiten.

## ── Bundle-Liste ───────────────────────────────────────────────

bundle-list-title = Bundles
bundle-list-new = Neues Bundle

## ── Bundle-Formular ────────────────────────────────────────────

bundle-form-title = Neues Bundle
bundle-form-slug-placeholder = my-bundle

## ── Bundle-Details ─────────────────────────────────────────────

bundle-detail-hide = Im öffentlichen Katalog ausblenden
bundle-detail-skill-channels = Skill Channels
bundle-detail-select-skill = Skill/Channel auswählen...

## ── MCP Server-Liste ───────────────────────────────────────────

mcp-server-list-title = MCP Server
mcp-server-list-new = Neuer MCP Server

## ── MCP Server-Details ─────────────────────────────────────────

mcp-server-slug-placeholder = my-server
mcp-server-config-json = Config JSON
mcp-server-hide = Im öffentlichen Katalog ausblenden
mcp-server-nix-deps = Nix-Abhängigkeiten
mcp-server-nix-placeholder = package-name
mcp-server-no-nix-deps = Keine Nix-Abhängigkeiten.
mcp-server-required-by = Benötigt von Skills
mcp-server-no-skills-depend = Kein Skill hängt von diesem MCP Server ab.
mcp-server-new-title = Neuer MCP Server
mcp-server-edit-title = MCP Server bearbeiten

## ── MCP Bundle-Liste ───────────────────────────────────────────

mcp-bundle-list-title = MCP Bundles
mcp-bundle-list-new = Neues MCP Bundle

## ── MCP Bundle-Formular ────────────────────────────────────────

mcp-bundle-form-title = Neues MCP Bundle
mcp-bundle-slug-placeholder = my-mcp-bundle

## ── MCP Bundle-Details ─────────────────────────────────────────

mcp-bundle-detail-hide = Im öffentlichen Katalog ausblenden
mcp-bundle-detail-mcp-servers = MCP Server
mcp-bundle-detail-select-mcp = MCP Server auswählen...

## ── Rollout-Liste ──────────────────────────────────────────────

rollout-list-title = Rollouts
rollout-list-manage-groups = Gruppen verwalten
rollout-list-new = Neues Rollout
rollout-list-no-rollouts = Noch keine Rollouts vorhanden.
rollout-list-col-stages = Stufen
rollout-list-col-health = Zustand
rollout-status-rolling = läuft
rollout-status-completed = abgeschlossen
rollout-status-paused = pausiert
rollout-status-failed = fehlgeschlagen
rollout-status-pending = ausstehend
rollout-health-pass = bestanden
rollout-health-fail = fehlgeschlagen
rollout-health-grace = Karenzzeit
rollout-health-no-data = keine Daten
rollout-health-tooltip = { $evaluated } Stufe(n) ausgewertet, { $failing } fehlgeschlagen
rollout-health-tooltip-summary = { $evaluated } Stufe(n) ausgewertet, { $failing } fehlgeschlagen — { $summary }

## ── Rollout-Formular ───────────────────────────────────────────

rollout-form-title = Neues Versions-Rollout
rollout-form-name-label = Name (optional)
rollout-form-name-placeholder = z.B. v0.2.0 Rollout, Nixpkgs-Sicherheitsupdate
rollout-form-name-help = Ein kurzes Label zur Identifizierung dieses Rollouts. Wird in der Listen- und Detailansicht angezeigt.
rollout-form-target-version = Zielversion
rollout-form-none-nixpkgs = — keine (nur Nixpkgs) —
rollout-form-no-versions = Keine Daemon-Versionen verfügbar. Synchronisieren Sie diese auf der Daemon-Versionen-Seite.
rollout-form-version-help = Wählen Sie eine über xzar hochgeladene Version. Downgrades sind gesperrt.
rollout-form-nixpkgs-label = Nixpkgs-Commit (optional)
rollout-form-nixpkgs-placeholder = z.B. 170a4b510ad7ee95dde01adf2fe21704498dbb5c
rollout-form-nixpkgs-help = Nixpkgs-Quelle auf diesen Commit festlegen. Leer lassen, um den bestehenden Pin jedes Clusters beizubehalten.
rollout-form-stages-label = Stufen (in Reihenfolge auswählen)
rollout-form-all-clusters = Alle Cluster
rollout-form-stage-num = (Stufe { $num })
rollout-form-create-groups-prefix = Gruppen erstellen
rollout-form-create-groups-suffix = { " " }um stufenweise auszurollen.
rollout-form-health-gate = Zustandsprüfung
rollout-form-auto-pause = Stufen automatisch pausieren, wenn Prüfdaten unter Schwellenwerte fallen
rollout-form-heartbeat-pct = Heartbeat-Aktualität % (min)
rollout-form-heartbeat-window = Heartbeat-Aktualitätsfenster (Sek.)
rollout-form-grace-period = Karenzzeit nach Start (Sek.)
rollout-form-probe-thresholds = Probe-Erfolgsschwellenwerte
rollout-form-service-placeholder = Service (z.B. ollama)
rollout-form-pct-symbol = %
rollout-form-remove-service = entfernen
rollout-form-add-service = + Service hinzufügen
rollout-form-gate-help = Eine Stufe scheitert an der Prüfung, wenn ein aufgeführter Service in den letzten 30 Min. unter den Schwellenwert fällt.
rollout-form-create = Rollout erstellen

## ── Rollout-Details ────────────────────────────────────────────

rollout-detail-health = Rollout-Zustand
rollout-detail-stages-failing = { $failing }/{ $total } Stufen fehlgeschlagen
rollout-detail-stages-passing = { $evaluated }/{ $total } Stufen bestanden
rollout-detail-cohort = Kohorte:{ " " }
rollout-detail-heartbeats = Heartbeats aktuell:{ " " }
rollout-detail-service-probe = { $service }:{ " " }
rollout-detail-top-reason = Hauptgrund: { $reason }
rollout-detail-rollout-prefix = Rollout { $id }
rollout-detail-created-label = Erstellt: { $date }
rollout-detail-rollback = Rollback
rollout-detail-rollback-confirm = Dieses Rollout zurücksetzen? Die pinned_version und der nixpkgs_commit jedes Kohorten-Clusters werden auf den beim Start erfassten Stand zurückgesetzt. Daemons, die bereits die neue Version übernommen haben, führen beim nächsten Tick ein Downgrade durch.
rollout-detail-delete-confirm = Dieses Rollout löschen? Die Rollout-Zeile und der Stufenverlauf werden entfernt. Cluster-Pins bleiben unverändert — dies bereinigt nur den Rollout-Datensatz.
rollout-detail-start = Rollout starten
rollout-detail-advance = Zur nächsten Stufe
rollout-detail-pause = Pausieren
rollout-detail-resume = Fortsetzen
rollout-detail-complete-all = Alle abschließen
rollout-detail-complete-confirm = Dieses Rollout jetzt abschließen? Jede Stufe wird als abgeschlossen markiert und die Zielversion wird in die pinned_version jedes Kohorten-Clusters geschrieben, einschließlich nie ausgerollter Stufen. Überspringt die Zustandsprüfung.
rollout-detail-stages = Stufen
rollout-detail-stage-num = Stufe { $num }
rollout-detail-started = Gestartet: { $date }
rollout-detail-completed = Abgeschlossen: { $date }
rollout-detail-online = { $healthy }/{ $total } online
rollout-detail-no-heartbeats = keine Heartbeats
rollout-detail-version-progress = { $upgraded }/{ $total } Version
rollout-detail-nixpkgs-progress = { $upgraded }/{ $total } Nixpkgs
rollout-detail-no-gate = Keine Zustandsprüfung konfiguriert.
rollout-detail-view-fleet = Flotte anzeigen
rollout-detail-add-gate = Prüfung hinzufügen
rollout-detail-assessment-gate = Bewertungsprüfung
rollout-detail-gate-pass = Prüfung: bestanden
rollout-detail-gate-fail = Prüfung: fehlgeschlagen
rollout-detail-gate-grace = Prüfung: Karenzzeit
rollout-detail-gate-no-data = Prüfung: keine Daten
rollout-detail-evaluated = ausgewertet { $ts }
rollout-detail-edit-gate = Prüfung bearbeiten
rollout-detail-reevaluating = Wird neu ausgewertet…
rollout-detail-reevaluate-now = Jetzt neu auswerten
rollout-detail-request-assessment = Neue Bewertung anfordern
rollout-detail-requesting = wird angefordert…
rollout-detail-no-clusters = keine Cluster in der Kohorte
rollout-detail-no-daemons = 0 von { $cohort } Daemons erreichbar — keiner hat derzeit eine aktive SSE-Verbindung
rollout-detail-pushed = an { $dispatched } von { $cohort } Daemons gesendet (Ergebnisse in ca. 30 Sek.)
rollout-detail-trigger-self-update = Self-Update auslösen
rollout-detail-triggering = wird ausgelöst…
rollout-detail-push-result = an { $dispatched } von { $cohort } Daemons gesendet
rollout-detail-push-none = 0 von { $cohort } Daemons erreichbar
rollout-detail-trigger-sync-nixpkgs = Nixpkgs-Sync auslösen
rollout-detail-gate-config = Prüfungskonfiguration
rollout-detail-gate-enabled = Prüfung aktiviert (deaktiviert = keine Prüfung, immer bestanden)
rollout-detail-freshness-window = Aktualitätsfenster (Sek.)
rollout-detail-grace-period = Karenzzeit (Sek.)
rollout-detail-apply-all = Diese Prüfung auf alle Stufen dieses Rollouts anwenden
rollout-detail-target = Ziel
rollout-detail-version-label = Version:{ " " }
rollout-detail-nixpkgs-label = Nixpkgs-Commit:{ " " }
rollout-detail-rollback-baseline = Rollback-Ausgangszustand
rollout-detail-samples = Stichproben:{ " " }
rollout-detail-cpu-load = CPU-Last:{ " " }
rollout-detail-mem = Speicher:{ " " }
rollout-detail-max-disk = Max. Festplatte:{ " " }
rollout-detail-gpu-util = GPU-Auslastung:{ " " }
rollout-detail-thermal-alerts = Temperaturwarnungen: { $count }
rollout-detail-col-service = Service
rollout-detail-col-ok-total = OK / Gesamt
rollout-detail-col-avg-ms = Durchschn. ms
rollout-detail-col-ttft = TTFT
rollout-detail-col-tokens-out = Tokens out
rollout-detail-col-last-failure = Letzter Fehler
rollout-status-rolled-back = zurückgesetzt

## ── Rollout-Gruppenliste ───────────────────────────────────────

rollout-group-list-title = Rollout-Gruppen
rollout-group-list-create = Gruppe erstellen
rollout-group-list-name-placeholder = Gruppenname
rollout-group-list-desc-placeholder = Beschreibung
rollout-group-list-col-members = Mitglieder

## ── Rollout-Gruppendetails ─────────────────────────────────────

rollout-group-cannot-delete = Die Gruppe „Alle Cluster" kann nicht gelöscht werden
rollout-group-delete = Gruppe löschen
rollout-group-members = Mitglieder
rollout-group-select-cluster = Cluster zum Hinzufügen auswählen...
rollout-group-add-all = Alle Cluster hinzufügen
rollout-group-no-members = Noch keine Mitglieder.
rollout-group-col-cluster = Cluster

## ── Admin Tokens ───────────────────────────────────────────────

admin-tokens-title = Admin Tokens
admin-tokens-description = Admin Tokens sind nicht auf einen Cluster beschränkt. Sie können alle Cluster auflisten und Sync-/Einstellungs-Tokens für jeden Cluster erstellen.
admin-token-new = Neuer Token (jetzt kopieren, wird nur einmal angezeigt):
admin-token-label-placeholder = Admin-Token-Label
admin-token-create = Admin Token erstellen
admin-token-revoke = Widerrufen

## ── Federation Tokens ──────────────────────────────────────────

federation-tokens-title = Federation Tokens
federation-tokens-description = Federation Tokens ermöglichen entfernten Management-Servern den Zugriff auf den Skill-Center-Katalog dieser Instanz und das Auflösen von Skills. Teilen Sie diese mit Management-Servern, die von diesem Skill Center beziehen.
federation-token-new = Neuer Federation Token (jetzt kopieren, wird nur einmal angezeigt):
federation-token-label = Federation-Token-Label
federation-token-create = Federation Token erstellen
federation-token-none = Noch keine Federation Tokens vorhanden.
federation-token-revoke = Widerrufen

## ── Sync Tokens ────────────────────────────────────────────────

sync-token-new = Neuer Token (jetzt kopieren, wird nur einmal angezeigt):
sync-token-label = Sync-Token-Label
sync-token-create = Sync Token erstellen
sync-token-expired = abgelaufen { $date }
sync-token-expires = läuft ab { $date }
sync-token-revoke = Widerrufen

## ── Einstellungs-Tokens ────────────────────────────────────────

setting-token-new = Neuer Token (jetzt kopieren, wird nur einmal angezeigt):
setting-token-label = Einstellungs-Token-Label
setting-token-create = Einstellungs-Token erstellen
setting-token-expired = abgelaufen { $date }
setting-token-expires = läuft ab { $date }
setting-token-revoke = Widerrufen

## ── Organisationsliste ─────────────────────────────────────────

org-list-title = Organisationen
org-list-new = Neue Organisation
org-list-col-members = Mitglieder
org-list-col-clusters = Cluster

## ── Organisationsformular ──────────────────────────────────────

org-form-title = Neue Organisation
org-form-name-required = Name ist erforderlich
org-form-name-placeholder = Organisationsname

## ── Organisationsdetails ───────────────────────────────────────

org-detail-created = Erstellt { $date }
org-detail-confirm = Sind Sie sicher?
org-detail-confirm-delete = Löschen bestätigen
org-detail-delete = Organisation löschen
org-detail-members = Mitglieder
org-detail-select-user = Benutzer zum Hinzufügen auswählen...
org-detail-role-read = Lesen
org-detail-role-write = Schreiben
org-detail-role-admin = Admin
org-detail-no-members = Noch keine Mitglieder.
org-detail-col-email = E-Mail
org-detail-col-name = Name
org-detail-col-role = Rolle
org-detail-clusters = Cluster
org-detail-select-cluster = Cluster zum Hinzufügen auswählen...
org-detail-no-clusters = Noch keine Cluster.
org-detail-tokens = Tokens
org-detail-token-label-placeholder = Token-Label...
org-detail-create-token = Token erstellen
org-detail-token-created = Token erstellt! Jetzt kopieren — wird nicht erneut angezeigt.
org-detail-no-tokens = Noch keine Tokens.

## ── Benutzerliste ──────────────────────────────────────────────

user-list-title = Benutzer
user-list-new = Neuer Benutzer
user-list-col-orgs = Organisationen

## ── Benutzerformular ───────────────────────────────────────────

user-form-title = Neuer Benutzer
user-form-email-placeholder = benutzer@beispiel.de
user-form-name-placeholder = Anzeigename

## ── Benutzerdetails ────────────────────────────────────────────

user-detail-col-email = E-Mail
user-detail-col-name = Name
user-detail-created = Erstellt { $date }
user-detail-impersonate = Identitätswechsel
user-detail-confirm = Sind Sie sicher?
user-detail-yes-delete = Ja, löschen
user-detail-delete = Benutzer löschen
user-detail-orgs = Organisationen
user-detail-select-org = Organisation zum Hinzufügen auswählen...
user-detail-no-orgs = Kein Mitglied einer Organisation.
user-detail-col-org = Organisationsname
user-detail-col-role = Rolle

## ── Skill Center-Liste ─────────────────────────────────────────

skill-center-list-title = Skill Centers
skill-center-list-new = Neues Skill Center
skill-center-list-none = Keine Skill Centers registriert.
skill-center-list-col-url = URL
skill-center-list-col-priority = Priorität
skill-center-list-col-enabled = Aktiviert

## ── Skill Center-Formular ──────────────────────────────────────

skill-center-form-title = Neues Skill Center
skill-center-form-name-required = Name ist erforderlich
skill-center-form-url-required = URL ist erforderlich
skill-center-form-token-required = Federation Token ist erforderlich
skill-center-form-name-placeholder = Mein Skill Center
skill-center-form-url-label = URL
skill-center-form-url-placeholder = https://skills.example.com:7378
skill-center-form-token-label = Federation Token
skill-center-form-token-placeholder = fed_...
skill-center-form-priority-label = Priorität
skill-center-form-priority-help = Höhere Priorität gewinnt bei Slug-Kollisionen zwischen Skill Centers.
skill-center-form-enabled = Aktiviert

## ── Skill Center-Details ───────────────────────────────────────

skill-center-detail-url = URL
skill-center-detail-priority = Priorität
skill-center-detail-enabled = Aktiviert
skill-center-detail-created = Erstellt
skill-center-detail-updated = Aktualisiert
skill-center-detail-token-label = Federation Token
skill-center-detail-token-hint = Leer lassen, um den aktuellen Token beizubehalten
skill-center-detail-catalog = Zwischengespeicherter Katalog
skill-center-detail-syncing = Wird synchronisiert...
skill-center-detail-sync-now = Jetzt synchronisieren
skill-center-detail-no-catalog = Noch keine Katalogdaten zwischengespeichert. Der Katalog wird automatisch abgerufen.
skill-center-detail-skill-channels = Skill Channels
skill-center-detail-bundles = Bundles
skill-center-detail-mcp-servers = MCP Server
skill-center-detail-mcp-bundles = MCP Bundles
skill-center-detail-last-synced = Zuletzt synchronisiert: { $time }
skill-center-detail-catalog-error = Katalogzusammenfassung konnte nicht geladen werden: { $error }
skill-center-detail-loading-catalog = Katalog wird geladen...
skill-center-detail-not-found = Skill Center nicht gefunden.

## ── Daemon-Versionsliste ───────────────────────────────────────

daemon-version-list-title = Daemon-Versionen
daemon-version-list-syncing = Wird synchronisiert...
daemon-version-list-sync = Von xzar synchronisieren
daemon-version-list-none = Noch keine Daemon-Versionen. Führen Sie xzar.sh aus, um Binaries hochzuladen, und klicken Sie dann auf Synchronisieren.
daemon-version-list-col-added = Hinzugefügt

## ── Daemon-Versionsdetails ─────────────────────────────────────

daemon-version-detail-title = Daemon { $version }
daemon-version-detail-all = ← Alle Versionen
daemon-version-detail-no-paths = Keine Store-Pfade in xzar für diese Version gefunden.
daemon-version-detail-col-system = System
daemon-version-detail-col-store-path = Store-Pfad
daemon-version-detail-clusters = Cluster mit dieser Version
daemon-version-detail-no-daemons = Keine Daemons melden diese Version.
daemon-version-detail-col-cluster = Cluster
daemon-version-detail-col-instances = Instanzen
daemon-version-detail-rollouts = Rollouts für diese Version
daemon-version-detail-no-rollouts = Keine Rollouts zielen auf diese Version ab.
daemon-version-detail-col-rollout = Rollout
daemon-version-detail-pinned = Cluster mit dieser Version gepinnt
daemon-version-detail-no-pinned = Keine Cluster auf diese Version gepinnt.

## ── Healer-Seite ───────────────────────────────────────────────

healer-title = Healer Agent
healer-instance = Instanz: { $instance_id } ({ $hostname })
healer-unhealthy = Fehlerhafte Services: { $services }
healer-settings = Einstellungen
healer-new-session = Neue Sitzung
healer-model = Modell
healer-ollama-free = Ollama (lokal, kostenlos)
healer-anthropic-cloud = Anthropic (Cloud)
healer-openrouter-cloud = OpenRouter (Cloud)
healer-ollama-hint = Kostenlos ausführbar, aber lokale Modelle sind weniger leistungsfähig als Cloud-Modelle.
healer-openrouter-hint = Nutzt OpenRouter-API-Guthaben. Unterliegt dem Token-Budget.
healer-anthropic-hint = Nutzt Anthropic-API-Guthaben. Leistungsfähiger, unterliegt dem Token-Budget.
healer-fix-model = Fix-Modell (Behebung)
healer-fix-model-hint = Optional: ein anderes Modell für die Behebungsphase nach der Diagnose verwenden.
healer-same-as-diagnosis = Wie Diagnosemodell
healer-instructions-placeholder = Optionale Anweisungen (leer lassen für automatische Diagnose)...
healer-auto-approve = Behebung automatisch genehmigen
healer-skip-approval = (Genehmigungsschritt zwischen Diagnose und Behebung überspringen)
healer-start = Heilung starten
healer-pause = Pausieren
healer-cancel-session = Abbrechen
healer-resume = Fortsetzen
healer-back-to-sessions = Zurück zu Sitzungen
healer-thinking = Agent denkt nach...
healer-previous-sessions = Vorherige Sitzungen
healer-approval-pending = Genehmigung ausstehend
healer-auto = auto
healer-auto-approve-label = auto-genehmigen
healer-view = Anzeigen
healer-session-title = Healer-Sitzung
healer-session-subtitle = Instanz: { $instance_id } — Sitzung: { $session_id }
healer-auto-triggered = automatisch ausgelöst
healer-more-tokens = Mehr Tokens (1M)
healer-paused-by-user = vom Benutzer pausiert
healer-token-budget = Token-Budget überschritten
healer-proxy-expiring = Proxy-Token läuft ab
healer-server-shutdown = Server heruntergefahren
healer-state-initializing = Initialisierung
healer-state-diagnosing = Diagnose
healer-state-remediating = Behebung
healer-state-verifying = Überprüfung
healer-state-done = Fertig
healer-state-failed = Fehlgeschlagen
healer-state-cancelled = Abgebrochen
healer-state-paused = Pausiert
healer-state-awaiting-approval = Wartet auf Genehmigung
healer-state-awaiting-retry = Wartet auf Wiederholung
healer-state-needs-human = Menschliche Aufmerksamkeit erforderlich
healer-state-unknown = Unbekannt
healer-event-state-change = Statuswechsel
healer-event-system = System
healer-event-agent = Agent
healer-event-user = Benutzer
healer-event-summary = Zusammenfassung
healer-event-other = Sonstiges
healer-event-error = Fehler
healer-phase-diagnosis = Diagnose
healer-phase-diagnosis-short = D
healer-phase-d = D
healer-phase-remediation = Behebungsplan
healer-phase-remediation-short = R
healer-phase-r = R
healer-phase-final = Abschlussbericht
healer-phase-final-short = F
healer-phase-f = F
healer-phase-final-report = Abschlussbericht

## ── Konfigurationseditor ───────────────────────────────────────

config-editor-raw-json = Roh-JSON
config-editor-paste-placeholder = JSON-Konfiguration hier einfügen...
config-editor-schema-error = Schema konnte nicht geladen werden: { $error }
config-editor-loading-schema = Schema wird geladen...
config-editor-save = Konfiguration speichern
config-editor-last-saved = Zuletzt gespeichert: { $time }
config-editor-no-config = Noch keine Konfiguration gespeichert.
config-editor-key-hash = key_hash
config-editor-key-hash-help = Hex-kodierter Multihash des API-Schlüssels (der Rohschlüssel wird nur einmal bei der Erstellung angezeigt)
config-editor-generate = Generieren
config-editor-key-warning = Diesen Schlüssel jetzt speichern — er wird nicht erneut angezeigt:
config-editor-dismiss = Schließen
config-editor-add-entry = + Eintrag hinzufügen
config-editor-reset-default = auf Standard zurücksetzen
config-editor-add-item = Element hinzufügen...
config-editor-select = -- auswählen --
config-editor-secret-placeholder = secret:NAME oder Rohwert
config-editor-secret-hint = Nutze secret:NAME um ein Vault-Secret zu referenzieren, oder gib einen Rohwert ein
config-editor-convert-to-secret = In Vault verschieben
config-editor-converting = Wird konvertiert...

## ── Konfigurationsverlauf ──────────────────────────────────────

config-history-left = Links (älter)
config-history-right = Rechts (neuer)
config-history-select = Version auswählen...
config-history-compare = Vergleichen
config-history-no-history = Noch kein Konfigurationsverlauf.
config-history-insert = eingefügt
config-history-delete = gelöscht
config-history-equal = gleich

## ── Extra-Config-Dialog ────────────────────────────────────────

extra-config-label = extra_config
extra-config-help = Beliebige openclaw.json-Schlüssel, die nach typisierten Feldern zusammengeführt werden.
extra-config-values-set = { $count } Wert(e) gesetzt
extra-config-edit = extra_config bearbeiten…
extra-config-title = openclaw extra_config bearbeiten
extra-config-filter = Nach Pfad filtern… (leerzeichengetrennte Begriffe in Reihenfolge)
extra-config-legend = Legende:
extra-config-sensitive = sensibel
extra-config-array-of-objects = Array von Objekten
extra-config-string-map = String-Schlüssel-Map
extra-config-type = Typ
extra-config-active-union = aktiver Union-Modus
extra-config-schema-error = Schema konnte nicht geladen werden: { $error }
extra-config-loading-schema = Schema wird geladen…
extra-config-key = Schlüssel
extra-config-add = + hinzufügen
extra-config-add-item = + Element hinzufügen

## ── Generieren-Schaltfläche ────────────────────────────────────

generate-generating = Wird generiert...
generate-with-ai = Mit KI generieren

## ── Alle-Generieren-Schaltfläche ───────────────────────────────

generate-all-generating = Wird generiert...
generate-all-done = Alle haben Beschreibungen
generate-all-pending = Alle generieren ({ $count })
generate-all-progress = { $done } / { $total }

## ── Tabellenhilfen ─────────────────────────────────────────────

table-via = über { $source }
table-version-prefix = v{ $version }
table-showing-filtered = Zeige { $shown } von { $filtered } (gefiltert aus { $total })
table-showing = Zeige { $shown } von { $total }
table-per-page-20 = 20 pro Seite
table-per-page-50 = 50 pro Seite
table-per-page-100 = 100 pro Seite

## ── Versteckt-Badge ────────────────────────────────────────────

hidden-badge = Versteckt
hidden-col-visibility = Sichtbarkeit { $indicator }

## ── Push-Menü ──────────────────────────────────────────────────

push-menu-button = Push ↓
push-sync-config = Config synchronisieren
push-sync-config-desc = Konfiguration an Daemons senden
push-sync-skills = Skills synchronisieren
push-sync-skills-desc = Skill-Zuweisungen senden
push-sync-mcp = MCP Server synchronisieren
push-sync-mcp-desc = MCP-Server-Zuweisungen senden
push-sync-ssh = SSH Keys synchronisieren
push-sync-ssh-desc = SSH-Key-Änderungen senden
push-sync-nixpkgs = Nixpkgs synchronisieren
push-sync-nixpkgs-desc = Nixpkgs-Pin senden
push-self-update = Self-Update
push-self-update-desc = Daemon-Binary-Update auslösen
push-request-assessment = Bewertung anfordern
push-request-assessment-desc = Sofortige Zustandsprüfung auslösen

## ── Schnellzugriff ─────────────────────────────────────────────

easy-access-title = Schnellzugriff
easy-access-no-nodes = Keine Online-Knoten mit konfigurierten Tunneln.
easy-access-files = Dateien
easy-access-shell = Shell

## ── Dokumentation ──────────────────────────────────────────────

docs-title = Dokumentation
docs-user-guides = Benutzerhandbücher
docs-administration = Administration
docs-none-prefix = Keine Dokumentationsseiten gefunden. Fügen Sie
docs-none-md = .md
docs-none-suffix = server/docs/
docs-back = ← Zurück zur Dokumentation
docs-badge-admin = Admin
docs-badge-user = Benutzer

## ── Mitarbeiter-Pings ──────────────────────────────────────────

staff-pings-title = Mitarbeiter-Pings
staff-pings-description = Handlungsrelevante Benachrichtigungen vom Healer Agent.
staff-pings-open = Offen ({ $count })
staff-pings-no-open = Keine offenen Mitarbeiter-Pings
staff-pings-resolved = Gelöst ({ $count })
staff-pings-view-session = Sitzung anzeigen
staff-pings-resolve = Lösen
staff-pings-resolved-by = Gelöst von { $by }
staff-pings-loading = Mitarbeiter-Pings werden geladen...

## ── Flotten-Dashboard Erreichbarkeit ───────────────────────────

fleet-reachable-zero = 0 von { $total } Daemons erreichbar
fleet-reachable-zero-hint = 0 von { $total } Daemons erreichbar — keiner hat derzeit eine aktive SSE-Verbindung

## ── Cluster-Konfigurationsseite ────────────────────────────────────

cluster-config-back = Zurück zum Cluster
cluster-detail-open-config = Konfiguration & Secrets öffnen

## ── Secrets ────────────────────────────────────────────────────────

secrets-title = Secret-Vault
secrets-description = Secrets werden verschlüsselt gespeichert und können in Konfigurationsfeldern als secret:NAME referenziert werden.
secrets-empty = Noch keine Secrets gespeichert.
secrets-col-name = Name
secrets-col-reference = Konfig-Referenz
secrets-col-value = Wert
secrets-col-created = Erstellt
secrets-col-actions = Aktionen
secrets-add = Secret hinzufügen
secrets-update = Aktualisieren
secrets-new-value = Neuer Wert...
secrets-value-placeholder = Secret-Wert...
secrets-confirm-delete = Löschen?
