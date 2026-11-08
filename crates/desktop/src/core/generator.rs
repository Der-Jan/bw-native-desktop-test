use super::*;
use crate::services::sdk::ClientExt;
use bitwarden_generators::{
    PassphraseGeneratorRequest, PasswordGeneratorRequest, UsernameGeneratorRequest,
};
pub enum Generate {
    Password(PasswordGeneratorRequest),
    Passphrase(PassphraseGeneratorRequest),
    Username(UsernameGeneratorRequest),
}
impl Core {
    pub fn generate(&mut self, request: Generate) {
        let Some(scope) = self.scope() else {
            return;
        };
        let Some(client) = self.manager.client_for(&scope.uid) else {
            return;
        };
        let uid = scope.uid;
        self.generator.sequence += 1;
        let sequence = self.generator.sequence;
        self.generator.busy = true;
        self.spawn_scoped(
            scope,
            async move {
                match request {
                    Generate::Password(r) => client.generate_password(r).await,
                    Generate::Passphrase(r) => client.generate_passphrase(r).await,
                    Generate::Username(r) => client.generate_username(r).await,
                }
            },
            move |core, result| {
                if core.generator.sequence != sequence {
                    return;
                }
                core.generator.busy = false;
                match result {
                    Ok(value) => {
                        core.manager.push_history(uid, value.clone());
                        core.generator.output = Zeroizing::new(value);
                    }
                    Err(e) => core.notify(e, true),
                }
            },
        );
    }
    pub fn generate_ssh_key(&mut self) {
        let Some(scope) = self.scope() else {
            return;
        };
        let source = self.editor.as_ref().map(|e| e.id);
        let epoch = self.clear_epoch;
        self.spawn_scoped(
            scope,
            tokio::task::spawn_blocking(|| {
                bitwarden_ssh::generator::generate_sshkey(
                    bitwarden_ssh::generator::KeyAlgorithm::Ed25519,
                )
            }),
            move |core, result| {
                if core.clear_epoch != epoch || core.editor.as_ref().map(|e| e.id) != source {
                    return;
                }
                match result {
                    Ok(Ok(key)) => {
                        if let Some(editor) = core.editor.as_mut() {
                            editor.data["sshKey"] = serde_json::json!({"privateKey":key.private_key,"publicKey":key.public_key,"fingerprint":key.fingerprint});
                        }
                    }
                    _ => core.notify("SSH key generation failed", true),
                }
            },
        );
    }
}
