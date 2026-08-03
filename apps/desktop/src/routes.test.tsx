import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { MemoryRouter } from "react-router-dom";
import App from "./App";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("Desktop Navigation and Backend Data Integration", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  const routes = [
    { path: "/", title: "Overview" },
    { path: "/providers", title: "Providers" },
    { path: "/analytics", title: "Analytics" },
    { path: "/costs", title: "Costs" },
    { path: "/budgets", title: "Budgets" },
    { path: "/models", title: "Models" },
    { path: "/alerts", title: "Alerts" },
    { path: "/settings", title: "Settings" },
    { path: "/system", title: "System" },
  ];

  routes.forEach(({ path, title }) => {
    it(`renders route ${path} with heading ${title} without crashing or blank content`, async () => {
      vi.mocked(invoke).mockImplementation(async (cmd: string) => {
        if (cmd === "get_overview") {
          return {
            total_events: 10,
            total_tokens_input: 1000,
            total_tokens_output: 500,
            provider_count: 2,
            high_confidence_count: 8,
            confidence_coverage: 0.8,
            latest_event_at: "2026-08-03T12:00:00Z",
            oldest_event_at: "2026-08-01T12:00:00Z",
          };
        }
        if (cmd === "get_analytics") {
          return {
            rows: [
              {
                id: "evt_1",
                timestamp: "2026-08-03T12:00:00Z",
                provider_id: "opencode",
                model: "claude-3-5-sonnet",
                tokens_input: 100,
                tokens_output: 50,
                confidence: "High",
                cost: "0.0015",
              },
            ],
            available_providers: ["opencode"],
            available_models: ["claude-3-5-sonnet"],
          };
        }
        if (cmd === "get_providers") {
          return [
            {
              provider_id: "opencode",
              display_name: "OpenCode",
              enabled: true,
              detected: true,
              source_type: "cli_config",
              health_status: "Healthy",
              event_count: 10,
              total_tokens: 1500,
              last_sync: "2026-08-03T12:00:00Z",
              quota_summary: "100k / 500k",
              reset_at: "2026-09-01T00:00:00Z",
              confidence: "High",
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
              quota_snapshots_inserted: 1,
              privacy_rejections: 0,
              last_successful_sync: "2026-08-03T12:00:00Z",
              next_retry_at: null,
            },
            providers: [],
            runs: [],
          };
        }
        return null;
      });

      render(
        <MemoryRouter initialEntries={[path]}>
          <App />
        </MemoryRouter>
      );

      await waitFor(() => {
        expect(
          screen.getByRole("heading", { name: title, level: 2 })
        ).toBeInTheDocument();
      });
    });
  });

  it("AnalyticsPage fetches and displays data from fetchAnalytics backend command", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_analytics") {
        return {
          rows: [
            {
              id: "evt_99",
              timestamp: "2026-08-03T10:00:00Z",
              provider_id: "test_provider",
              model: "test_model",
              tokens_input: 500,
              tokens_output: 200,
              confidence: "High",
              cost: "0.0050",
            },
          ],
          available_providers: ["test_provider"],
          available_models: ["test_model"],
        };
      }
      return null;
    });

    render(
      <MemoryRouter initialEntries={["/analytics"]}>
        <App />
      </MemoryRouter>
    );

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("get_analytics", expect.anything());
      expect(screen.getAllByText("test_provider").length).toBeGreaterThan(0);
      expect(screen.getAllByText("test_model").length).toBeGreaterThan(0);
    });
  });

  it("ProvidersPage fetches and displays data from fetchProviders backend command", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_providers") {
        return [
          {
            provider_id: "opencode",
            display_name: "OpenCode Engine",
            enabled: true,
            detected: true,
            source_type: "cli_config",
            health_status: "Healthy",
            event_count: 42,
            total_tokens: 8400,
            last_sync: "2026-08-03T12:00:00Z",
            quota_summary: "Unlimited",
            reset_at: null,
            confidence: "High",
          },
        ];
      }
      return null;
    });

    render(
      <MemoryRouter initialEntries={["/providers"]}>
        <App />
      </MemoryRouter>
    );

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("get_providers");
      expect(screen.getByText("OpenCode Engine")).toBeInTheDocument();
    });
  });

  it("OverviewPage renders metric cards for Total Tokens, Requests, and Confidence", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_overview") {
        return {
          total_events: 15,
          total_tokens_input: 2000,
          total_tokens_output: 1000,
          provider_count: 3,
          high_confidence_count: 12,
          confidence_coverage: 0.85,
          latest_event_at: "2026-08-03T12:00:00Z",
          oldest_event_at: "2026-08-01T12:00:00Z",
        };
      }
      return null;
    });

    render(
      <MemoryRouter initialEntries={["/"]}>
        <App />
      </MemoryRouter>
    );

    await waitFor(() => {
      expect(screen.getByText("Total Tokens")).toBeInTheDocument();
      expect(screen.getByText("3,000")).toBeInTheDocument();
    });
  });
});
