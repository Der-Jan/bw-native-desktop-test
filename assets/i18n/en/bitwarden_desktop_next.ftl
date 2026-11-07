## App-wide
app-title = Bitwarden [Next]
about-window-title = About Bitwarden

## Login — unlock
login-unlock-title = Your vault is locked
login-unlock-password-placeholder = Master password (required)
login-unlock-pin-placeholder = PIN (required)
login-unlock-button = Unlock
login-unlock-or = or
login-unlock-biometrics-button = Unlock with biometrics
login-unlock-pin-button = Unlock with PIN
login-unlock-master-password-button = Unlock with master password
login-log-out = Log out

## Login — email entry
login-email-title = Log in to Bitwarden
login-email-placeholder = Email address (required)
login-email-remember = Remember email
login-email-continue = Continue
login-email-or = Or
login-email-sso = Use single sign-on
login-email-new-prompt = New to Bitwarden?
login-email-create-account = Create account

## Login — password entry
login-password-title = Welcome back
login-password-placeholder = Master password (required)
login-password-get-hint = Get master password hint
login-password-submit = Log in with master password
login-password-back = Back

## Login — server selector
login-server-accessing = Accessing { $server }
login-server-accessing-label = Accessing:
login-server-self-hosted = Self-hosted

## Login — self-hosted environment modal
login-self-hosted-modal-title = Self-hosted environment
login-self-hosted-modal-url-label = Server URL
login-self-hosted-modal-url-helper = Specify the base URL of your on-premises hosted Bitwarden installation. Example: https://bitwarden.company.com
login-self-hosted-modal-url-error = The URL must start with https://
login-self-hosted-modal-save = Save
login-self-hosted-modal-cancel = Cancel

## Login — toasts
login-toast-unlock-failed-title = Unlock failed
login-toast-unlock-failed-body = Check your master password and try again.
login-toast-login-failed-title = Login failed
login-toast-login-failed-body = Check your email and password and try again.
login-toast-pin-unsupported = PIN unlock is not yet supported
login-toast-biometrics-unsupported = Biometric unlock is not yet supported

## About dialog
about-version-label = Version
about-sdk-version-label = SDK version
about-os-label = OS
about-architecture-label = Architecture
about-copy-button = Copy
about-close-button = Close

## Sidebar — sections and filters
sidebar-section-vault = Vault
sidebar-section-send = Send
sidebar-filter-my-vault = My Vault
sidebar-filter-favorites = Favorites
sidebar-filter-logins = Logins
sidebar-filter-cards = Cards
sidebar-filter-identities = Identities
sidebar-filter-notes = Notes
sidebar-filter-ssh-keys = SSH keys
sidebar-filter-archive = Archive
sidebar-filter-trash = Trash
sidebar-filter-text-send = Text
sidebar-filter-file-send = File
sidebar-item-generator = Generator
sidebar-item-import = Import
sidebar-item-export = Export

## Vault list
vault-title = Vault
vault-new-button = New
vault-search-placeholder = Search
vault-column-name = Name
vault-column-options = Options
vault-toast-item-saved = Item saved
vault-toast-save-failed-title = Save failed
vault-toast-save-failed-body = Couldn't save the item. Try again.
vault-toast-decrypt-failed-title = Decrypt failed
vault-toast-decrypt-failed-body = Couldn't load the item. Try again.
vault-toast-copied-username = Username copied
vault-toast-copied-password = Password copied
vault-toast-copied-website = Website copied
vault-toast-copied-totp = Verification code copied
vault-toast-copied-field = Field copied
vault-toast-copied-private-key = Private key copied
vault-toast-copied-public-key = Public key copied
vault-toast-copied-fingerprint = Fingerprint copied
vault-toast-item-deleted = Item moved to trash
vault-toast-delete-failed-title = Delete failed
vault-toast-delete-failed-body = Couldn't delete the item. Try again.
vault-delete-modal-title = Delete item?
vault-delete-modal-body = "{ $name }" will be moved to the trash.
vault-delete-modal-cancel = Cancel
vault-delete-modal-confirm = Delete

