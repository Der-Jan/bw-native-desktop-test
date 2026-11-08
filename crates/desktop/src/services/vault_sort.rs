//! Persistent column preference and transient, account-local vault sort keys.
use bitwarden_vault::{CipherId, CipherListView, CipherListViewType};
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, cmp::Ordering, collections::HashMap, sync::Arc};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortColumn {
    Username,
    LastChanged,
    BaseUrl,
    #[default]
    #[serde(other)]
    Name,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    Descending,
    #[default]
    #[serde(other)]
    Ascending,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct VaultSort {
    pub column: SortColumn,
    pub direction: SortDirection,
}
impl VaultSort {
    pub fn clicked(self, column: SortColumn) -> Self {
        let direction = if column == self.column {
            match self.direction {
                SortDirection::Ascending => SortDirection::Descending,
                SortDirection::Descending => SortDirection::Ascending,
            }
        } else if column == SortColumn::LastChanged {
            SortDirection::Descending
        } else {
            SortDirection::Ascending
        };
        Self { column, direction }
    }
}

#[derive(Clone)]
pub struct SortKey {
    name: String,
    username: Option<String>,
    base_url: Option<String>,
}
impl SortKey {
    pub fn new(item: &CipherListView) -> Self {
        let base_url = match &item.r#type {
            CipherListViewType::Login(login) => login.uris.as_ref().and_then(|uris| {
                uris.iter()
                    .filter_map(|u| u.uri.as_deref())
                    .find_map(base_url_for_display)
            }),
            _ => None,
        };
        Self {
            name: item.name.to_lowercase(),
            username: match &item.r#type {
                CipherListViewType::Login(login) => login
                    .username
                    .as_deref()
                    .filter(|value| !value.is_empty())
                    .map(str::to_lowercase),
                _ => None,
            },
            base_url,
        }
    }
}

pub fn prepare_keys(items: &[Arc<CipherListView>]) -> HashMap<CipherId, SortKey> {
    items
        .iter()
        .filter_map(|item| item.id.map(|id| (id, SortKey::new(item))))
        .collect()
}

pub fn sort_items(
    items: &mut Vec<Arc<CipherListView>>,
    sort: VaultSort,
    keys: &HashMap<CipherId, SortKey>,
) {
    // Decorate once, including ID-less test/projection rows: no parsing inside comparisons.
    let mut prepared: Vec<_> = items
        .drain(..)
        .map(|item| {
            let key = item
                .id
                .and_then(|id| keys.get(&id))
                .map(Cow::Borrowed)
                .unwrap_or_else(|| Cow::Owned(SortKey::new(&item)));
            (item, key)
        })
        .collect();
    let directed = |order: Ordering| {
        if sort.direction == SortDirection::Descending {
            order.reverse()
        } else {
            order
        }
    };
    prepared.sort_by(|(a, ka), (b, kb)| {
        let primary = match sort.column {
            SortColumn::Name => directed(natural_cmp(&ka.name, &kb.name)),
            SortColumn::LastChanged => directed(a.revision_date.cmp(&b.revision_date)),
            SortColumn::Username | SortColumn::BaseUrl => {
                match if sort.column == SortColumn::Username {
                    (&ka.username, &kb.username)
                } else {
                    (&ka.base_url, &kb.base_url)
                } {
                    (Some(a), Some(b)) => directed(natural_cmp(a, b)),
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => Ordering::Equal,
                }
            }
        };
        primary
            .then_with(|| natural_cmp(&ka.name, &kb.name))
            .then_with(|| a.id.cmp(&b.id))
    });
    items.extend(prepared.into_iter().map(|(item, _)| item));
}

