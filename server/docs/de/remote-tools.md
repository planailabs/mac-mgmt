---
audience: user
---

# Remote-Tools

mac-mgmt bietet browserbasierte Werkzeuge, um über das Relay mit einzelnen Daemon-Instanzen zu arbeiten. Alle Tools erreichen Sie, indem Sie im **Flotten-Dashboard** auf eine Instanz klicken.

## Voraussetzungen

Alle Remote-Tools setzen voraus:

- Auf der Instanz muss `relay.url` konfiguriert sein (siehe [Konfigurationsreferenz](/docs/configuration-reference))
- Die Instanz muss mit dem Relay verbunden sein (im Flotten-Dashboard sichtbar)
- Sie benötigen mindestens **Lese**-Zugriff auf den Cluster der Instanz (Schreibzugriff für die Dateibearbeitung)

Die Web-UI erzeugt automatisch einen kurzlebigen Proxy-Token (6 Stunden gültig), sobald Sie eine Remote-Tool-Seite öffnen. Eine manuelle Token-Verwaltung ist nicht erforderlich.

## Log-Ansicht

Die Log-Ansicht streamt Service-Logs einer Daemon-Instanz in Echtzeit.

### Verwendung

1. Öffnen Sie die Instanz-Detailseite über das Flotten-Dashboard
2. Wählen Sie den Tab **Logs**
3. Filtern Sie optional über das Dropdown nach Service
4. Klicken Sie auf **Tailing starten** für kontinuierliches Streaming oder auf **Neueste abrufen** für einen einmaligen Snapshot

Die Ansicht lädt die letzten 500 Logzeilen und fragt bei aktivem Tailing alle 2 Sekunden neue Zeilen ab. Klicken Sie auf **Stopp**, um das Tailing anzuhalten, oder auf **Leeren**, um die Ausgabe zurückzusetzen.

## Shell-Befehle

Auf der Seite Shell-Befehle führen Sie vordefinierte Befehle auf einer Daemon-Instanz aus. Die Befehle werden von den einzelnen verwalteten Services registriert und nach Service-Namen gruppiert.

### Verwendung

1. Öffnen Sie die Instanz-Detailseite über das Flotten-Dashboard
2. Wählen Sie den Tab **Shell**
3. Suchen Sie den gewünschten Befehl
4. Erfordert der Befehl ein Argument, füllen Sie das Eingabefeld aus
5. Klicken Sie auf **Ausführen**

Die Ausgabe wird in Echtzeit zurückgestreamt und in einer Terminal-Ansicht dargestellt. Die Befehle werden von den verwalteten Services des Daemons definiert — beliebige Shell-Befehle lassen sich über diese Oberfläche nicht ausführen.

## Dateieditor

Der Dateieditor ermöglicht die browserbasierte Bearbeitung von Konfigurationsdateien auf einer Daemon-Instanz. Er unterstützt sowohl Einzeldatei- als auch Verzeichnis-Dateitunnel.

### Verwendung

1. Öffnen Sie die Instanz-Detailseite über das Flotten-Dashboard
2. Wählen Sie den Tab **Dateien**
3. Wählen Sie im linken Bereich eine Datei aus oder klappen Sie ein Verzeichnis auf
4. Bearbeiten Sie den Dateiinhalt im rechten Bereich
5. Klicken Sie auf **Speichern**, um die Änderungen auf die Instanz zurückzuschreiben

### Funktionen

- **Konflikterkennung** — der Editor verfolgt die Änderungszeitpunkte der Dateien. Ändert sich die Datei zwischen Laden und Speichern auf der Festplatte, wird das Speichern mit einem Konfliktfehler abgelehnt. Laden Sie die Datei in diesem Fall neu und versuchen Sie es erneut.
- **Schreibgeschützte Dateien** — manche Dateitunnel sind als schreibgeschützt markiert und können über den Editor nicht gespeichert werden.
- **Binärdateien** — Binärdateien werden automatisch erkannt und als Platzhalter angezeigt, statt sie darzustellen.

### Wichtige Hinweise

Änderungen über den Dateieditor gelten **nur für die jeweilige Instanz** und werden nicht im Cluster synchronisiert. Verwenden Sie für Einstellungen, die auf allen Instanzen einheitlich sein sollen, stattdessen den Cluster-Konfigurationseditor. Cluster-weite Einstellungen finden Sie in der [Konfigurationsreferenz](/docs/configuration-reference).

## Verwandte Themen

- [Remote-SSH](/docs/remote-ssh) — Terminalzugriff per SSH über das Relay
- [Flottenüberwachung](/docs/fleet-monitoring) — Heartbeat-Status und Benachrichtigungen
- [Healer Agent](/docs/healer) — KI-gestützte automatische Diagnose und Behebung
