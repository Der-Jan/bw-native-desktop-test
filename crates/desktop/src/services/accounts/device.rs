//! Stable installation identity, preserved across frontend migrations.
use super::storage::{atomic_write, read_bounded};
use std::path::Path;
use uuid::Uuid;
pub fn id(root: &Path) -> Result<Uuid, String> {
    let path = root.join("device-id");
    if path.exists() {
        return std::str::from_utf8(&read_bounded(&path, 100)?)
            .map_err(|_| "Invalid device ID")?
            .trim()
            .parse()
            .map_err(|_| "Invalid device ID".into());
    }
    let id = Uuid::new_v4();
    atomic_write(root, "device-id", id.to_string().as_bytes())
        .map_err(|_| "Could not save device ID")?;
    Ok(id)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn installation_id_survives_reopening_and_invalid_ids_are_not_replaced() {
        let root = std::env::temp_dir().join(format!("bw-device-test-{}", Uuid::new_v4()));
        let first = id(&root).unwrap();
        assert_eq!(id(&root).unwrap(), first);
        std::fs::write(root.join("device-id"), "invalid").unwrap();
        assert!(id(&root).is_err());
        assert_eq!(
            std::fs::read_to_string(root.join("device-id")).unwrap(),
            "invalid"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