## Account switcher
account-switcher-other-accounts = Other Bitwarden accounts
account-switcher-options = Options
account-switcher-lock-now = Lock now
account-switcher-log-out = Log out
account-switcher-lock-all = Lock all accounts
account-switcher-settings = Settings
account-switcher-add = Add account

## Detail pane — headers per cipher type
detail-header-login = View login
detail-header-card = View card
detail-header-identity = View identity
detail-header-note = View note
detail-header-ssh-key = View SSH key
detail-header-bank-account = View bank account
detail-header-drivers-license = View driver's license
detail-header-passport = View passport

## Detail pane — section labels
detail-section-item-details = Item details
detail-section-login-credentials = Login credentials
detail-section-autofill-options = Autofill options
detail-section-card-details = Card details
detail-section-personal-details = Personal details
detail-section-note = Note
detail-section-ssh-key = SSH key
detail-section-bank-account = Bank account
detail-section-drivers-license = Driver's license
detail-section-passport = Passport

## Detail pane — fields
detail-field-name = Name
detail-field-notes = Notes
detail-field-username = Username
detail-field-password = Password
detail-field-totp = Verification code (TOTP)
detail-totp-invalid = Invalid TOTP seed
detail-field-website = Website
detail-field-cardholder-name = Cardholder name
detail-field-brand = Brand
detail-field-number = Number
detail-field-expiration = Expiration
detail-field-security-code = Security code
detail-field-email = Email
detail-field-phone = Phone
detail-field-company = Company
detail-field-address = Address
detail-field-city-region = City / region
detail-field-public-key = Public key
detail-field-private-key = Private key
detail-field-fingerprint = Fingerprint
detail-empty-credentials = No credentials
detail-empty-card = No card details
detail-empty-identity = No identity details
detail-empty-bank-account = No bank-account details
detail-field-bank-name = Bank name
detail-field-bank-name-on-account = Name on account
detail-field-bank-account-type = Account type
detail-field-bank-account-number = Account number
detail-field-bank-routing-number = Routing number
detail-field-bank-branch-number = Branch / institution number
detail-field-bank-pin = PIN
detail-field-bank-swift-code = SWIFT code
detail-field-bank-iban = IBAN
detail-field-bank-contact-phone = Bank contact phone
detail-empty-drivers-license = No driver's license details
detail-field-dl-first-name = First name
detail-field-dl-middle-name = Middle name
detail-field-dl-last-name = Last name
detail-field-dl-date-of-birth = Date of birth
detail-field-dl-license-number = License number
detail-field-dl-issuing-country = Issuing country
detail-field-dl-issuing-state = Issuing state
detail-field-dl-issuing-authority = Issuing authority
detail-field-dl-issue-date = Issue date
detail-field-dl-expiration-date = Expiration date
detail-field-dl-license-class = License class
detail-empty-passport = No passport details
detail-field-pp-given-name = Given name
detail-field-pp-surname = Surname
detail-field-pp-date-of-birth = Date of birth
detail-field-pp-sex = Sex
detail-field-pp-birth-place = Place of birth
detail-field-pp-nationality = Nationality
detail-field-pp-passport-number = Passport number
detail-field-pp-passport-type = Passport type
detail-field-pp-national-id-number = National identification number
detail-field-pp-issuing-country = Issuing country
detail-field-pp-issuing-authority = Issuing authority
detail-field-pp-issue-date = Issue date
detail-field-pp-expiration-date = Expiration date
detail-section-custom-fields = Custom fields
detail-field-passkey = Passkey
detail-field-passkey-created = Created { $date }
detail-field-boolean-true = Yes
detail-field-boolean-false = No
detail-edit-button = Edit

