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

#[tokio::test]
async fn synchronization_preserves_an_open_folder_dialog() {
    let mut core = core();
    core.open_folder(None, false);
    let id = core.folder_editor.as_ref().unwrap().id;
    core.synchronize();
    assert_eq!(core.folder_editor.as_ref().unwrap().id, id);
    assert!(core.sync.is_empty());
}

#[tokio::test(start_paused = true)]
async fn incoming_sync_timer_wakes_the_tray_loop_without_a_renderer() {
    let mut core = core();
    core.hidden = true;
    core.start_incoming_timer();
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(15)).await;
    tokio::task::yield_now().await;
    let completion = core
        .receiver
        .try_recv()
        .expect("Incoming sync timer must wake the event loop while hidden");
    completion(&mut core);
}

fn duplicate_items(core: &mut Core, specs: Vec<CipherSpec>) -> Vec<CipherId> {
    let all: Vec<_> = specs
        .into_iter()
        .map(|spec| {
            let mut item = make_cipher(spec);
            Arc::get_mut(&mut item).unwrap().id = Some(CipherId::new_v4());
            item
        })
        .collect();
    let ids = all.iter().map(|i| i.id.unwrap()).collect();
    let cache = VaultCache {
        sort_keys: services::vault_sort::prepare_keys(&all),
        search_keys: all
            .iter()
            .map(|i| (i.id.unwrap(), i.name.to_lowercase()))
            .collect(),
        all,
        ..Default::default()
    };
    core.caches.insert(core.active.unwrap(), cache);
    ids
}
async fn finish_duplicates(core: &mut Core) {
    for _ in 0..400 {
        core.poll();
        if core
            .current_cache()
            .is_some_and(|c| c.duplicate_report.is_some() || c.duplicate_error.is_some())
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("Duplicate comparison did not finish");
}
#[tokio::test]
async fn duplicate_view_honors_facets_and_search_does_not_remove_the_matching_partner() {
    use services::vault_navigation::*;
    let mut core = core();
    let org = bitwarden_core::OrganizationId::new_v4();
    let folder = bitwarden_vault::FolderId::new_v4();
    let collection = bitwarden_collections::collection::CollectionId::new_v4();
    duplicate_items(
        &mut core,
        vec![
            CipherSpec {
                name: "Personal A",
                kind: login_kind(Some("https://a.personal.test/a")),
                folder: Some(folder),
                ..Default::default()
            },
            CipherSpec {
                name: "Personal B",
                kind: login_kind(Some("https://b.personal.test/b")),
                ..Default::default()
            },
            CipherSpec {
                name: "Engineering A",
                kind: login_kind(Some("https://a.company.test/a")),
                organization: Some(org),
                collections: vec![collection],
                ..Default::default()
            },
            CipherSpec {
                name: "Engineering B",
                kind: login_kind(Some("https://b.company.test/b")),
                organization: Some(org),
                collections: vec![collection],
                ..Default::default()
            },
            CipherSpec {
                name: "Archived",
                kind: login_kind(Some("https://a.personal.test/a")),
                archived: true,
                ..Default::default()
            },
            CipherSpec {
                name: "Trash",
                kind: login_kind(Some("https://a.personal.test/a")),
                deleted: true,
                ..Default::default()
            },
            CipherSpec {
                name: "Empty",
                kind: login_kind(None),
                ..Default::default()
            },
        ],
    );
    core.filter = VaultFilter::default().with_section(VaultSection::Duplicates);
    core.refilter();
    finish_duplicates(&mut core).await;
    assert_eq!(core.current_cache().unwrap().visible.len(), 4);
    core.filter.scope = VaultScope::Personal;
    core.refilter();
    finish_duplicates(&mut core).await;
    assert_eq!(core.current_cache().unwrap().visible.len(), 2);
    core.filter.folder = FolderFilter::Folder(folder);
    core.refilter();
    finish_duplicates(&mut core).await;
    assert!(core.current_cache().unwrap().visible.is_empty());
    core.filter.scope = VaultScope::Organization(org);
    core.filter.folder = FolderFilter::All;
    core.filter.collection = CollectionFilter::Collection(collection);
    core.refilter();
    finish_duplicates(&mut core).await;
    assert_eq!(core.current_cache().unwrap().visible.len(), 2);
    core.query = "engineering a".into();
    core.refilter();
    assert_eq!(core.current_cache().unwrap().visible.len(), 1);
    assert_eq!(
        core.current_cache()
            .unwrap()
            .duplicate_report
            .as_ref()
            .unwrap()
            .ids
            .len(),
        2
    );
    core.query.clear();
    core.filter.category = Some(CipherType::SecureNote);
    core.refilter();
    finish_duplicates(&mut core).await;
    assert!(core.current_cache().unwrap().visible.is_empty());
}
#[tokio::test]
async fn duplicate_view_keeps_reprompt_gates_and_cancels_when_leaving_the_view() {
    use services::vault_navigation::*;
    let mut core = core();
    let ids = duplicate_items(
        &mut core,
        vec![
            CipherSpec {
                name: "Protected",
                kind: login_kind(Some("https://example.test/a")),
                ..Default::default()
            },
            CipherSpec {
                name: "Second",
                kind: login_kind(Some("https://example.test/b")),
                ..Default::default()
            },
        ],
    );
    Arc::get_mut(&mut core.caches.get_mut(&core.active.unwrap()).unwrap().all[0])
        .unwrap()
        .reprompt = CipherRepromptType::Password;
    core.filter.section = VaultSection::Duplicates;
    core.refilter();
    finish_duplicates(&mut core).await;
    assert_eq!(core.current_cache().unwrap().visible.len(), 2);
    core.request(Request::Read(ids[0]));
    assert!(core.prompt.is_some());
    assert!(core.detail.is_none());
    core.cancel_prompt();
    core.caches
        .get_mut(&core.active.unwrap())
        .unwrap()
        .duplicate_report = None;
    core.refilter();
    let cancelled = core.duplicate_request.as_ref().unwrap().1.clone();
    core.filter.section = VaultSection::Active;
    core.refilter();
    assert!(cancelled.load(std::sync::atomic::Ordering::SeqCst));
    tokio::time::sleep(Duration::from_millis(20)).await;
    core.poll();
    assert!(core.current_cache().unwrap().duplicate_report.is_none());
    assert_eq!(core.current_cache().unwrap().visible.len(), 2);
}

#[tokio::test]
async fn duplicate_merge_secondary_trash_requires_its_own_reprompt_and_cancel_discards_merge() {
    let mut core = core();
    let id = install(&mut core, true);
    let revision = core.current_cache().unwrap().all[0].revision_date;
    core.merge = Some(super::merge::Merge {
        request: uuid::Uuid::new_v4(),
        ids: vec![CipherId::new_v4(), id],
        phase: super::merge::Phase::Trashing,
        error: None,
        loaded: vec![],
        preview: None,
        revisions: vec![revision, revision],
        next: 1,
        primary_revision: Some(revision),
    });
    core.request(Request::MergeTrash(id, revision));
    let prompt = core
        .prompt
        .as_ref()
        .expect("secondary reprompt must be shown");
    assert_eq!(prompt.target.cipher, Some(id));
    assert_eq!(prompt.target.action, services::sdk::Action::Delete);
    assert_eq!(prompt.target.revision, Some(revision));
    core.cancel_prompt();
    assert!(core.merge.is_none());
    assert!(core.prompt.is_none());
}

#[tokio::test]
async fn duplicate_merge_does_not_trash_a_secondary_changed_since_review() {
    let mut core = core();
    let id = install(&mut core, false);
    let revision = core.current_cache().unwrap().all[0].revision_date;
    let old = revision - chrono::Duration::seconds(1);
    core.merge = Some(super::merge::Merge {
        request: uuid::Uuid::new_v4(),
        ids: vec![CipherId::new_v4(), id],
        phase: super::merge::Phase::Trash,
        error: None,
        loaded: vec![],
        preview: None,
        revisions: vec![revision, old],
        next: 1,
        primary_revision: Some(revision),
    });
    core.poll_merge();
    assert!(core.merge.as_ref().unwrap().phase == super::merge::Phase::Stopped);
    assert!(
        core.merge
            .as_ref()
            .unwrap()
            .error
            .as_ref()
            .unwrap()
            .contains("changed")
    );
    core.clear_presentation();
    assert!(core.merge.is_none());
}
