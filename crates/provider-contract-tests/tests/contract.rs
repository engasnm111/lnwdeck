//! Contract suite shared by every built-in provider adapter.
//!
//! These tests codify the standardized-data invariants that the quota core,
//! dashboard, and widget rely on: stable identifiers, graceful detection and
//! collection (never panics), and — whenever a quota report is produced —
//! privacy-safe payloads with no fabricated percentages.

use lnwdeck_provider_claude::ClaudeAdapter;
use lnwdeck_provider_codex::CodexAdapter;
use lnwdeck_provider_copilot::CopilotAdapter;
use lnwdeck_provider_cursor::CursorAdapter;
use lnwdeck_provider_gemini::GeminiAdapter;
use lnwdeck_provider_grok::GrokAdapter;
use lnwdeck_provider_kiro::KiroAdapter;
use lnwdeck_provider_ollama::OllamaAdapter;
use lnwdeck_provider_opencode::OpenCodeAdapter;
use lnwdeck_provider_openrouter::OpenRouterAdapter;
use lnwdeck_provider_runtime::{
    AdapterHealthStatus, AdapterRegistry, ChannelSupport, ProviderAdapter, NOT_SUPPORTED,
};
use lnwdeck_security::PrivacyGuard;
use std::collections::HashSet;

fn builtin_adapters() -> Vec<Box<dyn ProviderAdapter>> {
    vec![
        Box::new(ClaudeAdapter::new()),
        Box::new(CodexAdapter::new()),
        Box::new(CopilotAdapter::new()),
        Box::new(CursorAdapter::new()),
        Box::new(GeminiAdapter::new()),
        Box::new(GrokAdapter::new()),
        Box::new(KiroAdapter::new()),
        Box::new(OllamaAdapter::new()),
        Box::new(OpenCodeAdapter::new(&[0u8; 32])),
        Box::new(OpenRouterAdapter::new()),
    ]
}

#[test]
fn every_adapter_has_a_stable_unique_identifier() {
    let adapters = builtin_adapters();
    let mut ids = HashSet::new();
    for adapter in &adapters {
        let id = adapter.id();
        assert!(!id.is_empty(), "provider id must not be empty");
        assert!(
            ids.insert(id),
            "provider ids must be unique: duplicate {id}"
        );
    }
    assert_eq!(adapters.len(), 10, "all built-in providers registered");
}

#[test]
fn every_adapter_has_a_display_name() {
    for adapter in builtin_adapters() {
        assert!(
            !adapter.name().is_empty(),
            "provider {} must have a display name",
            adapter.id()
        );
    }
}

#[test]
fn detection_never_panics_and_is_sanitized() {
    for adapter in builtin_adapters() {
        let result = adapter.detect().expect("detect must not fail");
        assert_eq!(result.provider_id, adapter.id(), "detection provider id");
        assert_eq!(
            result.display_name,
            adapter.name(),
            "detection display name"
        );
        assert!(
            !result.detection_error_code.contains('\\')
                && !result.detection_error_code.contains("Bearer"),
            "detection error codes are sanitized"
        );
        assert!(
            !result.source_type.contains('\\'),
            "source_type carries no paths"
        );
    }
}

#[test]
fn quota_collection_never_panics_and_reports_canonical_id() {
    for adapter in builtin_adapters() {
        let result = adapter.collect_quota_report();
        assert_eq!(
            result.outcome.provider_id,
            adapter.id(),
            "quota outcome provider id for {}",
            adapter.id()
        );
        assert!(
            !result.outcome.error_code.contains('\\'),
            "quota error codes are sanitized"
        );
    }
}

#[test]
fn quota_reports_are_privacy_safe() {
    for adapter in builtin_adapters() {
        let result = adapter.collect_quota_report();
        if let Some(report) = result.report {
            assert_eq!(
                report.provider_id,
                adapter.id(),
                "quota report must use the canonical provider id"
            );
            assert!(
                PrivacyGuard::validate_quota_report(&report).is_ok(),
                "quota report for {} must pass the privacy guard",
                adapter.id()
            );
        }
    }
}

#[test]
fn quota_reports_never_fabricate_percentages() {
    for adapter in builtin_adapters() {
        let result = adapter.collect_quota_report();
        if let Some(report) = result.report {
            for window in &report.windows {
                window.check_invariants().unwrap_or_else(|err| {
                    panic!(
                        "{} window {} is internally inconsistent: {err}",
                        adapter.id(),
                        window.window_key
                    )
                });
                // A percentage is allowed without an absolute limit only when
                // the provider published the percentage itself; in that case no
                // token count is invented, so `used` stays zero.
                if window.limit.is_none() && window.used_percent.is_some() {
                    assert_eq!(
                        window.used, 0,
                        "{} window {} publishes a percentage, so it must not also carry an invented used count",
                        adapter.id(),
                        window.window_key
                    );
                }
                if window.limit.is_none() && window.used > 0 {
                    assert_eq!(
                        window.used_percent,
                        None,
                        "{} window {} counted usage without a limit; a percentage would be fabricated",
                        adapter.id(),
                        window.window_key
                    );
                    assert_eq!(
                        window.remaining_percent,
                        None,
                        "{} window {} counted usage without a limit; a remaining percentage would be fabricated",
                        adapter.id(),
                        window.window_key
                    );
                }
                if window.is_unlimited {
                    assert_eq!(window.limit, None, "unlimited window has no limit");
                    assert_eq!(window.used_percent, None);
                }
                if let Some(percent) = window.remaining_percent {
                    assert!(
                        (0.0..=100.0).contains(&percent),
                        "remaining_percent in range"
                    );
                }
            }
        }
    }
}

