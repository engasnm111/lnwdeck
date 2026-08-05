import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { SettingsPage } from "./SettingsPage";
import * as native from "../../lib/native";

vi.mock("../../lib/native", async (importOriginal) => {
  const actual = await importOriginal<typeof native>();
  return {
    ...actual,
    fetchSettings: vi.fn(),
    saveSettings: vi.fn(),
    setProviderKey: vi.fn(),
    deleteProviderKey: vi.fn(),
    listWidgetPets: vi.fn(),
    getWidgetPet: vi.fn(),
    importWidgetPet: vi.fn(),
    setWidgetPet: vi.fn(),
    removeWidgetPet: vi.fn(),
  };
});

const view = (
  overrides: Partial<native.SettingsViewData> = {},
): native.SettingsViewData => ({
  settings: {
    launch_at_startup: false,
    theme: "dark",
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
  ...overrides,
});

describe("SettingsPage", () => {
  beforeEach(() => {
    vi.mocked(native.fetchSettings).mockReset();
    vi.mocked(native.saveSettings).mockReset();
    vi.mocked(native.setProviderKey).mockReset();
    vi.mocked(native.deleteProviderKey).mockReset();
    vi.mocked(native.listWidgetPets).mockReset();
    vi.mocked(native.getWidgetPet).mockReset();
    vi.mocked(native.importWidgetPet).mockReset();
    vi.mocked(native.setWidgetPet).mockReset();
    vi.mocked(native.removeWidgetPet).mockReset();
    vi.mocked(native.listWidgetPets).mockResolvedValue([]);
    vi.mocked(native.getWidgetPet).mockResolvedValue(null);
  });

  it("renders the stored state, not a default-checked form", async () => {
    vi.mocked(native.fetchSettings).mockResolvedValue(view());
    render(<SettingsPage />);

    await waitFor(() =>
      expect(
        screen.getByRole("switch", { name: /Start lnwdeck when Windows starts/i }),
      ).not.toBeChecked(),
    );
    expect(
      screen.getByRole("switch", { name: /Check for updates automatically/i }),
    ).toBeChecked();
    expect(screen.getByLabelText("Automatic refresh")).toHaveValue("300");
    expect(screen.getByLabelText("Theme")).toHaveValue("dark");
  });

  it("persists a changed interval through the backend and reports it", async () => {
    vi.mocked(native.fetchSettings).mockResolvedValue(view());
    vi.mocked(native.saveSettings).mockImplementation(async (settings) =>
      view({ settings }),
    );
    render(<SettingsPage />);

    await waitFor(() =>
      expect(screen.getByLabelText("Automatic refresh")).toBeInTheDocument(),
    );
    await userEvent.selectOptions(
      screen.getByLabelText("Automatic refresh"),
      "60",
    );
    await userEvent.click(screen.getByRole("button", { name: "Save settings" }));

    await waitFor(() => expect(native.saveSettings).toHaveBeenCalledTimes(1));
    expect(native.saveSettings).toHaveBeenCalledWith(
      expect.objectContaining({ refresh_interval_seconds: 60 }),
    );
    await waitFor(() => expect(screen.getByRole("status")).toBeInTheDocument());
  });

  it("shows a rejected save and keeps the previous stored value", async () => {
    vi.mocked(native.fetchSettings).mockResolvedValue(view());
    vi.mocked(native.saveSettings).mockRejectedValue(
      new Error("unsupported refresh interval: 7 seconds"),
    );
    render(<SettingsPage />);

    await waitFor(() =>
      expect(screen.getByLabelText("Automatic refresh")).toBeInTheDocument(),
    );
    await userEvent.click(screen.getByRole("button", { name: "Save settings" }));

    await waitFor(() =>
      expect(
        screen.getByText("unsupported refresh interval: 7 seconds"),
      ).toBeInTheDocument(),
    );
    expect(screen.queryByText(/Saved at/)).not.toBeInTheDocument();
  });

  it("stores a provider API key without displaying it", async () => {
    vi.mocked(native.fetchSettings).mockResolvedValue(view());
    vi.mocked(native.setProviderKey).mockResolvedValue(
      view({
        provider_credentials: [
          {
            provider_id: "openrouter_api",
            display_name: "OpenRouter",
            state: "configured",
          },
        ],
      }),
    );
    render(<SettingsPage />);

    const input = await screen.findByLabelText("OpenRouter API key");
    expect(input).toHaveAttribute("type", "password");
    await userEvent.type(input, "sk-secret");
    await userEvent.click(screen.getByRole("button", { name: "Store" }));

    await waitFor(() =>
      expect(native.setProviderKey).toHaveBeenCalledWith(
        "openrouter_api",
        "sk-secret",
      ),
    );
    await waitFor(() =>
      expect(screen.getByText("configured")).toBeInTheDocument(),
    );
    expect(screen.queryByDisplayValue("sk-secret")).not.toBeInTheDocument();
  });

  it("disables startup when the platform does not support it", async () => {
    vi.mocked(native.fetchSettings).mockResolvedValue(
      view({ startup_supported: false, credential_store_supported: false }),
    );
    render(<SettingsPage />);

    await waitFor(() =>
      expect(
        screen.getByRole("switch", { name: /Start lnwdeck when Windows starts/i }),
      ).toBeDisabled(),
    );
    expect(screen.getByLabelText("OpenRouter API key")).toBeDisabled();
  });

  it("imports a community pet from a codex-pets.net input", async () => {
    vi.mocked(native.fetchSettings).mockResolvedValue(view());
    vi.mocked(native.importWidgetPet).mockResolvedValue({
      id: "sprout",
      displayName: "Sprout",
      description: "Fixture",
      spritesheetPath: "spritesheet.webp",
      spriteVersionNumber: 1,
    });
    vi.mocked(native.listWidgetPets).mockResolvedValue([
      {
        id: "sprout",
        displayName: "Sprout",
        description: "Fixture",
        spritesheetPath: "spritesheet.webp",
        spriteVersionNumber: 1,
      },
    ]);
    render(<SettingsPage />);

    const input = await screen.findByLabelText("Codex Pets URL or pet id");
    await userEvent.type(input, "sprout");
    await userEvent.click(screen.getByRole("button", { name: "Import" }));

    await waitFor(() =>
      expect(native.importWidgetPet).toHaveBeenCalledWith("sprout"),
    );
    await waitFor(() =>
      expect(screen.getByText("Sprout")).toBeInTheDocument(),
    );
    expect(screen.getByRole("button", { name: "Use" })).toBeInTheDocument();
  });

  it("selects and removes installed pets through the backend", async () => {
    vi.mocked(native.fetchSettings).mockResolvedValue(view());
    const installed = [
      {
        id: "sprout",
        displayName: "Sprout",
        description: "Fixture",
        spritesheetPath: "spritesheet.webp",
        spriteVersionNumber: 1,
      },
    ];
    vi.mocked(native.listWidgetPets).mockResolvedValue(installed);
    vi.mocked(native.getWidgetPet).mockResolvedValue(null);
    vi.mocked(native.setWidgetPet).mockResolvedValue("sprout");
    render(<SettingsPage />);

    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Use" })).toBeInTheDocument(),
    );
    await userEvent.click(screen.getByRole("button", { name: "Use" }));
    await waitFor(() =>
      expect(native.setWidgetPet).toHaveBeenCalledWith("sprout"),
    );
    await waitFor(() =>
      expect(screen.getAllByText("Active").length).toBeGreaterThan(0),
    );

    vi.mocked(native.listWidgetPets).mockResolvedValue([]);
    const removeButtons = screen
      .getAllByRole("button", { name: "Remove" })
      .filter((button) => !button.hasAttribute("disabled"));
    await userEvent.click(removeButtons[0]);
    await waitFor(() =>
      expect(native.removeWidgetPet).toHaveBeenCalledWith("sprout"),
    );
    await waitFor(() =>
      expect(screen.getByText(/No community pets installed/)).toBeInTheDocument(),
    );
  });

  it("reports a refused pet import without hiding the failure", async () => {
    vi.mocked(native.fetchSettings).mockResolvedValue(view());
    vi.mocked(native.importWidgetPet).mockRejectedValue(
      new Error("Only https://codex-pets.net pet URLs are supported"),
    );
    render(<SettingsPage />);

    const input = await screen.findByLabelText("Codex Pets URL or pet id");
    await userEvent.type(input, "https://evil.example/pets/x");
    await userEvent.click(screen.getByRole("button", { name: "Import" }));

    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent(
        "Only https://codex-pets.net pet URLs are supported",
      ),
    );
  });
});
