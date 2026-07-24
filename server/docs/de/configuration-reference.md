---
audience: user
---

# Konfigurationsreferenz

Jeder Cluster hat eine JSON-Konfiguration, die über die Web-UI oder die Setting-API verwaltet wird. Dieses Dokument beschreibt alle verfügbaren Abschnitte und Felder.

## `daemon`

Steuert das Betriebsverhalten des Daemons selbst.

| Feld | Standard | Beschreibung |
|-------|---------|-------------|
| `update_interval` | `"1h"` | Wie oft nach Updates gesucht sowie Skills und MCP Server synchronisiert werden (z. B. `"30s"`, `"5m"`, `"1h"`) |
| `health_interval` | `"1m"` | Wie oft Health-Checks der verwalteten Services ausgeführt werden |
| `log_level` | `"info"` | Log-Detailgrad: `error`, `warn`, `info`, `debug` oder `trace` |
| `upgrade_window` | *keiner* | Zeitfenster für Upgrades im Format `HH:MM-HH:MM` (z. B. `"02:00-05:00"`). Weglassen, um Upgrades jederzeit zu erlauben |

## `global`

Übergeordnete Einstellungen, die festlegen, welche Standard-Provider aktiv sind. Einzelne Provider werden über ihr eigenes `enabled`-Flag ein- und ausgeschaltet.

| Feld | Standard | Beschreibung |
|-------|---------|-------------|
| `default_llm` | `"ollama"` | Standard-LLM-Backend: `ollama`, `lms`, `cloud` oder `none` |
| `default_agent` | `"openclaw"` | Standard-Agent-Provider: `openclaw`, `opencode` oder `none` |
| `agent_name` | *keiner* | Anzeigename dieses Agents |
| `user_name` | *keiner* | Anzeigename des Benutzers |

## `notifications`

