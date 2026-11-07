//! Bitwarden represents nesting with slash-separated names, not parent IDs.
use std::collections::BTreeMap;

#[derive(Debug, PartialEq)]
pub(super) struct Row<Id> {
    pub name: String,
    pub depth: usize,
    pub id: Option<Id>,
}

/// Preserve duplicate names/IDs and synthesize headings for missing parents.
/// Bound displayed nesting for server-controlled names; a deep tail stays one label.
pub(super) fn rows<Id: Copy>(entries: impl IntoIterator<Item = (Id, String)>) -> Vec<Row<Id>> {
    let mut paths: BTreeMap<Vec<String>, Vec<Id>> = BTreeMap::new();
    for (id, name) in entries {
        let path: Vec<_> = name.splitn(33, '/').map(str::to_owned).collect();
        for depth in 1..path.len() {
            paths.entry(path[..depth].to_vec()).or_default();
        }
        paths.entry(path).or_default().push(id);
    }
    let mut result = Vec::new();
    for (path, ids) in paths {
        let depth = path.len() - 1;
        let name = path[depth].clone();
        if ids.is_empty() {
            result.push(Row {
                name,
                depth,
                id: None,
            });
        } else {
            for id in ids {
                result.push(Row {
                    name: name.clone(),
                    depth,
                    id: Some(id),
                });
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nests_sorts_and_preserves_duplicate_names_and_missing_parents() {
        let result = rows([
            (1, "Work/SSH".into()),
            (2, "Work".into()),
            (3, "Work/SSH".into()),
            (4, "Home/Notes".into()),
        ]);
        assert_eq!(
            result,
            vec![
                Row {
                    name: "Home".into(),
                    depth: 0,
                    id: None
                },
                Row {
                    name: "Notes".into(),
                    depth: 1,
                    id: Some(4)
                },
                Row {
                    name: "Work".into(),
                    depth: 0,
                    id: Some(2)
                },
                Row {
                    name: "SSH".into(),
                    depth: 1,
                    id: Some(1)
                },
                Row {
                    name: "SSH".into(),
                    depth: 1,
                    id: Some(3)
                },
            ]
        );
    }
}