## Cipher form — header titles
form-title-new-item = New item
form-title-edit-login = Edit login
form-title-edit-card = Edit card
form-title-edit-identity = Edit identity
form-title-edit-note = Edit note
form-title-edit-ssh-key = Edit SSH key
form-title-edit-bank-account = Edit bank account
form-title-edit-drivers-license = Edit driver's license
form-title-edit-passport = Edit passport

## Cipher form — buttons
form-save = Save
form-saving = Saving…
form-cancel = Cancel

## Cipher form — sections
form-section-item-details = Item details
form-section-login-credentials = Login credentials
form-section-autofill-options = Autofill options
form-section-card-details = Card details
form-section-personal-details = Personal details
form-section-identification = Identification
form-section-contact-info = Contact info
form-section-address = Address
form-section-ssh-key = SSH key
form-section-bank-account = Bank account
form-section-drivers-license = Driver's license
form-section-passport = Passport
form-section-additional-options = Additional options
form-section-custom-fields = Custom fields

## Cipher form — item details
form-name = Name (required)
form-favorite = Favorite
form-reprompt = Master password re-prompt
form-notes = Notes
form-folder = Folder
form-folder-none = No folder
form-organization = Organization
form-organization-personal = Personal (me)
form-collections = Collections
form-collections-none = No collections
form-collections-selected = { $count } selected
form-collections-empty-in-org = No collections in this org

## Cipher form — login
form-username = Username
form-password = Password
form-totp = Authenticator key (TOTP)

## Cipher form — websites
form-uri = Website (URI)
form-uri-empty = No websites yet
form-add-website = Add website

## Cipher form — card
form-card-cardholder = Cardholder name
form-card-brand = Brand
form-card-number = Number
form-card-exp-month = Expiration month
form-card-exp-year = Expiration year
form-card-code = Security code
form-card-brand-placeholder = -- Select --
form-card-month-placeholder = -- Month --

## Cipher form — identity
form-identity-title = Title
form-identity-title-placeholder = -- Title --
# Identity title labels. Same convention as card brands — the stored value
# is the canonical English string; these keys only localize the display.
form-identity-title-mr = Mr
form-identity-title-mrs = Mrs
form-identity-title-ms = Ms
form-identity-title-mx = Mx
form-identity-title-dr = Dr
form-identity-first-name = First name
form-identity-middle-name = Middle name
form-identity-last-name = Last name
form-identity-username = Username
form-identity-company = Company
form-identity-ssn = Social Security number
form-identity-passport = Passport number
form-identity-license = License number
form-identity-email = Email
form-identity-phone = Phone
form-identity-address1 = Address line 1
form-identity-address2 = Address line 2
form-identity-address3 = Address line 3
form-identity-city = City / town
form-identity-state = State / province
form-identity-postal = Zip / postal code
form-identity-country = Country

## Cipher form — SSH key
form-ssh-public-key = Public key
form-ssh-private-key = Private key
form-ssh-fingerprint = Fingerprint

## Cipher form — bank account
form-bank-name = Bank name
form-bank-name-on-account = Name on account
form-bank-account-type = Account type
form-bank-account-type-placeholder = -- Account type --
# Account-type labels. Same convention as card brands / identity titles —
# the stored value is the canonical English string; these keys only
# localize the display.
form-bank-account-type-checking = Checking
form-bank-account-type-savings = Savings
form-bank-account-type-brokerage = Brokerage
form-bank-account-type-money-market = Money market
form-bank-account-type-cd = Certificate of deposit (CD)
form-bank-account-type-other = Other
form-bank-account-number = Account number
form-bank-routing-number = Routing number
form-bank-branch-number = Branch / institution number
form-bank-pin = PIN
form-bank-swift-code = SWIFT code
form-bank-iban = IBAN
form-bank-contact-phone = Bank contact phone

## Cipher form — drivers license
form-dl-first-name = First name
form-dl-middle-name = Middle name
form-dl-last-name = Last name
form-dl-date-of-birth = Date of birth
form-dl-license-number = License number
form-dl-issuing-country = Issuing country
form-dl-issuing-state = Issuing state
form-dl-issuing-authority = Issuing authority
form-dl-issue-date = Issue date
form-dl-expiration-date = Expiration date
form-dl-license-class = License class

