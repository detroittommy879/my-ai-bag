# My AI Bag Agent Notes

My AI Bag is a native Rust/Floem plus CLI experiment. Do not convert it to Tauri, Electron, or a webview app.

## Project Shape

- `src/lib.rs` exposes the shared catalog, scanner, preview, and pack/export logic.
- `src/bin/aibag.rs` is the main CLI and owns the stable user-facing workflow.
- `src/ui.rs` is the Floem desktop app module. Keep UI churn here and call shared library functions for real work.
- `src/bin/my_ai_bag.rs` is only a compatibility launcher for the UI.
- `docs/` and `README.md` are user-facing documentation.

## Safety Rules

- Do not upload files.
- Do not print secret file contents.
- Do not display auth/config contents in the UI.
- Preview output may show paths, categories, file counts, and byte counts.
- Encrypted export may include auth/config bytes, but only after an explicit local `pack --output` action and passphrase.

## Documentation Maintenance

When changing visible behavior, update `README.md` and any relevant file under `docs/` in the same change.

Keep docs clear about:

- what My AI Bag is
- how to run the CLI and UI
- what works now
- what is stubbed or prototype-only
- security warnings around secrets and auth files

## Testing

Use fake directories and fake credentials in tests. Tests should verify detection and packing behavior without relying on the real home directory.

Useful commands:

```powershell
cargo test
cargo run -- scan
cargo run -- pack --include codex --output test.aibag --passphrase "use a long test passphrase"
cargo run -- ui
```
