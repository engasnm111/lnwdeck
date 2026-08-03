import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import {
  fetchOverview,
  fetchProviders,
  fetchPipelineDiagnostics,
  fetchQuotaDashboard,
  refreshAll,
} from "./native";

describe("native.ts production contract", () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it.each([
    ["fetchOverview", fetchOverview],
    ["fetchProviders", fetchProviders],
    ["fetchPipelineDiagnostics", fetchPipelineDiagnostics],
    ["fetchQuotaDashboard", fetchQuotaDashboard],
    ["refreshAll", refreshAll],
  ] as const)(
    "%s propagates backend failures instead of returning demo data",
    async (_name, fetcher) => {
      vi.mocked(invoke).mockRejectedValue(new Error("backend down"));
      await expect(fetcher()).rejects.toThrow("backend down");
    },
  );

  it.each([
    ["fetchOverview", "get_overview"],
    ["fetchProviders", "get_providers"],
    ["fetchQuotaDashboard", "get_quota_dashboard"],
    ["refreshAll", "refresh_all"],
  ] as const)("%s invokes the correct command", async (_name, command) => {
    vi.mocked(invoke).mockResolvedValue({});
    await fetcherFor(command);
    expect(invoke).toHaveBeenCalledWith(command);
  });
});

async function fetcherFor(command: string): Promise<unknown> {
  switch (command) {
    case "get_overview":
      return fetchOverview();
    case "get_providers":
      return fetchProviders();
    case "get_quota_dashboard":
      return fetchQuotaDashboard();
    case "refresh_all":
      return refreshAll();
    default:
      throw new Error(`unknown command ${command}`);
  }
}
