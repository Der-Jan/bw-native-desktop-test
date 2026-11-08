## App-wide
app-title = Bitwarden [Next]
about-window-title = À propos de Bitwarden

## Login — unlock
login-unlock-title = Votre coffre est verrouillé
login-unlock-password-placeholder = Mot de passe principal (obligatoire)
login-unlock-pin-placeholder = PIN (obligatoire)
login-unlock-button = Déverrouiller
login-unlock-or = ou
login-unlock-biometrics-button = Déverrouiller avec la biométrie
login-unlock-pin-button = Déverrouiller avec le PIN
login-unlock-master-password-button = Déverrouiller avec le mot de passe principal
login-log-out = Se déconnecter

## Login — email entry
login-email-title = Connectez-vous à Bitwarden
login-email-placeholder = Adresse e-mail (obligatoire)
login-email-remember = Mémoriser l'adresse
login-email-continue = Continuer
login-email-or = Ou
login-email-sso = Utiliser l'authentification unique
login-email-new-prompt = Nouveau sur Bitwarden ?
login-email-create-account = Créer un compte

## Login — password entry
login-password-title = Bon retour
login-password-placeholder = Mot de passe principal (obligatoire)
login-password-get-hint = Obtenir l'indice du mot de passe principal
login-password-submit = Se connecter avec le mot de passe principal
login-password-back = Retour

## Login — server selector
login-server-accessing = Accès à { $server }
login-server-accessing-label = Accès à :
login-server-self-hosted = Auto-hébergé

## Login — self-hosted environment modal
login-self-hosted-modal-title = Environnement auto-hébergé
login-self-hosted-modal-url-label = URL du serveur
login-self-hosted-modal-url-helper = Indiquez l'URL de base de votre installation Bitwarden auto-hébergée. Exemple : https://bitwarden.entreprise.com
login-self-hosted-modal-url-error = L'URL doit commencer par https://
login-self-hosted-modal-save = Enregistrer
login-self-hosted-modal-cancel = Annuler

## Login — toasts
login-toast-unlock-failed-title = Échec du déverrouillage
login-toast-unlock-failed-body = Vérifiez votre mot de passe principal et réessayez.
login-toast-login-failed-title = Échec de la connexion
login-toast-login-failed-body = Vérifiez votre adresse e-mail et votre mot de passe puis réessayez.
login-toast-pin-unsupported = Le déverrouillage par PIN n'est pas encore pris en charge
login-toast-biometrics-unsupported = Le déverrouillage biométrique n'est pas encore pris en charge

## About dialog
about-version-label = Version
about-sdk-version-label = Version du SDK
about-os-label = Système
about-architecture-label = Architecture
about-copy-button = Copier
about-close-button = Fermer

## Sidebar — sections and filters
sidebar-section-vault = Coffre
sidebar-section-send = Send
sidebar-filter-my-vault = Mon coffre
sidebar-filter-favorites = Favoris
sidebar-filter-logins = Identifiants
sidebar-filter-cards = Cartes
sidebar-filter-identities = Identités
sidebar-filter-notes = Notes
sidebar-filter-ssh-keys = Clés SSH
sidebar-filter-archive = Archives
sidebar-filter-trash = Corbeille
sidebar-filter-text-send = Texte
sidebar-filter-file-send = Fichier
sidebar-item-generator = Générateur
sidebar-item-import = Importer
sidebar-item-export = Exporter

## Vault list
vault-title = Coffre
vault-new-button = Nouveau
vault-search-placeholder = Rechercher
vault-column-name = Nom
vault-column-options = Options
vault-toast-item-saved = Élément enregistré
vault-toast-save-failed-title = Échec de l'enregistrement
vault-toast-save-failed-body = Impossible d'enregistrer l'élément. Réessayez.
vault-toast-decrypt-failed-title = Échec du déchiffrement
vault-toast-decrypt-failed-body = Impossible de charger l'élément. Réessayez.
vault-toast-copied-username = Nom d'utilisateur copié
vault-toast-copied-password = Mot de passe copié
vault-toast-copied-website = Site web copié
vault-toast-copied-totp = Code de vérification copié
vault-toast-copied-field = Champ copié
vault-toast-copied-private-key = Clé privée copiée
vault-toast-copied-public-key = Clé publique copiée
vault-toast-copied-fingerprint = Empreinte copiée
vault-toast-item-deleted = Élément déplacé dans la corbeille
vault-toast-delete-failed-title = Échec de la suppression
vault-toast-delete-failed-body = Impossible de supprimer l'élément. Réessayez.
vault-delete-modal-title = Supprimer l'élément ?
vault-delete-modal-body = "{ $name }" sera déplacé dans la corbeille.
vault-delete-modal-cancel = Annuler
vault-delete-modal-confirm = Supprimer

