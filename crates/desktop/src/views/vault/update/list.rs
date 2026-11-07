//! Vault list interaction: row selection, list-loaded, filter recomputation.

use std::sync::Arc;

use bitwarden_vault::{CipherListView, CipherListViewType};

use crate::{
    app::{Outcome, UpdateCtx},
    debug_fmt::{NoDebug, Summary},
    domain::UserId,
    services::sdk::ClientExt,
};

use crate::views::vault::{
    CollectionFilter, FolderFilter, VaultFilter, VaultMessage, VaultScope, VaultSection,
    message::ForUserMessage, state::VaultView, widgets::item_list::ItemListMessage,
};

impl VaultView {
    pub(super) fn handle_item_list(
        &mut self,
        ctx: &UpdateCtx<'_>,
        msg: ItemListMessage,
    ) -> Outcome<Self> {
        match msg {
            ItemListMessage::SortBy(column) => {
                self.sort = self.sort.clicked(column);
                if let Some(uid) = ctx.active_user {
                    self.recompute_filtered(uid, ctx.active_vault_filter);
                }
                return Outcome::event(crate::views::vault::VaultEvent::SortChanged(self.sort));
            }

            ItemListMessage::ItemSelected(idx) => {
                let new_id = ctx
                    .active_user
                    .and_then(|uid| self.items.get(uid))
                    .and_then(|ic| ic.cached.get(idx))
                    .and_then(|i| i.id);
                // Switching to a different cipher drops any open edit form
                // and disarms the inline delete confirmation — user clicked
                // the list to move on, they don't want the previous item's
                // transient UI bleeding onto the new one. `detail` is left
                // as-is so the pane doesn't flash empty during the fetch
                // (same pattern as re-selecting after an existing detail).
                if new_id != self.selection.id {
                    self.selection.form = None;
                    self.selection.confirm_delete.close();
                }
                self.selection.item = Some(idx);
                self.selection.id = new_id;
                let Some(id) = self.selection.id else {
                    return Outcome::None;
                };
                return ctx.perform_with_active_client(
                    move |client| client.full_cipher(id),
                    move |uid, res| {
                        VaultMessage::ForUser(
                            uid,
                            ForUserMessage::DetailLoaded(id, res.map(|v| NoDebug(Box::new(v)))),
                        )
                    },
                );
            }
            ItemListMessage::CopyUsername(id) | ItemListMessage::CopyPassword(id) => {
                let password = matches!(msg, ItemListMessage::CopyPassword(_));
                // The SDK consumes a Copy grant against this exact cipher and revision.
                return ctx.perform_with_active_client(
                    move |client| async move {
                        let view = client
                            .full_cipher_for(id, crate::services::sdk::Action::Copy)
                            .await?;
                        Ok(view.login.and_then(|login| {
                            if password {
                                login.password
                            } else {
                                login.username
                            }
                        }))
                    },
                    move |uid, result| {
                        VaultMessage::ForUser(
                            uid,
                            ForUserMessage::LoginCopied {
                                password,
                                result: result.map(NoDebug),
                            },
                        )
                    },
                );
            }
            ItemListMessage::OpenExternal(_) => {}
            ItemListMessage::Scrolled(viewport) => {
                self.list_scroll.track(viewport);
            }
        }
        Outcome::None
    }

    pub(super) fn handle_list_loaded(
        &mut self,
        ctx: &UpdateCtx<'_>,
        msg_uid: UserId,
        result: super::super::message::VaultListResult,
    ) -> Outcome<Self> {
        match result {
            Ok((Summary(items), NoDebug(folders))) => {
                tracing::info!(uid = %msg_uid, count = items.len(), "vault list loaded");
                let organizations = ctx.client_manager.list_organizations(&msg_uid);
                let cache = self.items.entry(msg_uid).or_default();
                cache.sort_keys = crate::services::vault_sort::prepare_keys(&items);
                cache.all = items;
                cache.organizations = organizations;
                cache.folders = folders;
                cache.collections = ctx.client_manager.list_collections(&msg_uid);
                // Only recompute the filtered view if this is the active user
                // — search_query / active_filter are view-global state that
                // may not match a background user's context.
                if ctx.is_active_user(&msg_uid) {
                    self.recompute_filtered(&msg_uid, ctx.active_vault_filter);
                    if let Some(id) = self.restore_selection.take()
                        && let Some((index, protected)) = self.items.get(&msg_uid).and_then(|c| {
                            c.cached
                                .iter()
                                .enumerate()
                                .find(|(_, item)| item.id == Some(id))
                                .map(|(i, item)| {
                                    (
                                        i,
                                        item.reprompt
                                            == bitwarden_vault::CipherRepromptType::Password,
                                    )
                                })
                        })
                    {
                        self.selection.id = Some(id);
                        self.selection.item = Some(index);
                        if !protected {
                            return self
                                .handle_item_list(ctx, ItemListMessage::ItemSelected(index));
                        }
                    }
                }
            }
            Err(err) => {
                tracing::error!(uid = %msg_uid, %err, "list_ciphers failed");
            }
        }
        Outcome::None
    }
}