## Cipher form — passport
form-pp-given-name = Given name
form-pp-surname = Surname
form-pp-date-of-birth = Date of birth
form-pp-sex = Sex
form-pp-birth-place = Place of birth
form-pp-nationality = Nationality
form-pp-passport-number = Passport number
form-pp-passport-type = Passport type
form-pp-national-id-number = National identification number
form-pp-issuing-country = Issuing country
form-pp-issuing-authority = Issuing authority
form-pp-issue-date = Issue date
form-pp-expiration-date = Expiration date

## Cipher form — custom fields
form-custom-field-type = Type
form-custom-field-name = Name
form-custom-field-value = Value
form-custom-field-type-text = Text
form-custom-field-type-hidden = Hidden
form-custom-field-type-boolean = Boolean
form-custom-field-type-linked = Linked
form-custom-field-enabled = Enabled
form-custom-field-empty = No custom fields yet
form-add-custom-field = Add custom field
form-custom-field-linked-unsupported = Linked fields not yet supported

## Toast — shared messages
toast-required-fields = Please fill in the required fields.

## Menu bar — top-level
menu-file = File
menu-edit = Edit
menu-view = View
menu-account = Account
menu-window = Window
menu-help = Help

## Menu — File
menu-file-new-login = New login
menu-file-new-item = New item
menu-file-new-item-login = Login
menu-file-new-item-card = Card
menu-file-new-item-identity = Identity
menu-file-new-item-secure-note = Secure note
menu-file-new-item-ssh-key = SSH key
menu-file-new-item-bank-account = Bank account
menu-file-new-item-drivers-license = Driver's license
menu-file-new-item-passport = Passport
menu-file-new-folder = New folder
menu-file-sync-now = Sync now
menu-file-import = Import
menu-file-export = Export
menu-file-settings = Settings
menu-file-lock-vault = Lock vault
menu-file-lock-all-vaults = Lock all vaults
menu-file-log-out = Log out
menu-file-quit = Quit Bitwarden

## Menu — Edit
menu-edit-undo = Undo
menu-edit-redo = Redo
menu-edit-cut = Cut
menu-edit-copy = Copy
menu-edit-paste = Paste
menu-edit-select-all = Select all
menu-edit-copy-username = Copy username
menu-edit-copy-password = Copy password
menu-edit-copy-totp = Copy verification code (TOTP)

## Menu — View
menu-view-search = Search vault
menu-view-generator = Generator
menu-view-generator-history = Generator history
menu-view-zoom-in = Zoom in
menu-view-zoom-out = Zoom out
menu-view-reset-zoom = Reset zoom
menu-view-toggle-fullscreen = Toggle full screen

## Menu — Account
menu-account-change-password = Change master password
menu-account-two-step = Two-step login
menu-account-fingerprint = Fingerprint phrase
menu-account-delete = Delete account

## Menu — Window
menu-window-minimize = Minimize
menu-window-hide-to-tray = Hide to tray
menu-window-always-on-top = Always on top
menu-window-close = Close

## Menu — Help
menu-help-feedback = Help & feedback
menu-help-bug = File a bug report
menu-help-legal = Legal
menu-help-legal-tos = Terms of service
menu-help-legal-privacy = Privacy policy
menu-help-follow = Follow us
menu-help-web-vault = Go to web vault
menu-help-browser-extension = Get browser extension
menu-help-about = About Bitwarden

menu-toast-sync-success = Vault synced
menu-toast-sync-failed-title = Sync failed
menu-toast-sync-failed-body = Could not sync the vault. Try again later.

menu-fingerprint-title = Your account's fingerprint phrase:
menu-fingerprint-learn-more = Learn more
menu-fingerprint-close = Close

