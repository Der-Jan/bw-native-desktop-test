## App-wide
app-title = Bitwarden [Next]
about-window-title = Acerca de Bitwarden

## Login — unlock
login-unlock-title = Tu caja fuerte está bloqueada
login-unlock-password-placeholder = Contraseña maestra (obligatoria)
login-unlock-pin-placeholder = PIN (obligatorio)
login-unlock-button = Desbloquear
login-unlock-or = o
login-unlock-biometrics-button = Desbloquear con biometría
login-unlock-pin-button = Desbloquear con PIN
login-unlock-master-password-button = Desbloquear con contraseña maestra
login-log-out = Cerrar sesión

## Login — email entry
login-email-title = Inicia sesión en Bitwarden
login-email-placeholder = Dirección de correo (obligatoria)
login-email-remember = Recordar correo
login-email-continue = Continuar
login-email-or = O
login-email-sso = Usar inicio de sesión único (SSO)
login-email-new-prompt = ¿Nuevo en Bitwarden?
login-email-create-account = Crear cuenta

## Login — password entry
login-password-title = Bienvenido de nuevo
login-password-placeholder = Contraseña maestra (obligatoria)
login-password-get-hint = Obtener pista de contraseña maestra
login-password-submit = Iniciar sesión con contraseña maestra
login-password-back = Atrás

## Login — server selector
login-server-accessing = Accediendo a { $server }
login-server-accessing-label = Accediendo a:
login-server-self-hosted = Autoalojado

## Login — self-hosted environment modal
login-self-hosted-modal-title = Entorno autoalojado
login-self-hosted-modal-url-label = URL del servidor
login-self-hosted-modal-url-helper = Especifica la URL base de tu instalación local de Bitwarden. Ejemplo: https://bitwarden.empresa.com
login-self-hosted-modal-url-error = La URL debe empezar por https://
login-self-hosted-modal-save = Guardar
login-self-hosted-modal-cancel = Cancelar

## Login — toasts
login-toast-unlock-failed-title = Fallo al desbloquear
login-toast-unlock-failed-body = Comprueba tu contraseña maestra e inténtalo de nuevo.
login-toast-login-failed-title = Fallo al iniciar sesión
login-toast-login-failed-body = Comprueba tu correo y contraseña e inténtalo de nuevo.
login-toast-pin-unsupported = El desbloqueo por PIN aún no es compatible
login-toast-biometrics-unsupported = El desbloqueo biométrico aún no es compatible

## About dialog
about-version-label = Versión
about-sdk-version-label = Versión del SDK
about-os-label = SO
about-architecture-label = Arquitectura
about-copy-button = Copiar
about-close-button = Cerrar

## Sidebar — sections and filters
sidebar-section-vault = Caja fuerte
sidebar-section-send = Send
sidebar-filter-my-vault = Mi caja fuerte
sidebar-filter-favorites = Favoritos
sidebar-filter-logins = Inicios de sesión
sidebar-filter-cards = Tarjetas
sidebar-filter-identities = Identidades
sidebar-filter-notes = Notas
sidebar-filter-ssh-keys = Claves SSH
sidebar-filter-archive = Archivo
sidebar-filter-trash = Papelera
sidebar-filter-text-send = Texto
sidebar-filter-file-send = Archivo
sidebar-item-generator = Generador
sidebar-item-import = Importar
sidebar-item-export = Exportar

