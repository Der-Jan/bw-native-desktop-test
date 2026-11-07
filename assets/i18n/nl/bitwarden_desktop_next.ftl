## App-wide
app-title = Bitwarden [Next]
about-window-title = Over Bitwarden

## Login — unlock
login-unlock-title = Je kluis is vergrendeld
login-unlock-password-placeholder = Hoofdwachtwoord (verplicht)
login-unlock-pin-placeholder = PIN (verplicht)
login-unlock-button = Ontgrendelen
login-unlock-or = of
login-unlock-biometrics-button = Ontgrendelen met biometrie
login-unlock-pin-button = Ontgrendelen met PIN
login-unlock-master-password-button = Ontgrendelen met hoofdwachtwoord
login-log-out = Uitloggen

## Login — email entry
login-email-title = Inloggen bij Bitwarden
login-email-placeholder = E-mailadres (verplicht)
login-email-remember = E-mailadres onthouden
login-email-continue = Doorgaan
login-email-or = Of
login-email-sso = Single sign-on gebruiken
login-email-new-prompt = Nieuw bij Bitwarden?
login-email-create-account = Account aanmaken

## Login — password entry
login-password-title = Welkom terug
login-password-placeholder = Hoofdwachtwoord (verplicht)
login-password-get-hint = Hint van hoofdwachtwoord opvragen
login-password-submit = Inloggen met hoofdwachtwoord
login-password-back = Terug

## Login — server selector
login-server-accessing = Verbinding met { $server }
login-server-accessing-label = Verbinding met:
login-server-self-hosted = Self-hosted

## Login — self-hosted environment modal
login-self-hosted-modal-title = Self-hosted omgeving
login-self-hosted-modal-url-label = Server-URL
login-self-hosted-modal-url-helper = Geef de basis-URL van je self-hosted Bitwarden-installatie op. Voorbeeld: https://bitwarden.bedrijf.com
login-self-hosted-modal-url-error = De URL moet beginnen met https://
login-self-hosted-modal-save = Opslaan
login-self-hosted-modal-cancel = Annuleren

## Login — toasts
login-toast-unlock-failed-title = Ontgrendelen mislukt
login-toast-unlock-failed-body = Controleer je hoofdwachtwoord en probeer het opnieuw.
login-toast-login-failed-title = Inloggen mislukt
login-toast-login-failed-body = Controleer je e-mailadres en wachtwoord en probeer het opnieuw.
login-toast-pin-unsupported = PIN-ontgrendeling wordt nog niet ondersteund
login-toast-biometrics-unsupported = Biometrische ontgrendeling wordt nog niet ondersteund

## About dialog
about-version-label = Versie
about-sdk-version-label = SDK-versie
about-os-label = Systeem
about-architecture-label = Architectuur
about-copy-button = Kopiëren
about-close-button = Sluiten

## Sidebar — sections and filters
sidebar-section-vault = Kluis
sidebar-section-send = Send
sidebar-filter-my-vault = Mijn kluis
sidebar-filter-favorites = Favorieten
sidebar-filter-logins = Logins
sidebar-filter-cards = Kaarten
sidebar-filter-identities = Identiteiten
sidebar-filter-notes = Notities
sidebar-filter-ssh-keys = SSH-sleutels
sidebar-filter-archive = Archief
sidebar-filter-trash = Prullenbak
sidebar-filter-text-send = Tekst
sidebar-filter-file-send = Bestand
sidebar-item-generator = Generator
sidebar-item-import = Importeren
sidebar-item-export = Exporteren

