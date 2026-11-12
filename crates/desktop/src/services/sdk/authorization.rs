//! Single-use, item/revision/action scoped grants. No decrypted fields are stored here.
use bitwarden_vault::CipherId;
use chrono::{DateTime, Utc};
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

pub const REPROMPT_REQUIRED: &str = "Master-password authorization required";
pub const PERMISSION_DENIED: &str = "This action is not permitted for this item";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Read,
    Copy,
    Edit,
    Write,
    Delete,
    Restore,
    Export,
    Archive,
    OsUnlock,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Authorization {
    pub cipher: Option<CipherId>,
    pub revision: Option<DateTime<Utc>>,
    pub action: Action,
}
impl Authorization {
    pub fn export() -> Self {
        Self {
            cipher: None,
            revision: None,
            action: Action::Export,
        }
    }
}
struct Grant {
    target: Authorization,
    generation: u64,
    expires: Instant,
}
#[derive(Default)]
pub(super) struct Grants(Mutex<Vec<Grant>>);
impl Grants {
    #[cfg(test)]
    pub(super) fn authorize(&self, target: Authorization, generation: u64) {
        self.authorize_if(
            target,
            generation,
            &std::sync::atomic::AtomicBool::new(false),
        );
    }
    pub(super) fn authorize_if(
        &self,
        target: Authorization,
        generation: u64,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> bool {
        let mut grants = self.0.lock().expect("authorization poisoned");
        if cancelled.load(std::sync::atomic::Ordering::SeqCst) {
            return false;
        }
        grants.retain(|g| {
            g.expires > Instant::now() && g.generation == generation && g.target != target
        });
        grants.push(Grant {
            target,
            generation,
            expires: Instant::now() + Duration::from_secs(60),
        });
        true
    }
    pub(super) fn permits(&self, target: &Authorization, generation: u64, consume: bool) -> bool {
        let mut grants = self.0.lock().expect("authorization poisoned");
        grants.retain(|g| g.expires > Instant::now() && g.generation == generation);
        if let Some(index) = grants.iter().position(|g| &g.target == target) {
            if consume {
                grants.remove(index);
            }
            true
        } else {
            false
        }
    }
    pub(super) fn take(&self, target: &Authorization, generation: u64) -> Option<Instant> {
        let mut grants = self.0.lock().expect("authorization poisoned");
        grants.retain(|g| g.expires > Instant::now() && g.generation == generation);
        let index = grants.iter().position(|g| &g.target == target)?;
        Some(grants.remove(index).expires)
    }
    pub(super) fn clear(&self) {
        self.0.lock().expect("authorization poisoned").clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grants_are_single_use_and_match_generation_revision_and_action() {
        let grants = Grants::default();
        let target = Authorization {
            cipher: Some(CipherId::new_v4()),
            revision: Some(Utc::now()),
            action: Action::Copy,
        };
        grants.authorize(target.clone(), 5);
        let mut wrong = target.clone();
        wrong.action = Action::Write;
        assert!(!grants.permits(&wrong, 5, true));
        wrong = target.clone();
        wrong.revision = Some(Utc::now() + chrono::Duration::seconds(1));
        assert!(!grants.permits(&wrong, 5, true));
        assert!(grants.permits(&target, 5, true));
        assert!(!grants.permits(&target, 5, true));
        grants.authorize(target.clone(), 5);
        assert!(!grants.permits(&target, 6, true));
        grants.authorize(target.clone(), 6);
        grants.0.lock().unwrap()[0].expires = Instant::now() - Duration::from_secs(1);
        assert!(!grants.permits(&target, 6, true));
        grants.authorize(target.clone(), 6);
        grants.clear();
        assert!(!grants.permits(&target, 6, true));
    }
}
