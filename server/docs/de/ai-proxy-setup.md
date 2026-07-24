---
audience: user
---

# AI-Proxy einrichten

Der AI-Proxy stellt eine OpenAI-kompatible API (`/v1/chat/completions`, `/v1/models`) bereit, die vor lokalen LLM-Backends (Ollama, Unsloth) sitzt. Er übernimmt Authentifizierung und Token-Budgets pro Schlüssel und kann bei aktiviertem Relay Anfragen über Cluster-Peers lastverteilen.

## Proxy aktivieren

Setzen Sie `ai_proxy.enabled` in Ihrer Cluster-Konfiguration auf `true` und fügen Sie mindestens einen API-Schlüssel hinzu:

```json
{
  "ai_proxy": {
    "enabled": true,
    "keys": [
      {
        "name": "dev-team",
        "key_hash": "1220..."
      }
    ]
  }
}
```

Der Proxy lauscht standardmäßig auf `127.0.0.1:18900`. Ändern Sie bei Bedarf `host` und `port`.

## API-Schlüssel generieren

API-Schlüssel werden als SHA2-256-Multihashes gespeichert — der Server sieht oder speichert den Rohschlüssel nie. So erstellen Sie einen Schlüssel:

1. Generieren Sie einen zufälligen Schlüssel (jede Zeichenkette funktioniert):
   ```
   openssl rand -hex 32
   ```
   Das ergibt etwas wie `a1b2c3d4...` — mit Präfix versehen oder direkt verwenden.

2. Berechnen Sie den Multihash. Das Format ist `0x12` (SHA2-256-Code) + `0x20` (32-Byte-Länge) + SHA2-256-Digest, hex-kodiert:
   ```
   echo -n "your-api-key" | python3 -c "
   import hashlib, sys
   d = hashlib.sha256(sys.stdin.buffer.read()).digest()
   print(bytes([0x12, 0x20]).hex() + d.hex())
   "
   ```

3. Fügen Sie den Hash der Cluster-Konfiguration hinzu:
   ```json
   {
     "name": "my-key",
     "key_hash": "1220<sha256-hex>"
   }
   ```

4. Bewahren Sie den Rohschlüssel auf — er kann nicht aus dem Hash wiederhergestellt werden.

## Token-Budgets

Jeder Schlüssel kann ein gleitendes Token-Budget haben, das die Gesamt-Tokens (Input + Output) über einen Zeitraum begrenzt.

| Feld | Standard | Beschreibung |
|-------|---------|-------------|
| `token_budget` | `0` | Maximale Tokens im Zeitfenster. `0` = unbegrenzt |
| `budget_window` | `"24h"` | Fensterdauer (z. B. `"1h"`, `"7d"`) |

Überschreitet ein Schlüssel sein Budget, liefern Anfragen `429 Too Many Requests`, bis ältere Nutzung aus dem Zeitfenster herausfällt. Das Budget wird pro Schlüssel über alle Backends hinweg erfasst.

## Einen Schlüssel deaktivieren

Setzen Sie `enabled` beim Schlüsseleintrag auf `false`. Anfragen mit diesem Schlüssel erhalten `403 Forbidden`.

## Verfügbare Endpunkte

Alle Endpunkte außer `/health` erfordern `Authorization: Bearer <raw-key>`.

| Methode | Pfad | Beschreibung |
|--------|------|-------------|
| POST | `/v1/chat/completions` | Chat-Completion (Streaming und Nicht-Streaming) |
| GET | `/v1/models` | Modelle aller Backends auflisten |
| GET | `/v1/usage` | Token-Verbrauchsstatistik für den authentifizierten Schlüssel |
| GET | `/health` | Backend-Status (keine Authentifizierung erforderlich) |

## Backend-Routing

Der Proxy leitet Anfragen an das jeweils verfügbare lokale Backend weiter:

1. Ist Ollama aktiviert, wird es bevorzugt.
2. Ist nur Unsloth aktiviert, wird Unsloth verwendet.
3. Ist das Relay aktiv und `ai_proxy_distribution` aktiviert, vergleicht der Proxy die lokale Last mit den Cluster-Peers und leitet an den am wenigsten ausgelasteten Knoten weiter.

Das Feld `model` in der Anfrage wird an das Backend durchgereicht — der Proxy filtert nicht nach Modellname.

## Funktionale Proben

Bei aktiviertem Proxy führt der Daemon automatisch Folgendes aus:

- Alle ~60 Sekunden eine **Liveness-Probe** (`GET /health`).
- Alle ~30 Minuten eine **funktionale Probe**, die einen Canary-Prompt (`qwen3:0.6b`) mit einem internen Probe-Token durch `POST /v1/chat/completions` schickt. Das validiert die gesamte Pipeline vom Proxy über das Backend bis zur Modell-Inferenz.

Die Probe-Ergebnisse erscheinen im Flotten-Dashboard unter dem Zustandsstatus der Instanz.

## Test mit curl

```bash
# Health check (no auth)
curl http://127.0.0.1:18900/health

# List models
curl http://127.0.0.1:18900/v1/models \
  -H "Authorization: Bearer your-api-key"

# Chat completion
curl http://127.0.0.1:18900/v1/chat/completions \
  -H "Authorization: Bearer your-api-key" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "qwen3:0.6b",
    "messages": [{"role": "user", "content": "Hello"}],
    "max_tokens": 32
  }'

# Check usage
curl http://127.0.0.1:18900/v1/usage \
  -H "Authorization: Bearer your-api-key"
```

## Den Proxy über das Relay verfügbar machen

Ist das Relay konfiguriert, wird der Proxy-Port als TCP-Tunnel bereitgestellt. Entfernte Clients können sich über das Relay verbinden, ohne direkten Netzwerkzugriff auf die Instanz. Der Tunnel heißt `ai-proxy` und erscheint auf der Instanz-Detailseite.

Die vollständige Feldliste finden Sie in der [Konfigurationsreferenz](/docs/configuration-reference).
