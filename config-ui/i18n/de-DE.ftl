# Config-editor i18n keys, extracted (full multiline entries) from server/src/web/de-DE.ftl.
loading = Wird geladen…
save = Speichern
cancel = Abbrechen
cluster-detail-tab-config-history = Konfigurationsverlauf
config-editor-raw-json = Roh-JSON
config-editor-paste-placeholder = JSON-Konfiguration hier einfügen...
config-editor-schema-error = Schema konnte nicht geladen werden: { $error }
config-editor-loading-schema = Schema wird geladen...
config-editor-save = Konfiguration speichern
config-save-unsaved = Ungespeicherte Änderungen
config-save-summary = { $fields ->
    [one] { $fields } ungespeicherte Änderung
   *[other] { $fields } ungespeicherte Änderungen
} in { $sections ->
    [one] { $sections } Abschnitt
   *[other] { $sections } Abschnitten
}
config-save-discard = Verwerfen
config-save-draft = Entwurf
config-save-review-diff = Änderungen prüfen
config-save-review-diff-title = Ungespeicherte Änderungen prüfen
config-save-review-diff-help = Vergleich mit dem letzten Server-Stand. Klick außerhalb zum Schließen.
config-save-review-diff-empty = Keine textuellen Änderungen.
config-editor-last-saved = Zuletzt gespeichert: { $time }
config-editor-no-config = Noch keine Konfiguration gespeichert.
config-editor-key-hash = key_hash
config-editor-key-hash-help = Hex-kodierter Multihash des API-Schlüssels (der Rohschlüssel wird nur einmal bei der Erstellung angezeigt)
config-editor-generate = Generieren
config-editor-key-warning = Diesen Schlüssel jetzt speichern — er wird nicht erneut angezeigt:
config-editor-dismiss = Schließen
config-editor-add-entry = + Eintrag hinzufügen
config-add-cloud = + Cloud-LLM-Anbieter hinzufügen
config-add-custom-service = + Benutzerdef. Dienst hinzufügen
config-editor-reset-default = auf Standard zurücksetzen
config-editor-discard-field = ungespeicherte Änderungen verwerfen
config-editor-add-item = Element hinzufügen...
config-editor-select = -- auswählen --
config-editor-secret-placeholder = secret:NAME oder Rohwert
config-editor-secret-hint = Nutze secret:NAME um ein Vault-Secret zu referenzieren, oder gib einen Rohwert ein
config-editor-convert-to-secret = In Vault verschieben
config-editor-converting = Wird konvertiert...
config-editor-advanced = Erweitert · { $count } { $count ->
    [one] Feld
    *[other] Felder
}
config-editor-enabled-suffix = aktiviert
config-editor-remove = Entfernen
config-editor-on-this-page = Auf dieser Seite
config-editor-sections-count = Abschnitte

category-identity = Identität
category-llm-providers = LLM-Anbieter
category-agents = Agenten
category-infra = Infrastruktur
category-ops = Betrieb
category-custom = Benutzerdefinierte Dienste
category-other = Sonstiges

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

secrets-title = Secret-Vault
