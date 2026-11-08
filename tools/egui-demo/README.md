# egui interaction comparison

Independent native UI demo using pinned egui/eframe 0.37.0 and OpenGL (`glow`).
It uses synthetic data only and does not read the Iced app's profiles, SQLite
stores, credentials, certificates or settings. No login, sync, mTLS or SSH agent
is implemented in this demo. The master-password dialog is an interaction test:
its explicitly displayed demo password is **`demo`**.

```bash
cargo run --manifest-path tools/egui-demo/Cargo.toml --release
```

The separate Cargo workspace and lockfile keep the comparison independent of the
Iced application's pinned dependencies, including their incompatible wasm/web
version requirements. No changes to the main app's dependency lockfile are needed.

## Try it

- Pick 1,000, 10,000 or 50,000 entries from the top-right selector.
- Combine vault, item type, folder, favorites and text search. Search covers names,
  usernames, URLs, notes and synthetic license fields; multiple terms intersect.
- Click Name, Last changed or Base URL to sort; click again to reverse the order.
- Click a row or Tab into the list, then use Up/Down or Page Up/Page Down. The
  selected row remains visible. Arrow keys in a text field do not select rows.
- Escape closes details and preserves the list cursor; another arrow opens the
  next entry. Ctrl+F focuses search. Tab/Shift+Tab use egui's focus navigation.
- Every other synthetic software license asks for the demo password. Enter
  submits, incorrect passwords show an error and Escape cancels the modal.

The table virtualizes rows through `egui_extras::TableBody::rows`. Wheel smoothing
and precise touchpad input use egui's built-in scrolling. Filter searches scan
precomputed text in a cached sort order; rows are not built for offscreen entries.
There is no permanent repaint timer, UI transition animation or painted shadow.

Typography uses installed system fonts through fontdb. On KDE it reads the
configured family and point size through KConfig, then applies the compact rounded
point hierarchy used by the Iced preview. Monitor scaling remains eframe's job.
No text font is embedded in the normal binary; UI tests alone embed a fixture font.
The Bitwarden wordmark uses the original outlined SVG.

## Validation

```bash
cargo test --manifest-path tools/egui-demo/Cargo.toml
cargo clippy --manifest-path tools/egui-demo/Cargo.toml --all-targets -- -D warnings
```

Tests cover combined filtering, sort/selection identity, navigation boundaries,
actual egui keyboard focus and Escape handling, plus the blocking demo dialog.
