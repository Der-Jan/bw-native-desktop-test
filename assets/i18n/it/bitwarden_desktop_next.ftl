## App-wide
app-title = Bitwarden [Next]
about-window-title = Informazioni su Bitwarden

## Login — unlock
login-unlock-title = La tua cassaforte è bloccata
login-unlock-password-placeholder = Password principale (obbligatoria)
login-unlock-pin-placeholder = PIN (obbligatorio)
login-unlock-button = Sblocca
login-unlock-or = oppure
login-unlock-biometrics-button = Sblocca con biometria
login-unlock-pin-button = Sblocca con PIN
login-unlock-master-password-button = Sblocca con password principale
login-log-out = Esci

## Login — email entry
login-email-title = Accedi a Bitwarden
login-email-placeholder = Indirizzo email (obbligatorio)
login-email-remember = Ricorda email
login-email-continue = Continua
login-email-or = Oppure
login-email-sso = Usa Single Sign-On
login-email-new-prompt = Nuovo su Bitwarden?
login-email-create-account = Crea account

## Login — password entry
login-password-title = Bentornato
login-password-placeholder = Password principale (obbligatoria)
login-password-get-hint = Ottieni il suggerimento della password principale
login-password-submit = Accedi con la password principale
login-password-back = Indietro

## Login — server selector
login-server-accessing = Accesso a { $server }
login-server-accessing-label = Accesso a:
login-server-self-hosted = Self-hosted

## Login — self-hosted environment modal
login-self-hosted-modal-title = Ambiente self-hosted
login-self-hosted-modal-url-label = URL del server
login-self-hosted-modal-url-helper = Specifica l'URL di base della tua installazione Bitwarden self-hosted. Esempio: https://bitwarden.azienda.com
login-self-hosted-modal-url-error = L'URL deve iniziare con https://
login-self-hosted-modal-save = Salva
login-self-hosted-modal-cancel = Annulla

## Login — toasts
login-toast-unlock-failed-title = Sblocco non riuscito
login-toast-unlock-failed-body = Verifica la tua password principale e riprova.
login-toast-login-failed-title = Accesso non riuscito
login-toast-login-failed-body = Verifica la tua email e password e riprova.
login-toast-pin-unsupported = Lo sblocco con PIN non è ancora supportato
login-toast-biometrics-unsupported = Lo sblocco biometrico non è ancora supportato

## About dialog
about-version-label = Versione
about-sdk-version-label = Versione SDK
about-os-label = Sistema
about-architecture-label = Architettura
about-copy-button = Copia
about-close-button = Chiudi

## Sidebar — sections and filters
sidebar-section-vault = Cassaforte
sidebar-section-send = Send
sidebar-filter-my-vault = La mia cassaforte
sidebar-filter-favorites = Preferiti
sidebar-filter-logins = Accessi
sidebar-filter-cards = Carte
sidebar-filter-identities = Identità
sidebar-filter-notes = Note
sidebar-filter-ssh-keys = Chiavi SSH
sidebar-filter-archive = Archivio
sidebar-filter-trash = Cestino
sidebar-filter-text-send = Testo
sidebar-filter-file-send = File
sidebar-item-generator = Generatore
sidebar-item-import = Importa
sidebar-item-export = Esporta

## Vault list
vault-title = Cassaforte
vault-new-button = Nuovo
vault-search-placeholder = Cerca
vault-column-name = Nome
vault-column-options = Opzioni
vault-toast-item-saved = Elemento salvato
vault-toast-save-failed-title = Salvataggio non riuscito
vault-toast-save-failed-body = Impossibile salvare l'elemento. Riprova.
vault-toast-decrypt-failed-title = Decifratura non riuscita
vault-toast-decrypt-failed-body = Impossibile caricare l'elemento. Riprova.
vault-toast-copied-username = Nome utente copiato
vault-toast-copied-password = Password copiata
vault-toast-copied-website = Sito web copiato
vault-toast-copied-totp = Codice di verifica copiato
vault-toast-copied-field = Campo copiato
vault-toast-copied-private-key = Chiave privata copiata
vault-toast-copied-public-key = Chiave pubblica copiata
vault-toast-copied-fingerprint = Impronta copiata
vault-toast-item-deleted = Elemento spostato nel cestino
vault-toast-delete-failed-title = Eliminazione non riuscita
vault-toast-delete-failed-body = Impossibile eliminare l'elemento. Riprova.
vault-delete-modal-title = Eliminare l'elemento?
vault-delete-modal-body = "{ $name }" verrà spostato nel cestino.
vault-delete-modal-cancel = Annulla
vault-delete-modal-confirm = Elimina

