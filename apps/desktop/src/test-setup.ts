import "@testing-library/jest-dom/vitest";
import { vi } from "vitest";

/**
 * Default backend stub for tests that only need the app to render.
 *
 * Unknown commands reject instead of returning null, so a test that exercises a
 * command nobody stubbed fails loudly rather than rendering a page against
 * undefined data.
 */
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockImplementation(async (cmd: string) => {
    switch (cmd) {
      case "get_overview":
        return {
          total_events: 10,
          total_tokens_input: 5000,
          total_tokens_output: 2000,
          total_cost: 0.0425,
          cost_formatted: "0.042500",
          cost_status: "estimated",
          provider_count: 2,
          high_confidence_count: 10,
          confidence_coverage: 1,
          latest_event_at: "2026-08-04T00:00:00Z",
          oldest_event_at: "2026-08-03T12:00:00Z",
        };
      case "get_analytics":
        return {
          rows: [],
          available_providers: ["opencode", "anthropic_claude"],
          available_models: ["glm-5", "claude-test"],
        };
      case "get_providers":
        return [
          {
            provider_id: "opencode",
            display_name: "OpenCode",
            vendor: "OpenCode",
            enabled: true,
            detected: true,
            source_type: "local_sqlite",
            usage_support: "local estimate",
            quota_support: "local estimate",
            auth_requirement: "local files",
            health_status: "Healthy",
            event_count: 5,
            total_tokens: 3000,
            last_sync: "2026-08-04T00:00:00Z",
            last_error_code: "",
            quota_summary: "used 3000 tokens (estimate)",
            reset_at: null,
            confidence: "Medium",
            cost_support: "Priced",
          },
          {
            provider_id: "openrouter_api",
            display_name: "OpenRouter",
            vendor: "OpenRouter",
            enabled: true,
            detected: false,
            source_type: "remote_api",
            usage_support: "not supported",
            quota_support: "supported",
            auth_requirement: "API key",
            health_status: "Not configured",
            event_count: 0,
            total_tokens: 0,
            last_sync: null,
            last_error_code: "",
            quota_summary: "No quota data",
            reset_at: null,
            confidence: "n/a",
            cost_support: "No data",
          },
        ];
      case "get_quota_dashboard":
        return {
          generated_at: "2026-08-04T00:00:00Z",
          providers: [],
        };
      case "get_usage_history":
        return {
          window: "last_7d",
          generated_at: "2026-08-04T00:00:00Z",
          since: "2026-07-28T00:00:00Z",
          request_count: 5,
          tokens_input: 5000,
          tokens_output: 2000,
          models: [
            {
              model: "glm-5",
              provider_id: "opencode",
              request_count: 5,
              tokens_input: 5000,
              tokens_output: 2000,
              token_share_percent: 100,
              first_seen_at: "2026-08-03T12:00:00Z",
              last_seen_at: "2026-08-04T00:00:00Z",
            },
          ],
          daily: [
            {
              day: "2026-08-04",
              request_count: 5,
              tokens_input: 5000,
              tokens_output: 2000,
            },
          ],
          providers: ["opencode"],
        };
      case "get_costs":
        return {
          window: "last_30d",
          generated_at: "2026-08-04T00:00:00Z",
          rows: [
            {
              provider_id: "opencode",
              model: "glm-5",
              request_count: 5,
              tokens_input: 5000,
              tokens_output: 2000,
              cost: null,
              pricing_status: "no catalog entry",
            },
          ],
          priced_total: "0.000000",
          priced_rows: 0,
          unpriced_rows: 1,
          unpriced_tokens: 7000,
        };
      case "get_budgets":
        return { generated_at: "2026-08-04T00:00:00Z", budgets: [] };
      case "get_alerts":
        return {
          generated_at: "2026-08-04T00:00:00Z",
          open: [],
          history: [],
          open_count: 0,
          critical_count: 0,
          unacknowledged_count: 0,
        };
      case "get_settings":
        return {
          settings: {
            launch_at_startup: false,
            theme: "system",
            refresh_interval_seconds: 300,
            auto_update_check: true,
            widget_opacity: 1,
            widget_locked: false,
            widget_visible: false,
            retention_days: 90,
          },
          startup_supported: true,
          startup_registered: false,
          credential_store_supported: true,
          provider_credentials: [
            {
              provider_id: "openrouter_api",
              display_name: "OpenRouter",
              state: "missing",
            },
          ],
          allowed_refresh_intervals: [0, 30, 60, 300, 900, 3600],
          allowed_themes: ["dark", "light", "system"],
          allowed_retention_days: [7, 30, 90, 365, 0],
        };
      case "get_widget_settings":
        return { opacity: 1, locked: false, visible: true };
      case "get_app_events":
        return [];
      case "get_pipeline_diagnostics":
        return {
          app_version: "0.2.0",
          db_ok: true,
          integrity_ok: true,
          migration_version: 6,
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
              detection_method: "local_sqlite",
              source_type: "local_sqlite",
              source_exists: true,
              permission_state: "read_ok",
              adapter_version: "0.2.0",
              last_detection_at: "2026-08-04T00:00:00Z",
              detection_error_code: "",
            },
          ],
          runs: [
            {
              id: 1,
              provider_id: "opencode",
              collector_mode: "local_scan",
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
      case "refresh_all":
        return { usage: [], quota: [] };
      case "refresh_provider":
        return { usage: [], quota: [] };
      case "check_for_update":
        return {
          available: false,
          current_version: "0.2.0",
          version: null,
          notes: null,
          published_at: null,
        };
      default:
        throw new Error(`unstubbed command: ${cmd}`);
    }
  }),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));
