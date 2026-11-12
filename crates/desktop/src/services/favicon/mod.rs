//! Favicon / cipher-icon service.
//!
//! Lazy, session-scoped: the first [`FaviconService::get`] call for a given
//! `(uid, hostname)` pair returns [`IconState::Pending`] and spawns a fetch;
//! subsequent calls are pure read-lock lookups. Fetch completions broadcast
//! a [`FaviconMessage::IconResolved`] that the App subscription turns into
//! a redraw. Validated icons are also cached encrypted in account SQLite; requests use the account’s scoped
//! TLS/mTLS transport, with no external icon-service fallback.
//!
//! `view()` only calls `get()` for virtualized rows on screen, so "first
//! sight of a hostname in the viewport" is what triggers a fetch.

use std::{
    collections::HashMap,
    sync::{
        Arc, OnceLock, RwLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use eframe::egui::ColorImage;
use tokio::sync::{Semaphore, broadcast};

use crate::{domain::UserId, services::sdk::FaviconSource};

pub type Hostname = String;

/// Rendered size for cipher-row icons (logical px). Also the bitmap dimensions
/// we bake at fetch time so the rounded-corner alpha mask lines up exactly
/// with the rendered pixels.
pub const ICON_SIZE_PX: u32 = 32;

/// Matches `theme::RADIUS_SM` so the bitmap edge and any surrounding container
/// look identical.
const ICON_CORNER_RADIUS: f32 = 4.0;

const FETCH_CONCURRENCY: usize = 5;
const FETCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

// ── Public API ─────────────────────────────────────────────────────────────

/// Cheap-to-clone handle. Clones share the same internal state via `Arc`.
#[derive(Clone)]
pub struct FaviconService {
    inner: Arc<FaviconInner>,
}

struct FaviconInner {
    version: AtomicU64,
    caches: RwLock<HashMap<UserId, UserCache>>,
    fetch_budget: Arc<Semaphore>,
    /// Runtime handle captured at construction. `get()` spawns from `view()`
    /// which is a synchronous context; using the handle explicitly avoids
    /// relying on tokio TLS being set on every UI render pass.
    runtime: tokio::runtime::Handle,
    /// Broadcast sender for fetch completions. App's subscription clones this
    /// and calls [`broadcast::Sender::subscribe`] each time it (re)builds the
    /// stream, so receivers follow the App's lifetime instead of process
    /// lifetime.
    tx: broadcast::Sender<FaviconMessage>,
}

struct UserCache {
    id: uuid::Uuid,
    active: Arc<AtomicBool>,
    cancelled: tokio_util::sync::CancellationToken,
    source: FaviconSource,
    entries: HashMap<Hostname, IconState>,
    failures: HashMap<Hostname, (u8, tokio::time::Instant)>,
    last_error: Option<String>,
}

impl Drop for UserCache {
    fn drop(&mut self) {
        self.active.store(false, Ordering::SeqCst);
        self.cancelled.cancel();
    }
}

/// `Pending` deduplicates in-flight requests and renders like `Missing`.
/// Decoded pixels are shared; the UI uploads a texture once per hostname and
/// clears those textures when the source/cache version changes.
#[derive(Clone)]
pub enum IconState {
    Pending,
    Found(Arc<ColorImage>),
    Missing,
}

/// Fetch-completion message delivered via [`FaviconService::event_sender`].
/// Arrival drives the row's globe→favicon swap.
#[derive(Debug, Clone)]
pub enum FaviconMessage {
    IconResolved { uid: UserId, hostname: Hostname },
}

impl FaviconService {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel::<FaviconMessage>(1024);

        Self {
            inner: Arc::new(FaviconInner {
                version: AtomicU64::new(0),
                caches: RwLock::new(HashMap::new()),
                fetch_budget: Arc::new(Semaphore::new(FETCH_CONCURRENCY)),
                runtime: tokio::runtime::Handle::current(),
                tx,
            }),
        }
    }

    /// Sender clone for App's subscription wiring. Each
    /// `Subscription::run`-style call uses [`broadcast::Sender::subscribe`]
    /// to mint a fresh receiver, so this can be called freely from
    /// `App::subscription`.
    pub fn version(&self) -> u64 {
        self.inner.version.load(Ordering::Relaxed)
    }

    pub fn event_sender(&self) -> broadcast::Sender<FaviconMessage> {
        self.inner.tx.clone()
    }

    /// Render-time lookup. Fast path is a single read lock and a clone. Slow
    /// path (first sight) claims `Pending` under a write lock and spawns a
    /// background fetch.
    pub fn get(&self, uid: &UserId, hostname: &str) -> IconState {
        {
            let caches = self
                .inner
                .caches
                .read()
                .expect("favicon cache lock poisoned");
            let Some(cache) = caches.get(uid).filter(|cache| cache.source.current()) else {
                return IconState::Missing;
            };
            if let Some(state) = cache.entries.get(hostname) {
                let retry = matches!(state, IconState::Missing)
                    && cache
                        .failures
                        .get(hostname)
                        .is_some_and(|(_, at)| *at <= tokio::time::Instant::now());
                if !retry {
                    return state.clone();
                }
            }
        }
        self.claim_and_spawn(*uid, hostname.to_string());
        IconState::Pending
    }

    /// Resolve the favicon for a login's first URI to an `Arc<ColorImage>`,
    /// falling back to the globe icon when the URI is missing, unfetchable,
    /// or the favicon hasn't loaded yet. Triggers a background fetch on
    /// first sight via [`Self::get`].
    pub fn handle_for_login_uri(&self, uid: &UserId, uri: Option<&str>) -> Arc<ColorImage> {
        uri.and_then(hostname_for_fetch)
            .map(|h| self.get(uid, &h))
            .and_then(|state| match state {
                IconState::Found(handle) => Some(handle),
                _ => None,
            })
            .unwrap_or_else(globe_handle)
    }

    /// Drop the in-memory entry for `uid`. In-flight fetches for this user
    /// stay alive until they finish but their results no-op when the cache
    /// entry is gone.
    pub fn evict_user(&self, uid: &UserId) {
        if let Some(cache) = self
            .inner
            .caches
            .write()
            .expect("favicon cache lock poisoned")
            .remove(uid)
        {
            cache.active.store(false, Ordering::SeqCst);
            self.inner.version.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn set_source(&self, uid: UserId, source: Option<FaviconSource>) {
        let mut caches = self.inner.caches.write().expect("favicon cache poisoned");
        match source {
            Some(source) => {
                if !caches
                    .get(&uid)
                    .is_some_and(|cache| cache.source.same_authority(&source))
                {
                    self.inner.version.fetch_add(1, Ordering::Relaxed);
                    if let Some(old) = caches.insert(
                        uid,
                        UserCache {
                            id: uuid::Uuid::new_v4(),
                            active: Arc::new(AtomicBool::new(true)),
                            cancelled: tokio_util::sync::CancellationToken::new(),
                            source,
                            entries: HashMap::new(),
                            failures: HashMap::new(),
                            last_error: None,
                        },
                    ) {
                        old.active.store(false, Ordering::SeqCst);
                    }
                }
            }
            None => {
                if let Some(cache) = caches.remove(&uid) {
                    cache.active.store(false, Ordering::SeqCst);
                    self.inner.version.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    }
    /// Retry failures on the next visible-row lookup; retain successful icons.
    pub fn retry_missing(&self, uid: &UserId) {
        if let Some(cache) = self.inner.caches.write().unwrap().get_mut(uid) {
            cache
                .entries
                .retain(|_, state| !matches!(state, IconState::Missing));
            cache.failures.clear();
            cache.last_error = None;
        }
    }
    pub fn failure_message(&self, uid: &UserId) -> Option<String> {
        self.inner
            .caches
            .read()
            .unwrap()
            .get(uid)
            .and_then(|cache| cache.last_error.clone())
    }
    pub fn counts(&self, uid: &UserId) -> (usize, usize, usize) {
        let caches = self.inner.caches.read().unwrap();
        let Some(cache) = caches.get(uid) else {
            return (0, 0, 0);
        };
        cache.entries.values().fold((0, 0, 0), |mut counts, state| {
            match state {
                IconState::Found(_) => counts.0 += 1,
                IconState::Pending => counts.1 += 1,
                IconState::Missing => counts.2 += 1,
            }
            counts
        })
    }
    fn claim_and_spawn(&self, uid: UserId, hostname: Hostname) {
        let (source, cache_id, active, cancelled) = {
            let mut caches = self.inner.caches.write().expect("favicon cache poisoned");
            let Some(cache) = caches.get_mut(&uid).filter(|cache| cache.source.current()) else {
                return;
            };
            if let Some(state) = cache.entries.get(&hostname) {
                let retry = matches!(state, IconState::Missing)
                    && cache
                        .failures
                        .get(&hostname)
                        .is_some_and(|(_, at)| *at <= tokio::time::Instant::now());
                if !retry {
                    return;
                }
            }
            cache.entries.insert(hostname.clone(), IconState::Pending);
            (
                cache.source.clone(),
                cache.id,
                cache.active.clone(),
                cache.cancelled.clone(),
            )
        };
        let inner = self.inner.clone();
        self.inner.runtime.spawn(async move {
            fetch_and_commit(inner, uid, hostname, source, cache_id, active, cancelled).await;
        });
    }
}

// ── Hostname extraction ────────────────────────────────────────────────────

/// Extract the hostname we'd use to fetch a favicon, or `None` if the URI is
/// unsuitable (non-http(s) scheme, bare IP, single-label, tor / i2p).
///
/// Matches the Angular clients' `Utils.getHostname()` — full hostname, not
/// eTLD+1. `mail.google.com` and `google.com` produce different cache entries.
pub fn hostname_for_fetch(uri: &str) -> Option<Hostname> {
    let parsed = url::Url::parse(uri)
        .or_else(|_| url::Url::parse(&format!("http://{uri}")))
        .ok()?;
    match parsed.scheme() {
        "http" | "https" => {}
        _ => return None,
    }
    let host = parsed.host_str()?.to_ascii_lowercase();
    if host.parse::<std::net::IpAddr>().is_ok() {
        return None;
    }
    if !host.contains('.') {
        return None;
    }
    if host.ends_with(".onion") || host.ends_with(".i2p") {
        return None;
    }
    Some(host)
}

// ── Fetch pipeline ─────────────────────────────────────────────────────────

async fn fetch_and_commit(
    inner: Arc<FaviconInner>,
    uid: UserId,
    hostname: Hostname,
    source: FaviconSource,
    cache_id: uuid::Uuid,
    active: Arc<AtomicBool>,
    cancelled: tokio_util::sync::CancellationToken,
) {
    let (state, error) = tokio::select! {
        biased;
        _ = cancelled.cancelled() => return,
        state = fetch_one(&inner, &source, &hostname, active.clone()) => state,
    };
    let mut retry_at = None;
    let committed = {
        let mut caches = inner.caches.write().expect("favicon cache lock poisoned");
        if let Some(cache) = caches.get_mut(&uid).filter(|cache| {
            cache.id == cache_id && cache.source.same_authority(&source) && source.current()
        }) {
            if matches!(&state, IconState::Missing) {
                cache.last_error = error;
                let attempt = cache
                    .failures
                    .get(&hostname)
                    .map_or(1, |(n, _)| n.saturating_add(1));
                let seconds = (30u64 << attempt.saturating_sub(1).min(4)).min(300);
                let at = tokio::time::Instant::now() + std::time::Duration::from_secs(seconds);
                cache.failures.insert(hostname.clone(), (attempt, at));
                retry_at = Some(at);
            } else {
                cache.failures.remove(&hostname);
                if cache.failures.is_empty() {
                    cache.last_error = None;
                }
            }
            cache.entries.insert(hostname.clone(), state);
            true
        } else {
            // User was evicted mid-fetch; drop the result silently.
            false
        }
    };
    if committed {
        let _ = inner.tx.send(FaviconMessage::IconResolved {
            uid,
            hostname: hostname.clone(),
        });
    }
    if let Some(at) = retry_at {
        let weak = Arc::downgrade(&inner);
        // One wake-up, not a polling loop. Only a visible-row get() retries HTTP.
        inner.runtime.spawn(async move {
            tokio::select! {
                biased;
                _ = cancelled.cancelled() => return,
                _ = tokio::time::sleep_until(at) => {},
            }
            if !active.load(Ordering::SeqCst) {
                return;
            }
            let Some(inner) = weak.upgrade() else {
                return;
            };
            let current = inner.caches.read().unwrap().get(&uid).is_some_and(|cache| {
                cache.id == cache_id
                    && cache.source.current()
                    && cache
                        .failures
                        .get(&hostname)
                        .is_some_and(|(_, deadline)| *deadline == at)
            });
            if current {
                let _ = inner
                    .tx
                    .send(FaviconMessage::IconResolved { uid, hostname });
            }
        });
    }
}

async fn fetch_one(
    inner: &FaviconInner,
    source: &FaviconSource,
    hostname: &Hostname,
    active: Arc<AtomicBool>,
) -> (IconState, Option<String>) {
    let Ok(_permit) = inner.fetch_budget.clone().acquire_owned().await else {
        return (IconState::Missing, None);
    };
    if !active.load(Ordering::SeqCst) {
        return (IconState::Missing, None);
    }
    if let Some(bytes) = source.cached_icon(hostname).await
        && let Ok(state) = decode_cached(bytes).await
    {
        return (state, None);
    }
    if !active.load(Ordering::SeqCst) {
        return (IconState::Missing, None);
    }
    match tokio::time::timeout(FETCH_TIMEOUT, source.fetch(hostname)).await {
        Ok(Ok(bytes)) => match decode_to_handle(bytes).await {
            Ok((state, png)) => {
                if active.load(Ordering::SeqCst) {
                    let _ = source.cache_icon(hostname, png, active).await;
                }
                (state, None)
            }
            Err(_) => (
                IconState::Missing,
                Some("The server returned an unsupported or invalid icon image.".into()),
            ),
        },
        Ok(Err(error)) => (IconState::Missing, Some(error)),
        Err(_) => (IconState::Missing, Some("Icon download timed out.".into())),
    }
}

// ── Decode + alpha-mask pipeline ──────────────────────────────────────────
async fn decode_to_handle(raw: Vec<u8>) -> Result<(IconState, Vec<u8>), DecodeError> {
    tokio::task::spawn_blocking(move || {
        let rgba = decode_and_mask(&raw).map_err(|e| DecodeError(e.to_string()))?;
        let state = handle(&rgba);
        let mut png = std::io::Cursor::new(Vec::new());
        rgba.write_to(&mut png, image::ImageFormat::Png)
            .map_err(|e| DecodeError(e.to_string()))?;
        Ok((state, png.into_inner()))
    })
    .await
    .map_err(|e| DecodeError(e.to_string()))?
}
async fn decode_cached(bytes: Vec<u8>) -> Result<IconState, DecodeError> {
    tokio::task::spawn_blocking(move || {
        let mut reader =
            image::ImageReader::with_format(std::io::Cursor::new(bytes), image::ImageFormat::Png);
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(ICON_SIZE_PX);
        limits.max_image_height = Some(ICON_SIZE_PX);
        limits.max_alloc = Some(1024 * 1024);
        reader.limits(limits);
        let image = reader
            .decode()
            .map_err(|e| DecodeError(e.to_string()))?
            .into_rgba8();
        if image.dimensions() != (ICON_SIZE_PX, ICON_SIZE_PX) {
            return Err(DecodeError("Invalid cached icon dimensions".into()));
        }
        if image.pixels().all(|pixel| pixel.0[3] == 0) {
            return Err(DecodeError("Cached icon has no visible pixels".into()));
        }
        // Persistent pixels already have their corner mask. Do not apply it twice.
        Ok(handle(&image))
    })
    .await
    .map_err(|e| DecodeError(e.to_string()))?
}
fn handle(rgba: &image::RgbaImage) -> IconState {
    IconState::Found(Arc::new(ColorImage::from_rgba_unmultiplied(
        [ICON_SIZE_PX as usize, ICON_SIZE_PX as usize],
        rgba.as_raw(),
    )))
}

#[derive(Debug)]
struct DecodeError(String);

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn decode_and_mask(raw: &[u8]) -> Result<::image::RgbaImage, DecodeError> {
    let is_xml = std::str::from_utf8(raw).ok().is_some_and(|s| {
        s.trim_start_matches('\u{feff}')
            .trim_start()
            .starts_with('<')
    });
    let mut rgba = if is_xml {
        let options = resvg::usvg::Options {
            // Untrusted icons must never read local files, URLs or embedded images.
            image_href_resolver: resvg::usvg::ImageHrefResolver {
                resolve_string: Box::new(|_, _| None),
                resolve_data: Box::new(|_, _, _| None),
            },
            ..Default::default()
        };
        let tree =
            resvg::usvg::Tree::from_data(raw, &options).map_err(|e| DecodeError(e.to_string()))?;
        if tree.size().width() > 4096. || tree.size().height() > 4096. {
            return Err(DecodeError("SVG icon dimensions exceed limits".into()));
        }
        let mut pixmap = resvg::tiny_skia::Pixmap::new(ICON_SIZE_PX, ICON_SIZE_PX)
            .ok_or_else(|| DecodeError("Icon allocation failed".into()))?;
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::from_scale(
                ICON_SIZE_PX as f32 / tree.size().width(),
                ICON_SIZE_PX as f32 / tree.size().height(),
            ),
            &mut pixmap.as_mut(),
        );
        let pixels = pixmap
            .pixels()
            .iter()
            .flat_map(|p| {
                let p = p.demultiply();
                [p.red(), p.green(), p.blue(), p.alpha()]
            })
            .collect();
        image::RgbaImage::from_raw(ICON_SIZE_PX, ICON_SIZE_PX, pixels)
            .ok_or_else(|| DecodeError("Invalid icon pixels".into()))?
    } else {
        let mut reader = image::ImageReader::new(std::io::Cursor::new(raw))
            .with_guessed_format()
            .map_err(|e| DecodeError(e.to_string()))?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(4096);
        limits.max_image_height = Some(4096);
        limits.max_alloc = Some(32 * 1024 * 1024);
        reader.limits(limits);
        reader
            .decode()
            .map_err(|e| DecodeError(e.to_string()))?
            .resize_exact(
                ICON_SIZE_PX,
                ICON_SIZE_PX,
                image::imageops::FilterType::Lanczos3,
            )
            .to_rgba8()
    };
    apply_rounded_mask(&mut rgba, ICON_CORNER_RADIUS);
    if rgba.pixels().all(|pixel| pixel.0[3] == 0) {
        return Err(DecodeError("Icon has no visible pixels".into()));
    }
    Ok(rgba)
}

/// Multiply every pixel's alpha by a rounded-rect coverage value. The
/// 1-pixel band at the corner edge is antialiased so the result doesn't
/// shimmer at our 32px size.
fn apply_rounded_mask(img: &mut ::image::RgbaImage, radius: f32) {
    let w = img.width() as f32;
    let h = img.height() as f32;
    if radius <= 0.0 {
        return;
    }
    let r = radius.min(w / 2.0).min(h / 2.0);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let px = x as f32 + 0.5;
        let py = y as f32 + 0.5;
        // Closest point on the inner rectangle whose corner arcs have centre
        // (r, r), (w-r, r), (r, h-r), (w-r, h-r). When the pixel falls in the
        // straight-edge region the clamped point equals the pixel itself and
        // `dist` is 0 → fully covered.
        let cx = px.clamp(r, w - r);
        let cy = py.clamp(r, h - r);
        let dx = px - cx;
        let dy = py - cy;
        let dist = (dx * dx + dy * dy).sqrt();
        // Coverage: 1 fully inside, 0 fully outside, linear across a 1-pixel
        // band. Matches iced's own container antialiasing closely enough
        // that a Found icon sits on any theme background without a seam.
        let coverage = (r + 0.5 - dist).clamp(0.0, 1.0);
        if coverage < 1.0 {
            let a = pixel.0[3] as f32;
            pixel.0[3] = (a * coverage) as u8;
        }
    }
}

// ── Globe fallback handle ──────────────────────────────────────────────────

/// Process-wide cached globe handle. `Arc<ColorImage>::from_bytes` calls
/// `Id::unique()` internally, so naively re-constructing the handle on each
/// render produces a fresh id every frame, defeating iced's GPU texture
/// cache and visibly flickering the globe icon during scroll.
static GLOBE_HANDLE: OnceLock<Arc<ColorImage>> = OnceLock::new();

pub fn globe_handle() -> Arc<ColorImage> {
    GLOBE_HANDLE
        .get_or_init(|| {
            let image = image::load_from_memory(crate::assets::BWI_GLOBE_PNG)
                .expect("embedded globe")
                .into_rgba8();
            Arc::new(ColorImage::from_rgba_unmultiplied(
                [image.width() as usize, image.height() as usize],
                image.as_raw(),
            ))
        })
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── hostname_for_fetch ────────────────────────────────────────────────

    #[test]
    fn hostname_extracts_https_host() {
        assert_eq!(
            hostname_for_fetch("https://example.com/path?q=1"),
            Some("example.com".into())
        );
    }

    #[test]
    fn hostname_extracts_http_host() {
        assert_eq!(
            hostname_for_fetch("http://example.com"),
            Some("example.com".into())
        );
    }

    #[test]
    fn hostname_lowercases() {
        assert_eq!(
            hostname_for_fetch("https://Mail.Google.COM/inbox"),
            Some("mail.google.com".into())
        );
    }

    #[test]
    fn hostname_preserves_subdomain() {
        // The doc comment promises full hostname, not eTLD+1.
        assert_eq!(
            hostname_for_fetch("https://mail.google.com/"),
            Some("mail.google.com".into())
        );
        assert_eq!(
            hostname_for_fetch("https://google.com/"),
            Some("google.com".into())
        );
        assert_ne!(
            hostname_for_fetch("https://mail.google.com/"),
            hostname_for_fetch("https://google.com/")
        );
    }

    #[test]
    fn hostname_scheme_prepend_retry_recovers_bare_host() {
        assert_eq!(
            hostname_for_fetch("example.com"),
            Some("example.com".into())
        );
        assert_eq!(
            hostname_for_fetch("Example.com/path"),
            Some("example.com".into())
        );
    }

    #[test]
    fn hostname_rejects_ipv4() {
        assert_eq!(hostname_for_fetch("https://1.2.3.4/"), None);
        assert_eq!(hostname_for_fetch("http://192.168.0.1"), None);
    }

    #[test]
    fn hostname_rejects_ipv6() {
        assert_eq!(hostname_for_fetch("https://[::1]/"), None);
        assert_eq!(hostname_for_fetch("https://[2001:db8::1]/"), None);
    }

    #[test]
    fn hostname_rejects_single_label() {
        // No dot = unfit for a public favicon fetch (e.g. `localhost`,
        // intranet hostnames).
        assert_eq!(hostname_for_fetch("https://localhost/"), None);
        assert_eq!(hostname_for_fetch("myhost"), None);
    }

    #[test]
    fn hostname_rejects_onion_and_i2p() {
        assert_eq!(hostname_for_fetch("https://example.onion/"), None);
        assert_eq!(hostname_for_fetch("http://example.i2p/"), None);
        // The match is suffix-based, so a multi-label `.onion` still rejects.
        assert_eq!(hostname_for_fetch("https://foo.bar.onion/"), None);
    }

    #[test]
    fn hostname_rejects_non_http_schemes() {
        assert_eq!(hostname_for_fetch("ftp://example.com/"), None);
        assert_eq!(hostname_for_fetch("file:///etc/passwd"), None);
        assert_eq!(hostname_for_fetch("mailto:foo@example.com"), None);
        assert_eq!(hostname_for_fetch("javascript:alert(1)"), None);
    }

    #[test]
    fn hostname_rejects_unparseable() {
        assert_eq!(hostname_for_fetch(""), None);
        assert_eq!(hostname_for_fetch("   "), None);
        assert_eq!(hostname_for_fetch("not a url at all"), None);
    }

    #[test]
    fn svg_icons_cannot_read_external_or_embedded_images() {
        let root = std::env::temp_dir().join(format!("bw-svg-icon-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("private-image.png");
        image::RgbaImage::from_pixel(32, 32, image::Rgba([255, 0, 0, 255]))
            .save(&path)
            .unwrap();
        for href in [
            path.to_str().unwrap().to_owned(),
            format!("file://{}", path.display()),
            "https://never-fetch.invalid/image.png".into(),
            format!(
                "data:image/png;base64,{}",
                base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    std::fs::read(&path).unwrap()
                )
            ),
        ] {
            let svg = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="32" height="32"><image width="32" height="32" xlink:href="{href}"/></svg>"#
            );
            assert!(
                decode_and_mask(svg.as_bytes()).is_err(),
                "External-only SVGs must not render or enter the cache"
            );
        }
        assert!(
            decode_and_mask(
                br#"<svg xmlns="http://www.w3.org/2000/svg" width="1000000" height="1000000"/>"#
            )
            .is_err()
        );
        assert!(decode_and_mask(b"<html>not an icon</html>").is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    // ── apply_rounded_mask ─────────────────────────────────────────────────

    /// Build a 32×32 RGBA image with every pixel fully opaque white.
    fn opaque_white_32() -> ::image::RgbaImage {
        ::image::RgbaImage::from_pixel(32, 32, ::image::Rgba([255, 255, 255, 255]))
    }

    #[test]
    fn rounded_mask_zero_radius_is_no_op() {
        let mut img = opaque_white_32();
        apply_rounded_mask(&mut img, 0.0);
        // Every pixel still alpha = 255.
        assert!(img.pixels().all(|p| p.0[3] == 255));
    }

    #[test]
    fn rounded_mask_negative_radius_is_no_op() {
        let mut img = opaque_white_32();
        apply_rounded_mask(&mut img, -1.0);
        assert!(img.pixels().all(|p| p.0[3] == 255));
    }

    #[test]
    fn compressed_icons_cannot_exceed_decoder_dimension_limits() {
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(4097, 1)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        assert!(png.get_ref().len() < 2 * 1024 * 1024);
        assert!(decode_and_mask(png.get_ref()).is_err());
    }

    #[test]
    fn rounded_mask_clears_corner_pixel() {
        let mut img = opaque_white_32();
        apply_rounded_mask(&mut img, 4.0);
        // Pixel (0,0) sits well outside the inner-rectangle arc — coverage
        // collapses to 0 and the alpha is fully cleared.
        assert_eq!(img.get_pixel(0, 0).0[3], 0, "top-left corner not cleared");
        assert_eq!(img.get_pixel(31, 0).0[3], 0, "top-right corner not cleared");
        assert_eq!(
            img.get_pixel(0, 31).0[3],
            0,
            "bottom-left corner not cleared"
        );
        assert_eq!(
            img.get_pixel(31, 31).0[3],
            0,
            "bottom-right corner not cleared"
        );
    }

    #[test]
    fn rounded_mask_preserves_centre() {
        let mut img = opaque_white_32();
        apply_rounded_mask(&mut img, 4.0);
        assert_eq!(img.get_pixel(16, 16).0[3], 255, "centre not opaque");
    }

    #[test]
    fn rounded_mask_preserves_straight_edge_midpoint() {
        // Mid-edge pixels (16,0) etc. fall in the straight-edge region where
        // the clamped point equals the pixel itself, so coverage is full.
        let mut img = opaque_white_32();
        apply_rounded_mask(&mut img, 4.0);
        assert_eq!(
            img.get_pixel(16, 0).0[3],
            255,
            "top edge midpoint not opaque"
        );
        assert_eq!(
            img.get_pixel(0, 16).0[3],
            255,
            "left edge midpoint not opaque"
        );
        assert_eq!(
            img.get_pixel(31, 16).0[3],
            255,
            "right edge midpoint not opaque"
        );
        assert_eq!(
            img.get_pixel(16, 31).0[3],
            255,
            "bottom edge midpoint not opaque"
        );
    }

    #[test]
    fn rounded_mask_does_not_change_color_channels() {
        let mut img = opaque_white_32();
        apply_rounded_mask(&mut img, 4.0);
        // Every pixel keeps RGB = (255,255,255); only alpha is touched.
        assert!(
            img.pixels()
                .all(|p| p.0[0] == 255 && p.0[1] == 255 && p.0[2] == 255)
        );
    }
}