// ── Filter helpers ─────────────────────────────────────────────────────────

impl VaultView {
    /// Recompute the filtered item list for a specific user. Uses the
    /// view-global `search_query` plus the app-level active vault filter
    /// to derive `cached` from `all`. Also reconciles the selection index
    /// against the new filtered list.
    pub(in crate::views::vault) fn recompute_filtered(
        &mut self,
        uid: &UserId,
        filter: VaultFilter,
    ) {
        let query = self.search_query.to_lowercase();

        if let Some(cache) = self.items.get_mut(uid) {
            cache.cached = filter_items_with_navigation(
                &cache.all,
                filter,
                &query,
                &cache.folders,
                &cache.collections,
            );
            crate::services::vault_sort::sort_items(&mut cache.cached, self.sort, &cache.sort_keys);
        }

        if let Some(id) = self.selection.id {
            self.selection.item = self
                .items
                .get(uid)
                .and_then(|c| c.cached.iter().position(|i| i.id == Some(id)));
        } else {
            self.selection.item = None;
        }
        // Filter / search / reload shrinks or reshuffles the dataset —
        // reset the scroll offset so we don't end up past the new end.
        // The viewport_height stays cached from the last real scroll event.
        self.list_scroll.offset_y = 0.0;
    }
}

#[cfg(test)]
fn filter_items(
    all: &[Arc<CipherListView>],
    filter: VaultFilter,
    query: &str,
) -> Vec<Arc<CipherListView>> {
    filter_items_with_navigation(all, filter, query, &[], &[])
}

