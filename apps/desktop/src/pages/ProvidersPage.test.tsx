import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { I18nProvider } from "../app/I18nProvider";
import { ProvidersPage } from "./ProvidersPage";
import * as native from "../lib/native";

vi.mock("../lib/native", async (importOriginal) => {
  const actual = await importOriginal<typeof native>();
  return {
    ...actual,
    fetchProviders: vi.fn(),
    fetchQuotaDashboard: vi.fn(),
    refreshProvider: vi.fn(),
  };
});

const provider: native.DetailedProviderInfo = {
  provider_id: "openai_codex",
  display_name: "OpenAI Codex",
  vendor: "OpenAI",
  enabled: true,
  detected: true,
  source_type: "local logs",
  usage_support: "supported",
  quota_support: "supported",
  auth_requirement: "local files",
  health_status: "Healthy",
  event_count: 12,
  total_tokens: 1_234_567,
  last_sync: "2026-08-08T00:00:00Z",
  last_error_code: "",
  quota_summary: "No quota windows reported",
  reset_at: null,
  confidence: "High",
  cost_support: "Not available",
};

describe("ProvidersPage", () => {
  beforeEach(() => {
    vi.mocked(native.fetchProviders).mockResolvedValue([provider]);
    vi.mocked(native.fetchQuotaDashboard).mockResolvedValue({
      generated_at: "2026-08-08T00:00:00Z",
      providers: [],
    });
  });

  it("shows the provider display name instead of the internal provider key", async () => {
    render(
      <I18nProvider>
        <ProvidersPage />
      </I18nProvider>,
    );

    expect(await screen.findByText("OpenAI Codex")).toBeInTheDocument();
    expect(screen.getByText("OpenAI - Local")).toBeInTheDocument();
    expect(screen.queryByText("openai_codex")).not.toBeInTheDocument();
  });

  it("localizes provider capabilities and hides storage source identifiers", async () => {
    vi.mocked(native.fetchProviders).mockResolvedValue([
      {
        ...provider,
        provider_id: "opencode",
        display_name: "OpenCode",
        source_type: "local_sqlite",
        health_status: "Not configured",
        usage_support: "local estimate",
        quota_support: "not supported",
        auth_requirement: "API key",
        cost_support: "Missing pricing",
        quota_summary: "No quota data",
      },
    ]);

    render(
      <I18nProvider>
        <ProvidersPage />
      </I18nProvider>,
    );

    expect(await screen.findByText("OpenAI - Local")).toBeInTheDocument();
    expect(screen.getByText("history: Local estimate")).toBeInTheDocument();
    expect(screen.getByText("quota: Not supported")).toBeInTheDocument();
    expect(screen.getByText("auth: API key")).toBeInTheDocument();
    expect(screen.getByText("Missing pricing")).toBeInTheDocument();
    expect(screen.getByText("No quota data")).toBeInTheDocument();
    expect(screen.queryByText(/local_sqlite/)).not.toBeInTheDocument();
  });
});
