## App-wide
app-title = Bitwarden [Next]
about-window-title = Über Bitwarden

## Login — unlock
login-unlock-title = Dein Tresor ist gesperrt
login-unlock-password-placeholder = Master-Passwort (erforderlich)
login-unlock-pin-placeholder = PIN (erforderlich)
login-unlock-button = Entsperren
login-unlock-or = oder
login-unlock-biometrics-button = Mit Biometrie entsperren
login-unlock-pin-button = Mit PIN entsperren
login-unlock-master-password-button = Mit Master-Passwort entsperren
login-log-out = Abmelden

## Login — email entry
login-email-title = Bei Bitwarden anmelden
login-email-placeholder = E-Mail-Adresse (erforderlich)
login-email-remember = E-Mail merken
login-email-continue = Weiter
login-email-or = Oder
login-email-sso = Single Sign-On verwenden
login-email-new-prompt = Neu bei Bitwarden?
login-email-create-account = Konto erstellen

## Login — password entry
login-password-title = Willkommen zurück
login-password-placeholder = Master-Passwort (erforderlich)
login-password-get-hint = Hinweis zum Master-Passwort abrufen
login-password-submit = Mit Master-Passwort anmelden
login-password-back = Zurück

## Login — server selector
login-server-accessing = Zugriff auf { $server }
login-server-accessing-label = Zugriff auf:
login-server-self-hosted = Selbst gehostet

## Login — self-hosted environment modal
login-self-hosted-modal-title = Selbst gehostete Umgebung
login-self-hosted-modal-url-label = Server-URL
login-self-hosted-modal-url-helper = Gib die Basis-URL deiner selbst gehosteten Bitwarden-Installation an. Beispiel: https://bitwarden.firma.de
login-self-hosted-modal-url-error = Die URL muss mit https:// beginnen
login-self-hosted-modal-save = Speichern
login-self-hosted-modal-cancel = Abbrechen

## Login — toasts
login-toast-unlock-failed-title = Entsperren fehlgeschlagen
login-toast-unlock-failed-body = Überprüfe dein Master-Passwort und versuche es erneut.
login-toast-login-failed-title = Anmeldung fehlgeschlagen
login-toast-login-failed-body = Überprüfe deine E-Mail-Adresse und dein Passwort und versuche es erneut.
login-toast-pin-unsupported = PIN-Entsperrung wird noch nicht unterstützt
login-toast-biometrics-unsupported = Biometrische Entsperrung wird noch nicht unterstützt

## About dialog
about-version-label = Version
about-sdk-version-label = SDK-Version
about-os-label = Betriebssystem
about-architecture-label = Architektur
about-copy-button = Kopieren
about-close-button = Schließen

## Sidebar — sections and filters
sidebar-section-vault = Tresor
sidebar-section-send = Send
sidebar-filter-my-vault = Mein Tresor
sidebar-filter-favorites = Favoriten
sidebar-filter-logins = Logins
sidebar-filter-cards = Karten
sidebar-filter-identities = Identitäten
sidebar-filter-notes = Notizen
sidebar-filter-ssh-keys = SSH-Schlüssel
sidebar-filter-archive = Archiv
sidebar-filter-trash = Papierkorb
sidebar-filter-text-send = Text
sidebar-filter-file-send = Datei
sidebar-item-generator = Generator
sidebar-item-import = Importieren
sidebar-item-export = Exportieren

