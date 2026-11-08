# Design Reference

> The production frontend migrated to egui on 2026-11-08. Historical Iced
> implementation notes below describe the preserved checkpoint `0945673`.
> Current architecture and supported behavior: [architecture](architecture.md)
> and [migration](egui-migration.md).


This document captures findings from the official Bitwarden desktop app (`clients/`
submodule) that inform the native Rust UI.

## Reference App Location

The official app is at `clients/` (git submodule of https://github.com/bitwarden/clients).
The desktop app is in `clients/apps/desktop/`, but many UI components are shared in
`clients/libs/`.

## Button Styles

The app has two style systems. The **newer component library** (Tailwind-based) is what
the current app uses:

### New Component Library (active)

- **Source**: `clients/libs/components/src/button/button.component.ts`
- **Border radius**: `tw-rounded-xl` = **12px** (we use 20px for a more pill-shaped look
  matching the actual rendered app)
- **Default button padding**: `9px 16px` (vertical/horizontal)
- **Font size**: 14px (0.875rem)
- **Border width**: 1px

### Old Desktop SCSS (legacy, being phased out)

- **Source**: `clients/apps/desktop/src/scss/buttons.scss`
- **Border radius**: `$border-radius` = **3px**
- **Padding**: `7px 15px`

### Current Implementation

| Property      | Primary (Unlock)                            | Secondary (Log out)            |
| ------------- | ------------------------------------------- | ------------------------------ |
| Border radius | 20px                                        | 20px                           |
| Background    | `#6baefa` (transparent on hover: `#aac3ef`) | transparent (hover: `#1f2a3c`) |
| Text color    | dark (`#070b18`)                            | accent blue (`#6baefa`)        |
| Border        | same as bg                                  | accent blue (`#6baefa`)        |

## Logo

### Files Found

- **White SVG**: `clients/apps/web/src/images/logo-white.svg` (290x45 viewbox, white fill)
- **Desktop PNG (white)**: `clients/apps/desktop/src/images/logo-white@2x.png`
- **Icon components**: `clients/libs/assets/src/svg/svgs/bitwarden-logo.icon.ts`

### Current Implementation

Logo SVG copied to `assets/logo-white.svg` and rendered via iced's `svg` widget at 209x35.

## Lock Screen Icon

- **Source**: `clients/libs/assets/src/svg/svgs/lock.icon.ts`
- Extracted to `assets/lock-icon.svg` with dark theme colors baked in
- Rendered at 64x60 above the "Your vault is locked" title

## Login Page Background Illustrations

- **Left**: `clients/libs/assets/src/svg/svgs/background-left-illustration.ts`
- **Right**: `clients/libs/assets/src/svg/svgs/background-right-illustration.ts`
- Extracted to `assets/bg-left.svg` and `assets/bg-right.svg` with dark theme fill colors
- Rendered at bottom corners, fixed size, 11% opacity

### Illustration Colors (dark theme)

From `clients/libs/components/src/tw-theme.css` (dark mode section):

- `tw-fill-illustration-outline` → `rgb(23, 93, 220)` = `#175ddc`
- `tw-fill-illustration-bg-primary` → `rgb(170, 195, 239)` = `#aac3ef`
- `tw-fill-illustration-bg-secondary` → `rgb(121, 161, 233)` = `#79a1e9`
- `tw-fill-illustration-bg-tertiary` → `rgb(243, 246, 249)` = `#f3f6f9`
- `tw-fill-illustration-logo` → `rgb(255, 255, 255)` = `#ffffff`
- `tw-fill-illustration-tertiary` → `rgb(255, 191, 0)` = `#ffbf00` (gold stars on lock)

## Color Palette

**Important**: The SCSS values in `variables.scss` do NOT match the actual rendered
colors. Always verify with a color picker against the running app or design mockups.

### Light Theme (default — from design mockup)

| Role                        | Hex              | Notes                          |
| --------------------------- | ---------------- | ------------------------------ |
| Sidebar / header background | `#173792`        | Dark blue nav background       |
| Content background          | `#ffffff`        | White                          |
| Detail pane background      | `#f4f6f9`        | Light gray                     |
| Card background             | `#ffffff`        | White                          |
| Text primary                | `#1a2029`        | Near-black                     |
| Text secondary              | `#5a6d91`        | Muted blue-gray                |
| Nav text                    | `#ffffff`        | White (sidebar is always dark) |
| Sidebar selected            | `#011066`        | Dark navy                      |
| Nav item hover              | sidebar-specific | Lighter than selected          |
| Border                      | `#e7e9ef`        | Light gray                     |
| Primary accent              | `#165ddc`        | Bitwarden blue                 |

### Dark Theme (picked from running app)

| Role                   | Hex       | Source                              |
| ---------------------- | --------- | ----------------------------------- |
| Main window background | `#070b18` | Color picker                        |
| Header / account bar   | `#1e2939` | Color picker                        |
| Card background        | `#101828` | Color picker                        |
| Sidebar background     | `#1d293d` | `--color-nav-bg-primary` (gray-800) |
| Input/button hover     | `#1f2a3c` | Color picker                        |
| Border                 | `#4c525f` | `boxBorderColor` in SCSS            |
| Primary accent         | `#6baefa` | `--color-brand-400` (dark mode)     |
| Accent hover           | `#aac3ef` | Color picker                        |
| Text primary           | `#ffffff` |                                     |
| Text secondary         | `#bac0ce` | `mutedColor` in SCSS                |

### SCSS Values (for reference, may not match rendered)

From `clients/apps/desktop/src/scss/variables.scss` (dark theme map):

- `backgroundColor`: `#1f242e`
- `backgroundColorAlt2`: `#15181e` (used as window bg in `window.main.ts`)
- `boxBackgroundColor`: `#2f343d`
- `boxBorderColor`: `#4c525f`
- `buttonBackgroundColor`: `#272b32`
- `primaryColor` / `buttonPrimaryColor`: `#6f9df1`

## Font

- **Font family**: Inter 18pt (`$font-family-sans-serif: Inter` in `variables.scss`)
- **Source**: https://github.com/rsms/inter (bundled as `assets/Inter_18pt-Medium.ttf` +
  `assets/Inter_18pt-Bold.ttf`)