## Vault list
vault-title = Caja fuerte
vault-new-button = Nuevo
vault-search-placeholder = Buscar
vault-column-name = Nombre
vault-column-options = Opciones
vault-toast-item-saved = Elemento guardado
vault-toast-save-failed-title = Fallo al guardar
vault-toast-save-failed-body = No se pudo guardar el elemento. Inténtalo de nuevo.
vault-toast-decrypt-failed-title = Fallo al descifrar
vault-toast-decrypt-failed-body = No se pudo cargar el elemento. Inténtalo de nuevo.
vault-toast-copied-username = Usuario copiado
vault-toast-copied-password = Contraseña copiada
vault-toast-copied-website = Sitio web copiado
vault-toast-copied-totp = Código de verificación copiado
vault-toast-copied-field = Campo copiado
vault-toast-copied-private-key = Clave privada copiada
vault-toast-copied-public-key = Clave pública copiada
vault-toast-copied-fingerprint = Huella digital copiada
vault-toast-item-deleted = Elemento movido a la papelera
vault-toast-delete-failed-title = Fallo al eliminar
vault-toast-delete-failed-body = No se pudo eliminar el elemento. Inténtalo de nuevo.
vault-delete-modal-title = ¿Eliminar elemento?
vault-delete-modal-body = "{ $name }" se moverá a la papelera.
vault-delete-modal-cancel = Cancelar
vault-delete-modal-confirm = Eliminar

## Account switcher
account-switcher-other-accounts = Otras cuentas de Bitwarden
account-switcher-options = Opciones
account-switcher-lock-now = Bloquear ahora
account-switcher-log-out = Cerrar sesión
account-switcher-lock-all = Bloquear todas las cuentas
account-switcher-settings = Configuración
account-switcher-add = Añadir cuenta

## Detail pane — headers per cipher type
detail-header-login = Ver inicio de sesión
detail-header-card = Ver tarjeta
detail-header-identity = Ver identidad
detail-header-note = Ver nota
detail-header-ssh-key = Ver clave SSH
detail-header-bank-account = Ver cuenta bancaria
detail-header-drivers-license = Ver carnet de conducir
detail-header-passport = Ver pasaporte

## Detail pane — section labels
detail-section-item-details = Detalles del elemento
detail-section-login-credentials = Credenciales de inicio de sesión
detail-section-autofill-options = Opciones de autocompletado
detail-section-card-details = Detalles de la tarjeta
detail-section-personal-details = Datos personales
detail-section-note = Nota
detail-section-ssh-key = Clave SSH
detail-section-bank-account = Cuenta bancaria
detail-section-drivers-license = Carnet de conducir
detail-section-passport = Pasaporte

## Detail pane — fields
detail-field-name = Nombre
detail-field-notes = Notas
detail-field-username = Nombre de usuario
detail-field-password = Contraseña
detail-field-totp = Código de verificación (TOTP)
detail-totp-invalid = Semilla TOTP no válida
detail-field-website = Sitio web
detail-field-cardholder-name = Titular de la tarjeta
detail-field-brand = Marca
detail-field-number = Número
detail-field-expiration = Caducidad
detail-field-security-code = Código de seguridad
detail-field-email = Correo electrónico
detail-field-phone = Teléfono
detail-field-company = Empresa
detail-field-address = Dirección
detail-field-city-region = Ciudad / región
detail-field-public-key = Clave pública
detail-field-private-key = Clave privada
detail-field-fingerprint = Huella digital
detail-empty-credentials = Sin credenciales
detail-empty-card = Sin detalles de tarjeta
detail-empty-identity = Sin datos de identidad
detail-empty-bank-account = Sin datos de cuenta bancaria
detail-field-bank-name = Nombre del banco
detail-field-bank-name-on-account = Titular de la cuenta
detail-field-bank-account-type = Tipo de cuenta
detail-field-bank-account-number = Número de cuenta
detail-field-bank-routing-number = Número de ruta
detail-field-bank-branch-number = Número de sucursal / institución
detail-field-bank-pin = PIN
detail-field-bank-swift-code = Código SWIFT
detail-field-bank-iban = IBAN
detail-field-bank-contact-phone = Teléfono de contacto del banco
detail-empty-drivers-license = Sin datos del carnet de conducir
detail-field-dl-first-name = Nombre
detail-field-dl-middle-name = Segundo nombre
detail-field-dl-last-name = Apellido
detail-field-dl-date-of-birth = Fecha de nacimiento
detail-field-dl-license-number = Número de carnet
detail-field-dl-issuing-country = País emisor
detail-field-dl-issuing-state = Estado / comunidad emisora
detail-field-dl-issuing-authority = Autoridad emisora
detail-field-dl-issue-date = Fecha de expedición
detail-field-dl-expiration-date = Fecha de caducidad
detail-field-dl-license-class = Clase de carnet
detail-empty-passport = Sin datos del pasaporte
detail-field-pp-given-name = Nombre
detail-field-pp-surname = Apellidos
detail-field-pp-date-of-birth = Fecha de nacimiento
detail-field-pp-sex = Sexo
detail-field-pp-birth-place = Lugar de nacimiento
detail-field-pp-nationality = Nacionalidad
detail-field-pp-passport-number = Número de pasaporte
detail-field-pp-passport-type = Tipo de pasaporte
detail-field-pp-national-id-number = Número de identificación nacional
detail-field-pp-issuing-country = País emisor
detail-field-pp-issuing-authority = Autoridad emisora
detail-field-pp-issue-date = Fecha de expedición
detail-field-pp-expiration-date = Fecha de caducidad
detail-section-custom-fields = Campos personalizados
detail-field-passkey = Clave de acceso
detail-field-passkey-created = Creada { $date }
detail-field-boolean-true = Sí
detail-field-boolean-false = No
detail-edit-button = Editar