## Vault list
vault-title = Kluis
vault-new-button = Nieuw
vault-search-placeholder = Zoeken
vault-column-name = Naam
vault-column-options = Opties
vault-toast-item-saved = Item opgeslagen
vault-toast-save-failed-title = Opslaan mislukt
vault-toast-save-failed-body = Het item kon niet worden opgeslagen. Probeer het opnieuw.
vault-toast-decrypt-failed-title = Ontsleuteling mislukt
vault-toast-decrypt-failed-body = Het item kon niet worden geladen. Probeer het opnieuw.
vault-toast-copied-username = Gebruikersnaam gekopieerd
vault-toast-copied-password = Wachtwoord gekopieerd
vault-toast-copied-website = Website gekopieerd
vault-toast-copied-totp = Verificatiecode gekopieerd
vault-toast-copied-field = Veld gekopieerd
vault-toast-copied-private-key = Privésleutel gekopieerd
vault-toast-copied-public-key = Publieke sleutel gekopieerd
vault-toast-copied-fingerprint = Vingerafdruk gekopieerd
vault-toast-item-deleted = Item naar prullenbak verplaatst
vault-toast-delete-failed-title = Verwijderen mislukt
vault-toast-delete-failed-body = Het item kon niet worden verwijderd. Probeer het opnieuw.
vault-delete-modal-title = Item verwijderen?
vault-delete-modal-body = "{ $name }" wordt naar de prullenbak verplaatst.
vault-delete-modal-cancel = Annuleren
vault-delete-modal-confirm = Verwijderen

## Account switcher
account-switcher-other-accounts = Andere Bitwarden-accounts
account-switcher-options = Opties
account-switcher-lock-now = Nu vergrendelen
account-switcher-log-out = Uitloggen
account-switcher-lock-all = Alle accounts vergrendelen
account-switcher-settings = Instellingen
account-switcher-add = Account toevoegen

## Detail pane — headers per cipher type
detail-header-login = Login bekijken
detail-header-card = Kaart bekijken
detail-header-identity = Identiteit bekijken
detail-header-note = Notitie bekijken
detail-header-ssh-key = SSH-sleutel bekijken
detail-header-bank-account = Bankrekening bekijken
detail-header-drivers-license = Rijbewijs bekijken
detail-header-passport = Paspoort bekijken

## Detail pane — section labels
detail-section-item-details = Itemdetails
detail-section-login-credentials = Logingegevens
detail-section-autofill-options = Opties voor automatisch invullen
detail-section-card-details = Kaartgegevens
detail-section-personal-details = Persoonlijke gegevens
detail-section-note = Notitie
detail-section-ssh-key = SSH-sleutel

## Detail pane — fields
detail-field-name = Naam
detail-field-notes = Notities
detail-field-username = Gebruikersnaam
detail-field-password = Wachtwoord
detail-field-totp = Verificatiecode (TOTP)
detail-totp-invalid = Ongeldige TOTP-seed
detail-field-website = Website
detail-field-cardholder-name = Naam kaarthouder
detail-field-brand = Merk
detail-field-number = Nummer
detail-field-expiration = Vervaldatum
detail-field-security-code = Beveiligingscode
detail-field-email = E-mail
detail-field-phone = Telefoon
detail-field-company = Bedrijf
detail-field-address = Adres
detail-field-city-region = Plaats / regio
detail-field-public-key = Publieke sleutel
detail-field-private-key = Privésleutel
detail-field-fingerprint = Vingerafdruk
detail-empty-credentials = Geen inloggegevens
detail-empty-card = Geen kaartgegevens
detail-empty-identity = Geen identiteitsgegevens
detail-section-custom-fields = Aangepaste velden
detail-field-passkey = Passkey
detail-field-passkey-created = Aangemaakt op { $date }
detail-field-boolean-true = Ja
detail-field-boolean-false = Nee
detail-edit-button = Bewerken

## Cipher form — header titles
form-title-new-item = Nieuw item
form-title-edit-login = Login bewerken
form-title-edit-card = Kaart bewerken
form-title-edit-identity = Identiteit bewerken
form-title-edit-note = Notitie bewerken
form-title-edit-ssh-key = SSH-sleutel bewerken
form-title-edit-bank-account = Bankrekening bewerken
form-title-edit-drivers-license = Rijbewijs bewerken
form-title-edit-passport = Paspoort bewerken

## Cipher form — buttons
form-save = Opslaan
form-saving = Opslaan…
form-cancel = Annuleren

## Cipher form — sections
form-section-item-details = Itemdetails
form-section-login-credentials = Logingegevens
form-section-autofill-options = Opties voor automatisch invullen
form-section-card-details = Kaartgegevens
form-section-personal-details = Persoonlijke gegevens
form-section-identification = Identificatie
form-section-contact-info = Contactgegevens
form-section-address = Adres
form-section-ssh-key = SSH-sleutel
form-section-additional-options = Extra opties
form-section-custom-fields = Aangepaste velden