- **Weight**: Medium (500) for body, Bold for emphasis. The "18pt" optical size variant
  uses an open single-storey "g" matching the official app.
- **Family name**: `"Inter 18pt"` (used in `APP_FONT` / `APP_FONT_BOLD` constants)
- **Sizes**: 5 values — 12 (captions), 14 (body/buttons), 16 (emphasis), 18 (section
  headers), 28 (page titles)

## Account Switcher

### Original Behavior (see `img/original/Account.png`)

- Shows initials circle + email + server URL + up/down arrow
- Dropdown pops under the bar as a floating panel
- Dropdown shows other accounts (not the active one) + "+ Add account"

### Current Implementation

- Trigger: initials + email + server + arrow (⌃/⌄)
- Dropdown: floats via `stack!` at the page level (not inside the top bar)
- Excludes active account, includes "+ Add account"

## Native Menu Bar

### Source

Menu entries from `clients/apps/desktop/src/main/menu/`:

- `menu.file.ts` — Add items, sync/import/export, settings, lock, quit
- `menu.edit.ts` — Undo/redo, cut/copy/paste, copy username/password/TOTP
- `menu.view.ts` — Search, generator, zoom, fullscreen, reload
- `menu.account.ts` — Premium, change password, 2FA, fingerprint, delete
- `menu.window.ts` — Minimize, hide to menu bar, always on top, close
- `menu.help.ts` — Help, bug report, legal, follow us, web vault, mobile/browser, about
- `menu.bitwarden.ts` — macOS-only app menu (prepended on macOS)

### Current Implementation

- **macOS**: Native menu via `muda` crate (`init_for_nsapp()`)
- **Windows/Linux**: Custom-drawn menu bar inside iced window (avoids 1px transparent gap
  from native Win32 menus)
- Menu items are stubs (no event handling wired yet)
- Shortcuts shown in dropdown items, platform-specific (Ctrl on Win/Linux, Cmd on macOS)

### Enabled/Disabled State Rules

Source: `clients/apps/desktop/src/main/menu/menu.*.ts`

Most items depend on vault lock state. The key conditions our implementation uses
(`menu::EnabledWhen`):