new-folder-modal-title = New folder
new-folder-modal-field-label = Folder Name (required)
new-folder-modal-helper = Nest a folder by adding the parent folder's name followed by a "/". Example: Social/Forums
new-folder-modal-save = Save
new-folder-modal-cancel = Cancel
new-folder-toast-success = Folder created
new-folder-toast-failed-title = Could not create folder
new-folder-toast-failed-body = The folder could not be saved. Try again.

## New-item picker modal
picker-title = Choose item to add
picker-folder = Folder
picker-login-subtitle = Website or app
picker-card-subtitle = Credit or debit card
picker-bank-account-subtitle = Banking details
picker-drivers-license-subtitle = Driving credentials
picker-passport-subtitle = Travel document
picker-identity-subtitle = Personal info
picker-secure-note-subtitle = Important text
picker-ssh-key-subtitle = Server login token
picker-folder-subtitle = Organize your items

## Tray
tray-show-hide = Show / Hide
tray-lock-vault = Lock vault
tray-exit = Exit

# Native name (endonym) of this locale, shown in the Appearance tab's
# language picker. Each `.ftl` file declares its own — the runtime reads
# this key from each locale's bundle directly so the value renders in
# its own script regardless of the active locale.
language-name-self = English

## Settings modal
settings-title = Settings
settings-tab-security = Security
settings-tab-integrations = Integrations
settings-tab-autotype = Autotype and copy
settings-tab-appearance = Appearance
settings-tab-advanced = Advanced

settings-toast-not-supported = This setting isn't supported yet.

# Allow screenshots — confirm-still-visible dialog
settings-confirm-window-visible-title = Confirm window still visible
settings-confirm-window-visible-body = Click OK within 5 seconds to keep screen capture blocked. If you can't see this dialog, the setting will revert automatically.
settings-confirm-window-visible-ok = OK
settings-allow-screenshots-unsupported = Screen capture protection isn't supported on this platform.

# Security tab
settings-security-access-options = Access options
settings-security-open-at-login = Open Bitwarden at device login
settings-security-unlock-pin = Unlock with PIN
settings-security-unlock-touch = Unlock with Touch ID
settings-security-session-timeout = Session timeout
settings-security-lock-after = Lock after
settings-security-logout-after = Log out after
settings-security-lock-on-system-lock = Lock when the system is locked

# Shared duration strings used by all the time-based dropdowns (lock after,
# log out after, clear clipboard after). Plurals come from Fluent selectors
# so every language can pick the right variant for the supplied number.
settings-duration-seconds = { $n ->
    [one] { $n } second
   *[other] { $n } seconds
  }
settings-duration-minutes = { $n ->
    [one] { $n } minute
   *[other] { $n } minutes
  }
settings-duration-hours = { $n ->
    [one] { $n } hour
   *[other] { $n } hours
  }
settings-duration-never = Never

# Integrations tab
settings-integrations-browser = Browser integration
settings-integrations-browser-enable = Enable browser integration
settings-integrations-browser-fingerprint = Require verification fingerprint
settings-integrations-ssh = SSH agent
settings-integrations-ssh-enable = Enable SSH agent
settings-integrations-ssh-prompt = Prompt behavior
settings-ssh-prompt-always = Ask (default: allow once)
settings-ssh-prompt-never = Never
settings-ssh-prompt-remember = Ask (default: allow until lock)

# Autotype and copy tab
settings-autotype-heading = Autotype
settings-autotype-enable = Enable autotype
settings-clipboard-heading = Clipboard
settings-clipboard-clear-after = Clear clipboard after
settings-clipboard-minimize-on-copy = Minimize on copy

# Appearance tab
settings-appearance-theme-heading = Theme
settings-appearance-theme = Theme
settings-appearance-theme-system = System
settings-appearance-theme-light = Light
settings-appearance-theme-dark = Dark
settings-appearance-language = Language
settings-appearance-language-system = System
settings-appearance-display-heading = Display
settings-appearance-show-favicons = Show icons for URLs

