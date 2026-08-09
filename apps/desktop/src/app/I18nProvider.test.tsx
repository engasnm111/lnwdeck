import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { I18nProvider } from "./I18nProvider";
import * as native from "../lib/native";
import { useI18n } from "../lib/i18n";

vi.mock("../lib/native", () => ({
  fetchSettings: vi.fn(),
}));

function Probe() {
  const { language, t } = useI18n();
  return (
    <div>
      <span>{language}</span>
      <span>{t("nav.settings")}</span>
    </div>
  );
}

describe("I18nProvider", () => {
  beforeEach(() => {
    vi.mocked(native.fetchSettings).mockReset();
  });

  it("loads the stored language from the backend", async () => {
    vi.mocked(native.fetchSettings).mockResolvedValue({
      settings: {
        launch_at_startup: false,
        language: "th",
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
      },
      startup_supported: true,
      startup_registered: false,
      credential_store_supported: true,
      provider_credentials: [],
      opencode_go: { state: "missing" },
      allowed_refresh_intervals: [300],
      allowed_themes: ["dark", "light", "system"],
      allowed_retention_days: [90],
    });
    render(
      <I18nProvider>
        <Probe />
      </I18nProvider>,
    );
    await waitFor(() => {
      expect(screen.getByText("th")).toBeInTheDocument();
      expect(screen.getByText("ตั้งค่า")).toBeInTheDocument();
    });
  });

  it("falls back to English outside a Tauri runtime", async () => {
    vi.mocked(native.fetchSettings).mockRejectedValue(new Error("no runtime"));
    render(
      <I18nProvider>
        <Probe />
      </I18nProvider>,
    );
    await waitFor(() => {
      expect(screen.getByText("en")).toBeInTheDocument();
      expect(screen.getByText("Settings")).toBeInTheDocument();
    });
  });
});
