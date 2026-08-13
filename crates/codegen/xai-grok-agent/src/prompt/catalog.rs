//! Catalog of user-overridable LLM prompt templates.
//!
//! # Defaults vs user data
//!
//! - **Defaults** always come from the embedded agent templates / constants
//!   shipped in the binary. They are never snapshotted to disk as the source
//!   of truth — a Grok Build update that changes templates updates the
//!   default automatically for any prompt without a user override.
//! - **User overrides** live only under `$GROK_HOME` (typically `~/.grok`),
//!   outside any project git tree:
//!   - `$GROK_HOME/prompts/<id>.md` — active working overrides
//!   - `$GROK_HOME/prompt-presets/<name>/` — named snapshots of overrides
//!
//! Rebuild / update / force-push of the source repo therefore never ships
//! personal prompts to GitHub; the installed binary still carries the stock
//! defaults, and local presets under `$GROK_HOME` are preserved.

use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::prompt::subagent_prompts;
use crate::prompt::template::{
    apply_patch_template_source, base_template_source, subagent_template_source,
    COMPACT_SYSTEM_PROMPT,
};

/// Stable catalog identifier (also the override file stem).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PromptId {
    BaseSystem,
    SubagentShell,
    ApplyPatch,
    CompactSystem,
    SubagentGeneralPurpose,
    SubagentExplore,
    SubagentPlan,
}

impl PromptId {
    pub const ALL: &[PromptId] = &[
        PromptId::BaseSystem,
        PromptId::SubagentShell,
        PromptId::ApplyPatch,
        PromptId::CompactSystem,
        PromptId::SubagentGeneralPurpose,
        PromptId::SubagentExplore,
        PromptId::SubagentPlan,
    ];

    /// File-safe id used as `~/.grok/prompts/<id>.md`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BaseSystem => "base-system",
            Self::SubagentShell => "subagent-shell",
            Self::ApplyPatch => "apply-patch",
            Self::CompactSystem => "compact-system",
            Self::SubagentGeneralPurpose => "subagent-general-purpose",
            Self::SubagentExplore => "subagent-explore",
            Self::SubagentPlan => "subagent-plan",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|id| id.as_str() == s)
    }

    pub fn category(self) -> PromptCategory {
        match self {
            Self::BaseSystem | Self::SubagentShell | Self::ApplyPatch | Self::CompactSystem => {
                PromptCategory::Session
            }
            Self::SubagentGeneralPurpose | Self::SubagentExplore | Self::SubagentPlan => {
                PromptCategory::Subagents
            }
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::BaseSystem => "Base system",
            Self::SubagentShell => "Subagent shell",
            Self::ApplyPatch => "Apply-patch",
            Self::CompactSystem => "Compact system",
            Self::SubagentGeneralPurpose => "general-purpose",
            Self::SubagentExplore => "explore",
            Self::SubagentPlan => "plan",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::BaseSystem => {
                "Primary session system-prompt template (prompt.md). MiniJinja with ${{ }} delimiters."
            }
            Self::SubagentShell => {
                "Base system template for subagents. Persona body is appended (extend mode)."
            }
            Self::ApplyPatch => {
                "Coding / apply-patch profile system template used by the Codex-style agent."
            }
            Self::CompactSystem => {
                "Short system prompt used after conversation compaction replaces full history."
            }
            Self::SubagentGeneralPurpose => {
                "Persona body for the general-purpose subagent (appended to the subagent shell)."
            }
            Self::SubagentExplore => {
                "Persona body for the read-only explore subagent."
            }
            Self::SubagentPlan => {
                "Persona body for the read-only plan / architect subagent."
            }
        }
    }

    /// Whether the body is a MiniJinja template (`${{ }}` / `${% %}`).
    pub fn is_minijinja_template(self) -> bool {
        match self {
            Self::BaseSystem
            | Self::SubagentShell
            | Self::ApplyPatch
            | Self::SubagentGeneralPurpose
            | Self::SubagentExplore
            | Self::SubagentPlan => true,
            Self::CompactSystem => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PromptCategory {
    Session,
    Subagents,
}

impl PromptCategory {
    pub fn label(self) -> &'static str {
        match self {
            Self::Session => "Session",
            Self::Subagents => "Subagents",
        }
    }

    pub const ALL: &[PromptCategory] = &[PromptCategory::Session, PromptCategory::Subagents];
}

/// Static definition shown in the `/prompts` catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PromptDefinition {
    pub id: PromptId,
    pub title: &'static str,
    pub description: &'static str,
    pub category: PromptCategory,
    pub is_minijinja_template: bool,
}

