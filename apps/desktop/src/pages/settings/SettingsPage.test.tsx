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
    setOpenCodeGoConfig: vi.fn(),
    deleteOpenCodeGoConfig: vi.fn(),
    fetchWidgetSettings: vi.fn(),
    setWidgetView: vi.fn(),
    setWidgetSizePreset: vi.fn(),
    setLanguage: vi.fn(),
    showWidgetWindow: vi.fn(),
    hideWidgetWindow: vi.fn(),
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
    widget_size: "medium",
    retention_days: 90,
    pet_visible: false,
    pet_character: "robot",
    pet_speed: "normal",
    pet_opacity: 1,
    pet_auto_sleep: true,
    pet_size: "medium",
    pet_stay_in_place: false,
    pet_pose_wave: true,
    pet_pose_jump: true,
    pet_pose_look_left: true,
    pet_pose_look_right: true,
    pet_pose_waiting: true,
    pet_pose_review: true,
    language: "en",
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
  opencode_go: { state: "missing" },
  allowed_refresh_intervals: [0, 300, 900, 3600],
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
    vi.mocked(native.setOpenCodeGoConfig).mockReset();
    vi.mocked(native.deleteOpenCodeGoConfig).mockReset();
    vi.mocked(native.fetchWidgetSettings).mockReset();
    vi.mocked(native.setWidgetView).mockReset();
    vi.mocked(native.setWidgetSizePreset).mockReset();
    vi.mocked(native.setWidgetSizePreset).mockResolvedValue("medium");
    vi.mocked(native.setLanguage).mockReset();
    vi.mocked(native.setLanguage).mockResolvedValue("en");
    vi.mocked(native.showWidgetWindow).mockReset();
    vi.mocked(native.showWidgetWindow).mockResolvedValue(undefined);
    vi.mocked(native.hideWidgetWindow).mockReset();
    vi.mocked(native.hideWidgetWindow).mockResolvedValue(undefined);
    vi.mocked(native.fetchWidgetSettings).mockResolvedValue({
      opacity: 1,
      locked: false,
      visible: true,
      selected_providers: [],
      view: "bars",
      pet_id: "",
      size_preset: "medium",
    });
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

  it("persists a changed interval immediately without a save button", async () => {
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
      "900",
    );

    await waitFor(() => expect(native.saveSettings).toHaveBeenCalledTimes(1));
    expect(native.saveSettings).toHaveBeenCalledWith(
      expect.objectContaining({ refresh_interval_seconds: 900 }),
    );
    expect(screen.queryByRole("button", { name: /save/i })).not.toBeInTheDocument();
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
    await userEvent.selectOptions(
      screen.getByLabelText("Automatic refresh"),
      "900",
    );

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

  it("stores OpenCode Go credentials without displaying the cookie", async () => {
    vi.mocked(native.fetchSettings).mockResolvedValue(view());
    vi.mocked(native.setOpenCodeGoConfig).mockResolvedValue(
      view({ opencode_go: { state: "configured" } }),
    );
    render(<SettingsPage />);

    const workspace = await screen.findByLabelText("Workspace ID");
    const cookie = screen.getByLabelText("Auth cookie");
    await userEvent.type(workspace, "workspace-test-123");
    await userEvent.type(cookie, "cookie-secret-value");
    await userEvent.click(screen.getByRole("button", { name: "Store OpenCode Go" }));

    await waitFor(() =>
      expect(native.setOpenCodeGoConfig).toHaveBeenCalledWith(
        "workspace-test-123",
        "cookie-secret-value",
      ),
    );
    expect(screen.queryByDisplayValue("cookie-secret-value")).not.toBeInTheDocument();
    expect(screen.getByText("Configured")).toBeInTheDocument();
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

  it("switches the UI language through the backend", async () => {
    vi.mocked(native.fetchSettings).mockResolvedValue(view());
    vi.mocked(native.setLanguage).mockResolvedValue("th");
    render(<SettingsPage />);

    const select = await screen.findByLabelText("Language");
    await userEvent.selectOptions(select, "th");
    await waitFor(() =>
      expect(native.setLanguage).toHaveBeenCalledWith("th"),
    );
  });

  it("shows and hides the native widget window when toggled", async () => {
    vi.mocked(native.fetchSettings).mockResolvedValue(view());
    vi.mocked(native.saveSettings).mockImplementation(async (settings) =>
      view({ settings }),
    );
    render(<SettingsPage />);

    const toggle = await screen.findByRole("switch", {
      name: /Show the floating quota widget/i,
    });
    expect(toggle).not.toBeChecked();

    await userEvent.click(toggle);
    await waitFor(() =>
      expect(native.showWidgetWindow).toHaveBeenCalledTimes(1),
    );
    expect(native.hideWidgetWindow).not.toHaveBeenCalled();

    await userEvent.click(toggle);
    await waitFor(() =>
      expect(native.hideWidgetWindow).toHaveBeenCalledTimes(1),
    );
    expect(native.showWidgetWindow).toHaveBeenCalledTimes(1);
  });
});