## Account switcher
account-switcher-other-accounts = Autres comptes Bitwarden
account-switcher-options = Options
account-switcher-lock-now = Verrouiller maintenant
account-switcher-log-out = Se déconnecter
account-switcher-lock-all = Verrouiller tous les comptes
account-switcher-settings = Paramètres
account-switcher-add = Ajouter un compte

## Detail pane — headers per cipher type
detail-header-login = Voir l'identifiant
detail-header-card = Voir la carte
detail-header-identity = Voir l'identité
detail-header-note = Voir la note
detail-header-ssh-key = Voir la clé SSH
detail-header-bank-account = Voir le compte bancaire
detail-header-drivers-license = Voir le permis de conduire
detail-header-passport = Voir le passeport

## Detail pane — section labels
detail-section-item-details = Détails de l'élément
detail-section-login-credentials = Identifiants de connexion
detail-section-autofill-options = Options de remplissage automatique
detail-section-card-details = Détails de la carte
detail-section-personal-details = Détails personnels
detail-section-note = Note
detail-section-ssh-key = Clé SSH

## Detail pane — fields
detail-field-name = Nom
detail-field-notes = Notes
detail-field-username = Nom d'utilisateur
detail-field-password = Mot de passe
detail-field-totp = Code de vérification (TOTP)
detail-totp-invalid = Clé TOTP invalide
detail-field-website = Site web
detail-field-cardholder-name = Nom du titulaire
detail-field-brand = Marque
detail-field-number = Numéro
detail-field-expiration = Expiration
detail-field-security-code = Code de sécurité
detail-field-email = E-mail
detail-field-phone = Téléphone
detail-field-company = Société
detail-field-address = Adresse
detail-field-city-region = Ville / région
detail-field-public-key = Clé publique
detail-field-private-key = Clé privée
detail-field-fingerprint = Empreinte
detail-empty-credentials = Aucun identifiant
detail-empty-card = Aucun détail de carte
detail-empty-identity = Aucun détail d'identité
detail-section-custom-fields = Champs personnalisés
detail-field-passkey = Clé d'accès
detail-field-passkey-created = Créée le { $date }
detail-field-boolean-true = Oui
detail-field-boolean-false = Non
detail-edit-button = Modifier

## Cipher form — header titles
form-title-new-item = Nouvel élément
form-title-edit-login = Modifier l'identifiant
form-title-edit-card = Modifier la carte
form-title-edit-identity = Modifier l'identité
form-title-edit-note = Modifier la note
form-title-edit-ssh-key = Modifier la clé SSH
form-title-edit-bank-account = Modifier le compte bancaire
form-title-edit-drivers-license = Modifier le permis de conduire
form-title-edit-passport = Modifier le passeport

## Cipher form — buttons
form-save = Enregistrer
form-saving = Enregistrement…
form-cancel = Annuler

## Cipher form — sections
form-section-item-details = Détails de l'élément
form-section-login-credentials = Identifiants de connexion
form-section-autofill-options = Options de remplissage automatique
form-section-card-details = Détails de la carte
form-section-personal-details = Détails personnels
form-section-identification = Identification
form-section-contact-info = Coordonnées
form-section-address = Adresse
form-section-ssh-key = Clé SSH
form-section-additional-options = Options supplémentaires
form-section-custom-fields = Champs personnalisés

## Cipher form — item details
form-name = Nom (obligatoire)
form-favorite = Favori
form-reprompt = Redemander le mot de passe principal
form-notes = Notes
form-folder = Dossier
form-folder-none = Aucun dossier
form-organization = Organisation
form-organization-personal = Personnel (moi)
form-collections = Collections
form-collections-none = Aucune collection
form-collections-selected = { $count } sélectionnée(s)
form-collections-empty-in-org = Aucune collection dans cette organisation

## Cipher form — login
form-username = Nom d'utilisateur
form-password = Mot de passe
form-totp = Clé d'authentification (TOTP)

## Cipher form — websites
form-uri = Site web (URI)
form-uri-empty = Aucun site pour le moment
form-add-website = Ajouter un site