## Vault list
vault-title = Tresor
vault-new-button = Neu
vault-search-placeholder = Suchen
vault-column-name = Name
vault-column-options = Optionen
vault-toast-item-saved = Eintrag gespeichert
vault-toast-save-failed-title = Speichern fehlgeschlagen
vault-toast-save-failed-body = Eintrag konnte nicht gespeichert werden. Versuche es erneut.
vault-toast-decrypt-failed-title = Entschlüsselung fehlgeschlagen
vault-toast-decrypt-failed-body = Eintrag konnte nicht geladen werden. Versuche es erneut.
vault-toast-copied-username = Nutzername kopiert
vault-toast-copied-password = Passwort kopiert
vault-toast-copied-website = Webseite kopiert
vault-toast-copied-totp = Verifizierungscode kopiert
vault-toast-copied-field = Feld kopiert
vault-toast-copied-private-key = Privater Schlüssel kopiert
vault-toast-copied-public-key = Öffentlicher Schlüssel kopiert
vault-toast-copied-fingerprint = Fingerabdruck kopiert
vault-toast-item-deleted = Eintrag in den Papierkorb verschoben
vault-toast-delete-failed-title = Löschen fehlgeschlagen
vault-toast-delete-failed-body = Eintrag konnte nicht gelöscht werden. Versuche es erneut.
vault-delete-modal-title = Eintrag löschen?
vault-delete-modal-body = "{ $name }" wird in den Papierkorb verschoben.
vault-delete-modal-cancel = Abbrechen
vault-delete-modal-confirm = Löschen

## Account switcher
account-switcher-other-accounts = Andere Bitwarden-Konten
account-switcher-options = Optionen
account-switcher-lock-now = Jetzt sperren
account-switcher-log-out = Abmelden
account-switcher-lock-all = Alle Konten sperren
account-switcher-settings = Einstellungen
account-switcher-add = Konto hinzufügen

## Detail pane — headers per cipher type
detail-header-login = Login anzeigen
detail-header-card = Karte anzeigen
detail-header-identity = Identität anzeigen
detail-header-note = Notiz anzeigen
detail-header-ssh-key = SSH-Schlüssel anzeigen
detail-header-bank-account = Bankkonto anzeigen
detail-header-drivers-license = Führerschein anzeigen
detail-header-passport = Reisepass anzeigen

## Detail pane — section labels
detail-section-item-details = Eintragsdetails
detail-section-login-credentials = Login-Daten
detail-section-autofill-options = Auto-Ausfüllen-Optionen
detail-section-card-details = Kartendetails
detail-section-personal-details = Persönliche Daten
detail-section-note = Notiz
detail-section-ssh-key = SSH-Schlüssel

## Detail pane — fields
detail-field-name = Name
detail-field-notes = Notizen
detail-field-username = Nutzername
detail-field-password = Passwort
detail-field-totp = Verifizierungscode (TOTP)
detail-totp-invalid = Ungültiger TOTP-Schlüssel
detail-field-website = Webseite
detail-field-cardholder-name = Name des Karteninhabers
detail-field-brand = Marke
detail-field-number = Nummer
detail-field-expiration = Ablauf
detail-field-security-code = Sicherheitscode
detail-field-email = E-Mail
detail-field-phone = Telefon
detail-field-company = Firma
detail-field-address = Adresse
detail-field-city-region = Stadt / Region
detail-field-public-key = Öffentlicher Schlüssel
detail-field-private-key = Privater Schlüssel
detail-field-fingerprint = Fingerabdruck
detail-empty-credentials = Keine Anmeldedaten
detail-empty-card = Keine Kartendetails
detail-empty-identity = Keine Identitätsdetails
detail-section-custom-fields = Benutzerdefinierte Felder
detail-field-passkey = Passkey
detail-field-passkey-created = Erstellt am { $date }
detail-field-boolean-true = Ja
detail-field-boolean-false = Nein
detail-edit-button = Bearbeiten

## Cipher form — header titles
form-title-new-item = Neuer Eintrag
form-title-edit-login = Login bearbeiten
form-title-edit-card = Karte bearbeiten
form-title-edit-identity = Identität bearbeiten
form-title-edit-note = Notiz bearbeiten
form-title-edit-ssh-key = SSH-Schlüssel bearbeiten
form-title-edit-bank-account = Bankkonto bearbeiten
form-title-edit-drivers-license = Führerschein bearbeiten
form-title-edit-passport = Reisepass bearbeiten

