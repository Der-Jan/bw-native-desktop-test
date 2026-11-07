//! Clipboard + URL-launch helpers for the detail pane's copy/launch buttons.
//!
//! A single long-lived worker thread owns the `arboard::Clipboard` and
//! serves copy commands over an `mpsc` channel. The worker's main loop
//! does `recv_timeout(timeout)` — arriving commands reset the timer,
//! a timeout fires the auto-clear. On channel disconnect (e.g. app
//! shutdown, via `ClipboardManager`'s `Drop`) the worker runs a final
//! clear before returning.
//!
//! Sensitive values are marked via platform-specific arboard extensions
//! (Windows: exclude from history + cloud + monitoring; macOS / Linux:
//! exclude from history) — matches the official Bitwarden client.
//!
//! `timeout = None` disables both the auto-clear and the clear-on-close.

use std::{
    sync::mpsc::{self, RecvTimeoutError},
    thread::{self, JoinHandle},
    time::Duration,
};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy)]
pub enum Sensitivity {
    /// Allow the clipboard manager / cloud clipboard to retain the value.
    /// Use for usernames and URLs — matches the official Bitwarden client.
    Normal,
    /// Exclude from Windows clipboard history + cloud sync, macOS
    /// concealed-type pasteboard, Linux exclude-from-history. Use for
    /// passwords and TOTP.
    Sensitive,
}

enum Command {
    Copy {
        value: String,
        sensitivity: Sensitivity,
    },
    /// `None` disables auto-clear and clear-on-close, but retains ownership
    /// so lock/logout can explicitly clear an app-owned clipboard value.
    SetTimeout(Option<Duration>),
    /// Explicit security transition; applies even if timed clearing is disabled.
    ClearOwned,
}

pub struct ClipboardManager {
    /// `Option` so `Drop` can drop the sender (signals worker to terminate)
    /// before joining.
    tx: Option<mpsc::Sender<Command>>,
    worker: Option<JoinHandle<()>>,
}

impl ClipboardManager {
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel();

        let timeout = Some(DEFAULT_TIMEOUT);
        let worker = thread::Builder::new()
            .name("clipboard".into())
            .spawn(move || worker_loop(cmd_rx, timeout))
            .expect("spawn clipboard worker thread");

        Self {
            tx: Some(cmd_tx),
            worker: Some(worker),
        }
    }

    pub fn copy(&self, value: String, sensitivity: Sensitivity) {
        self.send(Command::Copy { value, sensitivity });
    }

    /// `None` disables auto-clear AND clear-on-close. Passing `Some` re-enables
    /// timed clearing for tracked copies. Explicit lock/logout clearing is
    /// independent of this preference.
    pub fn set_timeout(&self, timeout: Option<Duration>) {
        self.send(Command::SetTimeout(timeout));
    }

    pub fn clear_owned(&self) {
        self.send(Command::ClearOwned);
    }

    fn send(&self, cmd: Command) {
        let Some(tx) = self.tx.as_ref() else {
            return;
        };
        if let Err(e) = tx.send(cmd) {
            tracing::error!(%e, "clipboard worker unreachable");
        }
    }
}

impl Drop for ClipboardManager {
    fn drop(&mut self) {
        // Drop the sender → worker sees `Disconnected`, runs its final clear,
        // and exits. This is also our clear-on-close path: `App` drop runs
        // during iced's shutdown, before the process exits.
        self.tx = None;

        let Some(worker) = self.worker.take() else {
            return;
        };
        let _ = worker.join();
    }
}