## Account switcher
account-switcher-other-accounts = Altri account Bitwarden
account-switcher-options = Opzioni
account-switcher-lock-now = Blocca ora
account-switcher-log-out = Esci
account-switcher-lock-all = Blocca tutti gli account
account-switcher-settings = Impostazioni
account-switcher-add = Aggiungi account

## Detail pane — headers per cipher type
detail-header-login = Visualizza accesso
detail-header-card = Visualizza carta
detail-header-identity = Visualizza identità
detail-header-note = Visualizza nota
detail-header-ssh-key = Visualizza chiave SSH
detail-header-bank-account = Visualizza conto bancario
detail-header-drivers-license = Visualizza patente di guida
detail-header-passport = Visualizza passaporto

## Detail pane — section labels
detail-section-item-details = Dettagli elemento
detail-section-login-credentials = Credenziali di accesso
detail-section-autofill-options = Opzioni di compilazione automatica
detail-section-card-details = Dettagli carta
detail-section-personal-details = Dettagli personali
detail-section-note = Nota
detail-section-ssh-key = Chiave SSH

## Detail pane — fields
detail-field-name = Nome
detail-field-notes = Note
detail-field-username = Nome utente
detail-field-password = Password
detail-field-totp = Codice di verifica (TOTP)
detail-totp-invalid = Seed TOTP non valido
detail-field-website = Sito web
detail-field-cardholder-name = Nome del titolare
detail-field-brand = Marca
detail-field-number = Numero
detail-field-expiration = Scadenza
detail-field-security-code = Codice di sicurezza
detail-field-email = Email
detail-field-phone = Telefono
detail-field-company = Azienda
detail-field-address = Indirizzo
detail-field-city-region = Città / regione
detail-field-public-key = Chiave pubblica
detail-field-private-key = Chiave privata
detail-field-fingerprint = Impronta
detail-empty-credentials = Nessuna credenziale
detail-empty-card = Nessun dettaglio della carta
detail-empty-identity = Nessun dettaglio dell'identità
detail-section-custom-fields = Campi personalizzati
detail-field-passkey = Passkey
detail-field-passkey-created = Creata il { $date }
detail-field-boolean-true = Sì
detail-field-boolean-false = No
detail-edit-button = Modifica

## Cipher form — header titles
form-title-new-item = Nuovo elemento
form-title-edit-login = Modifica accesso
form-title-edit-card = Modifica carta
form-title-edit-identity = Modifica identità
form-title-edit-note = Modifica nota
form-title-edit-ssh-key = Modifica chiave SSH
form-title-edit-bank-account = Modifica conto bancario
form-title-edit-drivers-license = Modifica patente di guida
form-title-edit-passport = Modifica passaporto

## Cipher form — buttons
form-save = Salva
form-saving = Salvataggio…
form-cancel = Annulla

## Cipher form — sections
form-section-item-details = Dettagli elemento
form-section-login-credentials = Credenziali di accesso
form-section-autofill-options = Opzioni di compilazione automatica
form-section-card-details = Dettagli carta
form-section-personal-details = Dettagli personali
form-section-identification = Identificazione
form-section-contact-info = Contatti
form-section-address = Indirizzo
form-section-ssh-key = Chiave SSH
form-section-additional-options = Opzioni aggiuntive
form-section-custom-fields = Campi personalizzati

## Cipher form — item details
form-name = Nome (obbligatorio)
form-favorite = Preferito
form-reprompt = Richiedi nuovamente la password principale
form-notes = Note
form-folder = Cartella
form-folder-none = Nessuna cartella
form-organization = Organizzazione
form-organization-personal = Personale (io)
form-collections = Raccolte
form-collections-none = Nessuna raccolta
form-collections-selected = { $count } selezionate
form-collections-empty-in-org = Nessuna raccolta in questa organizzazione

## Cipher form — login
form-username = Nome utente
form-password = Password
form-totp = Chiave dell'autenticatore (TOTP)

## Cipher form — websites
form-uri = Sito web (URI)
form-uri-empty = Nessun sito web
form-add-website = Aggiungi sito web

## Cipher form — card
form-card-cardholder = Nome del titolare
form-card-brand = Marca
form-card-number = Numero
form-card-exp-month = Mese di scadenza
form-card-exp-year = Anno di scadenza
form-card-code = Codice di sicurezza
form-card-brand-placeholder = -- Seleziona --
form-card-month-placeholder = -- Mese --