# Advanced tab
settings-advanced-tray = Tray
settings-advanced-tray-enable = Show tray icon
settings-advanced-minimize-to-tray = Minimize to tray
settings-advanced-close-to-tray = Close to tray
settings-advanced-platform = Platform
settings-advanced-always-show-dock = Always show dock icon
settings-advanced-allow-screenshots = Allow screenshots

## Send list
send-title = Send
send-new-button = New
send-search-placeholder = Search Sends
send-column-name = Name
send-column-deletion = Deletion date
send-empty-body = You haven't created any Sends yet.
send-toast-item-saved = Send saved
send-toast-item-deleted = Send deleted
send-toast-copied-link = Send link copied
send-toast-copied-password = Password copied
send-toast-load-failed-title = Load failed
send-toast-load-failed-body = Couldn't load the Send. Try again.
send-toast-save-failed-title = Save failed
send-toast-save-failed-body = Couldn't save the Send. Try again.
send-toast-delete-failed-title = Delete failed
send-toast-delete-failed-body = Couldn't delete the Send. Try again.

## Send form
send-form-title-new-text = Create Text Send
send-form-title-new-file = Create File Send
send-form-title-edit-text = Edit Text Send
send-form-title-edit-file = Edit File Send
send-form-details-heading = Send details
send-form-additional-heading = Additional options
send-form-name = Name (required)
send-form-text = Text to share (required)
send-form-hide-text = Hide text by default
send-form-file-choose = Choose file
send-form-file-choose-placeholder = No file selected
send-form-file-name = File name
send-form-file-size = Size
send-form-deletion-date = Deletion date
send-form-deletion-hint = The Send will be permanently deleted on {$date}.
send-form-who-can-view = Who can view
send-form-access-link = Anyone with the link
send-form-access-people = Specific people
send-form-access-password = Anyone with a password set by you
send-form-password = Password (required)
send-form-password-hint = Individuals will need to enter the password to view this Send.
send-form-emails = Emails (required)
send-form-send-link = Send link
send-form-limit-views = Limit views
send-form-limit-views-hint = No one can view this Send after the limit is reached.
send-form-views-left = No one can view this Send after the limit is reached. {$count} views left.
send-form-hide-email = Hide your email address from viewers.
send-form-private-note = Private note
send-form-save = Save
send-form-cancel = Cancel

send-form-preset-1h = 1 hour
send-form-preset-1d = 1 day
send-form-preset-2d = 2 days
send-form-preset-3d = 3 days
send-form-preset-7d = 7 days
send-form-preset-14d = 14 days
send-form-preset-30d = 30 days

send-delete-modal-title = Delete Send
send-delete-modal-body = Are you sure you want to delete "{$name}"?
send-delete-modal-cancel = Cancel
send-delete-modal-confirm = Delete

## Generator
generator-title = Generator
generator-tab-password = Password
generator-tab-passphrase = Passphrase
generator-tab-username = Username

# Password tab
generator-length = Length
generator-length-hint = Value must be between 5 and 128.
generator-include = Include
generator-include-uppercase = A-Z
generator-include-lowercase = a-z
generator-include-numbers = 0-9
generator-include-special = !@#$%^&*
generator-min-number = Minimum numbers
generator-min-special = Minimum special
generator-avoid-ambiguous = Avoid ambiguous characters

# Passphrase tab
generator-num-words = Number of words
generator-num-words-hint = Value must be between 3 and 20. Use 6 words or more to generate a strong passphrase.
generator-word-separator = Word separator
generator-passphrase-capitalize = Capitalize
generator-passphrase-include-number = Include number

# Username tab
generator-username-type = Type
generator-username-kind-word = Random word
generator-username-kind-subaddress = Plus addressed email
generator-username-kind-catchall = Catch-all email
generator-username-capitalize = Capitalize
generator-username-include-number = Include number
generator-username-email = Email
generator-username-domain = Domain