## Cipher form — header titles
form-title-new-item = Nuevo elemento
form-title-edit-login = Editar inicio de sesión
form-title-edit-card = Editar tarjeta
form-title-edit-identity = Editar identidad
form-title-edit-note = Editar nota
form-title-edit-ssh-key = Editar clave SSH
form-title-edit-bank-account = Editar cuenta bancaria
form-title-edit-drivers-license = Editar carnet de conducir
form-title-edit-passport = Editar pasaporte

## Cipher form — buttons
form-save = Guardar
form-saving = Guardando…
form-cancel = Cancelar

## Cipher form — sections
form-section-item-details = Detalles del elemento
form-section-login-credentials = Credenciales de inicio de sesión
form-section-autofill-options = Opciones de autocompletado
form-section-card-details = Detalles de la tarjeta
form-section-personal-details = Datos personales
form-section-identification = Identificación
form-section-contact-info = Información de contacto
form-section-address = Dirección
form-section-ssh-key = Clave SSH
form-section-bank-account = Cuenta bancaria
form-section-drivers-license = Carnet de conducir
form-section-passport = Pasaporte
form-section-additional-options = Opciones adicionales
form-section-custom-fields = Campos personalizados

## Cipher form — item details
form-name = Nombre (obligatorio)
form-favorite = Favorito
form-reprompt = Solicitar contraseña maestra de nuevo
form-notes = Notas
form-folder = Carpeta
form-folder-none = Sin carpeta
form-organization = Organización
form-organization-personal = Personal (yo)
form-collections = Colecciones
form-collections-none = Sin colecciones
form-collections-selected = { $count } seleccionadas
form-collections-empty-in-org = Sin colecciones en esta organización

## Cipher form — login
form-username = Nombre de usuario
form-password = Contraseña
form-totp = Clave del autenticador (TOTP)

## Cipher form — websites
form-uri = Sitio web (URI)
form-uri-empty = Aún no hay sitios web
form-add-website = Añadir sitio web

## Cipher form — card
form-card-cardholder = Titular de la tarjeta
form-card-brand = Marca
form-card-number = Número
form-card-exp-month = Mes de caducidad
form-card-exp-year = Año de caducidad
form-card-code = Código de seguridad
form-card-brand-placeholder = -- Seleccionar --
form-card-month-placeholder = -- Mes --