## Cipher form — buttons
form-save = Speichern
form-saving = Speichern…
form-cancel = Abbrechen

## Cipher form — sections
form-section-item-details = Eintragsdetails
form-section-login-credentials = Login-Daten
form-section-autofill-options = Auto-Ausfüllen-Optionen
form-section-card-details = Kartendetails
form-section-personal-details = Persönliche Daten
form-section-identification = Identifikation
form-section-contact-info = Kontaktdaten
form-section-address = Adresse
form-section-ssh-key = SSH-Schlüssel
form-section-additional-options = Weitere Optionen
form-section-custom-fields = Benutzerdefinierte Felder

## Cipher form — item details
form-name = Name (erforderlich)
form-favorite = Favorit
form-reprompt = Master-Passwort erneut anfordern
form-notes = Notizen
form-folder = Ordner
form-folder-none = Kein Ordner
form-organization = Organisation
form-organization-personal = Persönlich (ich)
form-collections = Sammlungen
form-collections-none = Keine Sammlungen
form-collections-selected = { $count } ausgewählt
form-collections-empty-in-org = Keine Sammlungen in dieser Organisation

## Cipher form — login
form-username = Nutzername
form-password = Passwort
form-totp = Authenticator-Schlüssel (TOTP)

## Cipher form — websites
form-uri = Webseite (URI)
form-uri-empty = Noch keine Webseiten
form-add-website = Webseite hinzufügen

## Cipher form — card
form-card-cardholder = Name des Karteninhabers
form-card-brand = Marke
form-card-number = Nummer
form-card-exp-month = Ablaufmonat
form-card-exp-year = Ablaufjahr
form-card-code = Sicherheitscode
form-card-brand-placeholder = -- Auswählen --
form-card-month-placeholder = -- Monat --

## Cipher form — identity
form-identity-title = Anrede
form-identity-title-placeholder = -- Anrede --
# Identity title labels. Same convention as card brands — the stored value
# is the canonical English string; these keys only localize the display.
form-identity-title-mr = Herr
form-identity-title-mrs = Frau
form-identity-title-ms = Frau
form-identity-title-mx = Mx
form-identity-title-dr = Dr.
form-identity-first-name = Vorname
form-identity-middle-name = Zweiter Vorname
form-identity-last-name = Nachname
form-identity-username = Nutzername
form-identity-company = Firma
form-identity-ssn = Sozialversicherungsnummer
form-identity-passport = Reisepassnummer
form-identity-license = Führerscheinnummer
form-identity-email = E-Mail
form-identity-phone = Telefon
form-identity-address1 = Adresszeile 1
form-identity-address2 = Adresszeile 2
form-identity-address3 = Adresszeile 3
form-identity-city = Stadt / Ort
form-identity-state = Bundesland / Region
form-identity-postal = Postleitzahl
form-identity-country = Land

## Cipher form — SSH key
form-ssh-public-key = Öffentlicher Schlüssel
form-ssh-private-key = Privater Schlüssel
form-ssh-fingerprint = Fingerabdruck

## Cipher form — custom fields
form-custom-field-type = Typ
form-custom-field-name = Name
form-custom-field-value = Wert
form-custom-field-type-text = Text
form-custom-field-type-hidden = Verborgen
form-custom-field-type-boolean = Boolean
form-custom-field-type-linked = Verknüpft
form-custom-field-enabled = Aktiviert
form-custom-field-empty = Noch keine benutzerdefinierten Felder
form-add-custom-field = Benutzerdefiniertes Feld hinzufügen
form-custom-field-linked-unsupported = Verknüpfte Felder werden noch nicht unterstützt

## Toast — shared messages
toast-required-fields = Bitte fülle die erforderlichen Felder aus.

## Menu bar — top-level
menu-file = Datei
menu-edit = Bearbeiten
menu-view = Ansicht
menu-account = Konto
menu-window = Fenster
menu-help = Hilfe