## Cipher form — identity
form-identity-title = Titolo
form-identity-title-placeholder = -- Titolo --
# Identity title labels. Same convention as card brands — the stored value
# is the canonical English string; these keys only localize the display.
form-identity-title-mr = Sig.
form-identity-title-mrs = Sig.ra
form-identity-title-ms = Sig.na
form-identity-title-mx = Mx
form-identity-title-dr = Dott.
form-identity-first-name = Nome
form-identity-middle-name = Secondo nome
form-identity-last-name = Cognome
form-identity-username = Nome utente
form-identity-company = Azienda
form-identity-ssn = Codice fiscale
form-identity-passport = Numero di passaporto
form-identity-license = Numero di patente
form-identity-email = Email
form-identity-phone = Telefono
form-identity-address1 = Indirizzo riga 1
form-identity-address2 = Indirizzo riga 2
form-identity-address3 = Indirizzo riga 3
form-identity-city = Città / comune
form-identity-state = Stato / provincia
form-identity-postal = CAP
form-identity-country = Paese

## Cipher form — SSH key
form-ssh-public-key = Chiave pubblica
form-ssh-private-key = Chiave privata
form-ssh-fingerprint = Impronta

## Cipher form — custom fields
form-custom-field-type = Tipo
form-custom-field-name = Nome
form-custom-field-value = Valore
form-custom-field-type-text = Testo
form-custom-field-type-hidden = Nascosto
form-custom-field-type-boolean = Booleano
form-custom-field-type-linked = Collegato
form-custom-field-enabled = Abilitato
form-custom-field-empty = Nessun campo personalizzato
form-add-custom-field = Aggiungi campo personalizzato
form-custom-field-linked-unsupported = I campi collegati non sono ancora supportati

## Toast — shared messages
toast-required-fields = Compila i campi obbligatori.

## Menu bar — top-level
menu-file = File
menu-edit = Modifica
menu-view = Visualizza
menu-account = Account
menu-window = Finestra
menu-help = Aiuto

## Menu — File
menu-file-new-login = Nuovo accesso
menu-file-new-item = Nuovo elemento
menu-file-new-item-login = Accesso
menu-file-new-item-card = Carta
menu-file-new-item-identity = Identità
menu-file-new-item-secure-note = Nota sicura
menu-file-new-item-ssh-key = Chiave SSH
menu-file-new-folder = Nuova cartella
menu-file-sync-now = Sincronizza ora
menu-file-import = Importa
menu-file-export = Esporta
menu-file-settings = Impostazioni
menu-file-lock-vault = Blocca cassaforte
menu-file-lock-all-vaults = Blocca tutte le casseforti
menu-file-log-out = Esci
menu-file-quit = Esci da Bitwarden

## Menu — Edit
menu-edit-undo = Annulla
menu-edit-redo = Ripeti
menu-edit-cut = Taglia
menu-edit-copy = Copia
menu-edit-paste = Incolla
menu-edit-select-all = Seleziona tutto
menu-edit-copy-username = Copia nome utente
menu-edit-copy-password = Copia password
menu-edit-copy-totp = Copia codice di verifica (TOTP)

## Menu — View
menu-view-search = Cerca nella cassaforte
menu-view-generator = Generatore
menu-view-generator-history = Cronologia generatore
menu-view-zoom-in = Aumenta zoom
menu-view-zoom-out = Riduci zoom
menu-view-reset-zoom = Reimposta zoom
menu-view-toggle-fullscreen = Attiva/disattiva schermo intero

## Menu — Account
menu-account-change-password = Cambia password principale
menu-account-two-step = Verifica in due passaggi
menu-account-fingerprint = Frase impronta
menu-account-delete = Elimina account

## Menu — Window
menu-window-minimize = Riduci a icona
menu-window-hide-to-tray = Nascondi nella tray
menu-window-always-on-top = Sempre in primo piano
menu-window-close = Chiudi

## Menu — Help
menu-help-feedback = Aiuto e feedback
menu-help-bug = Segnala un bug
menu-help-legal = Note legali
menu-help-legal-tos = Termini di servizio
menu-help-legal-privacy = Informativa sulla privacy
menu-help-follow = Seguici
menu-help-web-vault = Vai alla cassaforte web
menu-help-browser-extension = Ottieni estensione del browser
menu-help-about = Informazioni su Bitwarden