## Cipher form — identity
form-identity-title = Tratamiento
form-identity-title-placeholder = -- Tratamiento --
form-identity-title-mr = Sr.
form-identity-title-mrs = Sra.
form-identity-title-ms = Srta.
form-identity-title-mx = Sr./Sra.
form-identity-title-dr = Dr.
form-identity-first-name = Nombre
form-identity-middle-name = Segundo nombre
form-identity-last-name = Apellidos
form-identity-username = Nombre de usuario
form-identity-company = Empresa
form-identity-ssn = Número de la Seguridad Social
form-identity-passport = Número de pasaporte
form-identity-license = Número de licencia
form-identity-email = Correo electrónico
form-identity-phone = Teléfono
form-identity-address1 = Dirección (línea 1)
form-identity-address2 = Dirección (línea 2)
form-identity-address3 = Dirección (línea 3)
form-identity-city = Ciudad / localidad
form-identity-state = Estado / provincia
form-identity-postal = Código postal
form-identity-country = País

## Cipher form — SSH key
form-ssh-public-key = Clave pública
form-ssh-private-key = Clave privada
form-ssh-fingerprint = Huella digital

## Cipher form — bank account
form-bank-name = Nombre del banco
form-bank-name-on-account = Titular de la cuenta
form-bank-account-type = Tipo de cuenta
form-bank-account-type-placeholder = -- Tipo de cuenta --
form-bank-account-type-checking = Corriente
form-bank-account-type-savings = Ahorros
form-bank-account-type-brokerage = Inversión
form-bank-account-type-money-market = Mercado monetario
form-bank-account-type-cd = Certificado de depósito (CD)
form-bank-account-type-other = Otro
form-bank-account-number = Número de cuenta
form-bank-routing-number = Número de ruta
form-bank-branch-number = Número de sucursal / institución
form-bank-pin = PIN
form-bank-swift-code = Código SWIFT
form-bank-iban = IBAN
form-bank-contact-phone = Teléfono de contacto del banco

## Cipher form — drivers license
form-dl-first-name = Nombre
form-dl-middle-name = Segundo nombre
form-dl-last-name = Apellido
form-dl-date-of-birth = Fecha de nacimiento
form-dl-license-number = Número de carnet
form-dl-issuing-country = País emisor
form-dl-issuing-state = Estado / comunidad emisora
form-dl-issuing-authority = Autoridad emisora
form-dl-issue-date = Fecha de expedición
form-dl-expiration-date = Fecha de caducidad
form-dl-license-class = Clase de carnet

## Cipher form — passport
form-pp-given-name = Nombre
form-pp-surname = Apellidos
form-pp-date-of-birth = Fecha de nacimiento
form-pp-sex = Sexo
form-pp-birth-place = Lugar de nacimiento
form-pp-nationality = Nacionalidad
form-pp-passport-number = Número de pasaporte
form-pp-passport-type = Tipo de pasaporte
form-pp-national-id-number = Número de identificación nacional
form-pp-issuing-country = País emisor
form-pp-issuing-authority = Autoridad emisora
form-pp-issue-date = Fecha de expedición
form-pp-expiration-date = Fecha de caducidad

## Cipher form — custom fields
form-custom-field-type = Tipo
form-custom-field-name = Nombre
form-custom-field-value = Valor
form-custom-field-type-text = Texto
form-custom-field-type-hidden = Oculto
form-custom-field-type-boolean = Booleano
form-custom-field-type-linked = Vinculado
form-custom-field-enabled = Activado
form-custom-field-empty = Aún no hay campos personalizados
form-add-custom-field = Añadir campo personalizado
form-custom-field-linked-unsupported = Los campos vinculados aún no son compatibles

## Toast — shared messages
toast-required-fields = Rellena los campos obligatorios.

## Menu bar — top-level
menu-file = Archivo
menu-edit = Editar
menu-view = Ver
menu-account = Cuenta
menu-window = Ventana
menu-help = Ayuda