fn worker_loop(cmd_rx: mpsc::Receiver<Command>, initial_timeout: Option<Duration>) {
    let mut clipboard = match arboard::Clipboard::new() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(%e, "arboard init failed; clipboard manager disabled");
            return;
        }
    };

    let mut last: Option<String> = None;
    let mut timeout = initial_timeout;

    loop {
        // Only wait with a timeout when we're tracking an outstanding
        // clear. Otherwise block indefinitely on recv.
        let recv = match (last.as_ref(), timeout) {
            (Some(_), Some(t)) => cmd_rx.recv_timeout(t),
            _ => cmd_rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };

        match recv {
            Ok(Command::Copy { value, sensitivity }) => {
                if let Err(e) = arboard_write(&mut clipboard, &value, sensitivity) {
                    tracing::warn!(%e, "clipboard write failed");
                    continue;
                }

                // Keep ownership for security-transition clearing even when
                // the user has disabled the timed clear.
                last = Some(value);
            }
            Ok(Command::SetTimeout(new_timeout)) => {
                timeout = new_timeout;
            }
            Ok(Command::ClearOwned) => clear_tracked(&mut clipboard, &mut last),
            Err(e) => {
                // Explicit ClearOwned is independent of this timeout policy.
                if timeout.is_some() {
                    clear_tracked(&mut clipboard, &mut last);
                }

                if matches!(e, RecvTimeoutError::Disconnected) {
                    break;
                }
            }
        }
    }
    tracing::info!("clipboard worker exiting");
}

/// Only clears a value still owned by this process. Read failure is not ownership.
trait OwnedClipboard {
    fn read_owned_text(&mut self) -> Option<String>;
    fn clear_owned_text(&mut self) -> Result<(), String>;
}

impl OwnedClipboard for arboard::Clipboard {
    fn read_owned_text(&mut self) -> Option<String> {
        self.get_text().ok()
    }
    fn clear_owned_text(&mut self) -> Result<(), String> {
        self.clear().map_err(|e| e.to_string())
    }
}

fn clear_tracked(clipboard: &mut impl OwnedClipboard, last: &mut Option<String>) {
    if let Some(expected) = last.take()
        && clipboard.read_owned_text().as_deref() == Some(expected.as_str())
        && let Err(error) = clipboard.clear_owned_text()
    {
        tracing::warn!(%error, "clipboard clear failed");
    }
}

// ── arboard + platform extensions ─────────────────────────────────────────

fn arboard_write(
    clipboard: &mut arboard::Clipboard,
    value: &str,
    sensitivity: Sensitivity,
) -> Result<(), arboard::Error> {
    let mut set = clipboard.set();

    if matches!(sensitivity, Sensitivity::Sensitive) {
        #[cfg(target_os = "windows")]
        {
            use arboard::SetExtWindows;
            set = set
                .exclude_from_history()
                .exclude_from_cloud()
                .exclude_from_monitoring();
        }

        #[cfg(target_os = "macos")]
        {
            use arboard::SetExtApple;
            set = set.exclude_from_history();
        }

        #[cfg(target_os = "linux")]
        {
            use arboard::SetExtLinux;
            set = set.exclude_from_history();
        }
    }

    set.text(value)
}

// ── URL launching ─────────────────────────────────────────────────────────

/// Validate that `uri` is safe to hand to `open::that_detached`. Returns
/// the parsed URL when the scheme is `http`/`https` and the host is
/// non-empty; rejects everything else. `file://`, `javascript:`, and
/// `data:` must never reach the launcher.
///
/// Bitwarden frequently stores URIs without a scheme (e.g.
/// `acmecorp.atlassian.net`); when the input has no `://` the function
/// prepends `https://` so bare hosts launch correctly. Inputs that
/// already contain `://` are parsed strictly — without that gate, an
/// input like `https://` would fall back to `https://https://` and
/// accept it with host = `"https"`.
fn validate_launchable_url(uri: &str) -> Option<url::Url> {
    let parsed = if uri.contains("://") {
        url::Url::parse(uri).ok()?
    } else {
        url::Url::parse(&format!("https://{uri}")).ok()?
    };
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    if parsed.host_str().is_none_or(str::is_empty) {
        return None;
    }
    Some(parsed)
}