/// Compare normalized UTF-8 text with ASCII numeric runs as arbitrary-sized numbers.
/// Unicode letters keep deterministic code-point ordering; this is not OS locale collation.
fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut i, mut j) = (0, 0);
    let (ab, bb) = (a.as_bytes(), b.as_bytes());
    while i < ab.len() && j < bb.len() {
        if ab[i].is_ascii_digit() && bb[j].is_ascii_digit() {
            let (start_i, start_j) = (i, j);
            while i < ab.len() && ab[i].is_ascii_digit() {
                i += 1;
            }
            while j < bb.len() && bb[j].is_ascii_digit() {
                j += 1;
            }
            let an = a[start_i..i].trim_start_matches('0');
            let bn = b[start_j..j].trim_start_matches('0');
            let order = an.len().cmp(&bn.len()).then_with(|| an.cmp(bn));
            if !order.is_eq() {
                return order;
            }
        } else {
            let order = ab[i].cmp(&bb[j]);
            if !order.is_eq() {
                return order;
            }
            i += 1;
            j += 1;
        }
    }
    (ab.len() - i).cmp(&(bb.len() - j))
}

/// Display only a web hostname and non-default port. Never expose URI paths,
/// queries, fragments, usernames or passwords in the list column.
pub fn base_url_for_display(value: &str) -> Option<String> {
    let value = value.trim();
    let parsed = match url::Url::parse(value) {
        Ok(url) if matches!(url.scheme(), "http" | "https") => url,
        Ok(url) if url.has_host() || value.contains("://") => return None,
        _ if value.contains("://") => return None,
        _ => {
            let url = url::Url::parse(&format!("https://{value}")).ok()?;
            if !url.username().is_empty() || url.password().is_some() {
                return None;
            }
            url
        }
    };
    let host = parsed.host()?.to_string();
    Some(match parsed.port() {
        Some(port) => format!("{host}:{port}"),
        None => host,
    })
}

#[cfg(test)]
mod url_display_tests {
    use super::base_url_for_display;
    #[test]
    fn url_column_redacts_credentials_paths_queries_and_fragments() {
        assert_eq!(
            base_url_for_display(
                "https://alice:secret@Example.com:8443/private?token=secret#secret"
            ),
            Some("example.com:8443".into())
        );
        assert_eq!(
            base_url_for_display("https://example.com:443/path"),
            Some("example.com".into())
        );
        assert_eq!(
            base_url_for_display("http://[::1]:8080/path"),
            Some("[::1]:8080".into())
        );
        assert_eq!(
            base_url_for_display("localhost:8443/path?token=secret"),
            Some("localhost:8443".into())
        );
        assert_eq!(
            base_url_for_display("example.com/login"),
            Some("example.com".into())
        );
    }
    #[test]
    fn url_column_ignores_non_web_and_invalid_uris() {
        for uri in [
            "",
            "https://",
            "https://?token=secret",
            "androidapp://com.example",
            "file:///private/secret",
            "mailto:alice@example.com",
            "javascript:alert(1)",
        ] {
            assert_eq!(base_url_for_display(uri), None, "URI: {uri}");
        }
    }
}