## Cipher form — card
form-card-cardholder = Nom du titulaire
form-card-brand = Marque
form-card-number = Numéro
form-card-exp-month = Mois d'expiration
form-card-exp-year = Année d'expiration
form-card-code = Code de sécurité
form-card-brand-placeholder = -- Sélectionner --
form-card-month-placeholder = -- Mois --

## Cipher form — identity
form-identity-title = Civilité
form-identity-title-placeholder = -- Civilité --
# Identity title labels. Same convention as card brands — the stored value
# is the canonical English string; these keys only localize the display.
form-identity-title-mr = M.
form-identity-title-mrs = Mme
form-identity-title-ms = Mlle
form-identity-title-mx = Mx
form-identity-title-dr = Dr
form-identity-first-name = Prénom
form-identity-middle-name = Deuxième prénom
form-identity-last-name = Nom
form-identity-username = Nom d'utilisateur
form-identity-company = Société
form-identity-ssn = Numéro de sécurité sociale
form-identity-passport = Numéro de passeport
form-identity-license = Numéro de permis
form-identity-email = E-mail
form-identity-phone = Téléphone
form-identity-address1 = Adresse ligne 1
form-identity-address2 = Adresse ligne 2
form-identity-address3 = Adresse ligne 3
form-identity-city = Ville / commune
form-identity-state = État / province
form-identity-postal = Code postal
form-identity-country = Pays

## Cipher form — SSH key
form-ssh-public-key = Clé publique
form-ssh-private-key = Clé privée
form-ssh-fingerprint = Empreinte

## Cipher form — custom fields
form-custom-field-type = Type
form-custom-field-name = Nom
form-custom-field-value = Valeur
form-custom-field-type-text = Texte
form-custom-field-type-hidden = Masqué
form-custom-field-type-boolean = Booléen
form-custom-field-type-linked = Lié
form-custom-field-enabled = Activé
form-custom-field-empty = Aucun champ personnalisé pour le moment
form-add-custom-field = Ajouter un champ personnalisé
form-custom-field-linked-unsupported = Les champs liés ne sont pas encore pris en charge

## Toast — shared messages
toast-required-fields = Veuillez remplir les champs obligatoires.

## Menu bar — top-level
menu-file = Fichier
menu-edit = Édition
menu-view = Affichage
menu-account = Compte
menu-window = Fenêtre
menu-help = Aide

## Menu — File
menu-file-new-login = Nouvel identifiant
menu-file-new-item = Nouvel élément
menu-file-new-item-login = Identifiant
menu-file-new-item-card = Carte
menu-file-new-item-identity = Identité
menu-file-new-item-secure-note = Note sécurisée
menu-file-new-item-ssh-key = Clé SSH
menu-file-new-folder = Nouveau dossier
menu-file-sync-now = Synchroniser maintenant
menu-file-import = Importer
menu-file-export = Exporter
menu-file-settings = Paramètres
menu-file-lock-vault = Verrouiller le coffre
menu-file-lock-all-vaults = Verrouiller tous les coffres-forts
menu-file-log-out = Se déconnecter
menu-file-quit = Quitter Bitwarden

## Menu — Edit
menu-edit-undo = Annuler
menu-edit-redo = Rétablir
menu-edit-cut = Couper
menu-edit-copy = Copier
menu-edit-paste = Coller
menu-edit-select-all = Tout sélectionner
menu-edit-copy-username = Copier le nom d'utilisateur
menu-edit-copy-password = Copier le mot de passe
menu-edit-copy-totp = Copier le code de vérification (TOTP)

## Menu — View
menu-view-search = Rechercher dans le coffre
menu-view-generator = Générateur
menu-view-generator-history = Historique du générateur
menu-view-zoom-in = Zoom avant
menu-view-zoom-out = Zoom arrière
menu-view-reset-zoom = Réinitialiser le zoom

## Menu — Account
menu-account-change-password = Changer le mot de passe principal
menu-account-two-step = Authentification à deux facteurs
menu-account-fingerprint = Phrase d'empreinte
menu-account-delete = Supprimer le compte

## Menu — Window
menu-window-minimize = Réduire
menu-window-hide-to-tray = Masquer dans la barre d'état
menu-window-always-on-top = Toujours au premier plan
menu-window-close = Fermer

## Menu — Help
menu-help-feedback = Aide et commentaires
menu-help-bug = Signaler un bug
menu-help-legal = Mentions légales
menu-help-legal-tos = Conditions d'utilisation
menu-help-legal-privacy = Politique de confidentialité
menu-help-follow = Nous suivre
menu-help-web-vault = Aller au coffre web
menu-help-browser-extension = Obtenir l'extension de navigateur
menu-help-about = À propos de Bitwarden

