import "@testing-library/jest-dom/vitest";
import { vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockImplementation(async (cmd: string) => {
    if (cmd === "get_overview") {
      return {
        total_events: 10,
        total_tokens_input: 5000,
        total_tokens_output: 2000,
        total_cost: 0.0425,
        cost_formatted: "$0.0425",
        cost_status: "estimated",
        provider_count: 5,
        high_confidence_count: 10,
        confidence_coverage: 1.0,
        latest_event_at: "2026-08-04T00:00:00Z",
        oldest_event_at: "2026-08-03T12:00:00Z",
      };
    }
    if (cmd === "get_analytics") {
      return {
        rows: [],
        available_providers: ["opencode", "openai_codex", "google_gemini", "kiro_ai", "anthropic_claude"],
        available_models: ["gpt-4o", "claude-3-5-sonnet", "gemini-1.5-pro", "moonshot-v1-8k"],
      };
    }
    if (cmd === "get_providers") {
      return [
        {
          provider_id: "opencode",
          display_name: "OpenCode",
          enabled: true,
          detected: true,
          source_type: "Local CLI / JSON",
          health_status: "Healthy",
          event_count: 5,
          total_tokens: 3000,
          last_sync: "2026-08-04T00:00:00Z",
          quota_summary: "5 events recorded",
          reset_at: null,
          confidence: "High",
          cost_support: "Exact",
        },
        {
          provider_id: "openai_codex",
          display_name: "Codex (OpenAI)",
          enabled: true,
          detected: false,
          source_type: "API / Credential",
          health_status: "Not configured",
          event_count: 0,
          total_tokens: 0,
          last_sync: null,
          quota_summary: "Not configured",
          reset_at: null,
          confidence: "High",
          cost_support: "Exact",
        },
        {
          provider_id: "google_gemini",
          display_name: "Gemini (Google)",
          enabled: true,
          detected: false,
          source_type: "API / Credential",
          health_status: "Not configured",
          event_count: 0,
          total_tokens: 0,
          last_sync: null,
          quota_summary: "Not configured",
          reset_at: null,
          confidence: "High",
          cost_support: "Exact",
        },
        {
          provider_id: "kiro_ai",
          display_name: "Kimi",
          enabled: true,
          detected: false,
          source_type: "API / Credential",
          health_status: "Not configured",
          event_count: 0,
          total_tokens: 0,
          last_sync: null,
          quota_summary: "Not configured",
          reset_at: null,
          confidence: "High",
          cost_support: "Estimated",
        },
        {
          provider_id: "anthropic_claude",
          display_name: "Claude (Anthropic)",
          enabled: true,
          detected: false,
          source_type: "API / Credential",
          health_status: "Not configured",
          event_count: 0,
          total_tokens: 0,
          last_sync: null,
          quota_summary: "Not configured",
          reset_at: null,
          confidence: "High",
          cost_support: "Exact",
        },
      ];
    }
    if (cmd === "get_pipeline_diagnostics") {
      return {
        app_version: "0.1.0",
        db_ok: true,
        integrity_ok: true,
        migration_version: 3,
        total_events: 10,
        totals: {
          events_seen: 10,
          events_parsed: 10,
          events_normalized: 10,
          events_rejected: 0,
          duplicates_skipped: 0,
          events_inserted: 10,
          quota_snapshots_inserted: 0,
          privacy_rejections: 0,
          last_successful_sync: "2026-08-04T00:00:00Z",
          next_retry_at: null,
        },
        providers: [
          {
            provider_id: "opencode",
            display_name: "OpenCode",
            enabled: true,
            detected: true,
            detection_method: "cli_config",
            source_type: "Local CLI / JSON",
            source_exists: true,
            permission_state: "Granted",
            adapter_version: "0.1.0",
            last_detection_at: "2026-08-04T00:00:00Z",
            detection_error_code: "",
          },
        ],
        runs: [
          {
            id: 1,
            provider_id: "opencode",
            collector_mode: "passive",
            started_at: "2026-08-04T00:00:00Z",
            finished_at: "2026-08-04T00:00:01Z",
            duration_ms: 100,
            source_records_seen: 10,
            records_parsed: 10,
            events_normalized: 10,
            events_rejected: 0,
            duplicates_skipped: 0,
            events_inserted: 10,
            quota_snapshots_inserted: 0,
            warning_codes: [],
            error_code: "",
            next_retry_at: null,
          },
        ],
      };
    }
    if (cmd === "refresh_all") {
      return [];
    }
    return null;
  }),
}));
