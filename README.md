# My AI Bag

Your AI coding bag is packed and ready.

My AI Bag is a Rust experiment with one main command:

- `aibag`, a CLI that scans your machine for AI coding tool setup folders and can write a local encrypted bag.
- `aibag ui`, an optional native Floem desktop UI for trying the same scanner without Tauri, Electron, or a webview.

The prototype can either select known skills/settings/auth/MCP items or pack complete detected agent folders. It can also build a redacted credential/model inventory and collect duplicate-free skills into one local library. It never uploads anything and never prints secret values or file contents.

The built-in coding-agent list lives in `agents.toml` so new tools can be reviewed in simple pull requests without changing Rust source code.

## Run

```powershell
cargo run -- scan
cargo run -- scan --scope home
cargo run -- scan --scope project
cargo run -- pack
cargo run -- pack --include codex,cursor --output my-bag.aibag --passphrase "use a long test passphrase"
cargo run -- pack --include codex:skills,codex:mcp --output codex-skills-and-mcp.aibag --passphrase "use a long test passphrase"
cargo run -- pack --mode folders --include codex --output codex-complete.aibag --passphrase "use a long test passphrase"
cargo run -- credentials --include codex
cargo run -- credentials --output credentials.aibag --passphrase "use a long test passphrase"
cargo run -- skills
cargo run -- skills --store .my-ai-bag\skills
cargo run -- ui
```

`aibag pack` defaults to a preview. It writes an encrypted archive only when `--output` is provided and a passphrase is supplied with `--passphrase` or `AIBAG_PASSPHRASE`.

`pack --mode selective` is the default and includes detected categories. `pack --mode folders` includes every regular file under each selected agent root, including files the category scanner does not recognize. Folder archives use `home/...` and `project/...` paths so they are not tied to the source OS path. Symbolic links are skipped.

`aibag credentials` parses JSON, TOML, YAML, and `.env`-style files under known agent roots. Its normal and `--json` previews are redacted: they show field names, providers, source paths, and model IDs, but never credential values. Supplying `--output` writes the real values and model inventory only inside an encrypted local vault.

`aibag skills` finds folders containing `SKILL.md` and deduplicates identical packages by SHA-256. `--store PATH` copies each unique package, including scripts, into a central folder and writes `index.json`.

Scan scope defaults to `both`, meaning My AI Bag checks known home-directory agent folders and known current-project agent folders. It does not crawl the whole disk. Home `~/.agents` is skipped by default because `.agents` is treated as a project convention.

## Encryption

Encrypted `.aibag` exports use Argon2id to derive a 256-bit key from your passphrase, then XChaCha20-Poly1305 for authenticated encryption. Each archive gets a random salt and nonce. The preview remains plaintext paths/counts only; the actual file bytes are only written inside the encrypted archive.

## What Works

- Built-in catalog for the initial AI coding tools listed in the project brief.
- Home/root and project-root scanning.
- Explicit scan scopes: `home`, `project`, or `both`.
- CLI include filters by tool or tool/category, such as `--include codex` or `--include codex:skills,codex:mcp`.
- Whole-agent-folder preview/export with `pack --mode folders`.
- Detection for global skills, project skills, MCP files, settings files, and auth/secret-looking files.
- Redacted API credential and model discovery from JSON, TOML, YAML, and `.env` files, with encrypted vault export.
- Content-deduplicated central skill collection with a local JSON index.
- Pack preview with detected tools, selected tools, unique folders, missing folders, duplicate/shared folders, and safety notes.
- Local encrypted export in the CLI using Argon2id and XChaCha20-Poly1305.
- Unit tests with fake directories, fake settings, fake skills, and fake credentials.
- Native Floem window titled `My AI Bag` with scan, selection checkboxes, preview, and a tool-candidate queue.

## Still Stubbed Or Prototype-Only

- The Floem UI is preview-only for export. Use `aibag pack --output` for encrypted export.
- The UI is intentionally modular and experimental; the CLI/core flow is the stable surface for now.
- There is no restore/unpack command yet.
- Credential/model injection into other coding agents is not implemented yet.
- Skill enable/disable and installation into agents is not implemented yet.
- Tool candidates added in the UI are queued for human review but are not persisted into the built-in catalog.
- The scanner uses conservative filename/path heuristics. It does not understand every tool's private schema yet.
- Credential and model extraction is heuristic and can miss custom field names or include stale values. The whole-folder mode is the lossless backup option for known roots.
- There is no cloud sync. That is intentional for the first safety pass.

## Security Notes

- Do not share `.aibag` files unless you intend to share the secrets inside.
- The preview lists paths, categories, file counts, and byte counts only. It does not show secret file contents.
- The encrypted archive contains real file bytes for selected detected files and folders.
- Encrypted credential vaults contain real key/token values. Treat them like password-manager exports.
- A central skills store is plaintext and includes scripts. Review collected skills before running them and do not assume they are sandboxed.
- Use a long passphrase. The current encryption choice is reasonable for a prototype, but the archive format is not final.
- Auth/secret export should stay opt-in and locally encrypted until a reviewed restore and sync design exists.
