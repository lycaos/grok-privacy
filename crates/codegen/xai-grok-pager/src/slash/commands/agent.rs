//! `/agent` — open a new session on a named agent definition.
//!
//! The agent a session runs on is decided at session creation: the pager
//! stamps `_meta.agentProfile` and the shell gives it priority over
//! `[agent] name` and `GROK_AGENT`. That field was only ever filled from
//! `--agent` at launch, so choosing an agent meant restarting the process.
//! This drives the same field from inside a running session.

use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};

/// Open a new session on a named agent definition.
pub struct AgentCommand;

impl SlashCommand for AgentCommand {
    fn name(&self) -> &str {
        "agent"
    }

    fn aliases(&self) -> &[&str] {
        &["use-agent"]
    }

    fn description(&self) -> &str {
        "Open a new session on a named agent definition"
    }

    fn usage(&self) -> &str {
        "/agent <name>"
    }

    fn run(&self, _ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        let name = args.trim();
        if name.is_empty() {
            return CommandResult::Error(
                "Usage: /agent <name> — opens a new session running that agent. \
                 Browse and edit definitions with /agents; `grok --agent <name>` \
                 (or GROK_AGENT=<name>) does the same at launch."
                    .into(),
            );
        }
        CommandResult::Action(Action::OpenSessionWithAgent(name.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::model_state::ModelState;
    use crate::app::bundle::BundleState;
    use crate::settings::PagerLocalSnapshot;

    fn exec_ctx<'a>(models: &'a ModelState, bundle: &'a BundleState) -> CommandExecCtx<'a> {
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
    fn a_name_opens_a_session_on_that_agent() {
        let models = ModelState::default();
        let bundle = BundleState::default();
        let result = AgentCommand.run(&mut exec_ctx(&models, &bundle), "  reviewer  ");
        match result {
            CommandResult::Action(Action::OpenSessionWithAgent(name)) => {
                assert_eq!(name, "reviewer", "the name must be trimmed")
            }
            other => panic!("expected OpenSessionWithAgent, got {other:?}"),
        }
    }

    #[test]
    fn no_name_explains_the_usage() {
        let models = ModelState::default();
        let bundle = BundleState::default();
        match AgentCommand.run(&mut exec_ctx(&models, &bundle), "") {
            CommandResult::Error(msg) => {
                assert!(msg.contains("/agent <name>"));
                assert!(msg.contains("--agent"), "point at the launch-time equivalent");
            }
            other => panic!("expected a usage error, got {other:?}"),
        }
    }
}
