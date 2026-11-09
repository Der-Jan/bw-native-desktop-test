//! Local website-overlap analysis using saved Bitwarden URI matching modes.
//! No secrets are decrypted and no URL is fetched. Results are candidate items,
//! not a claim that two accounts or passwords are identical.
use bitwarden_vault::{CipherId, CipherListView, CipherListViewType, UriMatchType};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
pub const LABELS: [&str; 6] = [
    "Base domain",
    "Host",
    "Starts with",
    "Exact",
    "Regular expression",
    "Never",
];
pub fn from_number(value: Option<u64>) -> Option<UriMatchType> {
    value.and_then(|v| match v {
        0 => Some(UriMatchType::Domain),
        1 => Some(UriMatchType::Host),
        2 => Some(UriMatchType::StartsWith),
        3 => Some(UriMatchType::Exact),
        4 => Some(UriMatchType::RegularExpression),
        5 => Some(UriMatchType::Never),
        _ => None,
    })
}
pub fn description(mode: Option<UriMatchType>, default: Option<UriMatchType>) -> String {
    let effective = mode.or(default).unwrap_or(UriMatchType::Domain);
    if mode.is_some() {
        LABELS[effective as usize].into()
    } else {
        format!(
            "Default: {}{}",
            LABELS[effective as usize],
            if default.is_some() {
                " (organization)"
            } else {
                ""
            }
        )
    }
}
#[derive(Default)]
pub struct Report {
    pub ids: HashSet<CipherId>,
    pub invalid_rules: usize,
    pub limited: bool,
}
#[derive(Default)]
struct Group {
    sources: HashSet<CipherId>,
    targets: HashSet<CipherId>,
}
fn link(sources: &HashSet<CipherId>, targets: &HashSet<CipherId>, result: &mut HashSet<CipherId>) {
    result.extend(
        sources
            .iter()
            .filter(|id| targets.iter().any(|target| target != *id))
            .copied(),
    );
    result.extend(
        targets
            .iter()
            .filter(|id| sources.iter().any(|source| source != *id))
            .copied(),
    );
}
fn web(value: &str) -> Option<(String, Option<String>)> {
    let value = value.trim();
    let url = match url::Url::parse(value) {
        Ok(url) if matches!(url.scheme(), "http" | "https") => url,
        _ if !value.contains("://") => url::Url::parse(&format!("https://{value}")).ok()?,
        _ => return None,
    };
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let host = url.host()?.to_string();
    let domain = match url.host()? {
        url::Host::Domain("localhost") => Some("localhost".to_owned()),
        url::Host::Domain(host) => psl::domain_str(host).map(str::to_owned),
        other => Some(other.to_string()),
    };
    let host = match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host,
    };
    Some((host, domain))
}
pub fn find(
    items: Vec<Arc<CipherListView>>,
    default: Option<UriMatchType>,
    cancelled: Arc<AtomicBool>,
) -> Result<Report, String> {
    let mut domains: HashMap<String, Group> = HashMap::new();
    let mut hosts: HashMap<String, Group> = HashMap::new();
    let mut exact: BTreeMap<String, Group> = BTreeMap::new();
    let mut prefixes: BTreeMap<String, HashSet<CipherId>> = BTreeMap::new();
    let mut patterns: BTreeMap<String, HashSet<CipherId>> = BTreeMap::new();
    let mut result = Report::default();
    let deadline = Instant::now() + Duration::from_secs(5);
    for item in items {
        if cancelled.load(Ordering::SeqCst) {
            return Err("Website comparison cancelled".into());
        }
        if item.deleted_date.is_some() || item.archived_date.is_some() || item.partial {
            continue;
        }
        let (Some(id), CipherListViewType::Login(login)) = (item.id, &item.r#type) else {
            continue;
        };
        for uri in login.uris.iter().flatten() {
            let Some(raw) = uri.uri.as_deref().filter(|v| !v.is_empty()) else {
                continue;
            };
            let mode = uri.r#match.or(default).unwrap_or(UriMatchType::Domain);
            if mode == UriMatchType::Never {
                continue;
            }
            if raw.len() > 8192 {
                result.invalid_rules += 1;
                continue;
            }
            if let Some((host, domain)) = web(raw) {
                // Same exception used by the regular client's base-domain matcher.
                if let Some(domain) = &domain
                    && host != "script.google.com"
                {
                    domains
                        .entry(domain.clone())
                        .or_default()
                        .targets
                        .insert(id);
                }
                hosts.entry(host.clone()).or_default().targets.insert(id);
                exact.entry(raw.to_owned()).or_default().targets.insert(id);
                match mode {
                    UriMatchType::Domain => {
                        if let Some(domain) = domain {
                            domains.entry(domain).or_default().sources.insert(id);
                        }
                    }
                    UriMatchType::Host => {
                        hosts.entry(host).or_default().sources.insert(id);
                    }
                    UriMatchType::Exact => {
                        exact.entry(raw.to_owned()).or_default().sources.insert(id);
                    }
                    _ => {}
                }
            }
            match mode {
                UriMatchType::StartsWith => {
                    prefixes.entry(raw.to_owned()).or_default().insert(id);
                }
                UriMatchType::RegularExpression => {
                    patterns.entry(raw.to_owned()).or_default().insert(id);
                }
                _ => {}
            }
        }
    }
    for group in domains.values().chain(hosts.values()).chain(exact.values()) {
        link(&group.sources, &group.targets, &mut result.ids);
    }
    for (prefix, sources) in prefixes {
        if cancelled.load(Ordering::SeqCst) {
            return Err("Website comparison cancelled".into());
        }
        let mut targets = HashSet::new();
        for (url, group) in exact.range(prefix.clone()..) {
            if !url.starts_with(&prefix) {
                break;
            }
            targets.extend(group.targets.iter().copied());
        }
        link(&sources, &targets, &mut result.ids);
        if Instant::now() > deadline {
            result.limited = true;
            return Ok(result);
        }
    }
    for (pattern, sources) in patterns {
        if cancelled.load(Ordering::SeqCst) {
            return Err("Website comparison cancelled".into());
        }
        let Ok(regex) = fancy_regex::RegexBuilder::new(&pattern)
            .case_insensitive(true)
            .backtrack_limit(10_000)
            .delegate_size_limit(512 * 1024)
            .delegate_dfa_size_limit(512 * 1024)
            .build()
        else {
            result.invalid_rules += 1;
            continue;
        };
        let mut targets = HashSet::new();
        for (url, group) in &exact {
            if cancelled.load(Ordering::SeqCst) {
                return Err("Website comparison cancelled".into());
            }
            if Instant::now() > deadline {
                result.limited = true;
                return Ok(result);
            }
            match regex.is_match(url) {
                Ok(true) => targets.extend(group.targets.iter().copied()),
                Ok(false) => {}
                Err(_) => {
                    result.invalid_rules += 1;
                    break;
                }
            }
        }
        link(&sources, &targets, &mut result.ids);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::fixtures::{CipherSpec, login_kind, make_cipher};
    use bitwarden_vault::LoginUriView;
    fn item(name: &'static str, uris: &[(&str, Option<UriMatchType>)]) -> Arc<CipherListView> {
        let mut item = make_cipher(CipherSpec {
            name,
            kind: login_kind(None),
            ..Default::default()
        });
        let value = Arc::get_mut(&mut item).unwrap();
        value.id = Some(CipherId::new_v4());
        if let CipherListViewType::Login(login) = &mut value.r#type {
            login.uris = Some(
                uris.iter()
                    .map(|(uri, mode)| LoginUriView {
                        uri: Some((*uri).into()),
                        r#match: *mode,
                        uri_checksum: None,
                    })
                    .collect(),
            );
        }
        item
    }
    fn matches(items: Vec<Arc<CipherListView>>, default: Option<UriMatchType>) -> HashSet<String> {
        let report = find(items.clone(), default, Arc::new(AtomicBool::new(false))).unwrap();
        items
            .iter()
            .filter(|v| report.ids.contains(&v.id.unwrap()))
            .map(|v| v.name.clone())
            .collect()
    }
    #[test]
    fn base_domain_uses_public_suffixes_and_private_tenant_boundaries() {
        let values = vec![
            item("a", &[("https://login.example.co.uk/a", None)]),
            item("b", &[("http://accounts.example.co.uk/b", None)]),
            item("other", &[("https://evil-example.co.uk", None)]),
            item("tenant-a", &[("https://one.github.io", None)]),
            item("tenant-b", &[("https://two.github.io", None)]),
        ];
        assert_eq!(
            matches(values, None),
            HashSet::from(["a".into(), "b".into()])
        );
        assert_eq!(
            matches(
                vec![
                    item("unicode", &[("https://bücher.de", None)]),
                    item("punycode", &[("https://xn--bcher-kva.de", None)])
                ],
                None
            )
            .len(),
            2
        );
    }
    #[test]
    fn host_mode_preserves_ports_and_supports_local_hosts_and_ip_addresses() {
        let mode = Some(UriMatchType::Host);
        assert_eq!(
            matches(
                vec![
                    item("a", &[("https://EXAMPLE.test:443/a", mode)]),
                    item("b", &[("http://example.test:80/b", mode)]),
                    item("port", &[("https://example.test:8443/a", mode)]),
                    item("sub", &[("https://sub.example.test/a", mode)])
                ],
                None
            ),
            HashSet::from(["a".into(), "b".into()])
        );
        for host in ["intranet", "localhost", "127.0.0.1", "[::1]"] {
            assert_eq!(
                matches(
                    vec![
                        item("a", &[(&format!("https://{host}/a"), mode)]),
                        item("b", &[(&format!("https://{host}/b"), mode)])
                    ],
                    None
                )
                .len(),
                2,
                "{host}"
            );
        }
    }
    #[test]
    fn exact_and_prefix_rules_compare_complete_urls() {
        let exact = Some(UriMatchType::Exact);
        assert!(
            matches(
                vec![
                    item("a", &[("https://example.test/A", exact)]),
                    item("b", &[("https://example.test/a", exact)])
                ],
                None
            )
            .is_empty()
        );
        assert_eq!(
            matches(
                vec![
                    item("a", &[("https://example.test/A", exact)]),
                    item("b", &[("https://example.test/A", exact)])
                ],
                None
            )
            .len(),
            2
        );
        assert_eq!(
            matches(
                vec![
                    item(
                        "prefix",
                        &[(
                            "https://example.test/account/",
                            Some(UriMatchType::StartsWith)
                        )]
                    ),
                    item(
                        "target",
                        &[("https://example.test/account/security", exact)]
                    ),
                    item("other", &[("https://example.test/accountant", exact)])
                ],
                None
            ),
            HashSet::from(["prefix".into(), "target".into()])
        );
    }
    #[test]
    fn regex_rules_are_case_insensitive_support_lookahead_and_reject_invalid_patterns() {
        let values = vec![
            item(
                "pattern",
                &[(
                    r"^https://(?=app\.)app\.example\.com/(admin|account)/",
                    Some(UriMatchType::RegularExpression),
                )],
            ),
            item(
                "target",
                &[(
                    "https://APP.EXAMPLE.COM/admin/settings",
                    Some(UriMatchType::Exact),
                )],
            ),
            item(
                "other",
                &[("https://app.example.com/public/", Some(UriMatchType::Exact))],
            ),
            item("invalid", &[("[", Some(UriMatchType::RegularExpression))]),
        ];
        let report = find(values.clone(), None, Arc::new(AtomicBool::new(false))).unwrap();
        assert_eq!(report.invalid_rules, 1);
        assert!(report.ids.contains(&values[0].id.unwrap()));
        assert!(report.ids.contains(&values[1].id.unwrap()));
        assert_eq!(report.ids.len(), 2);
    }
    #[test]
    fn never_empty_and_self_matches_are_excluded_and_all_saved_urls_are_considered() {
        assert!(
            matches(
                vec![
                    item("a", &[("https://example.test", None)]),
                    item(
                        "never",
                        &[("https://example.test", Some(UriMatchType::Never))]
                    )
                ],
                None
            )
            .is_empty()
        );
        assert!(
            matches(
                vec![
                    item(
                        "self",
                        &[
                            ("https://example.test/a", None),
                            ("https://example.test/b", None)
                        ]
                    ),
                    item("empty", &[])
                ],
                None
            )
            .is_empty()
        );
        assert_eq!(
            matches(
                vec![
                    item(
                        "a",
                        &[
                            ("https://unrelated.test", None),
                            ("https://shared.test/path", None)
                        ]
                    ),
                    item("b", &[("https://shared.test/other", None)])
                ],
                None
            )
            .len(),
            2
        );
    }
    #[test]
    fn defaults_are_resolved_and_explicit_modes_override_them() {
        let values = vec![
            item("a", &[("https://a.example.test", None)]),
            item("b", &[("https://b.example.test", None)]),
        ];
        assert!(matches(values.clone(), Some(UriMatchType::Host)).is_empty());
        assert_eq!(matches(values, Some(UriMatchType::Domain)).len(), 2);
        assert_eq!(
            matches(
                vec![
                    item(
                        "a",
                        &[("https://a.example.test", Some(UriMatchType::Domain))]
                    ),
                    item(
                        "b",
                        &[("https://b.example.test", Some(UriMatchType::Domain))]
                    )
                ],
                Some(UriMatchType::Never)
            )
            .len(),
            2
        );
    }
    #[test]
    fn cancelled_scans_stop_and_regex_backtracking_is_bounded() {
        assert!(
            find(
                vec![item("a", &[("https://a.test", None)])],
                None,
                Arc::new(AtomicBool::new(true))
            )
            .is_err()
        );
        let target = format!("https://example.com/{}!", "ab".repeat(40));
        let started = Instant::now();
        let report = find(
            vec![
                item(
                    "pattern",
                    &[(
                        r"^https://example\.com/(a|b|ab)*(?>c)",
                        Some(UriMatchType::RegularExpression),
                    )],
                ),
                item("target", &[(&target, Some(UriMatchType::Exact))]),
            ],
            None,
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(report.ids.is_empty());
        assert!(report.invalid_rules > 0);
    }
}