## Cipher form — item details
form-name = Naam (verplicht)
form-favorite = Favoriet
form-reprompt = Hoofdwachtwoord opnieuw vragen
form-notes = Notities
form-folder = Map
form-folder-none = Geen map
form-organization = Organisatie
form-organization-personal = Persoonlijk (ik)
form-collections = Collecties
form-collections-none = Geen collecties
form-collections-selected = { $count } geselecteerd
form-collections-empty-in-org = Geen collecties in deze organisatie

## Cipher form — login
form-username = Gebruikersnaam
form-password = Wachtwoord
form-totp = Authenticatiesleutel (TOTP)

## Cipher form — websites
form-uri = Website (URI)
form-uri-empty = Nog geen websites
form-add-website = Website toevoegen

## Cipher form — card
form-card-cardholder = Naam kaarthouder
form-card-brand = Merk
form-card-number = Nummer
form-card-exp-month = Vervalmaand
form-card-exp-year = Vervaljaar
form-card-code = Beveiligingscode
form-card-brand-placeholder = -- Selecteren --
form-card-month-placeholder = -- Maand --

## Cipher form — identity
form-identity-title = Aanhef
form-identity-title-placeholder = -- Aanhef --
# Identity title labels. Same convention as card brands — the stored value
# is the canonical English string; these keys only localize the display.
form-identity-title-mr = Dhr.
form-identity-title-mrs = Mevr.
form-identity-title-ms = Mej.
form-identity-title-mx = Mx
form-identity-title-dr = Dr.
form-identity-first-name = Voornaam
form-identity-middle-name = Tweede naam
form-identity-last-name = Achternaam
form-identity-username = Gebruikersnaam
form-identity-company = Bedrijf
form-identity-ssn = BSN
form-identity-passport = Paspoortnummer
form-identity-license = Rijbewijsnummer
form-identity-email = E-mail
form-identity-phone = Telefoon
form-identity-address1 = Adresregel 1
form-identity-address2 = Adresregel 2
form-identity-address3 = Adresregel 3
form-identity-city = Plaats / gemeente
form-identity-state = Staat / provincie
form-identity-postal = Postcode
form-identity-country = Land

## Cipher form — SSH key
form-ssh-public-key = Publieke sleutel
form-ssh-private-key = Privésleutel
form-ssh-fingerprint = Vingerafdruk

## Cipher form — custom fields
form-custom-field-type = Type
form-custom-field-name = Naam
form-custom-field-value = Waarde
form-custom-field-type-text = Tekst
form-custom-field-type-hidden = Verborgen
form-custom-field-type-boolean = Booleaans
form-custom-field-type-linked = Gekoppeld
form-custom-field-enabled = Ingeschakeld
form-custom-field-empty = Nog geen aangepaste velden
form-add-custom-field = Aangepast veld toevoegen
form-custom-field-linked-unsupported = Gekoppelde velden worden nog niet ondersteund

## Toast — shared messages
toast-required-fields = Vul de verplichte velden in.

## Menu bar — top-level
menu-file = Bestand
menu-edit = Bewerken
menu-view = Weergave
menu-account = Account
menu-window = Venster
menu-help = Help

## Menu — File
menu-file-new-login = Nieuwe login
menu-file-new-item = Nieuw item
menu-file-new-item-login = Login
menu-file-new-item-card = Kaart
menu-file-new-item-identity = Identiteit
menu-file-new-item-secure-note = Veilige notitie
menu-file-new-item-ssh-key = SSH-sleutel
menu-file-new-folder = Nieuwe map
menu-file-sync-now = Nu synchroniseren
menu-file-import = Importeren
menu-file-export = Exporteren
menu-file-settings = Instellingen
menu-file-lock-vault = Kluis vergrendelen
menu-file-lock-all-vaults = Alle kluizen vergrendelen
menu-file-log-out = Uitloggen
menu-file-quit = Bitwarden afsluiten

