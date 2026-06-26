# Tool Catalog

The built-in catalog starts with the AI coding tools from the project brief and lives in `agents.toml`. Each entry has:

- a stable tool key
- a display name
- a global skills path relative to the scan/home root
- an optional project skills path relative to the project root
- a detected path relative to the scan/home root

`qwen_code` uses `.qwen` as its detection path because the brief listed skill paths but did not include a separate detection path.

New tool entries should be added to `agents.toml` after review. The UI candidate queue is only a scratchpad for possible additions.

The current schema is intentionally small. The planned next version will likely split skills, settings, MCP, and auth paths into explicit home/project arrays so users can select exactly what to pack for each agent.
