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
    // jsdom has no PointerEvent; React's onPointerDown/Move read button and
    // screenX off the native event, so polyfill with MouseEvent which carries
    // both in jsdom.
    if (typeof window.PointerEvent !== "function") {
      class PointerEventPolyfill extends MouseEvent {
        pointerId: number;
        constructor(type: string, init: PointerEventInit = {}) {
          super(type, init);
          this.pointerId = init.pointerId ?? 0;
        }
      }
      Object.defineProperty(window, "PointerEvent", {
        configurable: true,
        value: PointerEventPolyfill,
      });
    }
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

  it("keeps the full tooltip width inside the interactive hit rectangle", async () => {
    render(
      <I18nProvider>
        <DesktopPet />
      </I18nProvider>,
    );

    const sprite = await screen.findByTitle("Friend Pixel Pet");
    fireEvent.mouseEnter(sprite);

    await waitFor(() => {
      const calls = petTestMocks.setPetHitRect.mock.calls;
      const lastRect = calls[calls.length - 1]?.[0] as
        | [number, number, number, number]
        | undefined;
      expect(lastRect?.[2]).toBe(window.innerWidth);
    });
  });

  function spriteMetrics() {
    const viewW = window.innerWidth;
    const viewH = window.innerHeight;
    const spriteW = Math.max(48, Math.min(96, viewW - 12));
    const spriteH = Math.round(spriteW * (208 / 192));
    return {
      viewW,
      viewH,
      spriteW,
      spriteH,
      hOffset: (viewW - spriteW) / 2,
      vOffset: viewH - spriteH - 6,
      screenW: 1920,
      screenH: 1080,
    };
  }

  function startPosition(m: ReturnType<typeof spriteMetrics>) {
    const spriteLeft = (m.screenW - m.viewW) / 2 + m.hOffset;
    const spriteTop = m.screenH - m.viewH - 48 + m.vOffset;
    return {
      x: spriteLeft - m.hOffset,
      y: spriteTop - m.vOffset,
    };
  }

  function lastMove(): [number, number] {
    const calls = petTestMocks.movePetWindow.mock.calls;
    return [calls.at(-1)?.[0] as number, calls.at(-1)?.[1] as number];
  }

  async function dragSprite(dx: number, dy: number) {
    render(
      <I18nProvider>
        <DesktopPet />
      </I18nProvider>,
    );
    const sprite = await screen.findByTitle("Friend Pixel Pet");
    await act(async () => {
      fireEvent.pointerDown(sprite, {
        button: 0,
        pointerId: 1,
        screenX: 100,
        screenY: 100,
      });
      fireEvent.pointerMove(sprite, {
        button: 0,
        pointerId: 1,
        screenX: 100 + dx,
        screenY: 100 + dy,
      });
      fireEvent.pointerUp(sprite, {
        button: 0,
        pointerId: 1,
        screenX: 100 + dx,
        screenY: 100 + dy,
      });
    });
    return lastMove();
  }

  it("lets the pet sprite touch the left edge of the screen", async () => {
    const m = spriteMetrics();
    const start = startPosition(m);
    const [x, y] = await dragSprite(-10_000, 0);

    // The sprite's left edge must sit at screen x=0: the window itself may
    // go off screen because the sprite is centered inside the window box.
    expect(x).toBeCloseTo(0 - m.hOffset);
    expect(y).toBe(start.y);
  });

  it("lets the pet sprite touch the right edge of the screen", async () => {
    const m = spriteMetrics();
    const [x] = await dragSprite(10_000, 0);

    const expected = m.screenW - m.spriteW - m.hOffset;
    expect(x).toBeCloseTo(expected);
  });

  it("lets the pet sprite touch the top edge of the screen", async () => {
    const m = spriteMetrics();
    const [, y] = await dragSprite(0, -10_000);

    expect(y).toBeCloseTo(0 - m.vOffset);
  });

  it("lets the pet sprite touch the bottom edge of the screen", async () => {
    const m = spriteMetrics();
    const [, y] = await dragSprite(0, 10_000);

    // Sprite bottom must sit at the screen bottom: the window's y plus the
    // sprite's bottom offset inside the window box.
    expect(y).toBeCloseTo(m.screenH - m.spriteH - m.vOffset);
  });
});