## Menu — File
menu-file-new-login = Neues Login
menu-file-new-item = Neuer Eintrag
menu-file-new-item-login = Login
menu-file-new-item-card = Karte
menu-file-new-item-identity = Identität
menu-file-new-item-secure-note = Sichere Notiz
menu-file-new-item-ssh-key = SSH-Schlüssel
menu-file-new-folder = Neuer Ordner
menu-file-sync-now = Jetzt synchronisieren
menu-file-import = Importieren
menu-file-export = Exportieren
menu-file-settings = Einstellungen
menu-file-lock-vault = Tresor sperren
menu-file-lock-all-vaults = Alle Tresore sperren
menu-file-log-out = Abmelden
menu-file-quit = Bitwarden beenden

## Menu — Edit
menu-edit-undo = Rückgängig
menu-edit-redo = Wiederherstellen
menu-edit-cut = Ausschneiden
menu-edit-copy = Kopieren
menu-edit-paste = Einfügen
menu-edit-select-all = Alles auswählen
menu-edit-copy-username = Nutzername kopieren
menu-edit-copy-password = Passwort kopieren
menu-edit-copy-totp = Verifizierungscode kopieren (TOTP)

## Menu — View
menu-view-search = Tresor durchsuchen
menu-view-generator = Generator
menu-view-generator-history = Generator-Verlauf
menu-view-zoom-in = Vergrößern
menu-view-zoom-out = Verkleinern
menu-view-reset-zoom = Zoom zurücksetzen

## Menu — Account
menu-account-change-password = Master-Passwort ändern
menu-account-two-step = Zwei-Faktor-Authentifizierung
menu-account-fingerprint = Fingerabdruck-Phrase
menu-account-delete = Konto löschen

## Menu — Window
menu-window-minimize = Minimieren
menu-window-hide-to-tray = In Taskleiste minimieren
menu-window-always-on-top = Immer im Vordergrund
menu-window-close = Schließen

## Menu — Help
menu-help-feedback = Hilfe und Feedback
menu-help-bug = Fehler melden
menu-help-legal = Rechtliches
menu-help-legal-tos = Nutzungsbedingungen
menu-help-legal-privacy = Datenschutzerklärung
menu-help-follow = Folge uns
menu-help-web-vault = Zum Web-Tresor
menu-help-browser-extension = Browser-Erweiterung holen
menu-help-about = Über Bitwarden

menu-toast-sync-success = Tresor synchronisiert
menu-toast-sync-failed-title = Synchronisation fehlgeschlagen
menu-toast-sync-failed-body = Tresor konnte nicht synchronisiert werden. Versuche es später erneut.

menu-fingerprint-title = Die Fingerabdruck-Phrase deines Kontos:
menu-fingerprint-learn-more = Mehr erfahren
menu-fingerprint-close = Schließen

new-folder-modal-title = Neuer Ordner
new-folder-modal-field-label = Ordnername (erforderlich)
new-folder-modal-helper = Verschachtle einen Ordner, indem du den Namen des übergeordneten Ordners gefolgt von "/" hinzufügst. Beispiel: Sozial/Foren
new-folder-modal-save = Speichern
new-folder-modal-cancel = Abbrechen
new-folder-toast-success = Ordner erstellt
new-folder-toast-failed-title = Ordner konnte nicht erstellt werden
new-folder-toast-failed-body = Der Ordner konnte nicht gespeichert werden. Versuche es erneut.

## New-item picker modal
picker-title = Element zum Hinzufügen wählen
picker-folder = Ordner
picker-login-subtitle = Website oder App
picker-card-subtitle = Kredit- oder Debitkarte
picker-bank-account-subtitle = Bankdaten
picker-drivers-license-subtitle = Führerscheindaten
picker-passport-subtitle = Reisedokument
picker-identity-subtitle = Persönliche Informationen
picker-secure-note-subtitle = Wichtiger Text
picker-ssh-key-subtitle = Server-Anmeldetoken
picker-folder-subtitle = Elemente organisieren