menu-toast-sync-success = Coffre synchronisé
menu-toast-sync-failed-title = Échec de la synchronisation
menu-toast-sync-failed-body = Impossible de synchroniser le coffre. Réessayez plus tard.

menu-fingerprint-title = Phrase d'empreinte de votre compte :
menu-fingerprint-learn-more = En savoir plus
menu-fingerprint-close = Fermer

new-folder-modal-title = Nouveau dossier
new-folder-modal-field-label = Nom du dossier (obligatoire)
new-folder-modal-helper = Imbriquez un dossier en ajoutant le nom du dossier parent suivi de "/". Exemple : Social/Forums
new-folder-modal-save = Enregistrer
new-folder-modal-cancel = Annuler
new-folder-toast-success = Dossier créé
new-folder-toast-failed-title = Impossible de créer le dossier
new-folder-toast-failed-body = Le dossier n'a pas pu être enregistré. Réessayez.

## New-item picker modal
picker-title = Choisir l'élément à ajouter
picker-folder = Dossier
picker-login-subtitle = Site web ou application
picker-card-subtitle = Carte de crédit ou de débit
picker-bank-account-subtitle = Informations bancaires
picker-drivers-license-subtitle = Informations du permis
picker-passport-subtitle = Document de voyage
picker-identity-subtitle = Informations personnelles
picker-secure-note-subtitle = Texte important
picker-ssh-key-subtitle = Jeton de connexion au serveur
picker-folder-subtitle = Organisez vos éléments

## Tray
tray-show-hide = Afficher / Masquer
tray-lock-vault = Verrouiller le coffre
tray-exit = Quitter

# Endonym — see comment in the canonical en file.
language-name-self = Français

## Settings modal
settings-title = Paramètres
settings-tab-security = Sécurité
settings-tab-integrations = Intégrations
settings-tab-autotype = Saisie auto et copie
settings-tab-appearance = Apparence
settings-tab-advanced = Avancé

settings-toast-not-supported = Ce paramètre n'est pas encore pris en charge.

# Security tab
settings-security-access-options = Options d'accès
settings-security-open-at-login = Ouvrir Bitwarden au démarrage de l'appareil
settings-security-unlock-pin = Déverrouiller avec un PIN
settings-security-unlock-touch = Déverrouiller avec Touch ID
settings-security-session-timeout = Délai d'expiration de la session
settings-security-lock-after = Verrouiller après
settings-security-logout-after = Se déconnecter après
settings-security-lock-on-system-lock = Verrouiller quand le système est verrouillé

# Shared duration strings used by all the time-based dropdowns (lock after,
# log out after, clear clipboard after). Plurals come from Fluent selectors
# so every language can pick the right variant for the supplied number.
settings-duration-seconds = { $n ->
    [one] { $n } seconde
   *[other] { $n } secondes
  }
settings-duration-minutes = { $n ->
    [one] { $n } minute
   *[other] { $n } minutes
  }
settings-duration-hours = { $n ->
    [one] { $n } heure
   *[other] { $n } heures
  }
settings-duration-never = Jamais

# Integrations tab
settings-integrations-browser = Intégration au navigateur
settings-integrations-browser-enable = Activer l'intégration au navigateur
settings-integrations-browser-fingerprint = Exiger une empreinte de vérification
settings-integrations-ssh = Agent SSH
settings-integrations-ssh-enable = Activer l'agent SSH
settings-integrations-ssh-prompt = Comportement de la demande
settings-ssh-prompt-always = Ask (default: allow once)
settings-ssh-prompt-never = Jamais
settings-ssh-prompt-remember = Ask (default: allow until lock)

# Autotype and copy tab
settings-autotype-heading = Saisie auto
settings-autotype-enable = Activer la saisie auto
settings-clipboard-heading = Presse-papiers
settings-clipboard-clear-after = Effacer le presse-papiers après
settings-clipboard-minimize-on-copy = Réduire à la copie

# Appearance tab
settings-appearance-theme-heading = Thème
settings-appearance-theme = Thème
settings-appearance-theme-system = Système
settings-appearance-theme-light = Clair
settings-appearance-theme-dark = Sombre
settings-appearance-language = Langue
settings-appearance-language-system = Système
settings-appearance-display-heading = Affichage
settings-appearance-show-favicons = Afficher les icônes pour les URL