## Menu — File
menu-file-new-login = Nuevo inicio de sesión
menu-file-new-item = Nuevo elemento
menu-file-new-item-login = Inicio de sesión
menu-file-new-item-card = Tarjeta
menu-file-new-item-identity = Identidad
menu-file-new-item-secure-note = Nota segura
menu-file-new-item-ssh-key = Clave SSH
menu-file-new-item-bank-account = Cuenta bancaria
menu-file-new-item-drivers-license = Carnet de conducir
menu-file-new-item-passport = Pasaporte
menu-file-new-folder = Nueva carpeta
menu-file-sync-now = Sincronizar ahora
menu-file-import = Importar
menu-file-export = Exportar
menu-file-settings = Ajustes
menu-file-lock-vault = Bloquear caja fuerte
menu-file-lock-all-vaults = Bloquear todas las cajas fuertes
menu-file-log-out = Cerrar sesión
menu-file-quit = Salir de Bitwarden

## Menu — Edit
menu-edit-undo = Deshacer
menu-edit-redo = Rehacer
menu-edit-cut = Cortar
menu-edit-copy = Copiar
menu-edit-paste = Pegar
menu-edit-select-all = Seleccionar todo
menu-edit-copy-username = Copiar nombre de usuario
menu-edit-copy-password = Copiar contraseña
menu-edit-copy-totp = Copiar código de verificación (TOTP)

## Menu — View
menu-view-search = Buscar en la caja fuerte
menu-view-generator = Generador
menu-view-generator-history = Historial del generador
menu-view-zoom-in = Acercar
menu-view-zoom-out = Alejar
menu-view-reset-zoom = Restablecer zoom

## Menu — Account
menu-account-change-password = Cambiar contraseña maestra
menu-account-two-step = Inicio de sesión en dos pasos
menu-account-fingerprint = Frase de huella digital
menu-account-delete = Eliminar cuenta

## Menu — Window
menu-window-minimize = Minimizar
menu-window-hide-to-tray = Ocultar en la bandeja
menu-window-always-on-top = Siempre visible
menu-window-close = Cerrar

## Menu — Help
menu-help-feedback = Ayuda y comentarios
menu-help-bug = Informar de un error
menu-help-legal = Legal
menu-help-legal-tos = Términos del servicio
menu-help-legal-privacy = Política de privacidad
menu-help-follow = Síguenos
menu-help-web-vault = Ir a la caja fuerte web
menu-help-browser-extension = Obtener extensión del navegador
menu-help-about = Acerca de Bitwarden

menu-toast-sync-success = Caja fuerte sincronizada
menu-toast-sync-failed-title = Error de sincronización
menu-toast-sync-failed-body = No se pudo sincronizar la caja fuerte. Inténtalo de nuevo más tarde.

menu-fingerprint-title = Frase de huella digital de tu cuenta:
menu-fingerprint-learn-more = Más información
menu-fingerprint-close = Cerrar

new-folder-modal-title = Nueva carpeta
new-folder-modal-field-label = Nombre de la carpeta (obligatorio)
new-folder-modal-helper = Anida una carpeta añadiendo el nombre de la carpeta padre seguido de "/". Ejemplo: Social/Foros
new-folder-modal-save = Guardar
new-folder-modal-cancel = Cancelar
new-folder-toast-success = Carpeta creada
new-folder-toast-failed-title = No se pudo crear la carpeta
new-folder-toast-failed-body = No fue posible guardar la carpeta. Inténtalo de nuevo.

## New-item picker modal
picker-title = Elige un elemento para añadir
picker-folder = Carpeta
picker-login-subtitle = Sitio web o aplicación
picker-card-subtitle = Tarjeta de crédito o débito
picker-bank-account-subtitle = Datos bancarios
picker-drivers-license-subtitle = Credenciales de conducción
picker-passport-subtitle = Documento de viaje
picker-identity-subtitle = Información personal
picker-secure-note-subtitle = Texto importante
picker-ssh-key-subtitle = Token de inicio de sesión del servidor
picker-folder-subtitle = Organiza tus elementos

## Bandeja
tray-show-hide = Mostrar / Ocultar
tray-lock-vault = Bloquear caja fuerte
tray-exit = Salir

