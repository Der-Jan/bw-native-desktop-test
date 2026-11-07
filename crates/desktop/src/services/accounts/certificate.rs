use super::{
    profile::{CertificateInfo, ProfileStore, ServerProfile},
    storage::read_bounded,
};
use openssl::{asn1::Asn1Time, hash::MessageDigest, x509::X509};
use std::path::Path;
use uuid::Uuid;
use zeroize::Zeroizing;

/// Parse all private-key bags before applying an import policy. Never guess an identity.
pub fn import(
    store: &ProfileStore,
    profile: ServerProfile,
    path: &Path,
    password: Zeroizing<String>,
) -> Result<ServerProfile, String> {
    let der = Zeroizing::new(read_bounded(path, 1024 * 1024)?);
    let archive = p12_keystore::Pkcs12Archive::from_pkcs12(&der, &password)
        .map_err(|_| "Could not open PKCS#12 bundle; check its password and format")?;
    if archive.keys.len() != 1 {
        return Err("The bundle must contain exactly one private-key identity".into());
    }
    // OpenSSL validates key material; certificates are linked by issuer, not bag order.
    let parsed = openssl::pkcs12::Pkcs12::from_der(&der)
        .and_then(|p| p.parse2(&password))
        .map_err(|_| "Could not read the certificate and private key")?;
    let key = parsed
        .pkey
        .ok_or("The certificate bundle has no private key")?;
    let leaf = parsed
        .cert
        .ok_or("The certificate bundle has no leaf certificate")?;
    let public = leaf
        .public_key()
        .map_err(|_| "Invalid client certificate")?;
    if !key.public_eq(&public) {
        return Err("The private key does not match the client certificate".into());
    }
    let epoch = Asn1Time::from_unix(0).map_err(|_| "Certificate clock unavailable")?;
    let timestamp = |value: &openssl::asn1::Asn1TimeRef| -> Result<i64, String> {
        let diff = epoch
            .diff(value)
            .map_err(|_| "Invalid certificate validity period")?;
        Ok(i64::from(diff.days) * 86400 + i64::from(diff.secs))
    };
    let fingerprint = leaf
        .digest(MessageDigest::sha256())
        .map_err(|_| "Could not identify certificate")?
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let subject = leaf
        .subject_name()
        .entries()
        .filter_map(|e| e.data().to_string().ok())
        .collect::<Vec<_>>()
        .join(", ");
    let mut info = CertificateInfo {
        fingerprint,
        subject,
        not_before: timestamp(leaf.not_before())?,
        not_after: timestamp(leaf.not_after())?,
        secret: Uuid::nil(),
    };
    info.check_time()?;
    if leaf.extended_key_usage().is_some_and(|u| {
        !u.intersects(
            openssl::x509::X509ExtendedKeyUsage::SSL_CLIENT
                | openssl::x509::X509ExtendedKeyUsage::ANYEKU,
        )
    }) {
        return Err("The certificate is not permitted for TLS client authentication".into());
    }
    if leaf
        .key_usage()
        .is_some_and(|u| !u.contains(openssl::x509::X509KeyUsage::DIGITAL_SIGNATURE))
    {
        return Err("The client certificate cannot sign TLS authentication requests".into());
    }
    let mut remaining: Vec<X509> = archive
        .certs
        .iter()
        .map(|b| {
            X509::from_der(b.cert.as_der()).map_err(|_| "Invalid certificate in bundle".to_owned())
        })
        .collect::<Result<_, _>>()?;
    let leaf_der = leaf.to_der().map_err(|_| "Invalid client certificate")?;
    remaining.retain(|c| c.to_der().ok().as_ref() != Some(&leaf_der));
    let mut chain = vec![leaf];
    while !remaining.is_empty() {
        let last = chain.last().unwrap();
        let issuer = last.issuer_name().to_der().map_err(|_| "Invalid issuer")?;
        if issuer
            == last
                .subject_name()
                .to_der()
                .map_err(|_| "Invalid certificate")?
        {
            break;
        }
        let matches: Vec<_> = remaining
            .iter()
            .enumerate()
            .filter(|(_, c)| c.subject_name().to_der().ok().as_ref() == Some(&issuer))
            .map(|(i, _)| i)
            .collect();
        if matches.len() != 1 {
            return Err(
                "The certificate chain is ambiguous or contains unrelated certificates".into(),
            );
        }
        let parent = remaining.remove(matches[0]);
        let now = Asn1Time::days_from_now(0).map_err(|_| "Certificate clock unavailable")?;
        if parent.not_before() > now || parent.not_after() <= now {
            return Err(
                "The client certificate chain contains an expired or future certificate".into(),
            );
        }
        if parent
            .key_usage()
            .is_some_and(|u| !u.contains(openssl::x509::X509KeyUsage::KEY_CERT_SIGN))
        {
            return Err("The issuer certificate is not permitted to sign certificates".into());
        }
        let parent_key = parent.public_key().map_err(|_| "Invalid issuer key")?;
        if !last
            .verify(&parent_key)
            .map_err(|_| "Invalid certificate chain")?
        {
            return Err("The certificate chain signature is invalid".into());
        }
        chain.push(parent);
    }
    if !remaining.is_empty() {
        return Err("The bundle contains unrelated certificates".into());
    }
    let mut pem = Zeroizing::new(
        key.private_key_to_pem_pkcs8()
            .map_err(|_| "Unsupported private key")?,
    );
    for cert in chain {
        pem.extend(cert.to_pem().map_err(|_| "Invalid certificate")?);
    }
    // TLS identity construction also rejects unsupported key algorithms before persistence.
    reqwest_account::Identity::from_pem(&pem)
        .map_err(|_| "This client key is unsupported by the TLS backend")?;
    let secret = store.secrets.put(&pem)?;
    info.secret = secret;
    let expected = store.get(profile.id).map(|p| p.revision);
    if expected != Some(profile.revision) && expected.is_some() {
        let _ = store.secrets.delete(secret);
        return Err("The target server settings changed during import".into());
    }
    let mut updated = profile;
    updated.certificate = Some(info);
    match store.save(updated, expected) {
        Ok(p) => Ok(p),
        Err(e) => {
            let _ = store.secrets.delete(secret);
            Err(e)
        }
    }
}

pub fn validate_ca(pem: &[u8]) -> Result<String, String> {
    let certs = X509::stack_from_pem(pem).map_err(|_| "Invalid PEM server CA bundle")?;
    if certs.is_empty() {
        return Err("The server CA bundle contains no certificates".into());
    }
    // Importing this separate, explicit server trust setting never uses PKCS#12 CAs implicitly.
    for cert in &certs {
        let _ = cert
            .public_key()
            .map_err(|_| "Invalid server CA certificate")?;
    }
    String::from_utf8(pem.to_vec()).map_err(|_| "Invalid PEM server CA bundle".into())
}
