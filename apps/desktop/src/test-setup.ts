import "@testing-library/jest-dom/vitest";
import { vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockImplementation(async (cmd: string) => {
    if (cmd === "get_overview") {
      return {
        total_events: 0,
        total_tokens_input: 0,
        total_tokens_output: 0,
        provider_count: 0,
        high_confidence_count: 0,
        confidence_coverage: 0,
        latest_event_at: null,
        oldest_event_at: null,
      };
    }
    if (cmd === "get_analytics") {
      return {
        rows: [],
        available_providers: [],
        available_models: [],
      };
    }
    if (cmd === "get_providers") {
      return [];
    }
    if (cmd === "get_pipeline_diagnostics") {
      return {
        app_version: "0.1.0",
        db_ok: true,
        integrity_ok: true,
        migration_version: 3,
        total_events: 0,
        totals: {
          events_seen: 0,
          events_parsed: 0,
          events_normalized: 0,
          events_rejected: 0,
          duplicates_skipped: 0,
          events_inserted: 0,
          quota_snapshots_inserted: 0,
          privacy_rejections: 0,
          last_successful_sync: null,
          next_retry_at: null,
        },
        providers: [],
        runs: [],
      };
    }
    if (cmd === "refresh_all") {
      return [];
    }
    return null;
  }),
}));