## Menu — Edit
menu-edit-undo = Ongedaan maken
menu-edit-redo = Opnieuw
menu-edit-cut = Knippen
menu-edit-copy = Kopiëren
menu-edit-paste = Plakken
menu-edit-select-all = Alles selecteren
menu-edit-copy-username = Gebruikersnaam kopiëren
menu-edit-copy-password = Wachtwoord kopiëren
menu-edit-copy-totp = Verificatiecode kopiëren (TOTP)

## Menu — View
menu-view-search = Zoek in kluis
menu-view-generator = Generator
menu-view-generator-history = Generator-geschiedenis
menu-view-zoom-in = Inzoomen
menu-view-zoom-out = Uitzoomen
menu-view-reset-zoom = Zoom resetten
menu-view-toggle-fullscreen = Volledig scherm aan/uit

## Menu — Account
menu-account-change-password = Hoofdwachtwoord wijzigen
menu-account-two-step = Tweestapsaanmelding
menu-account-fingerprint = Vingerafdrukzin
menu-account-delete = Account verwijderen

## Menu — Window
menu-window-minimize = Minimaliseren
menu-window-hide-to-tray = Verbergen naar systeemvak
menu-window-always-on-top = Altijd op voorgrond
menu-window-close = Sluiten

## Menu — Help
menu-help-feedback = Help & feedback
menu-help-bug = Bug melden
menu-help-legal = Juridisch
menu-help-legal-tos = Servicevoorwaarden
menu-help-legal-privacy = Privacybeleid
menu-help-follow = Volg ons
menu-help-web-vault = Naar webkluis
menu-help-browser-extension = Browser-extensie downloaden
menu-help-about = Over Bitwarden

menu-toast-sync-success = Kluis gesynchroniseerd
menu-toast-sync-failed-title = Synchronisatie mislukt
menu-toast-sync-failed-body = De kluis kon niet worden gesynchroniseerd. Probeer het later opnieuw.

menu-fingerprint-title = De vingerafdrukzin van je account:
menu-fingerprint-learn-more = Meer informatie
menu-fingerprint-close = Sluiten

new-folder-modal-title = Nieuwe map
new-folder-modal-field-label = Mapnaam (verplicht)
new-folder-modal-helper = Nest een map door de naam van de bovenliggende map gevolgd door "/" toe te voegen. Voorbeeld: Sociaal/Forums
new-folder-modal-save = Opslaan
new-folder-modal-cancel = Annuleren
new-folder-toast-success = Map aangemaakt
new-folder-toast-failed-title = Map kon niet worden aangemaakt
new-folder-toast-failed-body = De map kon niet worden opgeslagen. Probeer het opnieuw.

## New-item picker modal
picker-title = Kies een item om toe te voegen
picker-folder = Map
picker-login-subtitle = Website of app
picker-card-subtitle = Creditcard of debetkaart
picker-bank-account-subtitle = Bankgegevens
picker-drivers-license-subtitle = Rijbewijsgegevens
picker-passport-subtitle = Reisdocument
picker-identity-subtitle = Persoonlijke informatie
picker-secure-note-subtitle = Belangrijke tekst
picker-ssh-key-subtitle = Server-aanmeldtoken
picker-folder-subtitle = Organiseer je items

## Tray
tray-show-hide = Tonen / Verbergen
tray-lock-vault = Kluis vergrendelen
tray-exit = Afsluiten

# Endonym — see comment in the canonical en file.
language-name-self = Nederlands

## Settings modal
settings-title = Instellingen
settings-tab-security = Beveiliging
settings-tab-integrations = Integraties
settings-tab-autotype = Auto-typen en kopiëren
settings-tab-appearance = Uiterlijk
settings-tab-advanced = Geavanceerd

settings-toast-not-supported = Deze instelling wordt nog niet ondersteund.

# Security tab
settings-security-access-options = Toegangsopties
settings-security-open-at-login = Bitwarden openen bij apparaatstart
settings-security-unlock-pin = Ontgrendelen met PIN
settings-security-unlock-touch = Ontgrendelen met Touch ID
settings-security-session-timeout = Sessietime-out
settings-security-lock-after = Vergrendelen na
settings-security-logout-after = Uitloggen na
settings-security-lock-on-system-lock = Vergrendelen wanneer systeem is vergrendeld