# Advanced tab
settings-advanced-tray = Barre d'état système
settings-advanced-tray-enable = Afficher l'icône dans la barre d'état
settings-advanced-minimize-to-tray = Réduire dans la barre d'état
settings-advanced-close-to-tray = Fermer dans la barre d'état
settings-advanced-platform = Plateforme
settings-advanced-always-show-dock = Toujours afficher l'icône du Dock
settings-advanced-allow-screenshots = Autoriser les captures d'écran

## Send list
send-title = Send
send-new-button = Nouveau
send-search-placeholder = Rechercher dans les Sends
send-column-name = Nom
send-column-deletion = Date de suppression
send-empty-body = Vous n'avez pas encore créé de Send.
send-toast-item-saved = Send enregistré
send-toast-item-deleted = Send supprimé
send-toast-copied-link = Lien Send copié
send-toast-copied-password = Mot de passe copié
send-toast-load-failed-title = Échec du chargement
send-toast-load-failed-body = Impossible de charger le Send. Réessayez.
send-toast-save-failed-title = Échec de l'enregistrement
send-toast-save-failed-body = Impossible d'enregistrer le Send. Réessayez.
send-toast-delete-failed-title = Échec de la suppression
send-toast-delete-failed-body = Impossible de supprimer le Send. Réessayez.

## Send form
send-form-title-new-text = Créer un Send texte
send-form-title-new-file = Créer un Send fichier
send-form-title-edit-text = Modifier le Send texte
send-form-title-edit-file = Modifier le Send fichier
send-form-details-heading = Détails du Send
send-form-additional-heading = Options supplémentaires
send-form-name = Nom (obligatoire)
send-form-text = Texte à partager (obligatoire)
send-form-hide-text = Masquer le texte par défaut
send-form-file-choose = Choisir un fichier
send-form-file-choose-placeholder = Aucun fichier sélectionné
send-form-file-name = Nom du fichier
send-form-file-size = Taille
send-form-deletion-date = Date de suppression
send-form-deletion-hint = Le Send sera supprimé définitivement le {$date}.
send-form-who-can-view = Qui peut voir
send-form-access-link = Toute personne disposant du lien
send-form-access-people = Personnes spécifiques
send-form-access-password = Toute personne avec un mot de passe défini par vous
send-form-password = Mot de passe (obligatoire)
send-form-password-hint = Les destinataires devront saisir le mot de passe pour voir ce Send.
send-form-emails = E-mails (obligatoire)
send-form-send-link = Lien du Send
send-form-limit-views = Limiter les vues
send-form-limit-views-hint = Personne ne pourra voir ce Send après l'atteinte de la limite.
send-form-views-left = Personne ne pourra voir ce Send après l'atteinte de la limite. {$count} vues restantes.
send-form-hide-email = Masquer votre adresse e-mail aux destinataires.
send-form-private-note = Note privée
send-form-save = Enregistrer
send-form-cancel = Annuler

send-form-preset-1h = 1 heure
send-form-preset-1d = 1 jour
send-form-preset-2d = 2 jours
send-form-preset-3d = 3 jours
send-form-preset-7d = 7 jours
send-form-preset-14d = 14 jours
send-form-preset-30d = 30 jours

send-delete-modal-title = Supprimer le Send
send-delete-modal-body = Voulez-vous vraiment supprimer "{$name}" ?
send-delete-modal-cancel = Annuler
send-delete-modal-confirm = Supprimer

## Generator
generator-title = Générateur
generator-tab-password = Mot de passe
generator-tab-passphrase = Phrase de passe
generator-tab-username = Identifiant

# Password tab
generator-length = Longueur
generator-length-hint = La valeur doit être entre 5 et 128.
generator-include = Inclure
generator-include-uppercase = A-Z
generator-include-lowercase = a-z
generator-include-numbers = 0-9
generator-include-special = !@#$%^&*
generator-min-number = Nombres minimum
generator-min-special = Caractères spéciaux minimum
generator-avoid-ambiguous = Éviter les caractères ambigus

# Passphrase tab
generator-num-words = Nombre de mots
generator-num-words-hint = La valeur doit être entre 3 et 20. Utilisez 6 mots ou plus pour générer une phrase de passe robuste.
generator-word-separator = Séparateur de mots
generator-passphrase-capitalize = Mettre en majuscule
generator-passphrase-include-number = Inclure un chiffre

# Username tab
generator-username-type = Type
generator-username-kind-word = Mot aléatoire
generator-username-kind-subaddress = E-mail à sous-adresse
generator-username-kind-catchall = E-mail catch-all
generator-username-capitalize = Mettre en majuscule
generator-username-include-number = Inclure un chiffre
generator-username-email = E-mail
generator-username-domain = Domaine