#[test]
fn error_quota_reports_carry_no_windows_or_data() {
    for adapter in builtin_adapters() {
        let result = adapter.collect_quota_report();
        if result.outcome.status.is_error() {
            assert!(
                result.report.is_none()
                    || result.report.as_ref().is_some_and(|r| r.windows.is_empty()),
                "{} error report must not fabricate windows",
                adapter.id()
            );
            assert!(
                !result.outcome.error_code.is_empty(),
                "{} error status requires an error code",
                adapter.id()
            );
        }
    }
}

#[test]
fn success_outcomes_carry_a_usable_report() {
    for adapter in builtin_adapters() {
        let result = adapter.collect_quota_report();
        if !result.outcome.status.is_error() && result.outcome.windows_collected > 0 {
            let report = result
                .report
                .expect("windows collected implies a report is present");
            assert!(report.is_usable(), "{} report is usable", adapter.id());
        }
    }
}

#[test]
fn every_descriptor_is_internally_consistent() {
    for adapter in builtin_adapters() {
        let descriptor = adapter.descriptor();
        descriptor
            .check()
            .unwrap_or_else(|err| panic!("descriptor for {} is invalid: {err}", adapter.id()));
        assert_eq!(
            descriptor.id,
            adapter.id(),
            "id must come from the descriptor"
        );
        assert_eq!(descriptor.display_name, adapter.name());
        assert!(
            !descriptor.vendor.trim().is_empty(),
            "{} must name its vendor",
            adapter.id()
        );
    }
}

#[test]
fn all_adapters_register_in_one_registry_without_id_collisions() {
    let mut registry = AdapterRegistry::new();
    for adapter in builtin_adapters() {
        let id = adapter.id();
        registry
            .register(adapter)
            .unwrap_or_else(|err| panic!("registering {id} failed: {err}"));
    }
    assert_eq!(registry.len(), 10, "all built-in providers registered");
    for descriptor in registry.descriptors() {
        assert_eq!(
            registry.display_name(descriptor.id),
            Some(descriptor.display_name),
            "the registry resolves the display name for {}",
            descriptor.id
        );
        assert!(registry.rank(descriptor.id).is_some());
        assert!(registry.find(descriptor.id).is_some());
    }
    assert_eq!(registry.display_name("not_a_provider"), None);
}

/// The defect this suite exists for: an adapter must not report a successful
/// collection while returning nothing. Either the descriptor declares the
/// channel unsupported (and the attempt is recorded as NOT_SUPPORTED), or the
/// channel returns data or a sanitized error code.
#[test]
fn unsupported_channels_never_produce_a_successful_empty_collection() {
    for adapter in builtin_adapters() {
        let descriptor = adapter.descriptor();
        let usage = adapter.collect_usage_with_cursor(None);

        if !descriptor.usage_support.is_supported() {
            assert!(
                usage.batch.is_none(),
                "{} declares no usage support but produced a batch",
                adapter.id()
            );
            assert_eq!(
                usage.outcome.error_code,
                NOT_SUPPORTED,
                "{} must record an unsupported usage attempt explicitly",
                adapter.id()
            );
            continue;
        }

        if let Some(batch) = &usage.batch {
            assert!(
                usage.outcome.error_code.is_empty(),
                "{} returned a batch together with error {}",
                adapter.id(),
                usage.outcome.error_code
            );
            assert_eq!(
                usage.outcome.events_normalized as usize,
                batch.events.len(),
                "{} must report the number of events it normalized",
                adapter.id()
            );
        } else {
            assert!(
                !usage.outcome.error_code.is_empty(),
                "{} produced neither a batch nor an error code",
                adapter.id()
            );
        }
    }
}

#[test]
fn quota_channels_match_their_declared_support() {
    for adapter in builtin_adapters() {
        let descriptor = adapter.descriptor();
        let result = adapter.collect_quota_report();

        if !descriptor.quota_support.is_supported() {
            assert!(result.report.is_none());
            assert_eq!(
                result.outcome.error_code,
                NOT_SUPPORTED,
                "{} declares no quota support",
                adapter.id()
            );
            continue;
        }

        match &result.report {
            Some(report) => assert!(
                report.is_usable(),
                "{} returned an unusable report",
                adapter.id()
            ),
            None => assert!(
                !result.outcome.error_code.is_empty(),
                "{} returned no report and no error code",
                adapter.id()
            ),
        }
    }
}