impl From<PromptId> for PromptDefinition {
    fn from(id: PromptId) -> Self {
        Self {
            id,
            title: id.title(),
            description: id.description(),
            category: id.category(),
            is_minijinja_template: id.is_minijinja_template(),
        }
    }
}

/// Resolved prompt body plus override metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectivePrompt {
    pub id: PromptId,
    pub body: String,
    pub is_override: bool,
    pub override_path: Option<PathBuf>,
}

#[derive(Debug, Error)]
pub enum PromptCatalogError {
    #[error("unknown prompt id: {0}")]
    UnknownId(String),
    #[error("could not resolve GROK_HOME for prompt overrides")]
    NoGrokHome,
    #[error("invalid preset name: {0}")]
    InvalidPresetName(String),
    #[error("preset not found: {0}")]
    PresetNotFound(String),
    #[error("preset already exists: {0}")]
    PresetExists(String),
    #[error("io error for prompt override: {0}")]
    Io(#[from] std::io::Error),
}

/// Metadata for a named prompt preset under `$GROK_HOME/prompt-presets/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptPresetInfo {
    pub name: String,
    /// Number of override files in the preset (catalog ids only).
    pub override_count: usize,
    pub is_active: bool,
}

/// List all catalog definitions in display order (grouped by category).
pub fn list_prompts() -> Vec<PromptDefinition> {
    PromptId::ALL.iter().copied().map(PromptDefinition::from).collect()
}

/// Built-in default body for `id` (always fresh; never reads overrides).
pub fn default_body(id: PromptId) -> String {
    match id {
        PromptId::BaseSystem => base_template_source().to_string(),
        PromptId::SubagentShell => subagent_template_source().to_string(),
        PromptId::ApplyPatch => apply_patch_template_source().to_string(),
        PromptId::CompactSystem => COMPACT_SYSTEM_PROMPT.to_string(),
        PromptId::SubagentGeneralPurpose => subagent_prompts::GENERAL_PURPOSE_PROMPT.to_string(),
        PromptId::SubagentExplore => subagent_prompts::EXPLORE_PROMPT.to_string(),
        PromptId::SubagentPlan => subagent_prompts::PLAN_PROMPT.to_string(),
    }
}

/// Optional test-only home override. `grok_home()` is a process-wide
/// `OnceLock`, so tests that need isolation set this instead of `GROK_HOME`.
#[cfg(test)]
static TEST_PROMPTS_HOME: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

/// Directory that holds user prompt overrides (`$GROK_HOME/prompts`).
pub fn prompts_dir() -> Option<PathBuf> {
    #[cfg(test)]
    {
        if let Ok(guard) = TEST_PROMPTS_HOME.lock()
            && let Some(ref home) = *guard
        {
            return Some(home.join("prompts"));
        }
    }
    xai_grok_config::user_grok_home().map(|h| h.join("prompts"))
}

/// Override file path for `id`, or `None` when GROK_HOME is unavailable.
pub fn override_path(id: PromptId) -> Option<PathBuf> {
    prompts_dir().map(|d| d.join(format!("{}.md", id.as_str())))
}

/// Override path under an explicit home (tests).
pub fn override_path_in(home: &Path, id: PromptId) -> PathBuf {
    home.join("prompts").join(format!("{}.md", id.as_str()))
}