# Shared duration strings used by all the time-based dropdowns (lock after,
# log out after, clear clipboard after). Plurals come from Fluent selectors
# so every language can pick the right variant for the supplied number.
settings-duration-seconds = { $n ->
    [one] { $n } seconde
   *[other] { $n } seconden
  }
settings-duration-minutes = { $n ->
    [one] { $n } minuut
   *[other] { $n } minuten
  }
settings-duration-hours = { $n ->
    [one] { $n } uur
   *[other] { $n } uur
  }
settings-duration-never = Nooit

# Integrations tab
settings-integrations-browser = Browser-integratie
settings-integrations-browser-enable = Browser-integratie inschakelen
settings-integrations-browser-fingerprint = Verificatie-vingerafdruk vereisen
settings-integrations-ssh = SSH-agent
settings-integrations-ssh-enable = SSH-agent inschakelen
settings-integrations-ssh-prompt = Promptgedrag
settings-ssh-prompt-always = Ask (default: allow once)
settings-ssh-prompt-never = Nooit
settings-ssh-prompt-remember = Ask (default: allow until lock)

# Autotype and copy tab
settings-autotype-heading = Auto-typen
settings-autotype-enable = Auto-typen inschakelen
settings-clipboard-heading = Klembord
settings-clipboard-clear-after = Klembord wissen na
settings-clipboard-minimize-on-copy = Minimaliseren bij kopiëren

# Appearance tab
settings-appearance-theme-heading = Thema
settings-appearance-theme = Thema
settings-appearance-theme-system = Systeem
settings-appearance-theme-light = Licht
settings-appearance-theme-dark = Donker
settings-appearance-language = Taal
settings-appearance-language-system = Systeem
settings-appearance-display-heading = Weergave
settings-appearance-show-favicons = Pictogrammen tonen voor URL's

# Advanced tab
settings-advanced-tray = Systeemvak
settings-advanced-tray-enable = Systeemvakpictogram tonen
settings-advanced-minimize-to-tray = Minimaliseren naar systeemvak
settings-advanced-close-to-tray = Sluiten naar systeemvak
settings-advanced-platform = Platform
settings-advanced-always-show-dock = Dock-pictogram altijd tonen
settings-advanced-allow-screenshots = Schermafbeeldingen toestaan

## Send list
send-title = Send
send-new-button = Nieuw
send-search-placeholder = Sends doorzoeken
send-column-name = Naam
send-column-deletion = Verwijderdatum
send-empty-body = Je hebt nog geen Sends aangemaakt.
send-toast-item-saved = Send opgeslagen
send-toast-item-deleted = Send verwijderd
send-toast-copied-link = Send-link gekopieerd
send-toast-copied-password = Wachtwoord gekopieerd
send-toast-load-failed-title = Laden mislukt
send-toast-load-failed-body = De Send kon niet worden geladen. Probeer het opnieuw.
send-toast-save-failed-title = Opslaan mislukt
send-toast-save-failed-body = De Send kon niet worden opgeslagen. Probeer het opnieuw.
send-toast-delete-failed-title = Verwijderen mislukt
send-toast-delete-failed-body = De Send kon niet worden verwijderd. Probeer het opnieuw.

## Send form
send-form-title-new-text = Tekst-Send aanmaken
send-form-title-new-file = Bestand-Send aanmaken
send-form-title-edit-text = Tekst-Send bewerken
send-form-title-edit-file = Bestand-Send bewerken
send-form-details-heading = Send-details
send-form-additional-heading = Extra opties
send-form-name = Naam (verplicht)
send-form-text = Te delen tekst (verplicht)
send-form-hide-text = Tekst standaard verbergen
send-form-file-choose = Bestand kiezen
send-form-file-choose-placeholder = Geen bestand geselecteerd
send-form-file-name = Bestandsnaam
send-form-file-size = Grootte
send-form-deletion-date = Verwijderdatum
send-form-deletion-hint = De Send wordt definitief verwijderd op {$date}.
send-form-who-can-view = Wie kan bekijken
send-form-access-link = Iedereen met de link
send-form-access-people = Specifieke personen
send-form-access-password = Iedereen met een wachtwoord dat door jou is ingesteld
send-form-password = Wachtwoord (verplicht)
send-form-password-hint = Personen moeten het wachtwoord invoeren om deze Send te bekijken.
send-form-emails = E-mails (verplicht)
send-form-send-link = Send-link
send-form-limit-views = Weergaven beperken
send-form-limit-views-hint = Niemand kan deze Send bekijken nadat de limiet is bereikt.
send-form-views-left = Niemand kan deze Send bekijken nadat de limiet is bereikt. Nog {$count} weergaven.
send-form-hide-email = Verberg je e-mailadres voor kijkers.
send-form-private-note = Privénotitie
send-form-save = Opslaan
send-form-cancel = Annuleren