#[cfg(test)]
mod sorting_tests {
    use super::*;
    use crate::test_support::fixtures::{CipherSpec, login_kind, make_cipher};
    fn item(name: &'static str, uri: Option<&str>, id: u128, days: i64) -> Arc<CipherListView> {
        let mut item = make_cipher(CipherSpec {
            name,
            kind: login_kind(uri),
            ..Default::default()
        });
        let value = Arc::get_mut(&mut item).unwrap();
        value.id = Some(CipherId::new(uuid::Uuid::from_u128(id)));
        value.revision_date += chrono::Duration::days(days);
        item
    }
    #[test]
    fn columns_toggle_default_to_newest_and_keep_url_less_rows_last() {
        let all = vec![
            item("Account 10", Some("https://b.example/path"), 10, 10),
            item("account 2", Some("https://a.example/private"), 2, 2),
            item("Empty", None, 3, 3),
        ];
        let keys = prepare_keys(&all);
        let mut rows = all;
        sort_items(&mut rows, VaultSort::default(), &keys);
        assert_eq!(
            rows.iter().map(|v| v.name.as_str()).collect::<Vec<_>>(),
            ["account 2", "Account 10", "Empty"]
        );
        let date = VaultSort::default().clicked(SortColumn::LastChanged);
        sort_items(&mut rows, date, &keys);
        assert_eq!(
            rows.iter().map(|v| v.id.unwrap()).collect::<Vec<_>>(),
            [10, 3, 2].map(|id| CipherId::new(uuid::Uuid::from_u128(id)))
        );
        sort_items(&mut rows, date.clicked(SortColumn::LastChanged), &keys);
        assert_eq!(rows[0].name, "account 2");
        let url = VaultSort::default().clicked(SortColumn::BaseUrl);
        sort_items(&mut rows, url, &keys);
        assert_eq!(rows[0].name, "account 2");
        assert_eq!(rows[2].name, "Empty");
        sort_items(&mut rows, url.clicked(SortColumn::BaseUrl), &keys);
        assert_eq!(rows[0].name, "Account 10");
        assert_eq!(rows[2].name, "Empty");
    }
    #[test]
    fn usernames_sort_naturally_and_keep_missing_values_last_in_both_directions() {
        let mut rows: Vec<_> = [
            ("Ten", Some("User10")),
            ("Two", Some("user2")),
            ("Empty", Some("")),
            ("Missing", None),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (name, username))| {
            let mut row = item(name, None, index as u128 + 1, 0);
            if let CipherListViewType::Login(login) = &mut Arc::get_mut(&mut row).unwrap().r#type {
                login.username = username.map(str::to_owned);
            }
            row
        })
        .collect();
        let keys = prepare_keys(&rows);
        let sort = VaultSort::default().clicked(SortColumn::Username);
        sort_items(&mut rows, sort, &keys);
        assert_eq!(
            rows.iter().map(|v| v.name.as_str()).collect::<Vec<_>>(),
            ["Two", "Ten", "Empty", "Missing"]
        );
        sort_items(&mut rows, sort.clicked(SortColumn::Username), &keys);
        assert_eq!(
            rows.iter().map(|v| v.name.as_str()).collect::<Vec<_>>(),
            ["Ten", "Two", "Empty", "Missing"]
        );
        let restored: VaultSort =
            serde_json::from_value(serde_json::to_value(sort).unwrap()).unwrap();
        assert_eq!(restored, sort);
    }
    #[test]
    fn equal_values_use_name_then_stable_id_and_numeric_runs_do_not_overflow() {
        let mut rows = vec![
            item("SAME", Some("example.com"), 20, 0),
            item("same", Some("example.com"), 10, 0),
        ];
        let keys = prepare_keys(&rows);
        for column in [
            SortColumn::Name,
            SortColumn::Username,
            SortColumn::LastChanged,
            SortColumn::BaseUrl,
        ] {
            for direction in [SortDirection::Ascending, SortDirection::Descending] {
                rows.reverse();
                sort_items(&mut rows, VaultSort { column, direction }, &keys);
                assert_eq!(
                    rows[0].id.unwrap(),
                    CipherId::new(uuid::Uuid::from_u128(10))
                );
            }
        }
        assert!(
            natural_cmp(
                "item 999999999999999999999999999999",
                "item 1000000000000000000000000000000"
            )
            .is_lt()
        );
        assert_eq!(natural_cmp("item 02", "item 2"), Ordering::Equal);
        assert!(natural_cmp("ä 2", "ä 10").is_lt());
    }
    #[test]
    fn preference_round_trips_and_unknown_settings_leave_other_fields_intact() {
        use crate::services::settings::Settings;
        let settings: Settings = serde_json::from_value(serde_json::json!({"language":"de", "vault_sort":{"column":"future", "direction":"future"}})).unwrap();
        assert_eq!(settings.language, "de");
        assert_eq!(settings.vault_sort, VaultSort::default());
        let sort = VaultSort::default().clicked(SortColumn::LastChanged);
        let restored: Settings = serde_json::from_value(
            serde_json::json!({"vault_sort":serde_json::to_value(sort).unwrap()}),
        )
        .unwrap();
        assert_eq!(restored.vault_sort, sort);
        assert_eq!(
            serde_json::to_value(sort).unwrap(),
            serde_json::json!({"column":"last_changed","direction":"descending"})
        );
    }
}