menu-toast-sync-success = Cassaforte sincronizzata
menu-toast-sync-failed-title = Sincronizzazione non riuscita
menu-toast-sync-failed-body = Impossibile sincronizzare la cassaforte. Riprova più tardi.

menu-fingerprint-title = La frase impronta del tuo account:
menu-fingerprint-learn-more = Scopri di più
menu-fingerprint-close = Chiudi

new-folder-modal-title = Nuova cartella
new-folder-modal-field-label = Nome cartella (obbligatorio)
new-folder-modal-helper = Annida una cartella aggiungendo il nome della cartella superiore seguito da "/". Esempio: Sociale/Forum
new-folder-modal-save = Salva
new-folder-modal-cancel = Annulla
new-folder-toast-success = Cartella creata
new-folder-toast-failed-title = Impossibile creare la cartella
new-folder-toast-failed-body = Non è stato possibile salvare la cartella. Riprova.

## New-item picker modal
picker-title = Scegli un elemento da aggiungere
picker-folder = Cartella
picker-login-subtitle = Sito web o app
picker-card-subtitle = Carta di credito o debito
picker-bank-account-subtitle = Dati bancari
picker-drivers-license-subtitle = Dati della patente
picker-passport-subtitle = Documento di viaggio
picker-identity-subtitle = Informazioni personali
picker-secure-note-subtitle = Testo importante
picker-ssh-key-subtitle = Token di accesso al server
picker-folder-subtitle = Organizza i tuoi elementi

## Tray
tray-show-hide = Mostra / Nascondi
tray-lock-vault = Blocca cassaforte
tray-exit = Esci

# Endonym — see comment in the canonical en file.
language-name-self = Italiano

## Settings modal
settings-title = Impostazioni
settings-tab-security = Sicurezza
settings-tab-integrations = Integrazioni
settings-tab-autotype = Digitazione automatica e copia
settings-tab-appearance = Aspetto
settings-tab-advanced = Avanzate

settings-toast-not-supported = Questa impostazione non è ancora supportata.

# Security tab
settings-security-access-options = Opzioni di accesso
settings-security-open-at-login = Apri Bitwarden all'avvio del dispositivo
settings-security-unlock-pin = Sblocca con PIN
settings-security-unlock-touch = Sblocca con Touch ID
settings-security-session-timeout = Timeout della sessione
settings-security-lock-after = Blocca dopo
settings-security-logout-after = Disconnetti dopo
settings-security-lock-on-system-lock = Blocca quando il sistema è bloccato

# Shared duration strings used by all the time-based dropdowns (lock after,
# log out after, clear clipboard after). Plurals come from Fluent selectors
# so every language can pick the right variant for the supplied number.
settings-duration-seconds = { $n ->
    [one] { $n } secondo
   *[other] { $n } secondi
  }
settings-duration-minutes = { $n ->
    [one] { $n } minuto
   *[other] { $n } minuti
  }
settings-duration-hours = { $n ->
    [one] { $n } ora
   *[other] { $n } ore
  }
settings-duration-never = Mai

# Integrations tab
settings-integrations-browser = Integrazione del browser
settings-integrations-browser-enable = Abilita integrazione del browser
settings-integrations-browser-fingerprint = Richiedi impronta di verifica
settings-integrations-ssh = Agente SSH
settings-integrations-ssh-enable = Abilita agente SSH
settings-integrations-ssh-prompt = Comportamento della richiesta
settings-ssh-prompt-always = Ask (default: allow once)
settings-ssh-prompt-never = Mai
settings-ssh-prompt-remember = Ask (default: allow until lock)

# Autotype and copy tab
settings-autotype-heading = Digitazione automatica
settings-autotype-enable = Abilita digitazione automatica
settings-clipboard-heading = Appunti
settings-clipboard-clear-after = Cancella appunti dopo
settings-clipboard-minimize-on-copy = Riduci a icona alla copia

# Appearance tab
settings-appearance-theme-heading = Tema
settings-appearance-theme = Tema
settings-appearance-theme-system = Sistema
settings-appearance-theme-light = Chiaro
settings-appearance-theme-dark = Scuro
settings-appearance-language = Lingua
settings-appearance-language-system = Sistema
settings-appearance-display-heading = Visualizzazione
settings-appearance-show-favicons = Mostra icone per gli URL