# Endonym — see comment in the canonical en file.
language-name-self = Español

## Modal de ajustes
settings-title = Ajustes
settings-tab-security = Seguridad
settings-tab-integrations = Integraciones
settings-tab-autotype = Autocompletado y portapapeles
settings-tab-appearance = Apariencia
settings-tab-advanced = Avanzado

settings-toast-not-supported = Esta opción aún no está disponible.

# Pestaña Seguridad
settings-security-access-options = Opciones de acceso
settings-security-open-at-login = Abrir Bitwarden al iniciar sesión
settings-security-unlock-pin = Desbloquear con PIN
settings-security-unlock-touch = Desbloquear con Touch ID
settings-security-session-timeout = Tiempo de sesión
settings-security-lock-after = Bloquear tras
settings-security-logout-after = Cerrar sesión tras
settings-security-lock-on-system-lock = Bloquear cuando se bloquee el sistema

# Cadenas de duración compartidas entre los desplegables de tiempo (bloquear,
# cerrar sesión, borrar portapapeles). Los plurales los resuelve Fluent.
settings-duration-seconds = { $n ->
    [one] { $n } segundo
   *[other] { $n } segundos
  }
settings-duration-minutes = { $n ->
    [one] { $n } minuto
   *[other] { $n } minutos
  }
settings-duration-hours = { $n ->
    [one] { $n } hora
   *[other] { $n } horas
  }
settings-duration-never = Nunca

# Pestaña Integraciones
settings-integrations-browser = Integración con el navegador
settings-integrations-browser-enable = Activar integración con el navegador
settings-integrations-browser-fingerprint = Requerir huella de verificación
settings-integrations-ssh = Agente SSH
settings-integrations-ssh-enable = Activar agente SSH
settings-integrations-ssh-prompt = Comportamiento de la solicitud
settings-ssh-prompt-always = Ask (default: allow once)
settings-ssh-prompt-never = Nunca
settings-ssh-prompt-remember = Ask (default: allow until lock)

# Pestaña Autocompletado y portapapeles
settings-autotype-heading = Autocompletado
settings-autotype-enable = Activar autocompletado
settings-clipboard-heading = Portapapeles
settings-clipboard-clear-after = Borrar portapapeles tras
settings-clipboard-minimize-on-copy = Minimizar al copiar

# Pestaña Apariencia
settings-appearance-theme-heading = Tema
settings-appearance-theme = Tema
settings-appearance-theme-system = Sistema
settings-appearance-theme-light = Claro
settings-appearance-theme-dark = Oscuro
settings-appearance-language = Idioma
settings-appearance-language-system = Sistema
settings-appearance-display-heading = Visualización
settings-appearance-show-favicons = Mostrar iconos de URL

# Pestaña Avanzado
settings-advanced-tray = Bandeja
settings-advanced-tray-enable = Mostrar icono en la bandeja
settings-advanced-minimize-to-tray = Minimizar a la bandeja
settings-advanced-close-to-tray = Cerrar a la bandeja
settings-advanced-platform = Plataforma
settings-advanced-always-show-dock = Mostrar siempre el icono del Dock
settings-advanced-allow-screenshots = Permitir capturas de pantalla

## Lista de Sends
send-title = Send
send-new-button = Nuevo
send-search-placeholder = Buscar Sends
send-column-name = Nombre
send-column-deletion = Fecha de eliminación
send-empty-body = Aún no has creado ningún Send.
send-toast-item-saved = Send guardado
send-toast-item-deleted = Send eliminado
send-toast-copied-link = Enlace del Send copiado
send-toast-copied-password = Contraseña copiada
send-toast-load-failed-title = Error al cargar
send-toast-load-failed-body = No se pudo cargar el Send. Inténtalo de nuevo.
send-toast-save-failed-title = Error al guardar
send-toast-save-failed-body = No se pudo guardar el Send. Inténtalo de nuevo.
send-toast-delete-failed-title = Error al eliminar
send-toast-delete-failed-body = No se pudo eliminar el Send. Inténtalo de nuevo.

