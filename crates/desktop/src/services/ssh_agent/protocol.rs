//! Bounded SSH-agent framing and public-key authentication/session binding only.
//! The agent does not offer generic signing, key import, removal or remote control.
use crate::services::sdk::ssh_agent::{signing_algorithm, verify_binding};
pub const MAX_PACKET: usize = 65_536;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Invalid;
impl std::fmt::Display for Invalid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Invalid SSH request")
    }
}
impl std::error::Error for Invalid {}
pub type Result<T> = std::result::Result<T, Invalid>;

pub struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    pub fn byte(&mut self) -> Result<u8> {
        let b = *self.bytes.get(self.offset).ok_or(Invalid)?;
        self.offset += 1;
        Ok(b)
    }
    pub fn u32(&mut self) -> Result<u32> {
        let end = self.offset.checked_add(4).ok_or(Invalid)?;
        let value = u32::from_be_bytes(
            self.bytes
                .get(self.offset..end)
                .ok_or(Invalid)?
                .try_into()
                .map_err(|_| Invalid)?,
        );
        self.offset = end;
        Ok(value)
    }
    pub fn string(&mut self) -> Result<&'a [u8]> {
        let count = self.u32()? as usize;
        let end = self.offset.checked_add(count).ok_or(Invalid)?;
        let value = self.bytes.get(self.offset..end).ok_or(Invalid)?;
        self.offset = end;
        Ok(value)
    }
    pub fn text(&mut self) -> Result<&'a str> {
        std::str::from_utf8(self.string()?).map_err(|_| Invalid)
    }
    pub fn end(self) -> Result<()> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(Invalid)
        }
    }
}
pub fn push_string(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    output.extend_from_slice(bytes);
}
#[derive(Clone, PartialEq, Eq)]
pub struct Binding {
    pub host: Vec<u8>,
    pub session: Vec<u8>,
    pub forwarded: bool,
    pub fingerprint: String,
}
#[derive(Default)]
pub struct Bindings {
    pub values: Vec<Binding>,
    pub failed: bool,
}
impl Bindings {
    pub fn extension(&mut self, packet: &[u8]) -> Vec<u8> {
        let mut reader = Reader::new(packet);
        if reader.byte() != Ok(27) {
            return vec![5];
        }
        let Ok(extension) = reader.text() else {
            self.failed = true;
            return vec![28];
        };
        if extension != "session-bind@openssh.com" {
            return vec![5];
        }
        match self.bind(reader) {
            Ok(()) => vec![6],
            Err(_) => {
                self.failed = true;
                vec![28]
            }
        }
    }
    fn bind(&mut self, mut reader: Reader<'_>) -> Result<()> {
        if self.failed
            || self.values.len() >= 16
            || self.values.last().is_some_and(|b| !b.forwarded)
        {
            return Err(Invalid);
        }
        let host = reader.string()?;
        let session = reader.string()?;
        let signature = reader.string()?;
        let forwarded = match reader.byte()? {
            0 => false,
            1 => true,
            _ => return Err(Invalid),
        };
        reader.end()?;
        let fingerprint = verify_binding(host, session, signature).map_err(|_| Invalid)?;
        let binding = Binding {
            host: host.into(),
            session: session.into(),
            forwarded,
            fingerprint,
        };
        if let Some(previous) = self.values.iter().find(|b| b.session == session) {
            if *previous == binding {
                return Ok(());
            } else {
                return Err(Invalid);
            }
        }
        self.values.push(binding);
        Ok(())
    }
}
#[derive(Clone)]
pub struct Authentication {
    pub username: String,
    pub hosts: Vec<[u8; 32]>,
    pub fingerprints: Vec<String>,
    /// Both a verified binding chain and an authentication signature covering
    /// its terminal host key are required before a grant may be reused.
    pub rememberable: bool,
}
pub struct SignRequest {
    pub public_key: Vec<u8>,
    pub data: Vec<u8>,
    pub flags: u32,
    pub authentication: Authentication,
}
pub fn parse_sign(packet: &[u8], bindings: &Bindings) -> Result<SignRequest> {
    if packet.len() > MAX_PACKET || bindings.failed {
        return Err(Invalid);
    }
    let mut reader = Reader::new(packet);
    if reader.byte()? != 13 {
        return Err(Invalid);
    }
    let public_key = reader.string()?;
    let data = reader.string()?;
    let flags = reader.u32()?;
    reader.end()?;
    let algorithm = signing_algorithm(public_key, flags).map_err(|_| Invalid)?;
    let mut auth = Reader::new(data);
    let session = auth.string()?;
    if !(16..=128).contains(&session.len()) || auth.byte()? != 50 {
        return Err(Invalid);
    }
    let user = auth.text()?;
    if user.is_empty()
        || user.len() > 256
        || user
            .chars()
            .any(|c| c.is_control() || matches!(c,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}'))
    {
        return Err(Invalid);
    }
    if auth.text()? != "ssh-connection" {
        return Err(Invalid);
    }
    let method = auth.text()?;
    if !matches!(method, "publickey" | "publickey-hostbound-v00@openssh.com")
        || auth.byte()? != 1
        || auth.text()? != algorithm
        || auth.string()? != public_key
    {
        return Err(Invalid);
    }
    let host = if method == "publickey-hostbound-v00@openssh.com" {
        Some(auth.string()?)
    } else {
        None
    };
    auth.end()?;
    if let Some(last) = bindings.values.last()
        && (last.forwarded || last.session != session || host.is_some_and(|h| h != last.host))
    {
        return Err(Invalid);
    }
    let authentication = Authentication {
        rememberable: host.is_some() && !bindings.values.is_empty(),
        username: user.into(),
        hosts: bindings
            .values
            .iter()
            .map(|b| openssl::sha::sha256(&b.host))
            .collect(),
        fingerprints: bindings
            .values
            .iter()
            .map(|b| b.fingerprint.clone())
            .collect(),
    };
    Ok(SignRequest {
        public_key: public_key.into(),
        data: data.into(),
        flags,
        authentication,
    })
}