/// A provider whose source is absent must never look healthy. On a machine
/// without a given tool installed the adapter reports Degraded, Unhealthy,
/// NotConfigured or Unsupported, never Healthy.
#[test]
fn health_is_never_healthy_without_a_readable_source() {
    for adapter in builtin_adapters() {
        let detection = adapter.detect().expect("detect");
        let health = adapter.health_check();
        if !detection.detected {
            assert_ne!(
                health.status,
                AdapterHealthStatus::Healthy,
                "{} claims health while its source is not detected ({}), message: {}",
                adapter.id(),
                detection.permission_state,
                health.message
            );
        }
        assert!(
            !health.message.contains(":\\") && !health.message.contains("Users"),
            "{} health message must not leak a path: {}",
            adapter.id(),
            health.message
        );
    }
}

/// Credential-backed adapters must not perform any network request before the
/// user has stored a key: they report NOT_CONFIGURED instead.
#[test]
fn credential_adapters_stay_inert_until_configured() {
    for adapter in builtin_adapters() {
        let descriptor = adapter.descriptor();
        if !descriptor.needs_credentials() {
            continue;
        }
        let detection = adapter.detect().expect("detect");
        if !detection.detected {
            assert_eq!(
                detection.detection_error_code,
                "NOT_CONFIGURED",
                "{} must state that a credential is required",
                adapter.id()
            );
            assert_eq!(
                adapter.health_check().status,
                AdapterHealthStatus::NotConfigured,
                "{} must report itself as not configured",
                adapter.id()
            );
        }
    }
}

#[test]
fn declared_support_covers_the_documented_provider_matrix() {
    let mut by_id = std::collections::HashMap::new();
    for adapter in builtin_adapters() {
        by_id.insert(adapter.id(), adapter.descriptor());
    }

    // Providers that publish per-window utilization to the credential their own
    // CLI already stores locally.
    for id in ["anthropic_claude", "openai_codex"] {
        let descriptor = by_id.get(id).unwrap_or_else(|| panic!("{id} registered"));
        assert_eq!(
            descriptor.usage_support,
            ChannelSupport::LocalEstimate,
            "{id} reads its usage history from local session files"
        );
        assert_eq!(
            descriptor.quota_support,
            ChannelSupport::Native,
            "{id} reads published quota from the provider API"
        );
        assert!(
            !descriptor.needs_credentials(),
            "{id} reuses the credential its own CLI stored, so the user enters nothing"
        );
    }

    // API-backed adapters that reuse a credential their own tool already
    // stores locally, so the user enters nothing.
    let cursor = by_id
        .get("cursor_ide")
        .unwrap_or_else(|| panic!("cursor_ide registered"));
    assert_eq!(
        cursor.usage_support,
        ChannelSupport::Native,
        "Cursor publishes per-request usage through its account API"
    );
    assert_eq!(cursor.quota_support, ChannelSupport::Native);
    assert!(!cursor.needs_credentials());

    // Local-artifact collectors with no published limit.
    for id in ["opencode", "google_gemini", "github_copilot", "kiro_ai"] {
        let descriptor = by_id.get(id).unwrap_or_else(|| panic!("{id} registered"));
        assert_eq!(
            descriptor.usage_support,
            ChannelSupport::LocalEstimate,
            "{id} collects usage from local artifacts"
        );
        assert_eq!(descriptor.quota_support, ChannelSupport::LocalEstimate);
        assert!(!descriptor.needs_credentials());
    }

    for id in ["openrouter_api", "xai_grok"] {
        let descriptor = by_id.get(id).unwrap_or_else(|| panic!("{id} registered"));
        assert_eq!(descriptor.quota_support, ChannelSupport::Native);
        assert_eq!(descriptor.usage_support, ChannelSupport::Unsupported);
        assert!(descriptor.needs_credentials());
    }

    let ollama = by_id.get("ollama_local").expect("ollama registered");
    assert_eq!(ollama.quota_support, ChannelSupport::Native);
    assert_eq!(ollama.usage_support, ChannelSupport::Unsupported);
    assert!(!ollama.needs_credentials());
}

/// An adapter whose source is actually present must produce data.
///
/// This is the check that the Claude and Codex collectors previously slipped
/// past: they declared usage support, found their session files, and still
/// returned an empty batch that the pipeline recorded as a successful run.
#[test]
fn a_detected_local_source_must_yield_data_or_an_error_code() {
    for adapter in builtin_adapters() {
        let descriptor = adapter.descriptor();
        let detection = adapter.detect().expect("detect");
        if !detection.detected || !descriptor.usage_support.is_supported() {
            continue;
        }
        let usage = adapter.collect_usage_with_cursor(None);
        match &usage.batch {
            Some(batch) => assert!(
                !batch.events.is_empty() || !usage.outcome.error_code.is_empty(),
                "{} detected its source but returned an empty batch with no error",
                adapter.id()
            ),
            None => assert!(
                !usage.outcome.error_code.is_empty(),
                "{} returned no batch and no error code",
                adapter.id()
            ),
        }
    }
}