/// Load override body if the file exists and is non-empty after trim.
pub fn load_override(id: PromptId) -> Result<Option<String>, PromptCatalogError> {
    let Some(path) = override_path(id) else {
        return Ok(None);
    };
    load_override_at(&path)
}

fn load_override_at(path: &Path) -> Result<Option<String>, PromptCatalogError> {
    match std::fs::read_to_string(path) {
        Ok(body) => {
            if body.trim().is_empty() {
                Ok(None)
            } else {
                Ok(Some(body))
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Effective body = override if present, else default.
pub fn effective_prompt(id: PromptId) -> Result<EffectivePrompt, PromptCatalogError> {
    let path = override_path(id);
    if let Some(ref p) = path
        && let Some(body) = load_override_at(p)?
    {
        return Ok(EffectivePrompt {
            id,
            body,
            is_override: true,
            override_path: Some(p.clone()),
        });
    }
    Ok(EffectivePrompt {
        id,
        body: default_body(id),
        is_override: false,
        override_path: path,
    })
}

/// Convenience: body only (falls back to default on IO / missing home).
pub fn resolve_body(id: PromptId) -> String {
    match effective_prompt(id) {
        Ok(ep) => ep.body,
        Err(e) => {
            tracing::warn!(error = %e, prompt = id.as_str(), "prompt override resolve failed; using default");
            default_body(id)
        }
    }
}

/// Whether an active (non-empty) override is in effect for `id`.
///
/// Matches [`effective_prompt`]: a present but empty/whitespace file is
/// treated as no override.
pub fn has_override(id: PromptId) -> bool {
    matches!(load_override(id), Ok(Some(_)))
}

/// Write `body` to the override file, creating `$GROK_HOME/prompts` as needed.
///
/// If `body` matches the built-in default (trimmed), the override is **removed**
/// instead so future binary updates keep delivering the stock template.
pub fn save_override(id: PromptId, body: &str) -> Result<PathBuf, PromptCatalogError> {
    let path = override_path(id).ok_or(PromptCatalogError::NoGrokHome)?;
    if body.trim() == default_body(id).trim() {
        reset_override(id)?;
        // Still return the canonical path (may not exist after clear).
        return Ok(path);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, body)?;
    // Keep the active preset in sync when one is selected.
    if let Some(name) = active_preset_name()? {
        let preset_path = preset_dir(&name)?.join(format!("{}.md", id.as_str()));
        if let Some(parent) = preset_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&preset_path, body)?;
    }
    Ok(path)
}

/// Materialize the default into the override path (for `$EDITOR` open).
///
/// If an override already exists, returns its path unchanged.
pub fn materialize_for_edit(id: PromptId) -> Result<PathBuf, PromptCatalogError> {
    if let Some(path) = override_path(id)
        && path.is_file()
    {
        return Ok(path);
    }
    // Force-write even if equal to default (editor needs a path on disk).
    let dir = prompts_dir().ok_or(PromptCatalogError::NoGrokHome)?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.md", id.as_str()));
    std::fs::write(&path, default_body(id))?;
    Ok(path)
}

/// Delete the override file. No-op if absent.
pub fn reset_override(id: PromptId) -> Result<(), PromptCatalogError> {
    let Some(path) = override_path(id) else {
        return Err(PromptCatalogError::NoGrokHome);
    };
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    // Mirror into active preset when set.
    if let Some(name) = active_preset_name()? {
        let preset_path = preset_dir(&name)?.join(format!("{}.md", id.as_str()));
        let _ = std::fs::remove_file(preset_path);
    }
    Ok(())
}

/// Remove every active catalog override (pure built-in defaults).
pub fn clear_all_overrides() -> Result<(), PromptCatalogError> {
    for id in PromptId::ALL {
        let path = match override_path(*id) {
            Some(p) => p,
            None => return Err(PromptCatalogError::NoGrokHome),
        };
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    set_active_preset_name(None)?;
    Ok(())
}

// ── Presets ────────────────────────────────────────────────────────────────

const PRESETS_SUBDIR: &str = "prompt-presets";
const ACTIVE_MARKER: &str = ".active";

/// `$GROK_HOME/prompt-presets`.
pub fn presets_root() -> Option<PathBuf> {
    #[cfg(test)]
    {
        if let Ok(guard) = TEST_PROMPTS_HOME.lock()
            && let Some(ref home) = *guard
        {
            return Some(home.join(PRESETS_SUBDIR));
        }
    }
    xai_grok_config::user_grok_home().map(|h| h.join(PRESETS_SUBDIR))
}

fn presets_root_required() -> Result<PathBuf, PromptCatalogError> {
    presets_root().ok_or(PromptCatalogError::NoGrokHome)
}

fn preset_dir(name: &str) -> Result<PathBuf, PromptCatalogError> {
    let name = validate_preset_name(name)?;
    Ok(presets_root_required()?.join(name))
}

/// Accept `a-zA-Z0-9._-` only, 1..=64 chars, not `.` / `..`.
pub fn validate_preset_name(name: &str) -> Result<String, PromptCatalogError> {
    let name = name.trim();
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.len() > 64
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        return Err(PromptCatalogError::InvalidPresetName(name.to_string()));
    }
    Ok(name.to_string())
}

/// Name of the currently active preset, if any.
pub fn active_preset_name() -> Result<Option<String>, PromptCatalogError> {
    let root = match presets_root() {
        Some(r) => r,
        None => return Ok(None),
    };
    let marker = root.join(ACTIVE_MARKER);
    match std::fs::read_to_string(&marker) {
        Ok(s) => {
            let t = s.trim();
            if t.is_empty() {
                Ok(None)
            } else {
                Ok(Some(t.to_string()))
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn set_active_preset_name(name: Option<&str>) -> Result<(), PromptCatalogError> {
    let root = presets_root_required()?;
    std::fs::create_dir_all(&root)?;
    let marker = root.join(ACTIVE_MARKER);
    match name {
        Some(n) => {
            let n = validate_preset_name(n)?;
            std::fs::write(marker, format!("{n}\n"))?;
        }
        None => match std::fs::remove_file(marker) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        },
    }
    Ok(())
}

fn count_preset_overrides(dir: &Path) -> usize {
    PromptId::ALL
        .iter()
        .filter(|id| {
            let p = dir.join(format!("{}.md", id.as_str()));
            p.is_file()
                && std::fs::read_to_string(&p)
                    .map(|b| !b.trim().is_empty())
                    .unwrap_or(false)
        })
        .count()
}

/// List named presets (sorted), marking the active one.
pub fn list_presets() -> Result<Vec<PromptPresetInfo>, PromptCatalogError> {
    let root = match presets_root() {
        Some(r) => r,
        None => return Ok(Vec::new()),
    };
    let active = active_preset_name()?;
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(&root) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        if validate_preset_name(name).is_err() {
            continue;
        }
        out.push(PromptPresetInfo {
            name: name.to_string(),
            override_count: count_preset_overrides(&path),
            is_active: active.as_deref() == Some(name),
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Snapshot the **current active overrides** into a named preset and mark it active.
///
/// Only override files are copied — built-in defaults are never duplicated.
pub fn save_preset(name: &str) -> Result<(), PromptCatalogError> {
    let name = validate_preset_name(name)?;
    let dest = preset_dir(&name)?;
    if dest.exists() {
        // Overwrite: clear previous catalog files first.
        for id in PromptId::ALL {
            let _ = std::fs::remove_file(dest.join(format!("{}.md", id.as_str())));
        }
    } else {
        std::fs::create_dir_all(&dest)?;
    }
    let src = prompts_dir().ok_or(PromptCatalogError::NoGrokHome)?;
    for id in PromptId::ALL {
        let from = src.join(format!("{}.md", id.as_str()));
        if let Ok(body) = std::fs::read_to_string(&from)
            && !body.trim().is_empty()
        {
            std::fs::write(dest.join(format!("{}.md", id.as_str())), body)?;
        }
    }
    set_active_preset_name(Some(&name))?;
    Ok(())
}

/// Replace active overrides with a preset's snapshot and mark it active.
pub fn apply_preset(name: &str) -> Result<(), PromptCatalogError> {
    let name = validate_preset_name(name)?;
    let src = preset_dir(&name)?;
    if !src.is_dir() {
        return Err(PromptCatalogError::PresetNotFound(name));
    }
    let dest = prompts_dir().ok_or(PromptCatalogError::NoGrokHome)?;
    std::fs::create_dir_all(&dest)?;
    // Clear working overrides first.
    for id in PromptId::ALL {
        let _ = std::fs::remove_file(dest.join(format!("{}.md", id.as_str())));
    }
    for id in PromptId::ALL {
        let from = src.join(format!("{}.md", id.as_str()));
        if let Ok(body) = std::fs::read_to_string(&from)
            && !body.trim().is_empty()
        {
            std::fs::write(dest.join(format!("{}.md", id.as_str())), body)?;
        }
    }
    set_active_preset_name(Some(&name))?;
    Ok(())
}

/// Delete a named preset. If it was active, clears the active marker only
/// (working overrides under `prompts/` are left as-is).
pub fn delete_preset(name: &str) -> Result<(), PromptCatalogError> {
    let name = validate_preset_name(name)?;
    let dir = preset_dir(&name)?;
    if !dir.exists() {
        return Err(PromptCatalogError::PresetNotFound(name));
    }
    std::fs::remove_dir_all(&dir)?;
    if active_preset_name()?.as_deref() == Some(name.as_str()) {
        set_active_preset_name(None)?;
    }
    Ok(())
}

/// Detach from any preset label without wiping working overrides.
pub fn clear_active_preset() -> Result<(), PromptCatalogError> {
    set_active_preset_name(None)
}

/// Test helpers: operate under an explicit home directory.
#[cfg(test)]
pub mod test_support {
    use super::*;

    pub fn save_override_in(home: &Path, id: PromptId, body: &str) -> PathBuf {
        let dir = home.join("prompts");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{}.md", id.as_str()));
        std::fs::write(&path, body).unwrap();
        path
    }

    pub fn load_override_in(home: &Path, id: PromptId) -> Option<String> {
        let path = override_path_in(home, id);
        load_override_at(&path).unwrap()
    }

    pub fn resolve_body_in(home: &Path, id: PromptId) -> String {
        load_override_in(home, id).unwrap_or_else(|| default_body(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use tempfile::TempDir;

    fn with_temp_grok_home<R>(f: impl FnOnce(&Path) -> R) -> R {
        let tmp = TempDir::new().unwrap();
        // Isolate from process-wide `grok_home()` OnceLock via test hook.
        *TEST_PROMPTS_HOME.lock().unwrap() = Some(tmp.path().to_path_buf());
        let result = f(tmp.path());
        *TEST_PROMPTS_HOME.lock().unwrap() = None;
        result
    }

    #[test]
    fn list_prompts_covers_all_ids() {
        let list = list_prompts();
        assert_eq!(list.len(), PromptId::ALL.len());
        assert_eq!(list[0].id, PromptId::BaseSystem);
        assert!(list.iter().any(|d| d.id == PromptId::SubagentExplore));
    }

    #[test]
    fn parse_round_trips() {
        for id in PromptId::ALL {
            assert_eq!(PromptId::parse(id.as_str()), Some(*id));
        }
        assert_eq!(PromptId::parse("nope"), None);
    }

    #[test]
    fn default_bodies_non_empty() {
        for id in PromptId::ALL {
            let body = default_body(*id);
            assert!(
                !body.trim().is_empty(),
                "default body empty for {}",
                id.as_str()
            );
        }
    }

    #[test]
    #[serial]
    fn save_load_reset_override() {
        with_temp_grok_home(|home| {
            let id = PromptId::CompactSystem;
            assert!(!has_override(id));
            let ep = effective_prompt(id).unwrap();
            assert!(!ep.is_override);
            assert_eq!(ep.body, COMPACT_SYSTEM_PROMPT);

            let custom = "You are a custom compact agent.";
            let path = save_override(id, custom).unwrap();
            assert_eq!(
                path.file_name().and_then(|n| n.to_str()),
                Some("compact-system.md")
            );
            assert!(path.is_file(), "override written under {}", path.display());
            let _ = home; // GROK_HOME points here; path may be re-resolved
            assert!(has_override(id));

            let ep = effective_prompt(id).unwrap();
            assert!(ep.is_override);
            assert_eq!(ep.body, custom);
            assert_eq!(resolve_body(id), custom);

            reset_override(id).unwrap();
            assert!(!has_override(id));
            assert_eq!(resolve_body(id), COMPACT_SYSTEM_PROMPT);
        });
    }

    #[test]
    #[serial]
    fn materialize_for_edit_creates_file_once() {
        with_temp_grok_home(|_| {
            let id = PromptId::BaseSystem;
            let path1 = materialize_for_edit(id).unwrap();
            assert!(path1.is_file());
            let body1 = std::fs::read_to_string(&path1).unwrap();
            assert_eq!(body1, default_body(id));

            // Second call keeps existing content (user may have edited).
            std::fs::write(&path1, "edited by user").unwrap();
            let path2 = materialize_for_edit(id).unwrap();
            assert_eq!(path1, path2);
            assert_eq!(std::fs::read_to_string(&path2).unwrap(), "edited by user");
        });
    }

    #[test]
    #[serial]
    fn empty_override_falls_through_to_default() {
        with_temp_grok_home(|_| {
            let id = PromptId::CompactSystem;
            // Force-write empty via direct IO (save_override clears defaults only).
            let path = override_path(id).unwrap();
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, "   \n  ").unwrap();
            let ep = effective_prompt(id).unwrap();
            assert!(!ep.is_override);
            assert_eq!(ep.body, COMPACT_SYSTEM_PROMPT);
        });
    }

    #[test]
    #[serial]
    fn save_equal_to_default_clears_override() {
        with_temp_grok_home(|_| {
            let id = PromptId::CompactSystem;
            save_override(id, "custom").unwrap();
            assert!(has_override(id));
            save_override(id, COMPACT_SYSTEM_PROMPT).unwrap();
            assert!(!has_override(id));
            assert_eq!(resolve_body(id), COMPACT_SYSTEM_PROMPT);
        });
    }

    #[test]
    #[serial]
    fn preset_save_apply_delete() {
        with_temp_grok_home(|_| {
            let id = PromptId::CompactSystem;
            save_override(id, "preset-body-a").unwrap();
            save_preset("alpha").unwrap();
            assert_eq!(active_preset_name().unwrap().as_deref(), Some("alpha"));

            clear_all_overrides().unwrap();
            assert!(!has_override(id));
            assert!(active_preset_name().unwrap().is_none());

            apply_preset("alpha").unwrap();
            assert_eq!(resolve_body(id), "preset-body-a");
            assert_eq!(active_preset_name().unwrap().as_deref(), Some("alpha"));

            let list = list_presets().unwrap();
            assert_eq!(list.len(), 1);
            assert!(list[0].is_active);
            assert_eq!(list[0].override_count, 1);

            delete_preset("alpha").unwrap();
            assert!(list_presets().unwrap().is_empty());
            // Working overrides remain after delete.
            assert_eq!(resolve_body(id), "preset-body-a");
        });
    }

    #[test]
    fn reject_bad_preset_names() {
        assert!(validate_preset_name("").is_err());
        assert!(validate_preset_name("..").is_err());
        assert!(validate_preset_name("has space").is_err());
        assert!(validate_preset_name("ok-name_1").is_ok());
    }
}
