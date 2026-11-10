//! Server profiles, certificate-scoped transport and pre-sync authentication.
pub mod certificate;
pub mod device;
pub mod login;
pub mod profile;
pub mod proxy;
pub mod storage;
pub mod transport;

#[cfg(test)]
mod tests;
