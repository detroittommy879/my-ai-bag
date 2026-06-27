# My AI Bag Plans

## Product Direction

My AI Bag should feel like one small Rust app named `aibag`, not a pile of separate tools. The native Floem UI can live inside the same binary and stay optional:

- `aibag` with no arguments should scan the likely places and show an actionable summary.
- `aibag ui` should open the Floem app.
- `aibag pack` should support headless packing for scripts, VMs, and fresh machines.
- `aibag credentials` should normalize API keys and model references into a local encrypted vault.
- `aibag skills` should provide one deduplicated local skill library.
- `aibag help` should explain the same workflow without requiring users to understand the implementation split.

The original two-binary prototype (`aibag` plus `my-ai-bag`) was useful, but the product direction is now CLI-first with `aibag ui` as the optional UI entry point. The old `my-ai-bag` binary can remain temporarily as a compatibility launcher.

## Scan Model

Scanning should not crawl the whole disk. It should only check known locations from the agent catalog plus any user-provided extra paths.

Default behavior should scan both scopes:

- Home scope: known agent folders under `~/`, such as `~/.claude`, `~/.codex`, `~/.gemini`, `~/.cursor`, and config paths under `~/.config`.
- Project scope: known agent folders under the current directory, such as `./.claude`, `./.codex`, `./.gemini`, and the shared project `./.agents`.

Important rule: ignore `~/.agents` in the home scope by default. Treat `.agents` as a project convention unless a future catalog entry proves a specific tool intentionally uses `~/.agents`.

Suggested CLI flags:

```powershell
aibag                 # scan home + current project, show default summary
aibag scan            # same as default
aibag scan --home     # home scope only
aibag scan --project  # current/project scope only
aibag scan --both     # explicit default
aibag scan --path G:\somewhere\special-agent-config
aibag ui              # open native Floem UI
```

## Packing UX

Two packing modes now exist:

- `selective` includes recognized skills, settings, MCP, and auth paths.
- `folders` includes every regular file under selected known agent roots and is the simplest migration experiment.

Complete-folder archives preserve `home/...` and `project/...` namespaces for future cross-OS restore mapping. Symbolic links are skipped. Restore is still pending.

Packing should be selectable at three levels:

- Source scope: home, project, or both.
- Coding agent: Codex, Claude Code, Cursor, etc.
- Item category found for that agent: skills, settings, MCP, auth/secrets.

In the UI, each agent row should expand to show what was found. Users should be able to check or uncheck individual categories or paths before packing.

The pack preview should keep the current safety rule: show paths, categories, counts, and notes, but never print file contents.

The pack file can contain both home and project entries, but the archive layout should keep them clearly separated:

```text
home/.codex/...
project/.agents/...
extra/<label>/...
```

## Agent Catalog

The agent list should be easy to review in pull requests. It should not live only in Rust code and it should not be JSON.

Current step: the built-in catalog now lives in `agents.toml`.

The current catalog can optionally declare `home_roots` and `project_roots` for complete-folder mode. If omitted, roots are derived from the existing detection and skills paths.

## Credentials And Models

The first inventory mode is implemented as `aibag credentials`:

- Reads supported JSON, TOML, YAML, and `.env` files only under known selected agent roots.
- Finds common API key/token fields and model/model-list fields.
- Prints and serializes only a redacted preview.
- Stores real credential values only in memory and in an explicitly requested encrypted `.aibag` vault.

The internal schema records tool, source path, field path, provider hint, value kind, and value. This gives a future injector enough provenance to map one provider credential/model into another agent's schema. Injection is deliberately not implemented until conflict handling, schema adapters, and backups exist.

## Central Skills

`aibag skills` now discovers folders containing `SKILL.md`, hashes their complete contents, and merges identical copies found under multiple agents. `--store PATH` writes a content-addressed plaintext library and `index.json`.

Future work: reviewed install adapters, per-agent enable/disable state, conflict handling for same-name/different-content skills, and trust/signature metadata. Scripts in collected skills are not sandboxed.

Next schema should probably grow from this:

```toml
[[agents]]
key = "codex"
display_name = "Codex"
home_roots = [".codex"]
project_roots = [".codex", ".agents"]
skills = { home = [".codex/skills"], project = [".agents/skills"] }
settings = { home = [".codex/config.toml"], project = [] }
mcp = { home = [".codex/mcp.json"], project = [".mcp.json"] }
auth = { home = [".codex/auth.json"], project = [] }
```

That shape makes per-category selection easier than the current simple `global_skills_dir` / `project_skills_dir` / `detected_if_exists` fields.

## Research Notes

I did a quick web scan for similar tools and patterns on June 26, 2026.

- [Agent Settings Backup](https://github.com/Dicklesworthstone/agent_settings_backup_script) is the closest match. It backs up AI coding agent folders to git-versioned repositories and supports multiple agents. Useful idea: broad agent coverage and portable backup/import. Difference for My AI Bag: keep local encrypted packing and explicit preview as the core, not scheduled Git sync as the default.
- [Claude Code Backup](https://github.com/mcpware/claude-code-backup) focuses on automatic GitHub backup for Claude Code settings. Useful idea: one-command setup. Difference for My AI Bag: avoid background automation until the restore/security model is boringly clear.
- [placenameday/claude-code-backup](https://github.com/placenameday/claude-code-backup) advertises secret sanitization for `~/.claude`. Useful idea: later add a redacted-export mode that is safe for sharing bug reports or examples.
- [chezmoi encryption docs](https://chezmoi.io/user-guide/encryption/) are a good reference for mature dotfile secret workflows. Useful idea: secrets may be stored encrypted or sourced from password managers, not just copied raw forever.
- [yadm encryption docs](https://yadm.io/docs/encryption) are a good reminder that encrypted archives can coexist with Git, but plaintext should not be committed by accident.
- [yadm alternates](https://yadm.io/docs/alternates) and [yadm templates](https://yadm.io/docs/templates) are useful for future restore: config may need OS/host-specific variants instead of blindly restoring the same file everywhere.

## Near-Term Implementation Slices

1. Collapse to one user-facing binary.
   Keep one `aibag` command and move the Floem UI behind `aibag ui`. Initial support is implemented; later we can remove the compatibility `my-ai-bag` binary.

2. Add scan scopes.
   Model home/project/extra sources explicitly. Default to both home and current project. Ignore home `.agents` unless explicitly requested. Initial home/project/both support is implemented.

3. Upgrade catalog schema.
   Optional whole-folder roots are implemented. Still move from one skills path and one detection path to category-specific home/project paths.

4. Add per-item selection.
   UI should let users choose agent/category/path before `Pack Bag`. CLI supports first-pass category filters like `--include codex:skills,codex:mcp`; path-level selection is still pending.

5. Add restore planning before restore implementation.
   Restore is riskier than pack. It needs dry-run, conflict handling, backups of overwritten files, and clear mapping from archive paths to destination paths.

6. Decide sync later.
   Keep sync out of the default prototype. If added, make it explicit and probably backed by local encrypted archives first, not direct cloud upload.

7. Add credential and skill adapters.
   Build reviewed, tool-specific import/injection adapters on top of the encrypted credential vault and central skill index. Every write needs dry-run output and a backup of the destination file first.
