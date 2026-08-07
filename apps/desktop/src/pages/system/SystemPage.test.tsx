import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SystemPage } from "./SystemPage";
import {
  exportDiagnostics,
  fetchPipelineDiagnostics,
  refreshAll,
  revealInExplorer,
  type PipelineDiagnostics,
} from "../../lib/native";

vi.mock("../../lib/native", () => ({
  fetchPipelineDiagnostics: vi.fn(),
  refreshAll: vi.fn(),
  exportDiagnostics: vi.fn(),
  revealInExplorer: vi.fn(),
}));

const fixture: PipelineDiagnostics = {
  app_version: "0.2.0",
  db_ok: true,
  integrity_ok: true,
  migration_version: 3,
  total_events: 15,
  totals: {
    events_seen: 15,
    events_parsed: 15,
    events_normalized: 15,
    events_rejected: 0,
    duplicates_skipped: 0,
    events_inserted: 15,
    quota_snapshots_inserted: 0,
    privacy_rejections: 0,
    last_successful_sync: "2026-08-03T12:00:00Z",
    next_retry_at: null,
  },
  providers: [
    {
      provider_id: "opencode_cli",
      display_name: "OpenCode",
      enabled: true,
      detected: true,
      detection_method: "local_sqlite",
      source_type: "sqlite",
      source_exists: true,
      permission_state: "read_ok",
      adapter_version: "0.2.0",
      last_detection_at: "2026-08-03T12:00:00Z",
      detection_error_code: "",
    },
  ],
  runs: [
    {
      id: 1,
      provider_id: "opencode_cli",
      collector_mode: "passive_scan",
      started_at: "2026-08-03T12:00:00Z",
      finished_at: "2026-08-03T12:00:01Z",
      duration_ms: 900,
      source_records_seen: 15,
      records_parsed: 15,
      events_normalized: 15,
      events_rejected: 0,
      duplicates_skipped: 0,
      events_inserted: 15,
      quota_snapshots_inserted: 0,
      warning_codes: [],
      error_code: "",
      next_retry_at: null,
    },
  ],
};

const emptyFixture: PipelineDiagnostics = {
  ...fixture,
  total_events: 0,
  totals: { ...fixture.totals, last_successful_sync: null },
  providers: [],
  runs: [],
};