# Advanced tab
settings-advanced-tray = Tray
settings-advanced-tray-enable = Mostra icona nella tray
settings-advanced-minimize-to-tray = Riduci a icona nella tray
settings-advanced-close-to-tray = Chiudi nella tray
settings-advanced-platform = Piattaforma
settings-advanced-always-show-dock = Mostra sempre l'icona del Dock
settings-advanced-allow-screenshots = Consenti screenshot

## Send list
send-title = Send
send-new-button = Nuovo
send-search-placeholder = Cerca tra i Send
send-column-name = Nome
send-column-deletion = Data di eliminazione
send-empty-body = Non hai ancora creato nessun Send.
send-toast-item-saved = Send salvato
send-toast-item-deleted = Send eliminato
send-toast-copied-link = Link Send copiato
send-toast-copied-password = Password copiata
send-toast-load-failed-title = Caricamento non riuscito
send-toast-load-failed-body = Impossibile caricare il Send. Riprova.
send-toast-save-failed-title = Salvataggio non riuscito
send-toast-save-failed-body = Impossibile salvare il Send. Riprova.
send-toast-delete-failed-title = Eliminazione non riuscita
send-toast-delete-failed-body = Impossibile eliminare il Send. Riprova.

## Send form
send-form-title-new-text = Crea Send di testo
send-form-title-new-file = Crea Send di file
send-form-title-edit-text = Modifica Send di testo
send-form-title-edit-file = Modifica Send di file
send-form-details-heading = Dettagli del Send
send-form-additional-heading = Opzioni aggiuntive
send-form-name = Nome (obbligatorio)
send-form-text = Testo da condividere (obbligatorio)
send-form-hide-text = Nascondi testo per impostazione predefinita
send-form-file-choose = Scegli file
send-form-file-choose-placeholder = Nessun file selezionato
send-form-file-name = Nome file
send-form-file-size = Dimensione
send-form-deletion-date = Data di eliminazione
send-form-deletion-hint = Il Send verrà eliminato definitivamente il {$date}.
send-form-who-can-view = Chi può visualizzare
send-form-access-link = Chiunque abbia il link
send-form-access-people = Persone specifiche
send-form-access-password = Chiunque con una password impostata da te
send-form-password = Password (obbligatoria)
send-form-password-hint = I destinatari dovranno inserire la password per visualizzare questo Send.
send-form-emails = Email (obbligatorio)
send-form-send-link = Link del Send
send-form-limit-views = Limita visualizzazioni
send-form-limit-views-hint = Nessuno potrà visualizzare questo Send dopo aver raggiunto il limite.
send-form-views-left = Nessuno potrà visualizzare questo Send dopo aver raggiunto il limite. {$count} visualizzazioni rimaste.
send-form-hide-email = Nascondi il tuo indirizzo email ai destinatari.
send-form-private-note = Nota privata
send-form-save = Salva
send-form-cancel = Annulla

send-form-preset-1h = 1 ora
send-form-preset-1d = 1 giorno
send-form-preset-2d = 2 giorni
send-form-preset-3d = 3 giorni
send-form-preset-7d = 7 giorni
send-form-preset-14d = 14 giorni
send-form-preset-30d = 30 giorni

send-delete-modal-title = Elimina Send
send-delete-modal-body = Sei sicuro di voler eliminare "{$name}"?
send-delete-modal-cancel = Annulla
send-delete-modal-confirm = Elimina

## Generator
generator-title = Generatore
generator-tab-password = Password
generator-tab-passphrase = Frase segreta
generator-tab-username = Nome utente

# Password tab
generator-length = Lunghezza
generator-length-hint = Il valore deve essere compreso tra 5 e 128.
generator-include = Includi
generator-include-uppercase = A-Z
generator-include-lowercase = a-z
generator-include-numbers = 0-9
generator-include-special = !@#$%^&*
generator-min-number = Numeri minimi
generator-min-special = Caratteri speciali minimi
generator-avoid-ambiguous = Evita caratteri ambigui

# Passphrase tab
generator-num-words = Numero di parole
generator-num-words-hint = Il valore deve essere compreso tra 3 e 20. Usa 6 parole o più per generare una frase segreta robusta.
generator-word-separator = Separatore di parole
generator-passphrase-capitalize = Maiuscolo iniziale
generator-passphrase-include-number = Includi numero

# Username tab
generator-username-type = Tipo
generator-username-kind-word = Parola casuale
generator-username-kind-subaddress = Email con sottoindirizzo
generator-username-kind-catchall = Email catch-all
generator-username-capitalize = Maiuscolo iniziale
generator-username-include-number = Includi numero
generator-username-email = Email
generator-username-domain = Dominio

