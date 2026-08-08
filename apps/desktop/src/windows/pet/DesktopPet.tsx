import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { currentMonitor } from "@tauri-apps/api/window";
import {
  applyPetClickThrough,
  fetchPetSpritesheetUrl,
  fetchPetWindowSettings,
  fetchQuotaDashboard,
  hidePetWindow,
  listWidgetPets,
  movePetWindow,
  setPetHitRect,
  showMainWindow,
  type PetManifest,
  type PetWindowSettingsData,
} from "../../lib/native";
import {
  tickMovement,
  pickNextPhase,
  AMBIENT_POSES,
  type PetMovementState,
  type PetSpeed,
} from "./petMovement";
import { pickPetQuip } from "./petQuips";
import { useI18n } from "../../lib/i18n";
import { PetTooltip } from "./PetTooltip";
import "./DesktopPet.css";

/** Ambient poses the current settings enable, as movement states. */
function enabledPosesOf(s: PetWindowSettingsData): PetMovementState[] {
  const poses: Array<{ key: string; state: PetMovementState }> = [
    { key: "pet_pose_wave", state: "wave" },
    { key: "pet_pose_jump", state: "jump" },
    { key: "pet_pose_look_left", state: "look-left" },
    { key: "pet_pose_look_right", state: "look-right" },
    { key: "pet_pose_waiting", state: "waiting" },
    { key: "pet_pose_review", state: "review" },
  ];
  const enabled = new Set<string>();
  if (s.poseWave) enabled.add("pet_pose_wave");
  if (s.poseJump) enabled.add("pet_pose_jump");
  if (s.poseLookLeft) enabled.add("pet_pose_look_left");
  if (s.poseLookRight) enabled.add("pet_pose_look_right");
  if (s.poseWaiting) enabled.add("pet_pose_waiting");
  if (s.poseReview) enabled.add("pet_pose_review");
  return AMBIENT_POSES.filter((pose) =>
    enabled.has(poses.find((entry) => entry.state === pose)?.key ?? ""),
  );
}

/**
 * Desktop pet window.
 *
 * The window is small and MOVES with the pet, so only the pet's own surface
 * intercepts clicks. All positions are LOGICAL (CSS) pixels: the backend
 * window position is physical, so every move is multiplied by the monitor
 * scale factor. Layout adapts to the window's real viewport
 * (`window.innerWidth/Height`), which varies with DPI, so the sprite is
 * always visible inside the window.
 */
/** Sprite base size per size preset (192x208 atlas cell scaled down). */
const SPRITE_BASE: Record<string, number> = {
  small: 64,
  medium: 96,
  large: 128,
};
const SPRITE_ASPECT = 208 / 192;
/** Movement tick: window moves are cheap, 20fps is smooth enough. */
const TICK_MS = 50;
/** Pressed distance (px) before a press becomes a drag instead of a click. */
const DRAG_THRESHOLD_PX = 6;
/** How much space above the sprite stays clickable for tooltips/quips. */
const TOOLTIP_ZONE_H = 260;

interface ContextMenuState {
  x: number;
  y: number;
}

/** Clamps a window position so the window never leaves the screen. */
function clampWindow(
  x: number,
  y: number,
  screenW: number,
  screenH: number,
  viewW: number,
  viewH: number,
): { x: number; y: number } {
  return {
    x: Math.max(0, Math.min(screenW - viewW, x)),
    y: Math.max(0, Math.min(screenH - viewH, y)),
  };
}

/**
 * Desktop pet: a walking character rendered from a codex-pets.net atlas
 * spritesheet. The pet walks across the screen; the small window follows it.
 * Hover shows a usage tooltip, right-click opens a small context menu, and
 * left-press + drag picks the pet up and moves it anywhere on screen.
 */