## Tray
tray-show-hide = Anzeigen / Ausblenden
tray-lock-vault = Tresor sperren
tray-exit = Beenden

# Endonym — see comment in the canonical en file.
language-name-self = Deutsch

## Settings modal
settings-title = Einstellungen
settings-tab-security = Sicherheit
settings-tab-integrations = Integrationen
settings-tab-autotype = Auto-Eingabe und Kopieren
settings-tab-appearance = Darstellung
settings-tab-advanced = Erweitert

settings-toast-not-supported = Diese Einstellung wird noch nicht unterstützt.

# Security tab
settings-security-access-options = Zugriffsoptionen
settings-security-open-at-login = Bitwarden beim Systemstart öffnen
settings-security-unlock-pin = Mit PIN entsperren
settings-security-unlock-touch = Mit Touch ID entsperren
settings-security-session-timeout = Sitzungs-Timeout
settings-security-lock-after = Sperren nach
settings-security-logout-after = Abmelden nach
settings-security-lock-on-system-lock = Sperren, wenn das System gesperrt wird

# Shared duration strings used by all the time-based dropdowns (lock after,
# log out after, clear clipboard after). Plurals come from Fluent selectors
# so every language can pick the right variant for the supplied number.
settings-duration-seconds = { $n ->
    [one] { $n } Sekunde
   *[other] { $n } Sekunden
  }
settings-duration-minutes = { $n ->
    [one] { $n } Minute
   *[other] { $n } Minuten
  }
settings-duration-hours = { $n ->
    [one] { $n } Stunde
   *[other] { $n } Stunden
  }
settings-duration-never = Niemals

# Integrations tab
settings-integrations-browser = Browser-Integration
settings-integrations-browser-enable = Browser-Integration aktivieren
settings-integrations-browser-fingerprint = Bestätigungs-Fingerabdruck verlangen
settings-integrations-ssh = SSH-Agent
settings-integrations-ssh-enable = SSH-Agent aktivieren
settings-integrations-ssh-prompt = Verhalten bei Anfrage
settings-ssh-prompt-always = Nachfragen (Standard: einmal erlauben)
settings-ssh-prompt-never = Niemals
settings-ssh-prompt-remember = Nachfragen (Standard: bis zur Sperre)

# Autotype and copy tab
settings-autotype-heading = Auto-Eingabe
settings-autotype-enable = Auto-Eingabe aktivieren
settings-clipboard-heading = Zwischenablage
settings-clipboard-clear-after = Zwischenablage leeren nach
settings-clipboard-minimize-on-copy = Beim Kopieren minimieren

# Appearance tab
settings-appearance-theme-heading = Design
settings-appearance-theme = Design
settings-appearance-theme-system = System
settings-appearance-theme-light = Hell
settings-appearance-theme-dark = Dunkel
settings-appearance-language = Sprache
settings-appearance-language-system = System
settings-appearance-display-heading = Anzeige
settings-appearance-show-favicons = Symbole für URLs anzeigen

# Advanced tab
settings-advanced-tray = Taskleiste
settings-advanced-tray-enable = Taskleisten-Symbol anzeigen
settings-advanced-minimize-to-tray = In Taskleiste minimieren
settings-advanced-close-to-tray = In Taskleiste schließen
settings-advanced-platform = Plattform
settings-advanced-always-show-dock = Dock-Symbol immer anzeigen
settings-advanced-allow-screenshots = Screenshots erlauben