# History
generator-history-open = Cronologia generatore
generator-history-title = Cronologia generatore
generator-history-heading = Recenti
generator-history-empty = Nessun valore recente.
generator-history-clear = Cancella cronologia
generator-history-just-now = adesso
generator-history-minutes-ago = {$count} min fa
generator-history-hours-ago = {$count} h fa
generator-history-days-ago = {$count} g fa

# Toasts
generator-toast-copied = Copiato
generator-toast-failed = Generazione non riuscita

# Magnify launcher
magnify-search-placeholder = Bitwarden Magnify
magnify-locked-title = La tua cassaforte è bloccata
magnify-locked-subtitle = Apri Bitwarden per sbloccare
magnify-open-bitwarden = Apri Bitwarden
magnify-no-results = Nessun elemento corrispondente
magnify-copy-password = Copia password
magnify-copy-username = Copia nome utente
magnify-hint-navigate = Naviga

## Import modal
import-modal-title = Importa dati
import-modal-section-destination = Destinazione
import-modal-vault-label = Cassaforte (obbligatoria)
import-modal-vault-personal = La mia cassaforte
import-modal-folder-label = Cartella
import-modal-folder-placeholder = - Seleziona una cartella -
import-modal-collection-label = Collezione (obbligatoria)
import-modal-collection-placeholder = - Seleziona una raccolta -
import-modal-section-data = Dati
import-modal-file-format-label = Formato file (obbligatorio)
import-modal-file-helper = Seleziona il file da importare
import-modal-choose-file = Scegli file
import-modal-no-file = Nessun file scelto
import-modal-paste-label = oppure copia/incolla il contenuto del file da importare
import-modal-submit = Importa dati
import-modal-cancel = Annulla
import-toast-unimplemented = L'importazione non è ancora collegata — in arrivo

## Export modal
export-modal-title = Esporta cassaforte
export-modal-vault-label = Esporta da (obbligatorio)
export-modal-vault-personal = La mia cassaforte
export-modal-banner-personal-title = Esportazione cassaforte individuale
export-modal-banner-personal = Verranno esportati solo gli elementi della cassaforte individuale associati a { $email }. Gli elementi delle casseforti delle organizzazioni non saranno inclusi. Verranno esportate solo le informazioni degli elementi e non gli allegati associati.
export-modal-banner-org-title = Esportazione cassaforte dell'organizzazione
export-modal-banner-org-body = Verrà esportata solo la cassaforte dell'organizzazione associata a { $name }. Gli elementi delle casseforti individuali o di altre organizzazioni non saranno inclusi.
export-modal-file-format-label = Formato file (obbligatorio)
export-modal-password-label = Password del file (obbligatoria)
export-modal-continue = Continua
export-modal-cancel = Annulla
export-confirm-title = Conferma esportazione cassaforte
export-confirm-warning-unencrypted = Questa esportazione contiene i dati della tua cassaforte in formato non cifrato. Non dovresti memorizzare o inviare il file esportato tramite canali non sicuri (come l'email). Eliminalo immediatamente dopo l'uso.
export-confirm-warning-file-encrypted = Questa esportazione del file sarà protetta da password e richiederà la password del file per essere decifrata.
export-confirm-password-label = Password principale (obbligatoria)
export-confirm-helper = Conferma la tua identità per continuare.
export-confirm-error = Password principale non valida.
export-toast-success = Cassaforte esportata in { $path }
export-toast-failed-title = Esportazione non riuscita
export-toast-failed-body = Impossibile esportare la cassaforte. Riprova.

# Composable vault filters
sidebar-group-vaults = Casseforti
sidebar-group-view = Vista
sidebar-group-types = Tipi di elementi
sidebar-filter-all-vaults = Tutte le casseforti
sidebar-filter-active-items = Elementi attivi
sidebar-filter-all-types = Tutti i tipi

sidebar-group-folders = Cartelle
sidebar-filter-all-folders = Tutte le cartelle
sidebar-filter-no-folder = Nessuna cartella
sidebar-group-collections = Raccolte
sidebar-filter-all-collections = Tutte le raccolte
sidebar-filter-no-collection = Nessuna raccolta

vault-column-last-changed = Ultima modifica
vault-column-base-url = URL di base
vault-filter-folder = Cartella: { $name }
vault-filter-collection = Raccolta: { $name }
vault-filter-search = Ricerca: «{ $query }»

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