export function DesktopPet() {
  const { t, language } = useI18n();
  const [settings, setSettings] = useState<PetWindowSettingsData>({
    visible: true,
    character: "",
    speed: "normal",
    opacity: 1.0,
    autoSleep: true,
    sizePreset: "medium",
    stayInPlace: false,
    poseWave: true,
    poseJump: true,
    poseLookLeft: true,
    poseLookRight: true,
    poseWaiting: true,
    poseReview: true,
  });
  const [pets, setPets] = useState<PetManifest[]>([]);
  const [scale, setScale] = useState(1);
  const [monitorSize, setMonitorSize] = useState({ w: 1920, h: 1080 });
  const [viewSize, setViewSize] = useState({ w: 280, h: 340 });
  const [pos, setPos] = useState({ x: 100, y: 200 });
  const [movementState, setMovementState] = useState<PetMovementState>("idle");
  const [direction, setDirection] = useState<"left" | "right">("right");
  const [hovering, setHovering] = useState(false);
  const [menu, setMenu] = useState<ContextMenuState | null>(null);
  const [spriteUrl, setSpriteUrl] = useState<string | null>(null);
  const [speech, setSpeech] = useState<string | null>(null);
  const speechTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const phaseTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const idleSince = useRef(Date.now());
  const dragging = useRef(false);
  const didDrag = useRef(false);
  const dragStart = useRef({ winX: 0, winY: 0, clientX: 0, clientY: 0 });
  const posRef = useRef(pos);
  posRef.current = pos;
  // Movement and scheduling read live state through refs so they never
  // restart or capture a stale closure.
  const movementStateRef = useRef(movementState);
  movementStateRef.current = movementState;
  const directionRef = useRef(direction);
  directionRef.current = direction;
  const settingsRef = useRef(settings);
  settingsRef.current = settings;
  const boundsRef = useRef({ w: 1920, h: 1080 });
  const viewRef = useRef(viewSize);
  viewRef.current = viewSize;

  // Logical screen size = monitor physical size / DPI scale.
  const screenSize = {
    w: Math.max(1, Math.round(monitorSize.w / scale)),
    h: Math.max(1, Math.round(monitorSize.h / scale)),
  };
  boundsRef.current = screenSize;

  const scheduleRef = useRef<() => void>(() => {});
  scheduleRef.current = () => {
    if (phaseTimer.current) clearTimeout(phaseTimer.current);
    const bounds = boundsRef.current;
    const view = viewRef.current;
    const s = settingsRef.current;
    const config = {
      petWidth: view.w,
      screenW: bounds.w,
      screenH: bounds.h,
      speed: s.speed as PetSpeed,
      autoSleep: s.autoSleep,
      stayInPlace: s.stayInPlace,
      enabledPoses: enabledPosesOf(s),
    };
    const next = pickNextPhase(
      movementStateRef.current,
      Date.now() - idleSince.current,
      config,
    );
    if (next.state === "idle") {
      idleSince.current = Date.now();
    }
    setMovementState(next.state);
    setDirection(next.direction);
    phaseTimer.current = setTimeout(scheduleRef.current, next.duration);
  };

  // Load settings + installed pets, subscribe to backend changes.
  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      try {
        const s = await fetchPetWindowSettings();
        if (!cancelled) setSettings(s);
      } catch {
        // Defaults apply outside a Tauri runtime.
      }
      try {
        const installed = await listWidgetPets();
        if (!cancelled) setPets(installed);
      } catch {
        // No pet store; the window shows nothing.
      }
    };
    void load();

    const unlisten = listen<PetWindowSettingsData>(
      "pet-window-settings-changed",
      (event) => {
        if (!cancelled) setSettings(event.payload);
      },
    );
    return () => {
      cancelled = true;
      void unlisten.then((fn) => fn());
    };
  }, []);

  // Monitor size + scale, and the window's own viewport (varies with DPI).
  useEffect(() => {
    let cancelled = false;
    void currentMonitor()
      .then((monitor) => {
        if (!cancelled && monitor) {
          setMonitorSize({ w: monitor.size.width, h: monitor.size.height });
          setScale(monitor.scaleFactor || 1);
        }
      })
      .catch(() => {
        // A wrong size only clamps movement.
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    const update = () =>
      setViewSize({ w: window.innerWidth, h: window.innerHeight });
    update();
    window.addEventListener("resize", update);
    return () => window.removeEventListener("resize", update);
  }, []);

  // The character to render: the selected one, falling back to the first
  // installed pet so a fresh install always shows something.
  const activePet =
    pets.find((pet) => pet.id === settings.character) ?? pets[0];

  // Refresh the roster whenever the character changes: a pet imported while
  // the app is running is not in the list loaded at mount, and without this
  // the pet window would silently fall back to the first default.
  useEffect(() => {
    let cancelled = false;
    void listWidgetPets()
      .then((installed) => {
        if (!cancelled) setPets(installed);
      })
      .catch(() => {
        // Keep the previous roster when the store cannot be read.
      });
    return () => {
      cancelled = true;
    };
  }, [settings.character]);

  // Load the spritesheet as an object URL whenever the character changes.
  useEffect(() => {
    let cancelled = false;
    setSpriteUrl(null);
    if (!activePet) return undefined;
    void fetchPetSpritesheetUrl(activePet.id)
      .then((url) => {
        if (!cancelled) setSpriteUrl(url);
      })
      .catch(() => {
        if (!cancelled) setSpriteUrl(null);
      });
    return () => {
      cancelled = true;
    };
  }, [activePet?.id]); // eslint-disable-line react-hooks/exhaustive-deps

  // Start centered near the bottom of the screen.
  useEffect(() => {
    const start = clampWindow(
      (screenSize.w - viewSize.w) / 2,
      screenSize.h - viewSize.h - 48,
      screenSize.w,
      screenSize.h,
      viewSize.w,
      viewSize.h,
    );
    setPos(start);
  }, [screenSize.w, screenSize.h, viewSize.w, viewSize.h]);

  // Sprite size follows the size preset, shrinking only when the window
  // viewport is too small for it.
  const base = SPRITE_BASE[settings.sizePreset] ?? SPRITE_BASE.medium;
  const spriteW = Math.max(48, Math.min(base, viewSize.w - 12));
  const spriteH = Math.round(spriteW * SPRITE_ASPECT);

  // Push the window position to the backend whenever it changes (physical).
  useEffect(() => {
    void movePetWindow(
      Math.round(pos.x * scale),
      Math.round(pos.y * scale),
    ).catch(() => {});
  }, [pos, scale]);

  // Report the clickable rectangle (sprite + tooltip, in logical screen
  // pixels) so the backend makes the transparent window click-through
  // everywhere else. The rect grows while a tooltip or quip is showing so it
  // stays interactive.
  useEffect(() => {
    const tooltipShown = speech !== null || hovering;
    const rect: [number, number, number, number] = [
      pos.x + (viewSize.w - spriteW) / 2,
      pos.y + viewSize.h - spriteH - 6 - (tooltipShown ? TOOLTIP_ZONE_H : 0),
      spriteW,
      spriteH + 6 + (tooltipShown ? TOOLTIP_ZONE_H : 0),
    ];
    void setPetHitRect(rect).catch(() => {});
  }, [pos, viewSize, spriteW, spriteH, speech, hovering]);

  // Periodically apply the click-through state the backend computed from the
  // cursor position. Window APIs must be called from this (UI) thread, so the
  // backend's polling thread only stores the result.
  useEffect(() => {
    const poll = setInterval(() => {
      void applyPetClickThrough().catch(() => {});
    }, 200);
    return () => clearInterval(poll);
  }, []);

  // Movement loop: advance the window and bounce off screen edges.
  useEffect(() => {
    const interval = setInterval(() => {
      if (dragging.current) return;
      const state = movementStateRef.current;
      const dir = directionRef.current;
      const bounds = boundsRef.current;
      const view = viewRef.current;
      const config = {
        petWidth: view.w,
        screenW: bounds.w,
        screenH: bounds.h,
        speed: settingsRef.current.speed as PetSpeed,
        autoSleep: settingsRef.current.autoSleep,
      };
      setPos((prev) => {
        const result = tickMovement(prev.x, prev.y, state, dir, config);
        if (result.state !== state) setMovementState(result.state);
        if (result.direction !== dir) setDirection(result.direction);
        return { x: result.x, y: result.y };
      });
    }, TICK_MS);
    return () => clearInterval(interval);
  }, []);

  // Phase scheduling: pick the next movement phase when the current ends.
  // Runs exactly once; the timer chain drives itself from then on.
  useEffect(() => {
    scheduleRef.current();
    return () => {
      if (phaseTimer.current) clearTimeout(phaseTimer.current);
    };
  }, []);

  // Drag: press + move picks the pet up. The pet stops walking immediately:
  // the phase timer is cancelled and the pose settles to idle, so the drag
  // never fights the movement loop. Deltas are tracked in SCREEN coordinates:
  // client coordinates shift whenever the window itself moves, which would
  // feed the drag's own motion back into it and make the pet jitter.
  const onPointerDown = useCallback((e: React.PointerEvent) => {
    if (e.button !== 0) return;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    dragging.current = true;
    if (phaseTimer.current) {
      clearTimeout(phaseTimer.current);
      phaseTimer.current = null;
    }
    setMovementState("idle");
    dragStart.current = {
      winX: posRef.current.x,
      winY: posRef.current.y,
      clientX: e.screenX,
      clientY: e.screenY,
    };
  }, []);

  const onPointerMove = useCallback((e: React.PointerEvent) => {
    if (!dragging.current) return;
    const start = dragStart.current;
    const dx = e.screenX - start.clientX;
    const dy = e.screenY - start.clientY;
    if (Math.abs(dx) < DRAG_THRESHOLD_PX && Math.abs(dy) < DRAG_THRESHOLD_PX) {
      return;
    }
    didDrag.current = true;
    const bounds = boundsRef.current;
    const view = viewRef.current;
    const next = clampWindow(
      start.winX + dx,
      start.winY + dy,
      bounds.w,
      bounds.h,
      view.w,
      view.h,
    );
    void movePetWindow(
      Math.round(next.x * scale),
      Math.round(next.y * scale),
    ).catch(() => {});
    setPos(next);
  }, [scale]);

  // A click without dragging makes the pet say something: a quip built from
  // the live quota dashboard, held for a few seconds.
  const triggerTap = useCallback(() => {
    void fetchQuotaDashboard()
      .then((dashboard) => {
        let lowest: number | null = null;
        let plan: string | null = null;
        let tokens = 0;
        let cost = 0;
        for (const provider of dashboard.providers) {
          if (provider.plan) plan = provider.plan;
          for (const window of provider.windows) {
            if (window.used_percent !== null) {
              const remaining = window.remaining_percent;
              if (remaining !== null && (lowest === null || remaining < lowest)) {
                lowest = remaining;
              }
            }
            tokens += window.used;
            cost += window.used_percent !== null ? 0 : 0;
          }
        }
        return pickPetQuip(
          {
            todayTokens: tokens,
            costUsd: cost,
            currencySymbol: "$",
            lowestRemainingPercent: lowest,
            plan,
          },
          language,
        );
      })
      .catch(() =>
        pickPetQuip(
          {
            todayTokens: 0,
            costUsd: 0,
            currencySymbol: "$",
            lowestRemainingPercent: null,
            plan: null,
          },
          language,
        ),
      )
      .then((quip) => {
        if (speechTimer.current) clearTimeout(speechTimer.current);
        setSpeech(quip);
        speechTimer.current = setTimeout(() => setSpeech(null), 3200);
      });
  }, []);

  const onPointerUp = useCallback(() => {
    dragging.current = false;
    idleSince.current = Date.now();
    // A plain click (no drag) makes the pet say something.
    if (!didDrag.current) {
      triggerTap();
    }
    didDrag.current = false;
    // Resume normal behaviour after the drag settles.
    setTimeout(() => {
      if (!dragging.current && !phaseTimer.current) {
        scheduleRef.current();
      }
    }, 120);
  }, [triggerTap]);

  // Right-click: small context menu with Close / Settings.
  const onContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    setMenu({ x: e.clientX, y: e.clientY });
  }, []);

  const closeMenu = useCallback(() => setMenu(null), []);

  const handleClosePet = useCallback(() => {
    closeMenu();
    void hidePetWindow().catch(() => {});
  }, [closeMenu]);

  const handleOpenSettings = useCallback(() => {
    closeMenu();
    void showMainWindow().catch(() => {});
  }, [closeMenu]);

  // Dismiss the menu with Escape or any click.
  useEffect(() => {
    if (!menu) return undefined;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") closeMenu();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [menu, closeMenu]);

  const stateClass =
    movementState === "walk-left"
      ? "pet-state-walk-left"
      : movementState === "walk-right"
        ? "pet-state-walk-right"
        : movementState === "sleep"
          ? "pet-state-sleep"
          : movementState === "wave"
            ? "pet-state-wave"
            : movementState === "jump"
              ? "pet-state-jump"
              : movementState === "look-left"
                ? "pet-state-look-left"
                : movementState === "look-right"
                  ? "pet-state-look-right"
                  : movementState === "waiting"
                    ? "pet-state-waiting"
                    : movementState === "review"
                      ? "pet-state-review"
                      : "pet-state-idle";

  return (
    <div
      className="pet-window"
      style={{ opacity: settings.opacity }}
      onClick={closeMenu}
      onContextMenu={(e) => e.preventDefault()}
    >
      <div className="pet-stage">
        <div
          className="pet-sprite"
          onMouseEnter={() => setHovering(true)}
          onMouseLeave={() => setHovering(false)}
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerUp}
          onContextMenu={onContextMenu}
          title={activePet?.displayName}
        >
          {activePet && spriteUrl && (
            <div className="pet-cell" style={{ width: spriteW, height: spriteH }}>
              <div
                className={`pet-atlas ${stateClass}`}
                data-sprite-version={activePet.spriteVersionNumber || 1}
                style={{
                  backgroundImage: `url(${spriteUrl})`,
                  "--pet-cell-w": `${spriteW}px`,
                } as React.CSSProperties}
                aria-hidden="true"
              />
            </div>
          )}
        </div>

        {(speech || (hovering && !menu && activePet)) && (
          <div
            className="pet-tooltip-anchor"
            onMouseEnter={() => setHovering(true)}
            onMouseLeave={() => setHovering(false)}
          >
            {speech ? (
              <div className="pet-tooltip" role="status" aria-live="polite">
                <div className="pet-tooltip-inner pet-tooltip-speech">
                  <span className="pet-tooltip-empty">{speech}</span>
                </div>
                <span className="pet-tooltip-arrow" />
              </div>
            ) : (
              <PetTooltip visible={hovering} />
            )}
          </div>
        )}
      </div>

      {menu && (
        <div
          className="pet-menu"
          style={{ left: menu.x, top: menu.y }}
          role="menu"
          aria-label={t("pet.menu.options")}
          onClick={(e) => e.stopPropagation()}
        >
          <button
            type="button"
            className="pet-menu-item"
            role="menuitem"
            onClick={handleOpenSettings}
          >
            {t("pet.menu.settings")}
          </button>
          <button
            type="button"
            className="pet-menu-item pet-menu-item-danger"
            role="menuitem"
            onClick={handleClosePet}
          >
            {t("pet.menu.close")}
          </button>
        </div>
      )}
    </div>
  );
}
