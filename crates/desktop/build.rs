use std::{collections::BTreeMap, path::Path};

const FONT_VERSION: &str = "1.13.1";

fn main() {
    generate_bootstrap_icons();
    embed_sdk_rev();
}

/// Read the resolved git revision of `bitwarden-core` out of the workspace
/// `Cargo.lock` and expose it as the `SDK_REV_SHORT` env var for `env!()`.
/// All `bitwarden-*` crates pin to the same rev, so any of them would work
/// — `bitwarden-core` is the canonical pick.
///
/// Reading from `Cargo.lock` rather than `Cargo.toml` is robust to changes
/// in how the dep is declared (`rev = "..."` vs `branch = "..."`): Cargo
/// always resolves to a `source = "git+...?rev=HEX#HEX"` entry.
fn embed_sdk_rev() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR set");
    let lock_path = Path::new(&manifest_dir).join("../../Cargo.lock");
    println!("cargo:rerun-if-changed={}", lock_path.display());

    let short = match std::fs::read_to_string(&lock_path) {
        Ok(contents) => match extract_sdk_rev(&contents) {
            Some(rev) => rev[..rev.len().min(7)].to_string(),
            None => {
                println!("cargo:warning=SDK_REV_SHORT: source rev not found in Cargo.lock");
                "unknown".into()
            }
        },
        Err(e) => {
            println!("cargo:warning=SDK_REV_SHORT: failed to read Cargo.lock ({e})");
            "unknown".into()
        }
    };

    println!("cargo:rustc-env=SDK_REV_SHORT={short}");
}

/// Walk `Cargo.lock` looking for the `bitwarden-core` `[[package]]` block
/// (or unpatched `bitwarden-crypto` when core has a local transport patch)
/// and return the hash from its `source = "git+...?rev=HEX#HEX"` line.
fn extract_sdk_rev(lock: &str) -> Option<String> {
    let mut in_target = false;
    for line in lock.lines() {
        let line = line.trim();
        if line == "[[package]]" {
            in_target = false;
            continue;
        }
        if line == r#"name = "bitwarden-core""# || line == r#"name = "bitwarden-crypto""# {
            in_target = true;
            continue;
        }
        if !in_target {
            continue;
        }
        let Some(src) = line.strip_prefix("source = \"") else {
            continue;
        };
        let src = src.strip_suffix('"').unwrap_or(src);
        // Expected shape: git+https://...?rev=HEX#HEX
        let (_, after) = src.split_once("?rev=")?;
        let rev = after.split_once('#').map_or(after, |(r, _)| r);
        if rev.len() >= 7 && rev.chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(rev.to_string());
        }
        return None;
    }
    None
}

fn generate_bootstrap_icons() {
    let css_file = format!("../../assets/bootstrap-icons-{FONT_VERSION}.css");
    let ttf_file = format!("../../assets/bootstrap-icons-{FONT_VERSION}.ttf");

    println!("cargo:rerun-if-changed={css_file}");
    println!("cargo:rerun-if-changed={ttf_file}");
    println!("cargo:rerun-if-changed=build.rs");

    let css = std::fs::read_to_string(&css_file)
        .unwrap_or_else(|e| panic!("Failed to read {css_file}: {e}"));

    let mut icons = BTreeMap::new();

    for line in css.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix(".bi-")
            && let Some((name, rest)) = rest.split_once("::before")
            && let Some(start) = rest.find("\"\\")
        {
            let hex_start = start + 2;
            if let Some(end) = rest[hex_start..].find('"') {
                let hex = &rest[hex_start..hex_start + end];
                let mut const_name = name.to_uppercase().replace('-', "_");
                if const_name.starts_with(|c: char| c.is_ascii_digit()) {
                    const_name = format!("_{const_name}");
                }
                icons.insert(const_name, hex.to_string());
            }
        }
    }

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let gen_path = Path::new(&out_dir).join("bootstrap_icons_generated.rs");

    let mut code = String::new();
    code.push_str(&format!(
        "// Auto-generated from bootstrap-icons-{FONT_VERSION} — do not edit\n\n"
    ));
    code.push_str(&format!(
        "pub const FONT_BYTES: &[u8] = include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/../../assets/bootstrap-icons-{FONT_VERSION}.ttf\"));\n\n"
    ));

    for (name, hex) in &icons {
        code.push_str(&format!("pub const {name}: Icon = Icon('\\u{{{hex}}}');\n"));
    }

    code.push_str(&format!("pub const ICON_COUNT: usize = {};\n", icons.len()));

    std::fs::write(&gen_path, code).expect("Failed to write generated icons");
}
