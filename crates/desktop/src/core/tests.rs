use super::*;
use crate::{services::sdk::test_support::unlocked_manager, test_support::fixtures::*};
use bitwarden_vault::{CipherId, CipherRepromptType, CipherType};
fn core() -> Core {
    Core::test(unlocked_manager(UserId::new_v4()))
}
fn install(core: &mut Core, protected: bool) -> CipherId {
    let mut item = make_cipher(CipherSpec::default());
    let id = CipherId::new_v4();
    let item_mut = Arc::get_mut(&mut item).unwrap();
    item_mut.id = Some(id);
    if protected {
        item_mut.reprompt = CipherRepromptType::Password;
    }
    let cache = VaultCache {
        all: vec![item.clone()],
        visible: vec![item.clone()],
        ..Default::default()
    };
    core.caches.insert(core.active.unwrap(), cache);
    id
}
#[tokio::test]
async fn session_scope_cannot_survive_lock_or_presentation_reset() {
    let mut core = core();
    let scope = core.scope().unwrap();
    assert!(scope.current(&core));
    core.clear_presentation();
    assert!(!scope.current(&core));
    let scope = core.scope().unwrap();
    core.lock_user(scope.uid);
    assert!(!scope.current(&core));
    assert!(!core.unlocked());
    assert!(core.caches.is_empty());
}
#[tokio::test]
async fn passive_polling_preserves_protected_prompt_but_cancel_revokes_it() {
    let mut core = core();
    let id = install(&mut core, true);
    core.request(Request::Read(id));
    let prompt = core.prompt.as_ref().unwrap().id;
    let cancelled = core.prompt.as_ref().unwrap().cancelled.clone();
    core.poll();
    assert_eq!(core.prompt.as_ref().unwrap().id, prompt);
    core.close_details();
    assert_eq!(core.prompt.as_ref().unwrap().id, prompt);
    core.cancel_prompt();
    assert!(cancelled.load(std::sync::atomic::Ordering::SeqCst));
    assert!(core.prompt.is_none());
}
#[tokio::test]
async fn filter_change_cancels_secret_prompt() {
    let mut core = core();
    let id = install(&mut core, true);
    core.request(Request::Read(id));
    core.set_filter(services::vault_navigation::VaultFilter::default());
    assert!(core.prompt.is_none());
    assert!(core.detail.is_none());
}
#[tokio::test]
async fn lock_clears_details_editor_generator_and_pending_prompt() {
    let mut core = core();
    let id = install(&mut core, true);
    core.request(Request::Read(id));
    core.detail = Some(make_cipher_view());
    core.editor = Some(Editor::from_cipher(&make_cipher_view()));
    core.generator.output = Zeroizing::new("test-generated-secret".into());
    core.query = "synthetic private search".into();
    core.notify("synthetic private notice", false);
    core.lock_all();
    assert!(core.detail.is_none() && core.editor.is_none() && core.prompt.is_none());
    assert!(core.generator.output.is_empty());
    assert!(core.query.is_empty() && core.notice.is_none());
    assert!(core.caches.is_empty());
}
#[tokio::test]
async fn all_sdk_item_drafts_roundtrip() {
    let mut core = core();
    for ty in [
        CipherType::Login,
        CipherType::SecureNote,
        CipherType::Card,
        CipherType::Identity,
        CipherType::SshKey,
        CipherType::BankAccount,
        CipherType::DriversLicense,
        CipherType::Passport,
    ] {
        core.new_item(ty);
        let editor = core
            .editor
            .as_ref()
            .unwrap_or_else(|| panic!("draft missing for {ty:?}"));
        assert_eq!(editor.cipher().unwrap().r#type, ty);
    }
}
#[tokio::test]
async fn late_worker_completion_cannot_publish_after_lock() {
    let mut core = core();
    let scope = core.scope().unwrap();
    core.spawn_scoped(scope, async { "stale plaintext" }, |core, value| {
        core.notify(value, false)
    });
    core.lock_all();
    tokio::time::sleep(Duration::from_millis(20)).await;
    core.poll();
    assert!(core.notice.is_none());
}
#[tokio::test]
async fn wrong_password_keeps_the_same_prompt_for_retry() {
    let mut core = core();
    let id = install(&mut core, true);
    core.request(Request::Read(id));
    let prompt = core.prompt.as_ref().unwrap().id;
    core.confirm_password(Zeroizing::new("wrong".into()));
    for _ in 0..200 {
        tokio::time::sleep(Duration::from_millis(10)).await;
        core.poll();
        if core.prompt.as_ref().is_some_and(|p| !p.busy) {
            break;
        }
    }
    let p = core.prompt.as_ref().unwrap();
    assert_eq!(p.id, prompt);
    assert!(!p.busy && p.error);
    assert!(core.detail.is_none());
}
#[tokio::test]
async fn widget_copy_after_editor_close_still_requires_copy_authority() {
    let mut core = core();
    let id = install(&mut core, true);
    core.request(Request::CopyText(
        id,
        Zeroizing::new("synthetic edited secret".into()),
    ));
    let prompt = core
        .prompt
        .as_ref()
        .expect("protected widget copy must prompt");
    assert_eq!(prompt.target.action, services::sdk::Action::Copy);
    assert_eq!(prompt.target.cipher, Some(id));
}

#[tokio::test]
async fn synchronization_does_not_discard_an_open_editor_or_allow_a_new_edit_mid_sync() {
    let mut core = core();
    let id = install(&mut core, true);
    core.new_item(CipherType::Login);
    let editor = core.editor.as_ref().unwrap().id;
    core.synchronize();
    assert_eq!(core.editor.as_ref().unwrap().id, editor);
    assert!(core.sync.is_empty());
    core.editor = None;
    core.sync.insert(
        core.active.unwrap(),
        SyncState {
            request: uuid::Uuid::new_v4(),
            busy: true,
            error: None,
        },
    );
    core.request(Request::Edit(id));
    assert!(
        core.prompt.is_none(),
        "An edit authorization flow must not race snapshot replacement"
    );
    assert!(core.editor.is_none());
}