## Send list
send-title = Send
send-new-button = Neu
send-search-placeholder = Sends durchsuchen
send-column-name = Name
send-column-deletion = Löschdatum
send-empty-body = Du hast noch keine Sends erstellt.
send-toast-item-saved = Send gespeichert
send-toast-item-deleted = Send gelöscht
send-toast-copied-link = Send-Link kopiert
send-toast-copied-password = Passwort kopiert
send-toast-load-failed-title = Laden fehlgeschlagen
send-toast-load-failed-body = Send konnte nicht geladen werden. Versuche es erneut.
send-toast-save-failed-title = Speichern fehlgeschlagen
send-toast-save-failed-body = Send konnte nicht gespeichert werden. Versuche es erneut.
send-toast-delete-failed-title = Löschen fehlgeschlagen
send-toast-delete-failed-body = Send konnte nicht gelöscht werden. Versuche es erneut.

## Send form
send-form-title-new-text = Text-Send erstellen
send-form-title-new-file = Datei-Send erstellen
send-form-title-edit-text = Text-Send bearbeiten
send-form-title-edit-file = Datei-Send bearbeiten
send-form-details-heading = Send-Details
send-form-additional-heading = Weitere Optionen
send-form-name = Name (erforderlich)
send-form-text = Zu teilender Text (erforderlich)
send-form-hide-text = Text standardmäßig ausblenden
send-form-file-choose = Datei auswählen
send-form-file-choose-placeholder = Keine Datei ausgewählt
send-form-file-name = Dateiname
send-form-file-size = Größe
send-form-deletion-date = Löschdatum
send-form-deletion-hint = Der Send wird am {$date} dauerhaft gelöscht.
send-form-who-can-view = Wer darf sehen
send-form-access-link = Jeder mit dem Link
send-form-access-people = Bestimmte Personen
send-form-access-password = Jeder mit einem von dir festgelegten Passwort
send-form-password = Passwort (erforderlich)
send-form-password-hint = Empfänger müssen das Passwort eingeben, um diesen Send zu sehen.
send-form-emails = E-Mail-Adressen (erforderlich)
send-form-send-link = Send-Link
send-form-limit-views = Aufrufe begrenzen
send-form-limit-views-hint = Niemand kann diesen Send nach Erreichen der Grenze sehen.
send-form-views-left = Niemand kann diesen Send nach Erreichen der Grenze sehen. {$count} Aufrufe übrig.
send-form-hide-email = Deine E-Mail-Adresse vor Empfängern verbergen.
send-form-private-note = Private Notiz
send-form-save = Speichern
send-form-cancel = Abbrechen

send-form-preset-1h = 1 Stunde
send-form-preset-1d = 1 Tag
send-form-preset-2d = 2 Tage
send-form-preset-3d = 3 Tage
send-form-preset-7d = 7 Tage
send-form-preset-14d = 14 Tage
send-form-preset-30d = 30 Tage

send-delete-modal-title = Send löschen
send-delete-modal-body = Möchtest du "{$name}" wirklich löschen?
send-delete-modal-cancel = Abbrechen
send-delete-modal-confirm = Löschen

## Generator
generator-title = Generator
generator-tab-password = Passwort
generator-tab-passphrase = Passphrase
generator-tab-username = Nutzername

# Password tab
generator-length = Länge
generator-length-hint = Der Wert muss zwischen 5 und 128 liegen.
generator-include = Einschließen
generator-include-uppercase = A-Z
generator-include-lowercase = a-z
generator-include-numbers = 0-9
generator-include-special = !@#$%^&*
generator-min-number = Mindestanzahl Ziffern
generator-min-special = Mindestanzahl Sonderzeichen
generator-avoid-ambiguous = Mehrdeutige Zeichen vermeiden

# Passphrase tab
generator-num-words = Anzahl der Wörter
generator-num-words-hint = Der Wert muss zwischen 3 und 20 liegen. Verwende 6 oder mehr Wörter, um eine starke Passphrase zu erzeugen.
generator-word-separator = Worttrennzeichen
generator-passphrase-capitalize = Großschreiben
generator-passphrase-include-number = Zahl einschließen

# Username tab
generator-username-type = Typ
generator-username-kind-word = Zufälliges Wort
generator-username-kind-subaddress = E-Mail mit Sub-Adresse
generator-username-kind-catchall = Catch-All-E-Mail
generator-username-capitalize = Großschreiben
generator-username-include-number = Zahl einschließen
generator-username-email = E-Mail
generator-username-domain = Domain