fn filter_items_with_navigation(
    all: &[Arc<CipherListView>],
    filter: VaultFilter,
    query: &str,
    folders: &[super::super::widgets::cipher_edit::FolderOption],
    collections: &[crate::services::sdk::Collection],
) -> Vec<Arc<CipherListView>> {
    // A real parent includes its descendants. Virtual parents are presentation-only.
    let folder_path = match filter.folder {
        FolderFilter::Folder(id) => folders.iter().find(|f| f.id == id).map(|f| f.name.as_str()),
        _ => None,
    };
    let collection_parent = match filter.collection {
        CollectionFilter::Collection(id) => collections.iter().find(|c| c.id == id),
        _ => None,
    };
    let folder_ids: std::collections::HashSet<_> = folders
        .iter()
        .filter(|f| folder_path.is_some_and(|path| in_subtree(&f.name, path)))
        .map(|f| f.id)
        .collect();
    let collection_ids: std::collections::HashSet<_> = collections
        .iter()
        .filter(|c| {
            collection_parent.is_some_and(|parent| {
                c.organization_id == parent.organization_id && in_subtree(&c.name, &parent.name)
            })
        })
        .map(|c| c.id)
        .collect();
    let items = all.iter().filter(|item| {
        let in_scope = match filter.scope {
            VaultScope::All => true,
            VaultScope::Personal => item.organization_id.is_none(),
            VaultScope::Organization(id) => item.organization_id == Some(id),
        };
        let in_category = filter
            .category
            .is_none_or(|kind| kind == cipher_list_view_type_to_type(&item.r#type));
        let in_section = match filter.section {
            VaultSection::Active => item.deleted_date.is_none() && item.archived_date.is_none(),
            VaultSection::Favorites => {
                item.favorite && item.deleted_date.is_none() && item.archived_date.is_none()
            }
            VaultSection::Archive => item.archived_date.is_some() && item.deleted_date.is_none(),
            VaultSection::Trash => item.deleted_date.is_some(),
        };
        let in_folder = match filter.folder {
            FolderFilter::All => true,
            FolderFilter::None => item.folder_id.is_none(),
            FolderFilter::Folder(id) => item
                .folder_id
                .is_some_and(|f| f == id || folder_ids.contains(&f)),
        };
        let in_collection = match filter.collection {
            CollectionFilter::All => true,
            CollectionFilter::None => item.collection_ids.is_empty(),
            CollectionFilter::Collection(id) => item
                .collection_ids
                .iter()
                .any(|c| *c == id || collection_ids.contains(c)),
        };
        in_scope && in_category && in_section && in_folder && in_collection
    });

    if query.is_empty() {
        items.cloned().collect()
    } else {
        items
            .filter(|item| crate::services::search::matches_query(item, query))
            .cloned()
            .collect()
    }
}

fn in_subtree(name: &str, parent: &str) -> bool {
    name == parent
        || name
            .strip_prefix(parent)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// Project a `CipherListViewType` down to its discriminant `CipherType`.
fn cipher_list_view_type_to_type(t: &CipherListViewType) -> bitwarden_vault::CipherType {
    use bitwarden_vault::CipherType;
    match t {
        CipherListViewType::Login(_) => CipherType::Login,
        CipherListViewType::SecureNote => CipherType::SecureNote,
        CipherListViewType::Card(_) => CipherType::Card,
        CipherListViewType::Identity => CipherType::Identity,
        CipherListViewType::SshKey => CipherType::SshKey,
        CipherListViewType::BankAccount(_) => CipherType::BankAccount,
        CipherListViewType::DriversLicense => CipherType::DriversLicense,
        CipherListViewType::Passport => CipherType::Passport,
    }
}

/// Build the `Outcome` for a clipboard-copy action. `value.is_none()` means
/// the underlying field was empty/missing; the handler falls through silently.
pub(super) fn clipboard_outcome(
    value: Option<String>,
    sensitivity: crate::services::clipboard::Sensitivity,
    toast_label: String,
) -> Outcome<VaultView> {
    Outcome::from_option(value.map(|value| {
        crate::views::vault::VaultEvent::ClipboardCopyRequested {
            value,
            sensitivity,
            toast_label,
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::filter_items;
    use crate::{
        test_support::fixtures::{CipherSpec, login_kind, make_cipher},
        views::vault::{VaultFilter, VaultScope, VaultSection},
    };
    use bitwarden_core::OrganizationId;
    use bitwarden_vault::{CipherListViewType, CipherType};

    #[test]
    fn all_items_excludes_deleted_and_archived() {
        let alive = make_cipher(CipherSpec {
            name: "Alive",
            ..Default::default()
        });
        let deleted = make_cipher(CipherSpec {
            name: "Trashed",
            deleted: true,
            ..Default::default()
        });
        let archived = make_cipher(CipherSpec {
            name: "Archived",
            archived: true,
            ..Default::default()
        });
        let all = vec![alive, deleted, archived];

        let out = filter_items(&all, VaultFilter::default(), "");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "Alive");
    }

    #[test]
    fn trash_shows_only_deleted() {
        let alive = make_cipher(CipherSpec {
            name: "Alive",
            ..Default::default()
        });
        let deleted = make_cipher(CipherSpec {
            name: "Trashed",
            deleted: true,
            ..Default::default()
        });

        let out = filter_items(
            &[alive, deleted],
            VaultFilter::default().with_section(VaultSection::Trash),
            "",
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "Trashed");
    }

    #[test]
    fn archive_shows_only_archived() {
        let alive = make_cipher(CipherSpec {
            name: "Alive",
            ..Default::default()
        });
        let archived = make_cipher(CipherSpec {
            name: "Archived",
            archived: true,
            ..Default::default()
        });

        let out = filter_items(
            &[alive, archived],
            VaultFilter::default().with_section(VaultSection::Archive),
            "",
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "Archived");
    }

    #[test]
    fn favorites_excludes_deleted() {
        // Soft-deleting a favorite shouldn't leave it in the Favorites view.
        let fave = make_cipher(CipherSpec {
            name: "Fave",
            favorite: true,
            ..Default::default()
        });
        let deleted_fave = make_cipher(CipherSpec {
            name: "TrashedFave",
            favorite: true,
            deleted: true,
            ..Default::default()
        });
        let plain = make_cipher(CipherSpec {
            name: "Plain",
            ..Default::default()
        });

        let out = filter_items(
            &[fave, deleted_fave, plain],
            VaultFilter::default().with_section(VaultSection::Favorites),
            "",
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "Fave");
    }

    #[test]
    fn personal_excludes_org_items() {
        let org = OrganizationId::new_v4();
        let personal = make_cipher(CipherSpec {
            name: "Personal",
            ..Default::default()
        });
        let org_item = make_cipher(CipherSpec {
            name: "Org",
            organization: Some(org),
            ..Default::default()
        });

        let out = filter_items(
            &[personal, org_item],
            VaultFilter::default().with_scope(VaultScope::Personal),
            "",
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "Personal");
    }

    #[test]
    fn organization_filter_shows_only_matching_org() {
        let org_a = OrganizationId::new_v4();
        let org_b = OrganizationId::new_v4();
        let a = make_cipher(CipherSpec {
            name: "A",
            organization: Some(org_a),
            ..Default::default()
        });
        let b = make_cipher(CipherSpec {
            name: "B",
            organization: Some(org_b),
            ..Default::default()
        });

        let out = filter_items(
            &[a, b],
            VaultFilter::default().with_scope(VaultScope::Organization(org_a)),
            "",
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "A");
    }

    #[test]
    fn category_filter_matches_only_that_kind() {
        let login = make_cipher(CipherSpec {
            name: "Login",
            kind: login_kind(None),
            ..Default::default()
        });
        let note = make_cipher(CipherSpec {
            name: "Note",
            kind: CipherListViewType::SecureNote,
            ..Default::default()
        });

        let out = filter_items(
            &[login, note],
            VaultFilter::default().with_category(Some(CipherType::Login)),
            "",
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "Login");
    }

    #[test]
    fn query_filter_combines_with_type_filter() {
        // "github" should match the login by URI, not the note that shares
        // the substring in its name — Category gates first, then query.
        let gh_login = make_cipher(CipherSpec {
            name: "GH",
            kind: login_kind(Some("https://github.com")),
            ..Default::default()
        });
        let gh_note = make_cipher(CipherSpec {
            name: "github tips",
            kind: CipherListViewType::SecureNote,
            ..Default::default()
        });

        let out = filter_items(
            &[gh_login, gh_note],
            VaultFilter::default().with_category(Some(CipherType::Login)),
            "github",
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "GH");
    }
}

#[cfg(test)]
mod combined_filter_tests {
    use super::*;
    use crate::test_support::fixtures::{CipherSpec, login_kind, make_cipher};
    use bitwarden_core::OrganizationId;
    #[test]
    fn ssh_filter_stays_inside_selected_vault_and_combines_with_view_and_search() {
        let org = OrganizationId::new_v4();
        let other = OrganizationId::new_v4();
        let all = vec![
            make_cipher(CipherSpec {
                name: "Deploy SSH",
                organization: Some(org),
                kind: CipherListViewType::SshKey,
                favorite: true,
                ..Default::default()
            }),
            make_cipher(CipherSpec {
                name: "Org login",
                organization: Some(org),
                kind: login_kind(None),
                ..Default::default()
            }),
            make_cipher(CipherSpec {
                name: "Other SSH",
                organization: Some(other),
                kind: CipherListViewType::SshKey,
                ..Default::default()
            }),
            make_cipher(CipherSpec {
                name: "Personal SSH",
                kind: CipherListViewType::SshKey,
                ..Default::default()
            }),
            make_cipher(CipherSpec {
                name: "Archived SSH",
                organization: Some(org),
                kind: CipherListViewType::SshKey,
                archived: true,
                ..Default::default()
            }),
            make_cipher(CipherSpec {
                name: "Deleted SSH",
                organization: Some(org),
                kind: CipherListViewType::SshKey,
                deleted: true,
                ..Default::default()
            }),
        ];
        let selected = VaultFilter::default()
            .with_scope(VaultScope::Organization(org))
            .with_category(Some(bitwarden_vault::CipherType::SshKey));
        let names = |filter, query| {
            filter_items(&all, filter, query)
                .iter()
                .map(|item| item.name.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(selected, ""), vec!["Deploy SSH"]);
        assert_eq!(
            names(selected.with_scope(VaultScope::Personal), ""),
            vec!["Personal SSH"]
        );
        assert_eq!(
            names(selected.with_section(VaultSection::Favorites), "deploy"),
            vec!["Deploy SSH"]
        );
        assert!(names(selected, "personal").is_empty());
        assert_eq!(
            names(selected.with_section(VaultSection::Archive), ""),
            vec!["Archived SSH"]
        );
        assert_eq!(
            names(selected.with_section(VaultSection::Trash), ""),
            vec!["Deleted SSH"]
        );
        assert_eq!(
            names(selected.with_category(None), ""),
            vec!["Deploy SSH", "Org login"]
        );
    }
}

#[cfg(test)]
mod navigation_filter_tests {
    use super::*;
    use crate::services::sdk::Collection;
    use crate::test_support::fixtures::{CipherSpec, make_cipher};
    use crate::views::vault::widgets::cipher_edit::FolderOption;
    use bitwarden_collections::collection::CollectionId;
    use bitwarden_core::OrganizationId;
    use bitwarden_vault::{CipherType, FolderId};

    #[test]
    fn folders_collections_scope_type_and_search_intersect_with_descendants() {
        let org = OrganizationId::new_v4();
        let other_org = OrganizationId::new_v4();
        let folder = FolderId::new_v4();
        let child = FolderId::new_v4();
        let unrelated = FolderId::new_v4();
        let collection = CollectionId::new_v4();
        let subcollection = CollectionId::new_v4();
        let other_collection = CollectionId::new_v4();
        let folders = vec![
            FolderOption {
                id: folder,
                name: "Work".into(),
            },
            FolderOption {
                id: child,
                name: "Work/SSH".into(),
            },
            FolderOption {
                id: unrelated,
                name: "Workstation".into(),
            },
        ];
        let collections = vec![
            Collection {
                id: collection,
                organization_id: org,
                name: "Infrastructure".into(),
            },
            Collection {
                id: subcollection,
                organization_id: org,
                name: "Infrastructure/SSH".into(),
            },
            Collection {
                id: other_collection,
                organization_id: other_org,
                name: "Infrastructure/SSH".into(),
            },
        ];
        let make = |name, org, folder, collections, kind| {
            make_cipher(CipherSpec {
                name,
                organization: Some(org),
                folder: Some(folder),
                collections,
                kind,
                ..Default::default()
            })
        };
        let all = vec![
            make(
                "Deploy SSH",
                org,
                child,
                vec![subcollection],
                CipherListViewType::SshKey,
            ),
            make(
                "Outside folder",
                org,
                unrelated,
                vec![subcollection],
                CipherListViewType::SshKey,
            ),
            make(
                "Outside collection",
                org,
                child,
                vec![other_collection],
                CipherListViewType::SshKey,
            ),
            make(
                "Other org",
                other_org,
                child,
                vec![other_collection],
                CipherListViewType::SshKey,
            ),
            make(
                "Note",
                org,
                child,
                vec![subcollection],
                CipherListViewType::SecureNote,
            ),
            make_cipher(CipherSpec {
                name: "Unfiled",
                ..Default::default()
            }),
        ];
        let filter = VaultFilter::default()
            .with_scope(VaultScope::Organization(org))
            .with_category(Some(CipherType::SshKey))
            .with_folder(FolderFilter::Folder(folder))
            .with_collection(CollectionFilter::Collection(collection));
        let result = filter_items_with_navigation(&all, filter, "deploy", &folders, &collections);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "Deploy SSH");
        // Prefix collisions and equal collection paths in another org are excluded.
        assert_eq!(
            filter_items_with_navigation(&all, filter, "", &folders, &collections).len(),
            1
        );
        assert!(
            filter_items_with_navigation(&all, filter, "note", &folders, &collections).is_empty()
        );
        let unfiled = VaultFilter::default()
            .with_folder(FolderFilter::None)
            .with_collection(CollectionFilter::None);
        let result = filter_items_with_navigation(&all, unfiled, "", &folders, &collections);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "Unfiled");
    }
}

#[cfg(test)]
mod sorting_selection_tests {
    use super::*;
    use crate::{
        services::vault_sort::{SortColumn, prepare_keys},
        test_support::fixtures::{CipherSpec, make_cipher},
    };
    use bitwarden_vault::CipherId;
    #[test]
    fn reorder_preserves_selected_id_and_detail_and_resets_virtual_scroll() {
        let uid = UserId::new_v4();
        let id = CipherId::new_v4();
        let mut b = make_cipher(CipherSpec {
            name: "B",
            ..Default::default()
        });
        Arc::get_mut(&mut b).unwrap().id = Some(id);
        Arc::get_mut(&mut b).unwrap().revision_date += chrono::Duration::days(1);
        let a = make_cipher(CipherSpec {
            name: "A",
            ..Default::default()
        });
        let mut view = VaultView::new();
        let all = vec![b, a];
        let cache = view.items.entry(uid).or_default();
        cache.sort_keys = prepare_keys(&all);
        cache.all = all;
        view.selection.id = Some(id);
        view.selection.detail = Some(crate::test_support::fixtures::make_cipher_view());
        view.list_scroll.offset_y = 1000.0;
        view.recompute_filtered(&uid, VaultFilter::default());
        assert_eq!(view.selection.item, Some(1));
        assert_eq!(view.selection.id, Some(id));
        assert!(view.selection.detail.is_some());
        assert_eq!(view.list_scroll.offset_y, 0.0);
        view.sort = view.sort.clicked(SortColumn::LastChanged);
        view.recompute_filtered(&uid, VaultFilter::default());
        assert_eq!(view.selection.item, Some(0));
        assert_eq!(view.items[&uid].cached[0].id, Some(id));
        view.remove_user_items(&uid);
        assert!(!view.items.contains_key(&uid));
    }
}
