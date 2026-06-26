# My AI Bag

Your AI coding bag is packed and ready.

My AI Bag is a Rust experiment with two fronts:

- `aibag`, a CLI that scans your machine for AI coding tool setup folders and can write a local encrypted bag.
- `my-ai-bag`, a native Floem desktop UI for trying the same scanner without Tauri, Electron, or a webview.

The prototype looks for skills folders, MCP config, settings files, and auth-looking files by path/name. It never uploads anything and never prints file contents.

## Run

```powershell
cargo run --bin aibag -- scan
cargo run --bin aibag -- pack
cargo run --bin aibag -- pack --include codex,cursor --output my-bag.aibag --passphrase "use a long test passphrase"
cargo run --bin my-ai-bag
```

`aibag pack` defaults to a preview. It writes an encrypted archive only when `--output` is provided and a passphrase is supplied with `--passphrase` or `AIBAG_PASSPHRASE`.

## Encryption

Encrypted `.aibag` exports use Argon2id to derive a 256-bit key from your passphrase, then XChaCha20-Poly1305 for authenticated encryption. Each archive gets a random salt and nonce. The preview remains plaintext paths/counts only; the actual file bytes are only written inside the encrypted archive.

## What Works

- Built-in catalog for the initial AI coding tools listed in the project brief.
- Home/root and project-root scanning.
- Detection for global skills, project skills, MCP files, settings files, and auth/secret-looking files.
- Pack preview with detected tools, selected tools, unique folders, missing folders, duplicate/shared folders, and safety notes.
- Local encrypted export in the CLI using Argon2id and XChaCha20-Poly1305.
- Unit tests with fake directories, fake settings, fake skills, and fake credentials.
- Native Floem window titled `My AI Bag` with scan, selection checkboxes, preview, and a tool-candidate queue.

## Still Stubbed Or Prototype-Only

- The Floem UI is preview-only for export. Use `aibag pack --output` for encrypted export.
- There is no restore/unpack command yet.
- Tool candidates added in the UI are queued for human review but are not persisted into the built-in catalog.
- The scanner uses conservative filename/path heuristics. It does not understand every tool's private schema yet.
- There is no cloud sync. That is intentional for the first safety pass.

## Security Notes

- Do not share `.aibag` files unless you intend to share the secrets inside.
- The preview lists paths, categories, file counts, and byte counts only. It does not show secret file contents.
- The encrypted archive contains real file bytes for selected detected files and folders.
- Use a long passphrase. The current encryption choice is reasonable for a prototype, but the archive format is not final.
- Auth/secret export should stay opt-in and locally encrypted until a reviewed restore and sync design exists.