# History
generator-history-open = Generator-Verlauf
generator-history-title = Generator-Verlauf
generator-history-heading = Zuletzt
generator-history-empty = Keine kürzlich erzeugten Werte.
generator-history-clear = Verlauf löschen
generator-history-just-now = gerade eben
generator-history-minutes-ago = vor {$count} Min.
generator-history-hours-ago = vor {$count} Std.
generator-history-days-ago = vor {$count} T.

# Toasts
generator-toast-copied = Kopiert
generator-toast-failed = Konnte nicht erzeugt werden

# Magnify launcher
magnify-search-placeholder = Bitwarden Magnify
magnify-locked-title = Dein Tresor ist gesperrt
magnify-locked-subtitle = Öffne Bitwarden zum Entsperren
magnify-open-bitwarden = Bitwarden öffnen
magnify-no-results = Keine passenden Einträge
magnify-copy-password = Passwort kopieren
magnify-copy-username = Nutzername kopieren
magnify-hint-navigate = Navigieren

## Import modal
import-modal-title = Daten importieren
import-modal-section-destination = Ziel
import-modal-vault-label = Tresor (erforderlich)
import-modal-vault-personal = Mein Tresor
import-modal-folder-label = Ordner
import-modal-folder-placeholder = - Ordner auswählen -
import-modal-collection-label = Sammlung (erforderlich)
import-modal-collection-placeholder = - Sammlung auswählen -
import-modal-section-data = Daten
import-modal-file-format-label = Dateiformat (erforderlich)
import-modal-file-helper = Wähle die Importdatei aus
import-modal-choose-file = Datei auswählen
import-modal-no-file = Keine Datei gewählt
import-modal-paste-label = oder kopiere/füge den Inhalt der Importdatei ein
import-modal-submit = Daten importieren
import-modal-cancel = Abbrechen
import-toast-unimplemented = Import ist noch nicht angebunden — bald verfügbar

## Export modal
export-modal-title = Tresor exportieren
export-modal-vault-label = Exportieren aus (erforderlich)
export-modal-vault-personal = Mein Tresor
export-modal-banner-personal-title = Persönlichen Tresor exportieren
export-modal-banner-personal = Es werden nur die Einträge des persönlichen Tresors exportiert, die mit { $email } verknüpft sind. Einträge aus Organisations-Tresoren werden nicht eingeschlossen. Es werden nur die Eintragsdaten exportiert; Anhänge sind nicht enthalten.
export-modal-banner-org-title = Organisations-Tresor exportieren
export-modal-banner-org-body = Es wird nur der Organisations-Tresor exportiert, der mit { $name } verknüpft ist. Einträge aus persönlichen Tresoren oder anderen Organisationen werden nicht eingeschlossen.
export-modal-file-format-label = Dateiformat (erforderlich)
export-modal-password-label = Datei-Passwort (erforderlich)
export-modal-continue = Weiter
export-modal-cancel = Abbrechen
export-confirm-title = Tresor-Export bestätigen
export-confirm-warning-unencrypted = Dieser Export enthält deine Tresor-Daten in unverschlüsselter Form. Du solltest die exportierte Datei nicht über unsichere Kanäle (z. B. E-Mail) speichern oder versenden. Lösche sie unmittelbar nach Gebrauch.
export-confirm-warning-file-encrypted = Dieser Datei-Export wird passwortgeschützt und benötigt das Datei-Passwort zur Entschlüsselung.
export-confirm-password-label = Master-Passwort (erforderlich)
export-confirm-helper = Bestätige deine Identität, um fortzufahren.
export-confirm-error = Ungültiges Master-Passwort.
export-toast-success = Tresor exportiert nach { $path }
export-toast-failed-title = Export fehlgeschlagen
export-toast-failed-body = Tresor konnte nicht exportiert werden. Versuche es erneut.

