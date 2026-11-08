pub const VAULTS: [&str; 3] = ["Privat", "Arbeit", "Team"];
pub const FOLDERS: [&str; 4] = ["Entwicklung", "Finanzen", "Infrastruktur", "Ohne Ordner"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Login,
    Note,
    Ssh,
    Card,
    Identity,
}
impl Kind {
    pub const ALL: [Self; 5] = [
        Self::Login,
        Self::Note,
        Self::Ssh,
        Self::Card,
        Self::Identity,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Login => "Logins",
            Self::Note => "Notizen / Lizenzen",
            Self::Ssh => "SSH-Schlüssel",
            Self::Card => "Karten",
            Self::Identity => "Identitäten",
        }
    }
    pub fn mark(self) -> &'static str {
        match self {
            Self::Login => "L",
            Self::Note => "N",
            Self::Ssh => "S",
            Self::Card => "K",
            Self::Identity => "I",
        }
    }
}

pub struct Item {
    pub id: usize,
    pub name: String,
    pub subtitle: String,
    pub kind: Kind,
    pub vault: usize,
    pub folder: usize,
    pub favorite: bool,
    pub protected: bool,
    pub url: String,
    pub changed: i64,
    pub changed_label: String,
    pub notes: String,
    pub fields: Vec<(&'static str, String)>,
    name_key: String,
    search_key: String,
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct Filters {
    pub vault: Option<usize>,
    pub kind: Option<Kind>,
    pub folder: Option<usize>,
    pub favorites: bool,
    pub query: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Column {
    Name,
    Changed,
    Url,
}
#[derive(Clone, Copy)]
pub struct Sort {
    pub column: Column,
    pub descending: bool,
}
impl Default for Sort {
    fn default() -> Self {
        Self {
            column: Column::Name,
            descending: false,
        }
    }
}

pub struct Model {
    pub items: Vec<Item>,
    /// Stable source IDs in the current filtered, sorted order.
    pub visible: Vec<usize>,
    pub filters: Filters,
    pub sort: Sort,
    pub selected: Option<usize>,
    order: Vec<usize>,
}
impl Model {
    pub fn new(count: usize) -> Self {
        let mut model = Self {
            items: Vec::new(),
            visible: Vec::new(),
            filters: Filters::default(),
            sort: Sort::default(),
            selected: None,
            order: Vec::new(),
        };
        model.replace_data(count);
        model
    }
    pub fn replace_data(&mut self, count: usize) {
        self.items = (0..count).map(make_item).collect();
        self.order = (0..count).collect();
        self.selected = None;
        self.sort_order();
        self.refilter();
    }
    pub fn sort_by(&mut self, column: Column) {
        self.sort = Sort {
            column,
            descending: if self.sort.column == column {
                !self.sort.descending
            } else {
                column == Column::Changed
            },
        };
        self.sort_order();
        self.refilter();
    }
    fn sort_order(&mut self) {
        self.order.sort_unstable_by(|a, b| {
            let (a, b) = (&self.items[*a], &self.items[*b]);
            let order = match self.sort.column {
                Column::Name => a.name_key.cmp(&b.name_key),
                Column::Changed => a.changed.cmp(&b.changed),
                Column::Url => a.url.cmp(&b.url),
            };
            let order = if self.sort.descending {
                order.reverse()
            } else {
                order
            };
            order.then_with(|| a.id.cmp(&b.id))
        });
    }
    pub fn refilter(&mut self) {
        let query = self.filters.query.to_lowercase();
        let terms: Vec<_> = query.split_whitespace().collect();
        self.visible = self
            .order
            .iter()
            .copied()
            .filter(|id| {
                let item = &self.items[*id];
                self.filters.vault.is_none_or(|v| v == item.vault)
                    && self.filters.kind.is_none_or(|k| k == item.kind)
                    && self.filters.folder.is_none_or(|f| f == item.folder)
                    && (!self.filters.favorites || item.favorite)
                    && terms.iter().all(|term| item.search_key.contains(term))
            })
            .collect();
        if self.selected.is_some_and(|id| !self.visible.contains(&id)) {
            self.selected = None;
        }
    }
    pub fn adjacent(&self, down: bool) -> Option<usize> {
        let Some(position) = self
            .selected
            .and_then(|id| self.visible.iter().position(|v| *v == id))
        else {
            return self.visible.first().copied();
        };
        let next = if down {
            (position + 1).min(self.visible.len().saturating_sub(1))
        } else {
            position.saturating_sub(1)
        };
        (next != position).then(|| self.visible[next])
    }
    pub fn select(&mut self, id: usize) {
        if self.visible.contains(&id) {
            self.selected = Some(id);
        }
    }
    pub fn selected_position(&self) -> Option<usize> {
        self.selected
            .and_then(|id| self.visible.iter().position(|v| *v == id))
    }
    pub fn filter_summary(&self) -> String {
        let mut parts = Vec::new();
        if let Some(v) = self.filters.vault {
            parts.push(VAULTS[v]);
        }
        if let Some(k) = self.filters.kind {
            parts.push(k.label());
        }
        if let Some(f) = self.filters.folder {
            parts.push(FOLDERS[f]);
        }
        if self.filters.favorites {
            parts.push("Favoriten");
        }
        if parts.is_empty() {
            "Alle Tresore · alle Eintragstypen".into()
        } else {
            parts.join(" · ")
        }
    }
}

fn make_item(id: usize) -> Item {
    let kind = match id % 8 {
        0..=3 => Kind::Login,
        4 => Kind::Note,
        5 => Kind::Ssh,
        6 => Kind::Card,
        _ => Kind::Identity,
    };
    let services = [
        "GitHub",
        "Cloudflare",
        "Mail",
        "Hetzner",
        "GitLab",
        "Nextcloud",
        "Monitoring",
        "Router",
        "NAS",
        "Forum",
        "CI Server",
        "Dokumentation",
    ];
    let service = services[(id / 8) % services.len()];
    let name = match kind {
        Kind::Login => format!("{service} · Konto {id:05}"),
        Kind::Note => format!("Softwarelizenz · Projekt {id:05}"),
        Kind::Ssh => format!("SSH · Server {id:05}"),
        Kind::Card => format!("Firmenkarte · {id:05}"),
        Kind::Identity => format!("Kontakt · Person {id:05}"),
    };
    let subtitle = match kind {
        Kind::Login => format!("demo.user.{id:05}@example.invalid"),
        Kind::Note => "Lizenzschlüssel und Notizen".into(),
        Kind::Ssh => format!("ED25519 · demo@server-{id:05}"),
        Kind::Card => format!("Visa · endet auf {:04}", id % 10000),
        Kind::Identity => format!("Person {id:05} · Beispiel GmbH"),
    };
    let url = if kind == Kind::Login {
        format!(
            "https://{}.example.invalid",
            service.to_lowercase().replace(' ', "-")
        )
    } else if kind == Kind::Ssh {
        format!("ssh://server-{id:05}.example.invalid")
    } else {
        String::new()
    };
    let changed = 1_794_096_000 - ((id * 7919) % 525_600) as i64 * 60;
    let changed_label = chrono::DateTime::from_timestamp(changed, 0)
        .unwrap()
        .format("%Y-%m-%d  %H:%M")
        .to_string();
    let vault = (id / 8) % VAULTS.len();
    let folder = (id / 24) % FOLDERS.len();
    let fields = match kind {
        Kind::Login => vec![
            ("Benutzername", subtitle.clone()),
            ("Passwort", format!("Demo-Passwort-{id:05}")),
        ],
        Kind::Note => vec![
            ("Lizenzschlüssel", format!("DEMO-{id:05}-TEST-ONLY")),
            ("Lizenznehmer", "Beispiel GmbH".into()),
            ("Produkt", format!("Entwicklungswerkzeug {id:05}")),
        ],
        Kind::Ssh => vec![
            (
                "Öffentlicher Schlüssel",
                format!("Nur Testdaten: SSH-Schlüssel {id:05}"),
            ),
            ("Kommentar", format!("demo@server-{id:05}")),
        ],
        Kind::Card => vec![
            ("Karteninhaber", "Demo Benutzer".into()),
            ("Kartennummer", format!("Testkarte … {:04}", id % 10000)),
        ],
        Kind::Identity => vec![
            ("Name", format!("Demo Person {id:05}")),
            ("E-Mail", format!("person.{id:05}@example.invalid")),
        ],
    };
    let notes = format!(
        "Synthetischer Eintrag für den egui-Vergleich.\nTresor: {}\nOrdner: {}\nKeine echten Zugangsdaten.",
        VAULTS[vault], FOLDERS[folder]
    );
    let search_key = format!(
        "{name} {subtitle} {url} {notes} {}",
        fields
            .iter()
            .map(|(_, v)| v.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    )
    .to_lowercase();
    let name_key = name.to_lowercase();
    Item {
        id,
        name,
        subtitle,
        kind,
        vault,
        folder,
        favorite: id.is_multiple_of(11),
        protected: kind == Kind::Note && (id / 8).is_multiple_of(2),
        url,
        changed,
        changed_label,
        notes,
        fields,
        name_key,
        search_key,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vault_kind_folder_favorite_and_search_filters_intersect() {
        let mut m = Model::new(10_000);
        let item = &m.items[132];
        m.filters = Filters {
            vault: Some(item.vault),
            kind: Some(item.kind),
            folder: Some(item.folder),
            favorites: true,
            query: "DEMO-00132-TEST-ONLY".into(),
        };
        m.refilter();
        assert_eq!(m.visible, vec![132]);
        m.filters.vault = Some((m.items[132].vault + 1) % 3);
        m.refilter();
        assert!(m.visible.is_empty());
    }
    #[test]
    fn navigation_uses_filtered_order_and_stops_at_boundaries() {
        let mut m = Model::new(100);
        m.filters.kind = Some(Kind::Ssh);
        m.refilter();
        let first = m.visible[0];
        m.select(first);
        assert_eq!(m.adjacent(false), None);
        let second = m.visible[1];
        assert_eq!(m.adjacent(true), Some(second));
        m.select(*m.visible.last().unwrap());
        assert_eq!(m.adjacent(true), None);
        m.filters.kind = Some(Kind::Card);
        m.refilter();
        assert!(m.selected.is_none());
    }
    #[test]
    fn column_sorting_keeps_selection_by_identity() {
        let mut m = Model::new(1000);
        m.select(42);
        m.sort_by(Column::Changed);
        assert_eq!(m.selected, Some(42));
        assert!(
            m.visible
                .windows(2)
                .all(|w| m.items[w[0]].changed >= m.items[w[1]].changed)
        );
        m.sort_by(Column::Changed);
        assert!(
            m.visible
                .windows(2)
                .all(|w| m.items[w[0]].changed <= m.items[w[1]].changed)
        );
    }
}
