# Prompt templates (`/prompts`)

Browse, edit, and preset the LLM system prompts Grok injects into sessions and subagents. Defaults always come from the **installed binary** (embedded templates); your personal overrides and presets live only under `$GROK_HOME` (typically `~/.grok`), outside any project repository.

```
/prompts
```

Aliases: `/prompt`, `/system-prompts`.

You can also open the modal from the command palette.

---

## Why this exists

- **See what the model actually gets** — catalog entries include the base session system prompt, compaction prompt, apply-patch profile, and built-in subagent personas.
- **Customize without forking the binary** — save an override; leave a template alone and a future Grok update still refreshes its stock default.
- **Keep personal prompts off GitHub** — overrides and presets are written under `~/.grok`, never under the project tree. Rebuilds, updates, and force-pushes of the source repo do not ship your private prompt text.

---

## Defaults vs overrides

| Source | Where it lives | When it updates |
|--------|----------------|-----------------|
| **Default** | Embedded MiniJinja templates / constants in the binary | Every install or rebuild that ships new templates |
| **Override** | `~/.grok/prompts/<id>.md` | Only when you save from `/prompts` or edit the file |
| **Preset** | `~/.grok/prompt-presets/<name>/` | Snapshot of the current override set (apply / update / delete from the Presets tab) |

Resolution for each catalog id:

1. If a **non-empty** override file exists → use it.
2. Otherwise → use the binary default.

Saving an override that is **byte-identical** to the current default **clears** the override file (you stay on the stock template). Reset does the same: delete the override and fall back to default.

Override `GROK_HOME` to relocate all of this (and the rest of Grok’s user data). See [Configuration](05-configuration.md#file-locations).

---

## Catalog entries

### Session

| Title | File id (`~/.grok/prompts/<id>.md`) | Role |
|-------|--------------------------------------|------|
| Base system | `base-system` | Primary session system-prompt template (`prompt.md`). MiniJinja with `${{ }}` delimiters. |
| Subagent shell | `subagent-shell` | Base system template for subagents; persona body is appended (extend mode). |
| Apply-patch | `apply-patch` | Coding / apply-patch profile system template. |
| Compact system | `compact-system` | Short system prompt used after conversation compaction replaces full history (plain text, not MiniJinja). |

### Subagents

| Title | File id | Role |
|-------|---------|------|
| general-purpose | `subagent-general-purpose` | Persona body for the general-purpose subagent. |
| explore | `subagent-explore` | Persona body for the read-only explore subagent. |
| plan | `subagent-plan` | Persona body for the read-only plan / architect subagent. |

> **Not the same as** `prompt_file = ".grok/prompts/researcher.md"` on a **custom role** in `config.toml`. That path is project-scoped role configuration ([Subagents](16-subagents.md)). The `/prompts` catalog only covers the **built-in** session and stock subagent templates under `$GROK_HOME/prompts/`.

---

## Using the modal

The modal mirrors other browsers such as `/memory` and `/settings`: chrome tabs, list + preview, filter, fullscreen.

### Tabs

| Tab | Purpose |
|-----|---------|
| **Catalog** | List every built-in prompt, show preview, edit or reset overrides. |
| **Presets** | Named snapshots of the full override set; apply, update, delete, or clear all overrides. |

Switch tabs with `Tab` (or the chrome tab strip).

### Catalog shortcuts

| Key | Action |
|-----|--------|
| `↑` / `↓` | Move through the list |
| `/` | Focus the filter field (type to narrow; `Esc` leaves filter) |
| `Enter` | Edit the selected prompt **in the TUI** |
| `r` | Reset selected override to the binary default (confirm with `r` again) |
| `n` | Save the **current** override set as a new named preset |
| `Ctrl+F` | Toggle fullscreen |
| `Esc` | Close the modal (or cancel edit / filter / confirm) |

Mouse: click a row to select; scroll the list or preview; drag scrollbars. In the editor, **click places the caret** like the main prompt field.

### Editing a prompt (in-TUI)

| Key | Action |
|-----|--------|
| (type) | Edit multi-line body |
| `Ctrl+S` | Save override (or clear it if the body matches the default) |
| `Ctrl+E` | Suspend and open the file in `$VISUAL` / `$EDITOR` |
| `Esc` | Cancel and discard unsaved changes |
| Click | Place the blinking caret in the field |

The editor uses the same hardware terminal caret as the main prompt. After save, the catalog marks overridden entries; preview shows the effective body.

### Presets tab

| Key | Action |
|-----|--------|
| `↑` / `↓` | Navigate presets |
| `Enter` | Apply preset (writes its files into `~/.grok/prompts/`) |
| `n` | Name and save a new preset from the **current** overrides |
| `u` | Update the selected preset from the current overrides |
| `d` | Delete the selected preset (confirm with `d`) |
| `c` | Clear **all** active overrides (pure built-in defaults; confirm) |
| `Tab` | Back to Catalog |

Preset names accept `a-z`, `0-9`, `_`, `-`, and `.`. The active preset (if any) is shown in the modal chrome.

---

## On-disk layout

```
$GROK_HOME/                    # default: ~/.grok
  prompts/                     # active working overrides
    base-system.md
    subagent-explore.md
    ...
  prompt-presets/
    .active                    # optional: last applied preset name
    my-coding-style/
      base-system.md
      ...
```

You may edit these Markdown files outside Grok; the next session / render of `/prompts` picks them up. Prefer the modal when you want preview, reset-to-default, and preset apply/update in one place.

---

## Tips

- **Binary update, keep customizations** — only override what you changed; untouched ids track stock templates automatically.
- **Experiment safely** — save a preset before a large edit; apply another preset or clear overrides to roll back.
- **MiniJinja** — most templates use `${{ }}` / `${% %}` delimiters. Broken templates can break session start; keep a preset of a known-good set.
- **Privacy / open source** — do not commit `~/.grok/prompts/` into a public tree; it is user data, not project config.

---

## Related

- [Slash commands](04-slash-commands.md#prompts) — `/prompts` entry
- [Configuration · File locations](05-configuration.md#file-locations)
- [Subagents and Personas](16-subagents.md) — custom roles / `prompt_file` (separate from this catalog)
- [Project Rules (AGENTS.md)](12-project-rules.md) — project instructions layered with the system prompt
