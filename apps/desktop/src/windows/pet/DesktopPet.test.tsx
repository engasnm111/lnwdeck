import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { I18nProvider } from "../../app/I18nProvider";
import { DesktopPet } from "./DesktopPet";

const petTestMocks = vi.hoisted(() => {
  const listeners = new Map<
    string,
    (event: { payload: unknown }) => void
  >();
  return {
    listeners,
    listen: vi.fn(
      (event: string, handler: (event: { payload: unknown }) => void) => {
        listeners.set(event, handler);
        return Promise.resolve(() => {});
      },
    ),
    fetchSettings: vi.fn(),
    applyPetClickThrough: vi.fn(),
    fetchPetSpritesheetUrl: vi.fn(),
    fetchPetWindowSettings: vi.fn(),
    fetchQuotaDashboard: vi.fn(),
    hidePetWindow: vi.fn(),
    listWidgetPets: vi.fn(),
    movePetWindow: vi.fn(),
    setPetHitRect: vi.fn(),
    showMainWindow: vi.fn(),
    currentMonitor: vi.fn(),
  };
});

vi.mock("@tauri-apps/api/event", () => ({
  listen: petTestMocks.listen,
}));

vi.mock("@tauri-apps/api/window", () => ({
  currentMonitor: petTestMocks.currentMonitor,
}));

vi.mock("../../lib/native", () => ({
  applyPetClickThrough: petTestMocks.applyPetClickThrough,
  fetchPetSpritesheetUrl: petTestMocks.fetchPetSpritesheetUrl,
  fetchPetWindowSettings: petTestMocks.fetchPetWindowSettings,
  fetchQuotaDashboard: petTestMocks.fetchQuotaDashboard,
  fetchSettings: petTestMocks.fetchSettings,
  hidePetWindow: petTestMocks.hidePetWindow,
  listWidgetPets: petTestMocks.listWidgetPets,
  movePetWindow: petTestMocks.movePetWindow,
  setPetHitRect: petTestMocks.setPetHitRect,
  showMainWindow: petTestMocks.showMainWindow,
}));

const settings = {
  visible: true,
  character: "friend-pixel-pet",
  speed: "normal",
  opacity: 1,
  autoSleep: true,
  sizePreset: "medium",
  stayInPlace: false,
  poseWave: true,
  poseJump: true,
  poseLookLeft: true,
  poseLookRight: true,
  poseWaiting: true,
  poseReview: true,
};

const pet = {
  id: "friend-pixel-pet",
  displayName: "Friend Pixel Pet",
  description: "Fixture pet",
  spritesheetPath: "spritesheet.webp",
  spriteVersionNumber: 1,
};

describe("DesktopPet", () => {
  beforeEach(() => {
    petTestMocks.listeners.clear();
    petTestMocks.fetchSettings.mockResolvedValue({ settings: { language: "en" } });
    petTestMocks.fetchPetWindowSettings.mockResolvedValue(settings);
    petTestMocks.listWidgetPets.mockResolvedValue([pet]);
    petTestMocks.fetchPetSpritesheetUrl.mockResolvedValue("spritesheet.webp");
    petTestMocks.fetchQuotaDashboard.mockResolvedValue({
      generated_at: "2026-08-08T00:00:00Z",
      providers: [
        {
          provider_id: "openai_codex",
          display_name: "OpenAI Codex",
          status: "fresh",
          plan: null,
          source: "local_estimate",
          collected_at: "2026-08-08T00:00:00Z",
          stale_at: "2026-08-08T01:00:00Z",
          error_code: null,
          windows: [
            {
              window_key: "30d",
              label: "30-day",
              scope: "rolling",
              kind: "tokens",
              used: 12_500_000,
              limit: null,
              remaining: null,
              remaining_percent: null,
              used_percent: null,
              reset_at: null,
              is_unlimited: false,
              confidence: "High",
            },
          ],
        },
      ],
    });
    petTestMocks.currentMonitor.mockResolvedValue({
      size: { width: 1920, height: 1080 },
      scaleFactor: 1,
    });
    for (const mock of [
      petTestMocks.applyPetClickThrough,
      petTestMocks.hidePetWindow,
      petTestMocks.movePetWindow,
      petTestMocks.setPetHitRect,
      petTestMocks.showMainWindow,
    ]) {
      mock.mockResolvedValue(undefined);
    }
    Object.defineProperty(HTMLElement.prototype, "setPointerCapture", {
      configurable: true,
      value: vi.fn(),
    });
    vi.spyOn(Math, "random").mockReturnValue(0);
  });

  it("uses the current locale when the pet is clicked after a language change", async () => {
    render(
      <I18nProvider>
        <DesktopPet />
      </I18nProvider>,
    );

    await waitFor(() => {
      expect(petTestMocks.listeners.has("language-changed")).toBe(true);
      expect(screen.getByTitle("Friend Pixel Pet")).toBeInTheDocument();
    });

    await act(async () => {
      petTestMocks.listeners.get("language-changed")?.({ payload: "th" });
    });

    const sprite = screen.getByTitle("Friend Pixel Pet");
    fireEvent.pointerDown(sprite, {
      button: 0,
      pointerId: 1,
      screenX: 100,
      screenY: 100,
    });
    fireEvent.pointerUp(sprite, {
      button: 0,
      pointerId: 1,
      screenX: 100,
      screenY: 100,
    });

    expect(await screen.findByRole("status")).toHaveTextContent("วันนี้ใช้ไป");
    expect(screen.getByRole("status")).not.toHaveTextContent(
      "Used 12.5M tokens today",
    );
  });
});