Legt fest, wohin der Daemon Ereignisbenachrichtigungen sendet — über [Apprise](https://github.com/caronc/apprise)-URLs.

| Feld | Standard | Beschreibung |
|-------|---------|-------------|
| `urls` | `[]` | Apprise-Benachrichtigungs-URLs (z. B. `tgram://bot/chat`, `ntfy://host/topic`) |
| `events` | *alle* | Welche Ereignisse Benachrichtigungen auslösen. Optionen: `daemon_started`, `daemon_stopped`, `service_crashed`, `service_unhealthy`, `service_recovered`, `upgrade_installed`, `upgrade_failed` |

## `ollama`

Einstellungen für den lokalen Ollama-LLM-Server. Wird installiert und gestartet, wenn `enabled` auf `true` steht.

| Feld | Standard | Beschreibung |
|-------|---------|-------------|
| `enabled` | `true` | Ob Ollama installiert und gestartet wird |
| `host` | `"127.0.0.1"` | Listen-Adresse |
| `port` | `11434` | Listen-Port |
| `models` | `["phi4-mini", "qwen3.5", "Flux_AI/Flux_AI"]` | Modelle, die beim Start gezogen werden; mindestens eines ist erforderlich |
| `default_model` | `"phi4-mini"` | Standardmodell für OpenClaw |
| `flavour` | `"cpu"` | Paketvariante: `cpu`, `rocm` (AMD), `cuda` (NVIDIA) oder `vulkan` |
| `context_length` | `16384` | Kontextlänge, als Umgebungsvariable `OLLAMA_CONTEXT_LENGTH` übergeben |

## `lms`

Einstellungen für LM Studio. Wird installiert und gestartet, wenn `enabled` auf `true` steht.

| Feld | Standard | Beschreibung |
|-------|---------|-------------|
| `enabled` | `true` | Ob LM Studio installiert und gestartet wird |
| `host` | `"127.0.0.1"` | Listen-Adresse |
| `port` | `1234` | Listen-Port |
| `models` | `[]` | Modellbezeichner, die beim Start via `lms load` geladen werden |
| `default_model` | `"qwen2.5-coder-7b-instruct"` | Standard-Modellbezeichner für OpenClaw |

## `cloud`

Eine Liste von Cloud-LLM-Provider-Einträgen. Steht `global.default_llm` auf `"cloud"`, werden **alle** aktivierten Einträge gleichzeitig im Agent (OpenClaw oder OpenCode) konfiguriert. Das Modell des ersten aktivierten Eintrags dient als Standard. So können Sie mehrere Provider einrichten und direkt im Agent zwischen ihnen wechseln, ohne den Daemon neu zu konfigurieren.

Jeder Eintrag hat diese Felder:

| Feld | Standard | Beschreibung |
|-------|---------|-------------|
| `enabled` | `true` | Ob dieser Cloud-Provider-Eintrag aktiv ist |
| `provider` | `"anthropic"` | Cloud-Provider (siehe unterstützte Provider unten) |
| `api_key` | *keiner* | API-Schlüssel für den Provider |
| `default_model` | `"anthropic/claude-sonnet-4-6"` | Modellbezeichner im Format `provider/model` |
| `base_url` | *keine* | Eigene Basis-URL für Proxys oder Bedrock |
| `api` | *automatisch* | API-Typ-Override: `anthropic-messages`, `openai-completions`, `openai-responses`, `google-generative-ai`, `bedrock-converse-stream`, `ollama` |
| `auth` | *automatisch* | Authentifizierungsmodus: `api-key`, `aws-sdk`, `oauth`, `token` |

### Unterstützte Cloud-Provider

| Provider | Standardmodell | Umgebungsvariable |
|----------|---------------|---------------------|
| `anthropic` | `anthropic/claude-sonnet-4-6` | `ANTHROPIC_API_KEY` |
| `openai` | `openai/gpt-5.4` | `OPENAI_API_KEY` |
| `google` | `google/gemini-3-flash-preview` | `GEMINI_API_KEY` |
| `mistral` | `mistral/mistral-large-latest` | `MISTRAL_API_KEY` |
| `groq` | `groq/llama-4-scout-17b-16e-instruct` | `GROQ_API_KEY` |
| `xai` | `xai/grok-3-mini` | `XAI_API_KEY` |
| `deepseek` | `deepseek/deepseek-chat` | `DEEPSEEK_API_KEY` |
| `openrouter` | `openrouter/auto` | `OPENROUTER_API_KEY` |
| `together` | `together/meta-llama/Llama-4-Maverick-17B-128E-Instruct-Turbo` | `TOGETHER_API_KEY` |
| `bedrock` | `amazon-bedrock/us.anthropic.claude-sonnet-4-6-v1:0` | `AWS_ACCESS_KEY_ID` |

## `openclaw`

Einstellungen für den OpenClaw-Agent. Wird installiert und gestartet, wenn `enabled` auf `true` steht.

| Feld | Standard | Beschreibung |
|-------|---------|-------------|
| `enabled` | `true` | Ob OpenClaw installiert und gestartet wird |

### `openclaw.gateway`

| Feld | Standard | Beschreibung |
|-------|---------|-------------|
| `port` | `18789` | Listen-Port des Gateways |
| `host` | `"127.0.0.1"` | Listen-Adresse des Gateways |

### `openclaw.skills`

| Feld | Standard | Beschreibung |
|-------|---------|-------------|
| `auto_update` | `true` | Skills automatisch im Update-Intervall aktualisieren |

### `openclaw.telegram`

| Feld | Standard | Beschreibung |
|-------|---------|-------------|
| `bot_token` | *erforderlich* | Telegram-Bot-Token von @BotFather |
| `allowed_chat_ids` | `[]` | Erlaubte Telegram-Chat-IDs. Leer bedeutet: alle Chats sind erlaubt |
| `enabled` | `true` | Telegram-Integration aktivieren |

### `openclaw.extra_config`

Beliebige Schlüssel-Wert-Paare, die nach den typisierten Feldern in `~/.openclaw/openclaw.json` zusammengeführt werden. Verwenden Sie dies für OpenClaw-Einstellungen, die das typisierte Schema noch nicht abdeckt.

## `opencode`

Einstellungen für den OpenCode-Agent. Wird installiert und gestartet, wenn `enabled` auf `true` steht.

| Feld | Standard | Beschreibung |
|-------|---------|-------------|
| `enabled` | `false` | Ob OpenCode installiert und gestartet wird |
| `port` | `18790` | Listen-Port des Servers |
| `host` | `"127.0.0.1"` | Listen-Adresse des Servers |

### `opencode.extra_config`

Beliebige Schlüssel-Wert-Paare, die nach den typisierten Feldern in die OpenCode-Konfiguration zusammengeführt werden. Verwenden Sie dies für OpenCode-Einstellungen, die das typisierte Schema nicht abdeckt (z. B. `model`, `provider`).

## `metrics`

| Feld | Standard | Beschreibung |
|-------|---------|-------------|
| `port` | `9396` | Port des Prometheus-Metrics-Endpunkts |

## `relay`

Einstellungen für das p2p-Relay, das für Remote-SSH, Service-Tunnel und die Peer-Discovery im Cluster verwendet wird.

| Feld | Standard | Beschreibung |
|-------|---------|-------------|
| `relay_multiaddr` | *keine* | libp2p-Multiaddress des Relay-Knotens (z. B. `"/dns4/relay.example.com/tcp/4001/wss"`). Akzeptiert auch den Alias `url` (z. B. `wss://relay.plan.ai`) |
| `remote_ssh_enabled` | `false` | Ob Remote-SSH-Zugriff beim Start aktiviert ist |
| `tunnels_enabled` | `true` | Ob Service-Tunnel (TCP, Datei, Shell) über das Relay bereitgestellt werden |
| `fake_origin_local` | `true` | Host-/Referer-/Origin-Header beim Proxying von TCP-Tunneln umschreiben. Verhindert, dass Services wie Ollama Anfragen mit nicht-lokalen Origins ablehnen |
| `cluster_psk` | *keiner* | Pre-Shared Key des Clusters für die p2p-Peer-Authentifizierung (hex-kodiert, 32 Bytes). Wird vom Server generiert |
| `mdns_enabled` | `true` | mDNS-Peer-Discovery im lokalen Netzwerk (LAN) aktivieren |
| `p2p_port` | `1122` | QUIC-Listen-Port für p2p-Verbindungen |
| `ai_proxy_distribution` | `true` | Verteilung von AI-Proxy-Anfragen über Cluster-Peers aktivieren. Wenn aktiviert, verteilt der [AI-Proxy](/docs/ai-proxy-setup) die Last auf den am wenigsten ausgelasteten Knoten |

## `healer`

Einstellungen für den KI-gestützten Agent zur automatischen Behebung. Details zur Nutzung siehe [Healer](/docs/healer).

| Feld | Standard | Beschreibung |
|-------|---------|-------------|
| `auto_trigger` | *keiner* | Automatisch ausgelöste Healer-Sitzungen aktivieren, wenn ein Service dauerhaft fehlerhaft bleibt |
| `auto_trigger_provider` | *keiner* | Cloud-Provider für automatisch ausgelöste Sitzungen (z. B. `"anthropic"`) |
| `auto_trigger_model` | *keines* | Modell für automatisch ausgelöste Sitzungen (z. B. `"claude-sonnet-4-6"`) |
| `auto_approve` | *keiner* | Behebungsaktionen automatisch genehmigen (Genehmigungsschritt überspringen) |
| `fix_provider` | *keiner* | Provider des Fix-Modells für die Behebungsphase |
| `fix_model` | *keines* | Modell für die Behebungsphase |

## `backup`

Restic-basierte Backup-Konfiguration. Wenn aktiviert, sichert der Daemon regelmäßig Service-Daten und Konfiguration.

| Feld | Standard | Beschreibung |
|-------|---------|-------------|
| `enabled` | `false` | Ob Restic-Backups aktiviert sind |
| `repository` | `""` | Pfad oder URL des Restic-Repositorys (z. B. `/backup/restic`, `s3:bucket/prefix`, `sftp:host:/path`) |
| `password_file` | *keine* | Pfad zur Restic-Passwort-/Schlüsseldatei. Wird automatisch generiert, wenn nicht angegeben |
| `interval` | `"6h"` | Wie oft Backups laufen (z. B. `"6h"`, `"1d"`) |
| `keep_within` | `"7d"` | Aufbewahrungsrichtlinie: Snapshots aus der letzten Zeitspanne N behalten (z. B. `"7d"`, `"30d"`) |
| `extra_paths` | `[]` | Zusätzliche Pfade, die über die automatisch erkannten Service-Pfade hinaus gesichert werden |
| `exclude` | `[]` | Glob-Muster, die von Backups ausgeschlossen werden |
| `include_models` | `false` | Ob große Modell-Caches (Ollama-Modelle, LM-Studio-Cache) mitgesichert werden |
| `env` | `{}` | Umgebungsvariablen für Restic (z. B. `AWS_ACCESS_KEY_ID` für S3-Backends) |

## `custom-service`

Definiert virtuelle Services vollständig über die Konfiguration. Jeder Eintrag erzeugt einen Service, der Tunnel, Dateizugriff, Shell-Befehle, Probes, Inventar und Sicherheitschecks bereitstellen kann, ohne dass Code geschrieben werden muss. Angegeben als TOML-Array (`[[custom-service]]`) oder JSON-Array.

| Feld | Standard | Beschreibung |
|-------|---------|-------------|
| `name` | *erforderlich* | Service-Name, der als Bezeichner in Heartbeats, Tunneln usw. dient |
| `enabled` | `true` | Ob dieser benutzerdefinierte Service aktiv ist |
| `spawn` | *keiner* | Zu startender und zu überwachender Prozess (siehe unten). Fehlt der Eintrag, ist der Service integriert (kein Prozess) |
| `health_check` | *keiner* | Health-Check-Befehl. Exit-Code 0 = fehlerfrei |
| `tunnels` | `[]` | TCP-Ports, die über das Relay bereitgestellt werden |
| `files` | `[]` | Dateien oder Ordner, die für die Fernbearbeitung freigegeben sind |
| `commands` | `[]` | Vordefinierte Shell-Befehle für die Fernausführung |
| `probes` | `[]` | HTTP- oder Exec-Probes für funktionale Health-Checks |
| `inventory` | `[]` | Statische Inventareinträge, die periodisch erfasst werden |
| `samples` | `[]` | Dynamische Stichproben-Einträge, die bei jedem Heartbeat erfasst werden |
| `security` | `[]` | Sicherheitschecks (Exec- oder dateibasiert) |

### `custom-service.spawn`

| Feld | Standard | Beschreibung |
|-------|---------|-------------|
| `command` | *erforderlich* | Auszuführendes Programm (z. B. `"grafana-server"`) |
| `args` | `[]` | Feste Argumente, die dem Befehl übergeben werden |
| `env` | `{}` | Umgebungsvariablen für den gestarteten Prozess |

### `custom-service.tunnels[]`

| Feld | Standard | Beschreibung |
|-------|---------|-------------|
| `name` | *erforderlich* | Kurzer, URL-sicherer Name (z. B. `"grafana"`) |
| `host` | `"127.0.0.1"` | Host, auf dem der Service lauscht |
| `port` | *erforderlich* | TCP-Portnummer |

## `ai_proxy`

Ein OpenAI-kompatibler API-Proxy vor lokalen LLM-Backends (Ollama, Unsloth), der optional die Last über Cluster-Peers verteilt. Eine ausführliche Anleitung finden Sie unter [AI-Proxy-Setup](/docs/ai-proxy-setup).

| Feld | Standard | Beschreibung |
|-------|---------|-------------|
| `enabled` | `false` | Ob der AI-Proxy gestartet wird |
| `host` | `"127.0.0.1"` | Listen-Adresse |
| `port` | `18900` | Listen-Port |
| `keys` | `[]` | API-Schlüssel mit Token-Budget pro Schlüssel (siehe unten) |

### `ai_proxy.keys[]`

| Feld | Standard | Beschreibung |
|-------|---------|-------------|
| `name` | `""` | Menschenlesbares Label für diesen Schlüssel |
| `key_hash` | *erforderlich* | Hex-kodierter SHA2-256-Multihash des API-Schlüssels |
| `token_budget` | `0` | Maximale Gesamtzahl an Tokens (Input+Output) innerhalb des Budgetfensters. `0` = unbegrenzt |
| `budget_window` | `"24h"` | Dauer des gleitenden Fensters (z. B. `"24h"`, `"7d"`, `"1h"`) |
| `enabled` | `true` | Ob dieser Schlüssel aktiv ist |

## `memvault`

Verteilter p2p-Speicher für gemeinsamen KI-Kontext über Cluster-Knoten hinweg. Erfordert einen Daemon, der mit `--features memvault` kompiliert wurde. Eine ausführliche Anleitung finden Sie unter [Memvault-Setup](/docs/memvault-setup).

| Feld | Standard | Beschreibung |
|-------|---------|-------------|
| `enabled` | `false` | Ob Memvault gestartet wird |
| `data_dir` | `"~/.local/share/memvault"` | Datenverzeichnis für die Speicherung (redb, Identität) |
| `cluster_id` | `""` | Base58-kodierte 32-Byte-Cluster-ID, oder `"auto"`, um sie beim ersten Start zu generieren |
| `bootstrap_peers` | `[]` | libp2p-Multiaddrs für Kademlia-Bootstrap und initiale Verbindungen. Fügen Sie das Suffix `/p2p/<peer-id>` an, damit der Peer in die DHT-Routing-Tabelle aufgenommen wird, bevor Identify abgeschlossen ist |
| `port` | `8401` | Port des API-Servers (Bearer-Token wird automatisch in `data_dir/api.token` generiert) |
| `kad_server` | `false` | Kademlia-DHT in den Server-Modus zwingen. Bei `false` erkennt libp2p den Modus automatisch anhand bestätigter externer Adressen — ein Knoten hinter NAT kann dadurch als Client hängen bleiben und ist über die DHT nicht auffindbar. Auf öffentlich erreichbaren Knoten (Relays/Bootstrap) auf `true` setzen |
| `kad_bootstrap_interval_secs` | `0` | Sekunden zwischen Kademlia-Bootstrap-Runden (ein Self-Lookup, der die Routing-Tabelle auffrischt/erweitert). `0` deaktiviert den periodischen Bootstrap; bei `> 0` läuft zusätzlich ein initialer Bootstrap beim Start |

## Beispielkonfiguration

```json
{
  "daemon": {
    "update_interval": "5m",
    "log_level": "info",
    "upgrade_window": "02:00-05:00"
  },
  "global": {
    "default_llm": "ollama",
    "default_agent": "openclaw"
  },
  "ollama": {
    "enabled": true,
    "models": ["phi4-mini", "qwen3.5"],
    "default_model": "phi4-mini",
    "flavour": "cpu"
  },
  "openclaw": {
    "enabled": true
  },
  "cloud": [
    {
      "enabled": true,
      "provider": "anthropic",
      "api_key": "sk-ant-...",
      "default_model": "anthropic/claude-sonnet-4-6"
    },
    {
      "enabled": true,
      "provider": "openai",
      "api_key": "sk-...",
      "default_model": "openai/gpt-5.4"
    }
  ],
  "notifications": {
    "urls": ["ntfy://ntfy.example.com/alerts"]
  },
  "relay": {
    "relay_multiaddr": "wss://relay.plan.ai",
    "remote_ssh_enabled": true,
    "tunnels_enabled": true,
    "mdns_enabled": true
  },
  "healer": {
    "auto_trigger": true,
    "auto_trigger_provider": "anthropic",
    "auto_trigger_model": "claude-sonnet-4-6"
  },
  "backup": {
    "enabled": true,
    "repository": "/backup/restic",
    "interval": "6h",
    "keep_within": "7d"
  },
  "ai_proxy": {
    "enabled": true,
    "keys": [
      {
        "name": "dev-team",
        "key_hash": "1220...",
        "token_budget": 1000000,
        "budget_window": "24h"
      }
    ]
  },
  "memvault": {
    "enabled": true,
    "port": 8401
  },
  "metrics": {
    "port": 9396
  }
}
```
