//! Privacy hard-off regression tests for telemetry / trace-upload resolvers.
//!
//! Integration tests (not `#[cfg(test)]` unit tests) so they compile against the
//! normal shell library — full `xai-grok-shell --lib` unit tests currently fail
//! upstream due to missing test helpers.
//!
//! These drive the **shipped** `Config::resolve_telemetry_mode` /
//! `resolve_trace_upload` entry points with env, config, and remote settings
//! that would re-enable product telemetry on upstream Grok Build.

use serial_test::serial;
use xai_grok_config_types::RemoteSettings;
use xai_grok_shell::agent::config::{Config, TelemetryMode};

#[test]
#[serial]
fn privacy_build_telemetry_mode_ignores_env_config_and_remote() {
    assert!(
        xai_grok_version::research_data_collection_forbidden(),
        "this fork must lock research collection off"
    );
    // SAFETY: #[serial]
    unsafe {
        std::env::set_var("GROK_TELEMETRY_ENABLED", "1");
    }
    let mut cfg = Config::default();
    cfg.features.telemetry = Some(TelemetryMode::Enabled);
    cfg.requirements.telemetry.pin(
        TelemetryMode::Enabled,
        xai_grok_shell::config::RequirementSource::Unknown,
    );
    cfg.remote_settings = Some(RemoteSettings {
        telemetry_enabled: Some(true),
        telemetry_mode: Some("enabled".into()),
        ..Default::default()
    });
    let r = cfg.resolve_telemetry_mode();
    assert!(
        r.value.is_disabled(),
        "privacy hard-off must win over env/config/remote: mode={:?}",
        r.value
    );
    unsafe {
        std::env::remove_var("GROK_TELEMETRY_ENABLED");
    }
}

#[test]
#[serial]
fn privacy_build_trace_upload_ignores_env_config_and_remote() {
    assert!(xai_grok_version::research_data_collection_forbidden());
    // SAFETY: #[serial]
    unsafe {
        std::env::set_var("GROK_TELEMETRY_ENABLED", "1");
        std::env::set_var("GROK_TELEMETRY_TRACE_UPLOAD", "1");
    }
    let mut cfg = Config::default();
    cfg.features.telemetry = Some(TelemetryMode::Enabled);
    cfg.telemetry.trace_upload = Some(true);
    cfg.requirements
        .trace_upload
        .pin(true, xai_grok_shell::config::RequirementSource::Unknown);
    cfg.remote_settings = Some(RemoteSettings {
        telemetry_enabled: Some(true),
        telemetry_mode: Some("enabled".into()),
        trace_upload_enabled: Some(true),
        ..Default::default()
    });
    let r = cfg.resolve_trace_upload();
    assert!(
        !r.value,
        "privacy hard-off must win over env/config/remote for trace upload"
    );
    assert!(!cfg.is_trace_upload_enabled());
    unsafe {
        std::env::remove_var("GROK_TELEMETRY_ENABLED");
        std::env::remove_var("GROK_TELEMETRY_TRACE_UPLOAD");
    }
}

// ── Session writeback ────────────────────────────────────────────────────
//
// Writeback pushes whole sessions (prompts, replies, tool calls, the absolute
// working directory) to the vendor session backend. These drive the shipped
// `StorageMode::resolve_privacy` — the one decision point every activation
// route funnels through — with the inputs that would turn it on upstream.

/// `GROK_STORAGE_MODE` is read inside the resolver, so each case sets it
/// explicitly rather than inheriting whatever the shell had.
fn with_storage_env<T>(value: Option<&str>, f: impl FnOnce() -> T) -> T {
    // SAFETY: every caller is #[serial]
    unsafe {
        match value {
            Some(v) => std::env::set_var("GROK_STORAGE_MODE", v),
            None => std::env::remove_var("GROK_STORAGE_MODE"),
        }
    }
    let out = f();
    unsafe {
        std::env::remove_var("GROK_STORAGE_MODE");
    }
    out
}

#[test]
#[serial]
fn privacy_writeback_refuses_flag_env_and_remote_without_opt_in() {
    use xai_grok_shell::config::StorageMode;
    let remote_on = RemoteSettings {
        writeback_enabled: Some(true),
        ..Default::default()
    };
    // The backend asking for it is not consent.
    with_storage_env(None, || {
        assert_eq!(
            StorageMode::resolve_privacy(None, Some(&remote_on), false),
            StorageMode::Local,
            "server-side writeback_enabled must not turn session upload on"
        );
        // Nor is a command-line flag, once the switch owns the decision.
        assert_eq!(
            StorageMode::resolve_privacy(Some("writeback"), None, false),
            StorageMode::Local,
            "--storage-mode writeback must not bypass the /config switch"
        );
    });
    // Nor an inherited environment variable.
    with_storage_env(Some("writeback"), || {
        assert_eq!(
            StorageMode::resolve_privacy(None, Some(&remote_on), false),
            StorageMode::Local,
            "GROK_STORAGE_MODE must not bypass the /config switch"
        );
    });
}

#[test]
#[serial]
fn privacy_writeback_opt_in_enables_it() {
    use xai_grok_shell::config::StorageMode;
    with_storage_env(None, || {
        assert_eq!(
            StorageMode::resolve_privacy(None, None, true),
            StorageMode::Writeback,
            "the /config switch alone must be enough to opt in"
        );
    });
}

#[test]
#[serial]
fn privacy_writeback_explicit_local_wins_over_opt_in() {
    use xai_grok_shell::config::StorageMode;
    // Asking for more privacy is never refused.
    with_storage_env(None, || {
        assert_eq!(
            StorageMode::resolve_privacy(Some("local"), None, true),
            StorageMode::Local
        );
    });
    with_storage_env(Some("local"), || {
        assert_eq!(
            StorageMode::resolve_privacy(None, None, true),
            StorageMode::Local
        );
    });
}

#[test]
#[serial]
fn privacy_writeback_unknown_storage_mode_value_stays_local() {
    use xai_grok_shell::config::StorageMode;
    let remote_on = RemoteSettings {
        writeback_enabled: Some(true),
        ..Default::default()
    };
    for junk in ["Writeback", "WRITEBACK", "writeback ", "remote", ""] {
        with_storage_env(Some(junk), || {
            assert_eq!(
                StorageMode::resolve_privacy(Some(junk), Some(&remote_on), false),
                StorageMode::Local,
                "unparsed storage-mode value {junk:?} must not fall through to writeback"
            );
        });
    }
}
