use super::profile::{ProfileStore, ServerProfile};
use reqwest_account::{Client, Response, Url};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

/// Wire-protocol compatibility, independent of this app's 0.x package version
/// and the SDK crate's version. Vaultwarden suppresses SSH ciphers below this
/// calendar-version baseline, or when the header is absent.
pub const BITWARDEN_PROTOCOL_VERSION: &str = "2024.12.0";

#[derive(Clone)]
pub struct Transport {
    pub profile: ServerProfile,
    pub(super) client: Client,
    epoch: Arc<AtomicU64>,
}
impl Transport {
    /// Call on a blocking worker: loading a private key may require OS credential-store unlock.
    pub fn new(store: &ProfileStore, profile: ServerProfile) -> Result<Self, String> {
        Self::build(store, profile, false)
    }
    /// RFC 6455 upgrades use HTTP/1.1 even when the HTTPS server also offers h2.
    pub(crate) fn notifications(
        store: &ProfileStore,
        profile: ServerProfile,
    ) -> Result<Self, String> {
        Self::build(store, profile, true)
    }
    fn build(
        store: &ProfileStore,
        profile: ServerProfile,
        notifications: bool,
    ) -> Result<Self, String> {
        let epoch = store.epoch(profile.id)?;
        let mut headers = reqwest_account::header::HeaderMap::new();
        headers.insert(
            "bitwarden-client-version",
            reqwest_account::header::HeaderValue::from_static(BITWARDEN_PROTOCOL_VERSION),
        );
        let mut builder = Client::builder()
            .default_headers(headers)
            .redirect(reqwest_account::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .user_agent("Bitwarden_Native_Desktop/0.1.0");
        if notifications {
            builder = builder.http1_only();
        }
        if let Some(info) = &profile.certificate {
            info.check_time()?;
            let pem = store.secrets.get(info.secret)?;
            builder = builder.identity(
                reqwest_account::Identity::from_pem(&pem)
                    .map_err(|_| "Stored certificate identity is invalid")?,
            );
        }
        if let Some(pem) = &profile.server_ca_pem {
            for cert in openssl::x509::X509::stack_from_pem(pem.as_bytes())
                .map_err(|_| "Invalid saved server CA")?
            {
                builder = builder.add_root_certificate(
                    reqwest_account::Certificate::from_der(
                        &cert.to_der().map_err(|_| "Invalid server CA")?,
                    )
                    .map_err(|_| "Invalid server CA")?,
                );
            }
        }
        let client = builder
            .build()
            .map_err(|_| "Could not construct secure account transport")?;
        let t = Self {
            profile,
            client,
            epoch,
        };
        t.check(&Url::parse(&t.profile.base).map_err(|_| "Invalid saved endpoint")?)?;
        Ok(t)
    }
    pub fn check(&self, url: &Url) -> Result<(), String> {
        if self.epoch.load(Ordering::SeqCst) != self.profile.revision {
            return Err(
                "Server certificate or endpoint settings changed; retry with the current settings"
                    .into(),
            );
        }
        self.profile.check(url)
    }
    pub async fn execute(&self, request: reqwest_account::Request) -> Result<Response, String> {
        self.check(request.url())?;
        let response = self
            .client
            .execute(request)
            .await
            .map_err(|error| connection_failure(&error))?;
        self.check(response.url())?;
        if response.status().is_redirection() {
            return Err(
                "Server redirects are blocked; configure the final trusted endpoint explicitly"
                    .into(),
            );
        }
        Ok(response)
    }
    pub fn rebind(&self, store: &ProfileStore, profile: ServerProfile) -> Result<Self, String> {
        let epoch = store.epoch(profile.id)?;
        let new = Self {
            profile,
            client: self.client.clone(),
            epoch,
        };
        new.check(&new.profile.default_api()?)?;
        Ok(new)
    }
    pub fn request(
        &self,
        method: reqwest_account::Method,
        url: Url,
    ) -> Result<reqwest_account::RequestBuilder, String> {
        self.check(&url)?;
        Ok(self.client.request(method, url))
    }
    pub fn configure_sdk(
        &self,
        builder: bitwarden_core::ClientBuilder,
    ) -> bitwarden_core::ClientBuilder {
        builder.with_account_transport(self.client.clone(), self.policy())
    }
    pub fn policy(&self) -> Arc<dyn reqwest_middleware::Middleware> {
        Arc::new(self.clone())
    }
}
#[async_trait::async_trait]
impl reqwest_middleware::Middleware for Transport {
    async fn handle(
        &self,
        req: reqwest_account::Request,
        ext: &mut http::Extensions,
        next: reqwest_middleware::Next<'_>,
    ) -> Result<Response, reqwest_middleware::Error> {
        self.check(req.url())
            .map_err(|e| reqwest_middleware::Error::Middleware(std::io::Error::other(e).into()))?;
        let response = next.run(req, ext).await.map_err(|error| {
            reqwest_middleware::Error::Middleware(
                std::io::Error::other(connection_failure(&error)).into(),
            )
        })?;
        self.check(response.url())
            .map_err(|e| reqwest_middleware::Error::Middleware(std::io::Error::other(e).into()))?;
        if response.status().is_redirection() {
            return Err(reqwest_middleware::Error::Middleware(
                std::io::Error::other("Account redirect rejected").into(),
            ));
        }
        Ok(response)
    }
}
pub async fn json_body(response: Response) -> Result<serde_json::Value, String> {
    json_body_limited(response, 2 * 1024 * 1024).await
}
pub async fn json_body_limited(
    response: Response,
    limit: usize,
) -> Result<serde_json::Value, String> {
    if response
        .content_length()
        .is_some_and(|len| len > limit as u64)
    {
        return Err("The server response is too large".into());
    }
    let mut response = response;
    let mut body = zeroize::Zeroizing::new(Vec::new());
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Could not read server response")?
    {
        if body.len() + chunk.len() > limit {
            return Err("The server response is too large".into());
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|_| "The server returned an invalid response".into())
}

/// Classify typed TLS causes, never expose raw error/URL/header contents in the UI.
/// A peer alert describes the peer's rejection of our identity, rather than a
/// local validation failure of the server certificate. Keep that distinction.
fn connection_failure(error: &(dyn std::error::Error + 'static)) -> String {
    let mut cause = Some(error);
    while let Some(error) = cause {
        if let Some(tls) = error.downcast_ref::<rustls::Error>()
            && let Some(message) = tls_failure(tls)
        {
            return message.into();
        }
        if let Some(request) = error.downcast_ref::<reqwest_account::Error>()
            && request.is_timeout()
        {
            return "Die Verbindung zum Server hat zu lange gedauert. Bitte die Erreichbarkeit prüfen.".into();
        }
        // TLS backends can be wrapped inside io::Error; inspect the inner error
        // itself as well as source chains (whose behavior differs by wrapper).
        if let Some(io) = error.downcast_ref::<std::io::Error>() {
            if let Some(inner) = io.get_ref() {
                cause = Some(inner);
                continue;
            }
            if io.kind() == std::io::ErrorKind::ConnectionRefused {
                return "Der Server lehnt die Verbindung ab. Bitte Serveradresse, Port und Erreichbarkeit prüfen.".into();
            }
        }
        cause = error.source();
    }
    "Sichere Verbindung fehlgeschlagen. Die Ursache konnte nicht eindeutig ermittelt werden; bitte Serververtrauen, mTLS-Clientzertifikat und Erreichbarkeit prüfen.".into()
}
fn tls_failure(error: &rustls::Error) -> Option<&'static str> {
    use rustls::{AlertDescription as Alert, CertificateError as Cert, Error};
    Some(match error {
        Error::InvalidCertificate(Cert::Expired | Cert::ExpiredContext { .. }) => {
            "Das TLS-Zertifikat des Servers oder seiner Zertifikatskette ist abgelaufen. Bitte den Serverbetreiber um Erneuerung bitten und die Systemzeit prüfen."
        }
        Error::InvalidCertificate(Cert::NotValidYet | Cert::NotValidYetContext { .. }) => {
            "Das TLS-Zertifikat des Servers oder seiner Zertifikatskette ist noch nicht gültig. Bitte Systemzeit und Zertifikatsgültigkeit prüfen."
        }
        Error::InvalidCertificate(Cert::UnknownIssuer) => {
            "Die Zertifizierungsstelle des Servers ist nicht vertrauenswürdig oder die Zertifikatskette ist unvollständig. Bei einer privaten CA deren PEM-Zertifikat in den Server-/mTLS-Einstellungen importieren."
        }
        Error::InvalidCertificate(Cert::NotValidForName | Cert::NotValidForNameContext { .. }) => {
            "Das TLS-Zertifikat passt nicht zum Hostnamen der Serveradresse. Bitte URL und Serverzertifikat prüfen."
        }
        Error::InvalidCertificate(Cert::Revoked) => {
            "Das TLS-Zertifikat des Servers wurde widerrufen. Bitte den Serverbetreiber kontaktieren."
        }
        Error::AlertReceived(Alert::CertificateExpired) => {
            "Der Server meldet ein abgelaufenes oder zeitlich ungültiges Zertifikat. Bitte das mTLS-Clientzertifikat und seine Zertifikatskette in den Server-/mTLS-Einstellungen prüfen und gegebenenfalls ersetzen."
        }
        Error::AlertReceived(Alert::CertificateRequired) => {
            "Der Server verlangt ein mTLS-Clientzertifikat. Bitte in den Server-/mTLS-Einstellungen eine passende PKCS#12-Datei importieren."
        }
        Error::AlertReceived(
            Alert::UnknownCA
            | Alert::BadCertificate
            | Alert::CertificateUnknown
            | Alert::CertificateRevoked
            | Alert::UnsupportedCertificate,
        ) => {
            "Der Server hat das mTLS-Clientzertifikat abgelehnt. Bitte Zertifikat, Zertifikatskette und die Freigabe beim Serverbetreiber prüfen."
        }
        _ => return None,
    })
}
#[cfg(test)]
mod error_tests {
    use super::*;
    #[test]
    fn certificate_errors_distinguish_server_validation_from_peer_rejection() {
        use rustls::{AlertDescription as Alert, CertificateError as Cert, Error};
        assert!(
            connection_failure(&std::io::Error::other(Error::InvalidCertificate(
                Cert::Expired
            )))
            .contains("Servers oder seiner Zertifikatskette ist abgelaufen")
        );
        assert!(
            connection_failure(&Error::InvalidCertificate(Cert::NotValidYet))
                .contains("noch nicht gültig")
        );
        assert!(
            connection_failure(&Error::AlertReceived(Alert::CertificateExpired))
                .contains("mTLS-Clientzertifikat")
        );
        assert!(
            connection_failure(&Error::AlertReceived(Alert::CertificateRequired))
                .contains("verlangt ein mTLS")
        );
        assert!(
            connection_failure(&Error::InvalidCertificate(Cert::UnknownIssuer))
                .contains("Zertifizierungsstelle")
        );
        let message = connection_failure(&std::io::Error::other(
            "secret https://user:password@example.invalid/?token=secret certificate expired",
        ));
        assert!(
            !message.contains("abgelaufen"),
            "Never infer expiry from arbitrary error strings"
        );
        assert!(!message.contains("password"));
        assert!(!message.contains("token=secret"));
    }
}
