//! Localization via [`i18n-embed`] + Fluent.
//!
//! Translation assets live at the workspace root under
//! `assets/i18n/{lang}/{crate}.ftl` alongside the other binary resources
//! (fonts, SVGs, icons). They are embedded into the binary at compile time
//! via [`rust_embed`] so the packaged binary carries all languages without
//! extra files on disk.
//!
//! ## Adding a string
//!
//! 1. Add a key + English value to `assets/i18n/en/bitwarden_desktop_next.ftl`.
//! 2. Use [`fl!`] at the call site: `fl!("my-key")` or `fl!("my-key", name = value)`.
//!
//! The [`fl!`] macro (defined at the crate root) wraps [`i18n_embed_fl::fl!`] so
//! the loader argument is implicit. Unknown keys or wrong argument names are a
//! compile error thanks to `i18n-embed-fl`'s static check against the `.ftl`
//! files.
//!
//! ## Runtime locale
//!
//! [`init`] is called once from `main` before the native window loop starts. It reads
//! the OS locale via `DesktopLanguageRequester` and selects the best match from
//! the embedded languages, falling back to English when none match.
//!
//! To change language at runtime, call [`set_language`] from a message handler.
//! egui will re-render on the next frame and every `fl!()` call will return the
//! new translation — no restart required.

use std::{collections::HashMap, sync::LazyLock};

use i18n_embed::{
    DesktopLanguageRequester, LanguageLoader,
    fluent::{FluentLanguageLoader, fluent_language_loader},
    unic_langid::LanguageIdentifier,
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../assets/i18n"]
pub(crate) struct Localizations;

#[doc(hidden)]
pub static LANGUAGE_LOADER: LazyLock<FluentLanguageLoader> = LazyLock::new(|| {
    let loader: FluentLanguageLoader = fluent_language_loader!();
    loader
        .load_fallback_language(&Localizations)
        .expect("fallback language (en) must load");
    loader
});

/// Select the OS-preferred language, falling back to the loader's fallback
/// when no match exists. Call once at app startup.
pub fn init() {
    let requested = DesktopLanguageRequester::requested_languages();
    if let Err(e) = i18n_embed::select(&*LANGUAGE_LOADER, &Localizations, &requested) {
        tracing::warn!(%e, "i18n language selection failed; falling back to default");
    }
}

/// Switch to a specific language. egui re-renders on the next frame and every
/// `fl!()` call picks up the new translation. Falls back to English (with a
/// warning) if no matching `.ftl` file exists under `assets/i18n/`.
pub fn set_language(lang: LanguageIdentifier) {
    if let Err(e) = i18n_embed::select(
        &*LANGUAGE_LOADER,
        &Localizations,
        std::slice::from_ref(&lang),
    ) {
        tracing::warn!(%e, %lang, "set_language failed; current language unchanged");
    }
}

/// Languages with at least one `.ftl` file under `assets/i18n/`. Computed
/// from the embedded assets, so a new `assets/i18n/<tag>/` directory surfaces
/// in the UI without code changes.
pub fn available_languages() -> Vec<LanguageIdentifier> {
    LANGUAGE_LOADER
        .available_languages(&Localizations)
        .unwrap_or_default()
}

/// Conventional key each locale defines to declare its own native name
/// (endonym), e.g. `language-name-self = Español` in `es/...ftl`. Read at
/// startup and cached in [`ENDONYMS`]; never resolved through the active
/// loader so the value always renders in its own script.
const ENDONYM_KEY: &str = "language-name-self";

/// Map of language tag → native name, populated once on first access by
/// loading each available locale into a throwaway [`FluentLanguageLoader`]
/// and reading [`ENDONYM_KEY`] out of it. Adding a new locale is purely a
/// filesystem operation: drop a new
/// `assets/i18n/<tag>/bitwarden_desktop_next.ftl` containing a
/// `language-name-self = <Endonym>` line and it surfaces in the picker.
static ENDONYMS: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    let mut out = HashMap::new();
    for tag in available_languages() {
        let probe: FluentLanguageLoader = fluent_language_loader!();
        if let Err(e) = probe.load_languages(&Localizations, std::slice::from_ref(&tag)) {
            tracing::warn!(%tag, %e, "failed to load locale for endonym lookup");
            continue;
        }
        if probe.has(ENDONYM_KEY) {
            out.insert(tag.to_string(), probe.get(ENDONYM_KEY));
        }
    }
    out
});

/// Human-readable label for a language tag, shown in the settings language
/// picker. Falls back to the raw tag when the locale's `.ftl` doesn't
/// declare `language-name-self`.
pub fn language_label(tag: &str) -> String {
    ENDONYMS
        .get(tag)
        .cloned()
        .unwrap_or_else(|| tag.to_string())
}