| Condition                 | Meaning                           | Affected Items                                                                                        |
| ------------------------- | --------------------------------- | ----------------------------------------------------------------------------------------------------- |
| `Unlocked` (`!is_locked`) | Vault is unlocked                 | Add items, Search, Generator, Settings, Import/Export, Copy username/password/TOTP, all Account items |
| `HasAccounts`             | At least one account exists       | Lock All, Log Out, Sync Now                                                                           |
| `HasLockable`             | Has accounts that support locking | Lock Vault submenu                                                                                    |
| `Always`                  | No condition                      | Undo/Redo/Cut/Copy/Paste, Zoom, Fullscreen, Minimize, Close, all Help items                           |

The upstream Electron code has a separate `hasAuthenticatedAccounts` gate for Sync that
behaves identically to `hasAccounts` in our setup (no separate "registered but not
authenticated" state exists yet); we collapsed it to `HasAccounts`.

### Keyboard Shortcuts

Source: Electron `accelerator` strings in menu.\*.ts. All use `CmdOrCtrl+` which maps to
Cmd on macOS, Ctrl on Windows/Linux.

**File**:

- New Login: Ctrl/Cmd+N
- Settings: Ctrl/Cmd+,
- Lock All Vaults: Ctrl/Cmd+L

**Edit**:

- Undo/Redo: Ctrl/Cmd+Z / Ctrl/Cmd+Y (macOS uses Cmd+Shift+Z for Redo)
- Cut/Copy/Paste: Ctrl/Cmd+X/C/V
- Select All: Ctrl/Cmd+A
- Copy Username: Ctrl/Cmd+U
- Copy Password: Ctrl/Cmd+P
- Copy TOTP: Ctrl/Cmd+T

**View**:

- Search: Ctrl/Cmd+F
- Generator: Ctrl/Cmd+G
- Zoom In/Out/Reset: Ctrl/Cmd+ +/-/0
- Toggle Fullscreen: F11 (macOS: Ctrl+Cmd+F)
- Reload: Ctrl/Cmd+Shift+R

**Window**:

- Minimize: Ctrl/Cmd+M
- Hide to Tray: Ctrl/Cmd+Shift+M
- Always on Top: Ctrl/Cmd+Shift+T
- Close: Ctrl/Cmd+W

### macOS-Specific App Menu ("Bitwarden")

On macOS, a "Bitwarden" menu is prepended (via `menu.bitwarden.ts`) with:

- About Bitwarden, Check for Updates
- Settings, Lock Vault, Lock All, Log Out (moved from File menu)
- Services, Hide/Hide Others/Show All, Quit These items are removed from the File menu on
  macOS to follow platform conventions.

### Submenus (not yet implemented)

Several items open submenus rather than performing a direct action:

- **New Item** → Login, Card, Identity, Secure Note, SSH Key (each with
  Ctrl/Cmd+Shift+L/C/I/S/K)
- **Lock Vault** → Lists each account email
- **Log Out** → Lists each account email
- **Legal** → Terms of Service, Privacy Policy
- **Follow Us** → Blog, Twitter, Facebook, GitHub, Mastodon
- **Get Mobile App** → iOS, Android
- **Get Browser Extension** → Chrome, Firefox, Opera, Edge, Safari
- **Troubleshooting** → Toggle Hardware Acceleration

## Screens

### Loading Screen

- Blank background, centered 48 px spinner (`colors.accent`)
- Shown briefly at startup while `ClientManager::load` parses the 24 MB mock vault on
  tokio's blocking pool
- Window title bar is rendered as normal; menus are attached (account-dependent items
  disabled until load completes)

### Login/Lock Screen

- Top bar: account switcher (right), bitwarden logo below on main background (left)
- Lock icon (shield with stars) centered
- "Your vault is locked" + email
- Card: floating label input, Unlock button (replaced in-place by spinner during the
  unlock task), "or" separator, alternate-method buttons, Log out button
- During the unlock task: the password input is read-only, primary button shows a spinner
  at the same padded height, alternate buttons + Log out stay visible but inert
- Background illustrations at bottom corners (11% opacity)
- "Accessing bitwarden.com" status text at bottom

### Main Vault Screen

- **Top bar**: Search input + account switcher
- **Sidebar**: "bitwarden / Password Manager" header, category tree navigation
- **Item list**: Virtualized scrollable over 20 k+ items, colored initial circles + name +
  subtitle
- **Detail pane**: Per-type cards (Login / Card / Identity / SecureNote / SshKey). Edit /
  Delete buttons are stubs today. SSH private keys and card CVVs currently render in
  plaintext — TODO: reveal-on-click matching the official app.