send-form-preset-1h = 1 uur
send-form-preset-1d = 1 dag
send-form-preset-2d = 2 dagen
send-form-preset-3d = 3 dagen
send-form-preset-7d = 7 dagen
send-form-preset-14d = 14 dagen
send-form-preset-30d = 30 dagen

send-delete-modal-title = Send verwijderen
send-delete-modal-body = Weet je zeker dat je "{$name}" wilt verwijderen?
send-delete-modal-cancel = Annuleren
send-delete-modal-confirm = Verwijderen

## Generator
generator-title = Generator
generator-tab-password = Wachtwoord
generator-tab-passphrase = Wachtwoordzin
generator-tab-username = Gebruikersnaam

# Password tab
generator-length = Lengte
generator-length-hint = De waarde moet tussen 5 en 128 liggen.
generator-include = Opnemen
generator-include-uppercase = A-Z
generator-include-lowercase = a-z
generator-include-numbers = 0-9
generator-include-special = !@#$%^&*
generator-min-number = Minimum aantal cijfers
generator-min-special = Minimum aantal speciale tekens
generator-avoid-ambiguous = Dubbelzinnige tekens vermijden

# Passphrase tab
generator-num-words = Aantal woorden
generator-num-words-hint = De waarde moet tussen 3 en 20 liggen. Gebruik 6 of meer woorden voor een sterke wachtwoordzin.
generator-word-separator = Woordscheiding
generator-passphrase-capitalize = Met hoofdletter
generator-passphrase-include-number = Cijfer opnemen

# Username tab
generator-username-type = Type
generator-username-kind-word = Willekeurig woord
generator-username-kind-subaddress = E-mail met subadres
generator-username-kind-catchall = Catch-all e-mail
generator-username-capitalize = Met hoofdletter
generator-username-include-number = Cijfer opnemen
generator-username-email = E-mail
generator-username-domain = Domein

# History
generator-history-open = Generator-geschiedenis
generator-history-title = Generator-geschiedenis
generator-history-heading = Recent
generator-history-empty = Geen recente waarden.
generator-history-clear = Geschiedenis wissen
generator-history-just-now = zojuist
generator-history-minutes-ago = {$count} m geleden
generator-history-hours-ago = {$count} u geleden
generator-history-days-ago = {$count} d geleden

# Toasts
generator-toast-copied = Gekopieerd
generator-toast-failed = Genereren mislukt

# Magnify launcher
magnify-search-placeholder = Bitwarden Magnify
magnify-locked-title = Je kluis is vergrendeld
magnify-locked-subtitle = Open Bitwarden om te ontgrendelen
magnify-open-bitwarden = Bitwarden openen
magnify-no-results = Geen overeenkomende items
magnify-copy-password = Wachtwoord kopiëren
magnify-copy-username = Gebruikersnaam kopiëren
magnify-hint-navigate = Navigeren

## Import modal
import-modal-title = Gegevens importeren
import-modal-section-destination = Bestemming
import-modal-vault-label = Kluis (verplicht)
import-modal-vault-personal = Mijn kluis
import-modal-folder-label = Map
import-modal-folder-placeholder = - Map selecteren -
import-modal-collection-label = Collectie (verplicht)
import-modal-collection-placeholder = - Collectie selecteren -
import-modal-section-data = Gegevens
import-modal-file-format-label = Bestandsformaat (verplicht)
import-modal-file-helper = Selecteer het importbestand
import-modal-choose-file = Bestand kiezen
import-modal-no-file = Geen bestand gekozen
import-modal-paste-label = of kopieer/plak de inhoud van het importbestand
import-modal-submit = Gegevens importeren
import-modal-cancel = Annuleren
import-toast-unimplemented = Importeren is nog niet aangesloten — komt binnenkort