# Local account storage and logout recovery
storage-accounts-unavailable = Der lokale Kontospeicher ist nicht verfügbar. Aus Sicherheitsgründen wurden keine Konten geladen.
storage-logout-incomplete = Das Konto ist abgemeldet. Lokale Kontodaten konnten noch nicht vollständig entfernt werden. Die Bereinigung wird beim nächsten Start erneut versucht.
storage-logout-not-stored = Das Abmelden konnte nicht gespeichert werden. Das Konto ist jetzt gesperrt, kann aber nach einem Neustart wieder erscheinen. Bitte prüfe den Speicherzugriff.
storage-logout-pending = Abmeldung wird abgeschlossen …

send-item-unsupported = Das Teilen von Tresoreinträgen per Send wird noch nicht unterstützt.

# Composable vault filters
sidebar-group-vaults = Tresore
sidebar-group-view = Ansicht
sidebar-group-types = Eintragstypen
sidebar-filter-all-vaults = Alle Tresore
sidebar-filter-active-items = Aktive Einträge
sidebar-filter-all-types = Alle Eintragstypen

sidebar-group-folders = Ordner
sidebar-filter-all-folders = Alle Ordner
sidebar-filter-no-folder = Ohne Ordner
sidebar-group-collections = Sammlungen
sidebar-filter-all-collections = Alle Sammlungen
sidebar-filter-no-collection = Ohne Sammlung

vault-column-last-changed = Zuletzt geändert
vault-column-base-url = Basis-URL
vault-filter-folder = Ordner: { $name }
vault-filter-collection = Sammlung: { $name }
vault-filter-search = Suche: „{ $query }“

ssh-approval-title = SSH-Anmeldung erlauben?
ssh-approval-key = Schlüssel: { $name }
ssh-approval-user = SSH-Benutzer: { $name }
ssh-approval-process = Lokaler Prozess: { $name }
ssh-approval-process-note = Der Prozessname ist ein Hinweis und bestätigt weder die Anwendung noch den Ziel-Hostnamen.
ssh-approval-unbound = Keine verifizierte Hostschlüssel-Bindung. Diese Anfrage kann nur einmal erlaubt werden.
ssh-approval-hosts = Verifizierte Hostschlüssel (Weiterleitungspfad → Ziel):
ssh-approval-deny = Ablehnen
ssh-approval-once = Einmal erlauben
ssh-approval-remember = Bis zum Sperren erlauben
ssh-approval-timeout = Ohne Antwort innerhalb von 45 Sekunden wird die Anfrage abgelehnt.
ssh-agent-copy-socket = Socket-Pfad kopieren
ssh-agent-socket-copied = SSH-Socketpfad kopiert
ssh-agent-help = SSH_AUTH_SOCK oder die SSH-Option IdentityAgent auf diesen Pfad setzen. Nur das aktive entsperrte Konto stellt Schlüssel bereit.
ssh-agent-unattended = Ohne Rückfragen kann jeder lokale Prozess deines Benutzers erlaubte SSH-Schlüssel verwenden, solange der aktive Tresor entsperrt ist.
ssh-agent-protected = Gelöschte, archivierte, verborgene und durch Master-Passwort-Abfrage geschützte Schlüssel werden nicht angeboten. Freigaben bis zur Sperre benötigen eine verifizierte Sitzungsbindung und eine hostgebundene Signatur.

ssh-agent-copy-setup = SSH_AUTH_SOCK-Befehl kopieren
ssh-agent-setup-copied = SSH_AUTH_SOCK-Befehl kopiert
ssh-approval-bound-once = Der Host-Schlüssel ist verifiziert, diese Signaturanfrage ist aber nicht hostgebunden. Nur einmalige Freigabe möglich.

ssh-approval-window-open = SSH-Freigabe wartet im Bestätigungsfenster. Esc bricht die Anfrage ab.