/// Open `uri` in the default browser, but only if it passes
/// [`validate_launchable_url`].
pub fn launch_url(uri: &str) {
    let Some(parsed) = validate_launchable_url(uri) else {
        tracing::debug!(uri, "launch_url: rejected");
        return;
    };
    if let Err(e) = open::that_detached(parsed.as_str()) {
        tracing::warn!(%e, "open::that_detached failed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestClipboard {
        value: Option<String>,
        cleared: bool,
    }
    impl OwnedClipboard for TestClipboard {
        fn read_owned_text(&mut self) -> Option<String> {
            self.value.clone()
        }
        fn clear_owned_text(&mut self) -> Result<(), String> {
            self.cleared = true;
            self.value = None;
            Ok(())
        }
    }

    #[test]
    fn security_clear_removes_owned_text_and_releases_the_worker_copy() {
        let mut clipboard = TestClipboard {
            value: Some("secret".into()),
            cleared: false,
        };
        let mut tracked = Some("secret".into());
        clear_tracked(&mut clipboard, &mut tracked);
        assert!(clipboard.cleared);
        assert!(tracked.is_none());
    }

    #[test]
    fn security_clear_preserves_foreign_or_unreadable_clipboard_contents() {
        for value in [Some("another application's text".into()), None] {
            let mut clipboard = TestClipboard {
                value,
                cleared: false,
            };
            let mut tracked = Some("secret".into());
            clear_tracked(&mut clipboard, &mut tracked);
            assert!(!clipboard.cleared);
            assert!(tracked.is_none());
        }
    }

    #[test]
    fn launch_https_url_is_accepted() {
        let url = validate_launchable_url("https://example.com/path").expect("accepted");
        assert_eq!(url.scheme(), "https");
        assert_eq!(url.host_str(), Some("example.com"));
    }

    #[test]
    fn launch_http_url_is_accepted() {
        let url = validate_launchable_url("http://example.com").expect("accepted");
        assert_eq!(url.scheme(), "http");
    }

    #[test]
    fn launch_bare_host_gets_https_prepended() {
        // Mirrors how Bitwarden stores URIs (no scheme).
        let url = validate_launchable_url("acmecorp.atlassian.net").expect("accepted");
        assert_eq!(url.scheme(), "https");
        assert_eq!(url.host_str(), Some("acmecorp.atlassian.net"));
    }

    #[test]
    fn launch_bare_host_with_path_gets_https_prepended() {
        let url = validate_launchable_url("example.com/login").expect("accepted");
        assert_eq!(url.scheme(), "https");
        assert_eq!(url.path(), "/login");
    }

    #[test]
    fn launch_rejects_javascript_scheme() {
        // The headline attack: stored URI with `javascript:` would otherwise
        // execute in whatever browser handles the protocol.
        assert!(validate_launchable_url("javascript:alert(1)").is_none());
    }

    #[test]
    fn launch_rejects_file_scheme() {
        assert!(validate_launchable_url("file:///etc/passwd").is_none());
    }

    #[test]
    fn launch_rejects_data_scheme() {
        assert!(validate_launchable_url("data:text/html,<script>alert(1)</script>").is_none());
    }

    #[test]
    fn launch_rejects_other_schemes() {
        // Schemes that use `://` go down the strict path and reject via
        // the scheme allowlist.
        assert!(validate_launchable_url("ftp://example.com/").is_none());
        assert!(validate_launchable_url("ssh://example.com").is_none());
        // Schemes that use opaque payloads (`scheme:payload`) hit the
        // fallback path; their payload mangles into an invalid host or
        // port, so the parse fails outright.
        assert!(validate_launchable_url("vbscript:msgbox").is_none());
        assert!(validate_launchable_url("tel:+15551234").is_none());
    }

    #[test]
    fn launch_rejects_empty_input() {
        assert!(validate_launchable_url("").is_none());
        assert!(validate_launchable_url("   ").is_none());
    }

    #[test]
    fn launch_rejects_empty_host_after_scheme() {
        // The fallback `https://`-prepend used to corrupt this into
        // `https://https://`, accepting it as host = "https". The strict
        // path now rejects it cleanly.
        assert!(validate_launchable_url("https://").is_none());
        assert!(validate_launchable_url("http://").is_none());
    }
}