# History
generator-history-open = Historique du générateur
generator-history-title = Historique du générateur
generator-history-heading = Récent
generator-history-empty = Aucune valeur récente.
generator-history-clear = Effacer l'historique
generator-history-just-now = à l'instant
generator-history-minutes-ago = il y a {$count} min
generator-history-hours-ago = il y a {$count} h
generator-history-days-ago = il y a {$count} j

# Toasts
generator-toast-copied = Copié
generator-toast-failed = Génération impossible

# Magnify launcher
magnify-search-placeholder = Bitwarden Magnify
magnify-locked-title = Votre coffre est verrouillé
magnify-locked-subtitle = Ouvrez Bitwarden pour déverrouiller
magnify-open-bitwarden = Ouvrir Bitwarden
magnify-no-results = Aucun élément correspondant
magnify-copy-password = Copier le mot de passe
magnify-copy-username = Copier le nom d'utilisateur
magnify-hint-navigate = Naviguer

## Import modal
import-modal-title = Importer des données
import-modal-section-destination = Destination
import-modal-vault-label = Coffre (obligatoire)
import-modal-vault-personal = Mon coffre
import-modal-folder-label = Dossier
import-modal-folder-placeholder = - Sélectionner un dossier -
import-modal-collection-label = Collection (obligatoire)
import-modal-collection-placeholder = - Sélectionner une collection -
import-modal-section-data = Données
import-modal-file-format-label = Format de fichier (obligatoire)
import-modal-file-helper = Sélectionnez le fichier à importer
import-modal-choose-file = Choisir un fichier
import-modal-no-file = Aucun fichier choisi
import-modal-paste-label = ou collez le contenu du fichier à importer
import-modal-submit = Importer les données
import-modal-cancel = Annuler
import-toast-unimplemented = L'importation n'est pas encore disponible — bientôt

## Export modal
export-modal-title = Exporter le coffre
export-modal-vault-label = Exporter depuis (obligatoire)
export-modal-vault-personal = Mon coffre
export-modal-banner-personal-title = Exportation du coffre individuel
export-modal-banner-personal = Seuls les éléments du coffre individuel associés à { $email } seront exportés. Les éléments du coffre d'organisation ne seront pas inclus. Seules les informations des éléments seront exportées et n'incluront pas les pièces jointes associées.
export-modal-banner-org-title = Exportation du coffre d'organisation
export-modal-banner-org-body = Seul le coffre de l'organisation associé à { $name } sera exporté. Les éléments des coffres-forts individuels ou d'autres organisations ne seront pas inclus.
export-modal-file-format-label = Format de fichier (obligatoire)
export-modal-password-label = Mot de passe du fichier (obligatoire)
export-modal-continue = Continuer
export-modal-cancel = Annuler
export-confirm-title = Confirmer l'exportation du coffre
export-confirm-warning-unencrypted = Cette exportation contient les données de votre coffre dans un format non chiffré. Vous ne devez pas stocker ni envoyer le fichier exporté par des canaux non sécurisés (par exemple, par e-mail). Supprimez-le immédiatement après utilisation.
export-confirm-warning-file-encrypted = Cette exportation de fichier sera protégée par mot de passe et nécessitera ce mot de passe pour être déchiffrée.
export-confirm-password-label = Mot de passe principal (obligatoire)
export-confirm-helper = Confirmez votre identité pour continuer.
export-confirm-error = Mot de passe principal invalide.
export-toast-success = Coffre exporté vers { $path }
export-toast-failed-title = Échec de l'exportation
export-toast-failed-body = Impossible d'exporter le coffre. Réessayez.

# Composable vault filters
sidebar-group-vaults = Coffres
sidebar-group-view = Vue
sidebar-group-types = Types d’éléments
sidebar-filter-all-vaults = Tous les coffres
sidebar-filter-active-items = Éléments actifs
sidebar-filter-all-types = Tous les types

sidebar-group-folders = Dossiers
sidebar-filter-all-folders = Tous les dossiers
sidebar-filter-no-folder = Sans dossier
sidebar-group-collections = Collections
sidebar-filter-all-collections = Toutes les collections
sidebar-filter-no-collection = Sans collection

vault-column-last-changed = Modifié le
vault-column-base-url = URL de base
vault-filter-folder = Dossier : { $name }
vault-filter-collection = Collection : { $name }
vault-filter-search = Recherche : « { $query } »

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