## Formulario de Send
send-form-title-new-text = Crear Send de texto
send-form-title-new-file = Crear Send de archivo
send-form-title-edit-text = Editar Send de texto
send-form-title-edit-file = Editar Send de archivo
send-form-details-heading = Detalles del Send
send-form-additional-heading = Opciones adicionales
send-form-name = Nombre (requerido)
send-form-text = Texto a compartir (requerido)
send-form-hide-text = Ocultar texto por defecto
send-form-file-choose = Elegir archivo
send-form-file-choose-placeholder = Ningún archivo seleccionado
send-form-file-name = Nombre del archivo
send-form-file-size = Tamaño
send-form-deletion-date = Fecha de eliminación
send-form-deletion-hint = El Send se eliminará permanentemente el {$date}.
send-form-who-can-view = Quién puede ver
send-form-access-link = Cualquiera con el enlace
send-form-access-people = Personas específicas
send-form-access-password = Cualquiera con la contraseña que definas
send-form-password = Contraseña (requerida)
send-form-password-hint = Los destinatarios deberán introducir la contraseña para ver este Send.
send-form-emails = Correos electrónicos (requerido)
send-form-send-link = Enlace del Send
send-form-limit-views = Límite de visualizaciones
send-form-limit-views-hint = Nadie podrá ver este Send una vez alcanzado el límite.
send-form-views-left = Nadie podrá ver este Send una vez alcanzado el límite. Quedan {$count} visualizaciones.
send-form-hide-email = Ocultar tu dirección de correo a los destinatarios.
send-form-private-note = Nota privada
send-form-save = Guardar
send-form-cancel = Cancelar

send-form-preset-1h = 1 hora
send-form-preset-1d = 1 día
send-form-preset-2d = 2 días
send-form-preset-3d = 3 días
send-form-preset-7d = 7 días
send-form-preset-14d = 14 días
send-form-preset-30d = 30 días

send-delete-modal-title = Eliminar Send
send-delete-modal-body = ¿Seguro que quieres eliminar "{$name}"?
send-delete-modal-cancel = Cancelar
send-delete-modal-confirm = Eliminar

## Generador
generator-title = Generador
generator-tab-password = Contraseña
generator-tab-passphrase = Frase de contraseña
generator-tab-username = Nombre de usuario

# Pestaña de contraseña
generator-length = Longitud
generator-length-hint = El valor debe estar entre 5 y 128.
generator-include = Incluir
generator-include-uppercase = A-Z
generator-include-lowercase = a-z
generator-include-numbers = 0-9
generator-include-special = !@#$%^&*
generator-min-number = Mínimo de números
generator-min-special = Mínimo de especiales
generator-avoid-ambiguous = Evitar caracteres ambiguos

# Pestaña de frase de contraseña
generator-num-words = Número de palabras
generator-num-words-hint = El valor debe estar entre 3 y 20. Usa 6 palabras o más para generar una frase de contraseña segura.
generator-word-separator = Separador de palabras
generator-passphrase-capitalize = Mayúsculas iniciales
generator-passphrase-include-number = Incluir número

# Pestaña de nombre de usuario
generator-username-type = Tipo
generator-username-kind-word = Palabra aleatoria
generator-username-kind-subaddress = Correo con sub-dirección
generator-username-kind-catchall = Correo catch-all
generator-username-capitalize = Mayúsculas iniciales
generator-username-include-number = Incluir número
generator-username-email = Correo
generator-username-domain = Dominio

# Historial
generator-history-open = Historial del generador
generator-history-title = Historial del generador
generator-history-heading = Reciente
generator-history-empty = Sin valores recientes.
generator-history-clear = Borrar historial
generator-history-just-now = ahora mismo
generator-history-minutes-ago = hace {$count} m
generator-history-hours-ago = hace {$count} h
generator-history-days-ago = hace {$count} d

