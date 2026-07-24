---
audience: user
---

# Memvault-Setup

Memvault ist ein verteilter p2p-Speicher, über den Cluster-Knoten KI-Kontext teilen (Konversationsverlauf, Embeddings, Dokumente). Er nutzt eine lokale redb-Datenbank für die persistente Speicherung und libp2p für die Peer-to-Peer-Replikation.

Memvault erfordert einen Daemon, der mit `--features memvault` kompiliert wurde.

## Memvault aktivieren

Setzen Sie `memvault.enabled` in Ihrer Cluster-Konfiguration auf `true`:

```json
{
  "memvault": {
    "enabled": true
  }
}
```

Mit den Standardwerten wird Memvault:

- Daten in `~/.local/share/memvault/` speichern
- den API-Server auf Port `8401` bereitstellen
- die Web-UI deaktiviert lassen
- keine Authentifizierung verlangen

## Konfigurationsfelder

| Feld | Standard | Beschreibung |
|-------|---------|-------------|
| `enabled` | `false` | Ob Memvault gestartet wird |
| `data_dir` | `"~/.local/share/memvault"` | Datenverzeichnis für die Speicherung (redb-Datenbank, Identität, Cluster-ID) |
| `cluster_id` | `""` | Base58-kodierte 32-Byte-Cluster-ID, oder `"auto"`, um sie beim ersten Start zu generieren |
| `bootstrap_peers` | `[]` | libp2p-Multiaddrs für den Kademlia-Bootstrap |
| `port` | `8401` | Port des API-Servers (`0` deaktiviert den API-Server vollständig) |

## Cluster-ID

Die `cluster_id` bestimmt, welche Knoten Daten miteinander teilen. Knoten mit derselben Cluster-ID bilden eine Replikationsgruppe.

- **Leerer String** (Standard): Der Knoten liest seine Cluster-ID aus `{data_dir}/cluster_id` auf der Festplatte. Existiert die Datei nicht, wird eine genullte ID verwendet.
- **`"auto"`**: erzeugt beim ersten Start eine zufällige 32-Byte-ID und speichert sie auf der Festplatte.
- **Expliziter Wert**: eine base58-kodierte 32-Byte-ID. Verwenden Sie diese Variante, damit garantiert alle Knoten eines Clusters dieselbe ID haben.

Setzen Sie bei Mehrknoten-Clustern auf allen Knoten dieselbe explizite `cluster_id`, damit sie sich gegenseitig als Peers erkennen.

## Bootstrap eines Mehrknoten-Clusters

1. **Wählen Sie eine Cluster-ID.** Erzeugen Sie eine mit:
   ```
   openssl rand 32 | basenc --base58
   ```

2. **Konfigurieren Sie alle Knoten** mit derselben Cluster-ID und denselben Bootstrap-Peers:
   ```json
   {
     "memvault": {
       "enabled": true,
       "cluster_id": "<base58-cluster-id>"
     },
     "relay": {
       "url": "wss://relay.plan.ai"
     }
   }
   ```

3. **Peer-Discovery.** Knoten finden einander über:
   - **mDNS** (standardmäßig aktiviert über `relay.mdns_enabled`): automatische Erkennung im lokalen Netzwerk.
   - **Relay-Circuit-Relay**: Knoten, die mit demselben Relay-Server verbunden sind, finden einander über das Gossip-Protokoll.
   - **Bootstrap-Peers**: explizite libp2p-Multiaddrs für die initialen Kademlia-Verbindungen. Nur nötig, wenn weder mDNS noch Relay verfügbar sind.

   In den meisten Setups genügt das aktivierte Relay — explizite Bootstrap-Peers sind nicht erforderlich.

4. **Prüfen Sie die Konnektivität.** Öffnen Sie die Instanz-Detailseite im Flotten-Dashboard. Memvault erscheint als integrierter Service mit:
   - Zustand (Store-Datei vorhanden + API antwortet)
   - Blockanzahl und Store-Größe
   - Cluster-ID und Bootstrap-Peers im Inventar

## API-Authentifizierung

Der API-Server lauscht auf localhost und ist mit einem Bearer-Token gesichert, das beim ersten Start automatisch generiert wird. Der Token liegt unter `{data_dir}/api.token` mit `0600`-Berechtigungen.

Der MCP Server (`plan-ai-memvault`) liest diese Token-Datei automatisch, um sich gegenüber der Daemon-API zu authentifizieren.

## Zustandsüberwachung

Der Daemon führt für Memvault als integrierten Service Health-Checks aus:

- **Synchroner Check**: prüft, ob `blocks.redb` im Datenverzeichnis existiert.
- **Asynchroner Check** (bei Port > 0): `GET /api/v1/health` am API-Server.

Der Zustand erscheint im Flotten-Dashboard neben den anderen verwalteten Services.

## Datenverwaltung

Memvault speichert alle Daten im konfigurierten `data_dir`:

| Datei | Zweck |
|------|---------|
| `blocks.redb` | Haupt-Blockspeicher (redb-Datenbank) |
| `cluster_id` | Gespeicherte Cluster-ID (hex-kodiert) |

Um das Memvault eines Knotens zurückzusetzen, stoppen Sie den Daemon und löschen Sie das Datenverzeichnis. Der Knoten beginnt beim nächsten Start von vorn.

Die vollständige Feldliste finden Sie in der [Konfigurationsreferenz](/docs/configuration-reference).
