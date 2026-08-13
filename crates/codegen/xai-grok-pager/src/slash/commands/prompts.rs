//! `/prompts` — open the LLM prompt templates browser.

use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};

/// Open the prompts catalog modal.
pub struct PromptsCommand;

impl SlashCommand for PromptsCommand {
    fn name(&self) -> &str {
        "prompts"
    }

    fn aliases(&self) -> &[&str] {
        &["prompt", "system-prompts"]
    }

    fn description(&self) -> &str {
        "Browse and edit system / subagent prompt templates"
    }

    fn usage(&self) -> &str {
        "/prompts"
    }

    fn run(&self, _ctx: &mut CommandExecCtx, _args: &str) -> CommandResult {
        CommandResult::Action(Action::OpenPromptsModal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::model_state::ModelState;
    use crate::app::bundle::BundleState;
    use crate::settings::PagerLocalSnapshot;

    fn exec_ctx<'a>(
        models: &'a ModelState,
        bundle: &'a BundleState,
    ) -> CommandExecCtx<'a> {
        CommandExecCtx {
            models,
            session_id: None,
            bundle_state: bundle,
            screen_mode: crate::app::ScreenMode::Inline,
            billing_surface_visible: true,
            usage_command_visible: true,
            pager_state: PagerLocalSnapshot::default(),
        }
    }

    #[test]
    fn opens_prompts_modal() {
        let models = ModelState::default();
        let bundle = BundleState::default();
        let result = PromptsCommand.run(&mut exec_ctx(&models, &bundle), "");
        assert!(matches!(
            result,
            CommandResult::Action(Action::OpenPromptsModal)
        ));
    }
}
