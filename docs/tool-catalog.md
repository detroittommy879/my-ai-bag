# Tool Catalog

The built-in catalog starts with the AI coding tools from the project brief and lives in `agents.toml`. Each entry has:

- a stable tool key
- a display name
- a global skills path relative to the scan/home root
- an optional project skills path relative to the project root
- a detected path relative to the scan/home root
- optional complete home/project root arrays for whole-folder mode

`qwen_code` uses `.qwen` as its detection path because the brief listed skill paths but did not include a separate detection path.

New tool entries should be added to `agents.toml` after review. The UI candidate queue is only a scratchpad for possible additions.

When `home_roots` is omitted, whole-folder mode uses `detected_if_exists`. When `project_roots` is omitted, it derives roots from `detected_if_exists` and the top-level project skills folder. Contributors can add explicit arrays when an agent uses different complete-folder locations.

The current schema is still intentionally small. A later version should split skills, settings, MCP, and auth paths into explicit home/project arrays so users can select exactly what to pack for each agent and credential parsing can use reviewed schemas instead of only heuristics.