describe("SystemPage Data Pipeline", () => {
  beforeEach(() => {
    vi.mocked(fetchPipelineDiagnostics).mockReset();
    vi.mocked(refreshAll).mockReset();
    vi.mocked(exportDiagnostics).mockReset();
    vi.mocked(revealInExplorer).mockReset();
  });

  it("renders the data pipeline section with database status", async () => {
    vi.mocked(fetchPipelineDiagnostics).mockResolvedValue(fixture);
    render(<SystemPage />);

    expect(
      await screen.findByRole("heading", { name: "Data Pipeline" }),
    ).toBeVisible();
    expect(screen.getByText("0.2.0")).toBeInTheDocument();
    expect(screen.getByText("Migration version")).toBeInTheDocument();
    expect(screen.getByText("3")).toBeInTheDocument();
    expect(screen.getByText("Events stored")).toBeInTheDocument();
    expect(screen.getAllByText("15").length).toBeGreaterThan(0);
  });

  it("renders every required provider table column", async () => {
    vi.mocked(fetchPipelineDiagnostics).mockResolvedValue(fixture);
    render(<SystemPage />);
    await screen.findByRole("heading", { name: "Data Pipeline" });

    const headers = [
      "Provider",
      "Detected",
      "Source",
      "Mode",
      "Last sync",
      "Seen",
      "Parsed",
      "Inserted",
      "Duplicates",
      "Rejected",
      "Health",
      "Next retry",
      "Action",
    ];
    for (const header of headers) {
      expect(
        screen.getByRole("columnheader", { name: header }),
      ).toBeInTheDocument();
    }
  });

  it("renders provider health evidence from the latest run", async () => {
    vi.mocked(fetchPipelineDiagnostics).mockResolvedValue(fixture);
    render(<SystemPage />);
    await screen.findByRole("heading", { name: "Data Pipeline" });

    expect(screen.getByRole("row", { name: /OpenCode/ })).toBeInTheDocument();
    expect(screen.getAllByText("Detected").length).toBeGreaterThan(0);
    expect(screen.getByText("sqlite")).toBeInTheDocument();
    expect(screen.getByText("passive_scan")).toBeInTheDocument();
    expect(screen.getAllByText("15").length).toBeGreaterThan(0);
  });

  it("shows the no-provider empty state when nothing is detected", async () => {
    vi.mocked(fetchPipelineDiagnostics).mockResolvedValue(emptyFixture);
    render(<SystemPage />);

    expect(
      await screen.findByText(/no supported ai tools were detected/i),
    ).toBeVisible();
  });

  it("shows detected-but-no-records state when provider has no runs", async () => {
    const noRuns: PipelineDiagnostics = {
      ...emptyFixture,
      providers: [...fixture.providers],
    };
    vi.mocked(fetchPipelineDiagnostics).mockResolvedValue(noRuns);
    render(<SystemPage />);

    expect(
      await screen.findByText(/no usage records were found yet/i),
    ).toBeVisible();
  });

  it("refresh button runs refresh-all and reloads diagnostics", async () => {
    vi.mocked(fetchPipelineDiagnostics)
      .mockResolvedValueOnce(emptyFixture)
      .mockResolvedValueOnce(fixture);
    vi.mocked(refreshAll).mockResolvedValue({ usage: fixture.runs, quota: [] });
    const user = userEvent.setup();
    render(<SystemPage />);

    expect(
      await screen.findByText(/no supported ai tools were detected/i),
    ).toBeVisible();

    await user.click(screen.getByRole("button", { name: /refresh all/i }));

    await waitFor(() => {
      expect(refreshAll).toHaveBeenCalledTimes(1);
    });
    expect(
      await screen.findByRole("row", { name: /OpenCode/ }),
    ).toBeInTheDocument();
    expect(fetchPipelineDiagnostics).toHaveBeenCalledTimes(2);
  });

  it("export button downloads a sanitized diagnostics JSON file", async () => {
    vi.mocked(fetchPipelineDiagnostics).mockResolvedValue(fixture);
    vi.mocked(exportDiagnostics).mockResolvedValue(
      "C:\\Users\\tester\\Downloads\\lnwdeck-diagnostics-20260807-160000.json",
    );
    const user = userEvent.setup();
    render(<SystemPage />);
    await screen.findByRole("heading", { name: "Data Pipeline" });

    await user.click(
      screen.getByRole("button", { name: /export sanitized diagnostics/i }),
    );

    await waitFor(() =>
      expect(exportDiagnostics).toHaveBeenCalledOnce(),
    );
    expect(
      screen.getByText(/lnwdeck-diagnostics-20260807-160000\.json/),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /show in folder/i }),
    ).toBeInTheDocument();
  });

  it("shows the collector error state with retry information", async () => {
    const failed: PipelineDiagnostics = {
      ...fixture,
      runs: [
        {
          ...fixture.runs[0],
          events_inserted: 0,
          error_code: "SOURCE_UNAVAILABLE",
          next_retry_at: "2026-08-03T13:00:00Z",
        },
      ],
    };
    vi.mocked(fetchPipelineDiagnostics).mockResolvedValue(failed);
    render(<SystemPage />);
    await screen.findByRole("heading", { name: "Data Pipeline" });

    expect(screen.getByText("Error")).toBeInTheDocument();
    expect(screen.getByText(/SOURCE_UNAVAILABLE/i)).toBeInTheDocument();
    expect(screen.getAllByText(/next retry/i).length).toBeGreaterThan(0);
  });

  it("surfaces command errors instead of swallowing them", async () => {
    vi.mocked(fetchPipelineDiagnostics).mockRejectedValue(
      new Error("storage not initialized"),
    );
    render(<SystemPage />);

    expect(
      await screen.findByText(/storage not initialized/i),
    ).toBeVisible();
  });
});
