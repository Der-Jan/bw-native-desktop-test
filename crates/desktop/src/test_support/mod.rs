//! Framework-neutral test fixtures for services and application lifecycle.
pub mod fixtures;
pub fn init() {
    crate::services::i18n::init();
}
