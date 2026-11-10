//! Read-only startup proxy configuration and in-memory Basic-auth credentials.
//! A failed CONNECT queues consent; it never resends an origin request.
use reqwest_account::{ClientBuilder, Proxy};
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use zeroize::Zeroizing;

pub const AUTH_REQUIRED: &str = "The proxy requires authentication. Enter proxy credentials, then retry sign-in or synchronize. Writes are not retried automatically.";
struct Endpoint {
    raw: Zeroizing<String>,
    display: String,
}
impl Endpoint {
    fn parse(raw: String) -> Result<Self, ()> {
        let value = if raw.contains("://") {
            raw.clone()
        } else {
            format!("http://{raw}")
        };
        let url = url::Url::parse(&value).map_err(|_| ())?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(());
        }
        Proxy::https(&value).map_err(|_| ())?;
        let mut redacted = url.clone();
        redacted.set_username("").map_err(|_| ())?;
        redacted.set_password(None).map_err(|_| ())?;
        Ok(Self {
            raw: Zeroizing::new(value),
            display: redacted.to_string(),
        })
    }
}
struct Config {
    rows: Vec<(String, String)>,
    endpoint: Option<Endpoint>,
    invalid: bool,
    bypass: Zeroizing<String>,
}
impl Config {
    fn environment() -> Self {
        Self::from_get(|key| std::env::var(key).ok())
    }
    fn from_get(get: impl Fn(&str) -> Option<String>) -> Self {
        let value = |upper: &str, lower: &str| {
            get(upper)
                .map(|v| (upper.to_owned(), v))
                .or_else(|| get(lower).map(|v| (lower.to_owned(), v)))
        };
        let https = value("HTTPS_PROXY", "https_proxy");
        let all = value("ALL_PROXY", "all_proxy");
        let http = value("HTTP_PROXY", "http_proxy");
        let no = value("NO_PROXY", "no_proxy");
        let mut rows = Vec::new();
        for (name, raw) in [&https, &all, &http].into_iter().flatten() {
            let display = if raw.trim().is_empty() {
                "Not set".into()
            } else {
                Endpoint::parse(raw.clone())
                    .map(|e| e.display)
                    .unwrap_or_else(|_| "Invalid or unsupported proxy (HTTP/HTTPS only)".into())
            };
            rows.push((name.clone(), display));
        }
        let selected = https
            .as_ref()
            .filter(|(_, v)| !v.is_empty())
            .or_else(|| all.as_ref().filter(|(_, v)| !v.is_empty()));
        let endpoint = selected.map(|(_, v)| Endpoint::parse(v.clone()));
        let invalid = endpoint.as_ref().is_some_and(|e| e.is_err());
        let endpoint = endpoint.and_then(Result::ok);
        let bypass = no.as_ref().map(|(_, v)| v.clone()).unwrap_or_default();
        if let Some((name, value)) = no {
            let redacted = value
                .split(',')
                .map(|v| {
                    if v.contains('@') || v.contains("://") || v.chars().any(char::is_control) {
                        "[invalid bypass entry]"
                    } else {
                        v
                    }
                })
                .collect::<Vec<_>>()
                .join(",");
            rows.push((name, redacted));
        }
        Self {
            rows,
            endpoint,
            invalid,
            bypass: Zeroizing::new(bypass),
        }
    }
}
#[derive(Clone)]
pub struct Challenge {
    pub id: Uuid,
    pub address: String,
    pub rejected: bool,
}
struct Credentials {
    username: Zeroizing<String>,
    password: Zeroizing<String>,
}
struct State {
    version: u64,
    credentials: Option<Credentials>,
    pending: Option<Challenge>,
    suppressed: bool,
}
struct Inner {
    config: Config,
    state: Mutex<State>,
    wake: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
}
#[derive(Clone)]
pub struct Broker(Arc<Inner>);
impl Broker {
    pub fn environment() -> Self {
        Self::new(Config::environment())
    }
    fn new(config: Config) -> Self {
        Self(Arc::new(Inner {
            config,
            state: Mutex::new(State {
                version: 0,
                credentials: None,
                pending: None,
                suppressed: false,
            }),
            wake: Mutex::new(None),
        }))
    }
    pub fn rows(&self) -> Vec<(String, String)> {
        self.0.config.rows.clone()
    }
    pub fn address(&self) -> Option<String> {
        self.0.config.endpoint.as_ref().map(|e| e.display.clone())
    }
    pub fn version(&self) -> u64 {
        self.0.state.lock().unwrap().version
    }
    pub fn has_credentials(&self) -> bool {
        self.0.state.lock().unwrap().credentials.is_some()
    }
    pub fn set_wake(&self, wake: Arc<dyn Fn() + Send + Sync>) {
        *self.0.wake.lock().unwrap() = Some(wake);
    }
    fn wake(&self) {
        if let Some(wake) = self.0.wake.lock().unwrap().clone() {
            wake();
        }
    }
    pub fn pending(&self) -> Option<Challenge> {
        self.0.state.lock().unwrap().pending.clone()
    }
    pub fn current(&self, id: Uuid) -> bool {
        self.0
            .state
            .lock()
            .unwrap()
            .pending
            .as_ref()
            .is_some_and(|p| p.id == id)
    }
    pub fn cancel_pending(&self) {
        let mut state = self.0.state.lock().unwrap();
        if state.pending.take().is_some() {
            state.suppressed = true;
        }
    }
    pub fn forget(&self) {
        let mut state = self.0.state.lock().unwrap();
        state.credentials = None;
        state.pending = None;
        state.suppressed = false;
        state.version += 1;
        drop(state);
        self.wake();
    }
    pub fn request_auth(&self) {
        self.queue(None, true);
    }
    fn queue(&self, version: Option<u64>, manual: bool) {
        let Some(address) = self.address() else {
            return;
        };
        let mut state = self.0.state.lock().unwrap();
        if version.is_some_and(|v| v != state.version)
            || state.pending.is_some()
            || (!manual && state.suppressed)
        {
            return;
        }
        let rejected = state.credentials.take().is_some();
        state.version += 1;
        state.pending = Some(Challenge {
            id: Uuid::new_v4(),
            address,
            rejected,
        });
        state.suppressed = false;
        drop(state);
        self.wake();
    }
    pub fn submit(
        &self,
        id: Uuid,
        username: Zeroizing<String>,
        password: Zeroizing<String>,
    ) -> Result<(), String> {
        if username.is_empty()
            || username.len() > 1024
            || username.contains(':')
            || username.chars().any(char::is_control)
            || password.len() > 8192
        {
            return Err("Enter a valid proxy username (without a colon) and password".into());
        }
        let mut state = self.0.state.lock().unwrap();
        if !state.pending.as_ref().is_some_and(|p| p.id == id) {
            return Err("The proxy authentication request is no longer current".into());
        }
        state.credentials = Some(Credentials { username, password });
        state.pending = None;
        state.suppressed = false;
        state.version += 1;
        drop(state);
        self.wake();
        Ok(())
    }
    pub fn configure(&self, builder: ClientBuilder) -> Result<(ClientBuilder, u64), String> {
        if self.0.config.invalid {
            return Err("The configured HTTPS proxy is invalid or unsupported; HTTP/HTTPS proxies are supported".into());
        }
        let state = self.0.state.lock().unwrap();
        let mut builder = builder.no_proxy();
        if let Some(endpoint) = &self.0.config.endpoint {
            let mut proxy = Proxy::https(endpoint.raw.as_str())
                .map_err(|_| "Invalid proxy configuration")?
                .no_proxy(reqwest_account::NoProxy::from_string(&self.0.config.bypass));
            if let Some(credentials) = &state.credentials {
                proxy = proxy.basic_auth(&credentials.username, &credentials.password);
            }
            builder = builder.proxy(proxy);
        }
        Ok((builder, state.version))
    }
    pub fn authentication_error(
        &self,
        error: &(dyn std::error::Error + 'static),
        version: u64,
    ) -> bool {
        // hyper-util's TunnelError is not re-exported. Compare only its exact
        // pinned, static 407 diagnostic; never print/return raw errors or URLs.
        let mut source = Some(error);
        while let Some(error) = source {
            if matches!(
                error.to_string().as_str(),
                "proxy authorization required" | "tunnel error: proxy authorization required"
            ) {
                self.queue(Some(version), false);
                return true;
            }
            if let Some(io) = error.downcast_ref::<std::io::Error>()
                && let Some(inner) = io.get_ref()
            {
                source = Some(inner);
                continue;
            }
            source = error.source();
        }
        false
    }
    #[cfg(test)]
    pub(crate) fn test(https: &str, no: &str) -> Self {
        Self::new(Config::from_get(|k| match k {
            "HTTPS_PROXY" => Some(https.into()),
            "NO_PROXY" => Some(no.into()),
            _ => None,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_redact_credentials_and_follow_https_precedence() {
        let config = Config::from_get(|key| match key {
            "HTTPS_PROXY" => Some("http://secret-user:secret-password@proxy.invalid:3128".into()),
            "https_proxy" => Some("http://ignored.invalid:8080".into()),
            "ALL_PROXY" => Some("http://fallback.invalid:8080".into()),
            "NO_PROXY" => Some("localhost,.internal.invalid,http://hidden:password@invalid".into()),
            _ => None,
        });
        let output = serde_json::to_string(&config.rows).unwrap();
        for secret in [
            "secret-user",
            "secret-password",
            "hidden:password",
            "ignored.invalid",
        ] {
            assert!(!output.contains(secret));
        }
        assert_eq!(
            config.endpoint.as_ref().unwrap().display,
            "http://proxy.invalid:3128/"
        );
        assert!(output.contains("localhost,.internal.invalid,[invalid bypass entry]"));
        let lowercase = Config::from_get(|k| {
            if k == "all_proxy" {
                Some("http://fallback.invalid:8080".into())
            } else {
                None
            }
        });
        assert_eq!(
            lowercase.endpoint.unwrap().display,
            "http://fallback.invalid:8080/"
        );
        let socks = Broker::test("socks5://127.0.0.1:1080", "");
        assert!(socks.configure(reqwest_account::Client::builder()).is_err());
    }
    #[test]
    fn prompts_deduplicate_cancel_and_reject_stale_credentials() {
        let broker = Broker::test("http://proxy.invalid:3128", "");
        let error = std::io::Error::other("tunnel error: proxy authorization required");
        assert!(broker.authentication_error(&error, 0));
        let first = broker.pending().unwrap();
        assert!(broker.authentication_error(&error, 0));
        assert_eq!(broker.pending().unwrap().id, first.id);
        broker.cancel_pending();
        broker.authentication_error(&error, broker.version());
        assert!(broker.pending().is_none());
        assert!(
            broker
                .submit(
                    first.id,
                    Zeroizing::new("test".into()),
                    Zeroizing::new("password".into())
                )
                .is_err()
        );
        broker.request_auth();
        let next = broker.pending().unwrap();
        assert_ne!(first.id, next.id);
        broker
            .submit(
                next.id,
                Zeroizing::new("test".into()),
                Zeroizing::new("password".into()),
            )
            .unwrap();
        assert!(broker.has_credentials());
        broker.authentication_error(&error, 0);
        assert!(broker.pending().is_none());
        broker.authentication_error(&error, broker.version());
        assert!(broker.pending().unwrap().rejected);
        assert!(!broker.has_credentials());
        broker.forget();
        assert!(broker.pending().is_none());
    }
}