## Export modal
export-modal-title = Kluis exporteren
export-modal-vault-label = Exporteren vanuit (verplicht)
export-modal-vault-personal = Mijn kluis
export-modal-banner-personal-title = Persoonlijke kluis exporteren
export-modal-banner-personal = Alleen items uit de persoonlijke kluis die zijn gekoppeld aan { $email } worden geëxporteerd. Items uit organisatie-kluizen worden niet meegenomen. Alleen iteminformatie wordt geëxporteerd; bijlagen worden niet meegenomen.
export-modal-banner-org-title = Organisatie-kluis exporteren
export-modal-banner-org-body = Alleen de organisatie-kluis gekoppeld aan { $name } wordt geëxporteerd. Items in persoonlijke kluizen of andere organisaties worden niet meegenomen.
export-modal-file-format-label = Bestandsformaat (verplicht)
export-modal-password-label = Bestandswachtwoord (verplicht)
export-modal-continue = Doorgaan
export-modal-cancel = Annuleren
export-confirm-title = Kluisexport bevestigen
export-confirm-warning-unencrypted = Deze export bevat je kluisgegevens in onversleuteld formaat. Bewaar of verstuur het geëxporteerde bestand niet via onveilige kanalen (zoals e-mail). Verwijder het direct na gebruik.
export-confirm-warning-file-encrypted = Deze bestandsexport wordt met een wachtwoord beveiligd en vereist het bestandswachtwoord om te ontsleutelen.
export-confirm-password-label = Hoofdwachtwoord (verplicht)
export-confirm-helper = Bevestig je identiteit om door te gaan.
export-confirm-error = Ongeldig hoofdwachtwoord.
export-toast-success = Kluis geëxporteerd naar { $path }
export-toast-failed-title = Export mislukt
export-toast-failed-body = De kluis kon niet worden geëxporteerd. Probeer het opnieuw.

# Composable vault filters
sidebar-group-vaults = Kluizen
sidebar-group-view = Weergave
sidebar-group-types = Itemtypen
sidebar-filter-all-vaults = Alle kluizen
sidebar-filter-active-items = Actieve items
sidebar-filter-all-types = Alle itemtypen

sidebar-group-folders = Mappen
sidebar-filter-all-folders = Alle mappen
sidebar-filter-no-folder = Geen map
sidebar-group-collections = Collecties
sidebar-filter-all-collections = Alle collecties
sidebar-filter-no-collection = Geen collectie

vault-column-last-changed = Laatst gewijzigd
vault-column-base-url = Basis-URL
vault-filter-folder = Map: { $name }
vault-filter-collection = Collectie: { $name }
vault-filter-search = Zoeken: “{ $query }”

ssh-approval-title = Allow SSH authentication?
ssh-approval-key = Key: { $name }
ssh-approval-user = SSH user: { $name }
ssh-approval-process = Local process: { $name }
ssh-approval-process-note = The process name is informational and does not verify an application or destination hostname.
ssh-approval-unbound = No verified host-key binding. This request can only be approved once.
ssh-approval-hosts = Verified host keys (forwarding path → destination):
ssh-approval-deny = Deny
ssh-approval-once = Allow once
ssh-approval-remember = Allow until lock
ssh-approval-timeout = No response within 45 seconds denies the request.
ssh-agent-copy-socket = Copy socket path
ssh-agent-socket-copied = SSH socket path copied
ssh-agent-help = Set SSH_AUTH_SOCK or the SSH IdentityAgent option to this path. Only the active unlocked account provides keys.
ssh-agent-unattended = Without prompts, any local process of your user can use permitted SSH keys while the active vault is unlocked.
ssh-agent-protected = Deleted, archived, hidden-password and master-password-reprompt keys are not offered. Remembered approvals require a verified session binding and a host-bound signature.

ssh-agent-copy-setup = Copy SSH_AUTH_SOCK command
ssh-agent-setup-copied = SSH_AUTH_SOCK command copied
ssh-approval-bound-once = The host key is verified, but this signing request is not host-bound. Only one-use approval is available.

ssh-approval-window-open = SSH approval is waiting in its confirmation window. Press Esc to cancel.