# Toasts
generator-toast-copied = Copiado
generator-toast-failed = No se pudo generar

# Magnify launcher
magnify-search-placeholder = Bitwarden Magnify
magnify-locked-title = Tu bóveda está bloqueada
magnify-locked-subtitle = Abre Bitwarden para desbloquear
magnify-open-bitwarden = Abrir Bitwarden
magnify-no-results = Sin coincidencias
magnify-copy-password = Copiar contraseña
magnify-copy-username = Copiar usuario
magnify-hint-navigate = Navegar

## Modal de importar
import-modal-title = Importar datos
import-modal-section-destination = Destino
import-modal-vault-label = Bóveda (obligatorio)
import-modal-vault-personal = Mi bóveda
import-modal-folder-label = Carpeta
import-modal-folder-placeholder = - Selecciona una carpeta -
import-modal-collection-label = Colección (obligatorio)
import-modal-collection-placeholder = - Selecciona una colección -
import-modal-section-data = Datos
import-modal-file-format-label = Formato de archivo (obligatorio)
import-modal-file-helper = Selecciona el archivo a importar
import-modal-choose-file = Elegir archivo
import-modal-no-file = Ningún archivo elegido
import-modal-paste-label = o pega el contenido del archivo
import-modal-submit = Importar datos
import-modal-cancel = Cancelar
import-toast-unimplemented = La importación aún no está disponible

## Modal de exportar
export-modal-title = Exportar bóveda
export-modal-vault-label = Exportar desde (obligatorio)
export-modal-vault-personal = Mi bóveda
export-modal-banner-personal-title = Exportando bóveda individual
export-modal-banner-personal = Solo se exportarán los elementos individuales asociados a { $email }. No se incluirán los elementos de bóvedas de organización ni los archivos adjuntos.
export-modal-banner-org-title = Exportando bóveda de la organización
export-modal-banner-org-body = Solo se exportará la bóveda de la organización asociada a { $name }. No se incluirán los elementos de las bóvedas individuales ni de otras organizaciones.
export-modal-file-format-label = Formato de archivo (obligatorio)
export-modal-password-label = Contraseña de archivo (obligatorio)
export-modal-continue = Continuar
export-modal-cancel = Cancelar
export-confirm-title = Confirmar la exportación de la caja fuerte
export-confirm-warning-unencrypted = Esta exportación contiene tus datos de la caja fuerte en un formato no cifrado. No deberías almacenar o enviar el archivo exportado por canales no seguros (como el correo electrónico). Elimínalo inmediatamente cuando termines de utilizarlo.
export-confirm-warning-file-encrypted = Esta exportación de archivo estará protegida por contraseña, y requerirá la contraseña del archivo para descifrarla.
export-confirm-password-label = Contraseña maestra (obligatorio)
export-confirm-helper = Confirma tu identidad para continuar.
export-confirm-error = Contraseña maestra no válida.
export-toast-success = Bóveda exportada a { $path }
export-toast-failed-title = Falló la exportación
export-toast-failed-body = No se pudo exportar la bóveda. Inténtalo de nuevo.

# Composable vault filters
sidebar-group-vaults = Cajas fuertes
sidebar-group-view = Vista
sidebar-group-types = Tipos de elementos
sidebar-filter-all-vaults = Todas las cajas fuertes
sidebar-filter-active-items = Elementos activos
sidebar-filter-all-types = Todos los tipos

sidebar-group-folders = Carpetas
sidebar-filter-all-folders = Todas las carpetas
sidebar-filter-no-folder = Sin carpeta
sidebar-group-collections = Colecciones
sidebar-filter-all-collections = Todas las colecciones
sidebar-filter-no-collection = Sin colección

vault-column-last-changed = Último cambio
vault-column-base-url = URL base
vault-filter-folder = Carpeta: { $name }
vault-filter-collection = Colección: { $name }
vault-filter-search = Búsqueda: «{ $query }»

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
