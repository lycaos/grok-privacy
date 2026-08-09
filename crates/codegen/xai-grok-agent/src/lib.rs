//! Agent builder, definition parsing, and system prompt assembly.
//!
//! This crate extracts a first-class `Agent` type from `xai-grok-shell`.
//! An `Agent` bundles tools, system prompt, system-reminder policy,
//! compaction policy, and model configuration into a single, portable
//! object that any host can consume.

pub mod agent;
pub mod builder;
pub mod compaction;
pub mod config;
pub mod discovery;
pub mod error;
pub mod plugins;
pub mod prompt;
pub mod repo;
pub mod system_reminder;
pub mod timing;

pub use agent::Agent;
pub use builder::AgentBuilder;
pub use compaction::CompactionPolicy;
pub use config::AgentDefinition;
pub use config::preset_names;
pub use config::toolset_for_preset;
pub use config::workspace_grok_build_toolset;
pub use error::AgentBuildError;
pub use prompt::catalog::{
    EffectivePrompt, PromptCatalogError, PromptCategory, PromptDefinition, PromptId,
    PromptPresetInfo, active_preset_name, apply_preset, clear_active_preset, clear_all_overrides,
    default_body, delete_preset, effective_prompt, has_override, list_presets, list_prompts,
    materialize_for_edit, override_path, presets_root, prompts_dir, reset_override, resolve_body,
    save_override, save_preset, validate_preset_name,
};
pub use prompt::context::{DEFAULT_SYSTEM_PROMPT_LABEL, PromptContext};
pub use prompt::template::{
    apply_patch_template_source, base_template_source, subagent_template_source,
    COMPACT_SYSTEM_PROMPT,
};
pub use system_reminder::ReminderPolicy;