# History
generator-history-open = Generator history
generator-history-title = Generator history
generator-history-heading = Recent
generator-history-empty = No recent values.
generator-history-clear = Clear history
generator-history-just-now = just now
generator-history-minutes-ago = {$count}m ago
generator-history-hours-ago = {$count}h ago
generator-history-days-ago = {$count}d ago

# Toasts
generator-toast-copied = Copied
generator-toast-failed = Couldn't generate

# Magnify launcher
magnify-search-placeholder = Bitwarden Magnify
magnify-locked-title = Your vault is locked
magnify-locked-subtitle = Open Bitwarden to unlock
magnify-open-bitwarden = Open Bitwarden
magnify-no-results = No matching items
magnify-copy-password = Copy password
magnify-copy-username = Copy username
magnify-hint-navigate = Navigate

## Import modal
import-modal-title = Import data
import-modal-section-destination = Destination
import-modal-vault-label = Vault (required)
import-modal-vault-personal = My vault
import-modal-folder-label = Folder
import-modal-folder-placeholder = - Select a folder -
import-modal-collection-label = Collection (required)
import-modal-collection-placeholder = - Select a collection -
import-modal-section-data = Data
import-modal-file-format-label = File format (required)
import-modal-file-helper = Select the import file
import-modal-choose-file = Choose file
import-modal-no-file = No file chosen
import-modal-paste-label = or copy/paste the import file contents
import-modal-submit = Import data
import-modal-cancel = Cancel
import-toast-unimplemented = Import isn't wired up yet — coming soon

## Export modal
export-modal-title = Export vault
export-modal-vault-label = Export from (required)
export-modal-vault-personal = My vault
export-modal-banner-personal-title = Exporting individual vault
export-modal-banner-personal = Only the individual vault items associated with { $email } will be exported. Organization vault items will not be included. Only vault item information will be exported and will not include associated attachments.
export-modal-banner-org-title = Exporting organization vault
export-modal-banner-org-body = Only the organization vault associated with { $name } will be exported. Items in individual vaults or other organizations will not be included.
export-modal-file-format-label = File format (required)
export-modal-password-label = File password (required)
export-modal-continue = Continue
export-modal-cancel = Cancel
export-confirm-title = Confirm vault export
export-confirm-warning-unencrypted = This export contains your vault data in an unencrypted format. You should not store or send the exported file over unsecure channels (such as email). Delete it immediately after you are done using it.
export-confirm-warning-file-encrypted = This file export will be password protected and require the file password to decrypt.
export-confirm-password-label = Master password (required)
export-confirm-helper = Confirm your identity to continue.
export-confirm-error = Invalid master password.
export-toast-success = Vault exported to { $path }
export-toast-failed-title = Export failed
export-toast-failed-body = Couldn't export the vault. Try again.

# Local account storage and logout recovery
storage-accounts-unavailable = Local account storage is unavailable. No accounts were loaded for safety.
storage-logout-incomplete = The account is signed out. Some local account data could not be removed yet. Cleanup will be retried on the next start.
storage-logout-not-stored = Logout could not be saved. The account is locked now but may reappear after restarting. Please check storage access.
storage-logout-pending = Finishing logout …

send-item-unsupported = Sharing vault items through Send is not yet supported.

# Composable vault filters
sidebar-group-vaults = Vaults
sidebar-group-view = View
sidebar-group-types = Item types
sidebar-filter-all-vaults = All vaults
sidebar-filter-active-items = Active items
sidebar-filter-all-types = All item types

sidebar-group-folders = Folders
sidebar-filter-all-folders = All folders
sidebar-filter-no-folder = No folder
sidebar-group-collections = Collections
sidebar-filter-all-collections = All collections
sidebar-filter-no-collection = No collection

vault-column-last-changed = Last changed
vault-column-base-url = Base URL
vault-filter-folder = Folder: { $name }
vault-filter-collection = Collection: { $name }
vault-filter-search = Search: “{ $query }”

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
