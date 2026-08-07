import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { currentMonitor } from "@tauri-apps/api/window";
import {
  fetchPetSpritesheetUrl,
  fetchPetWindowSettings,
  hidePetWindow,
  listWidgetPets,
  movePetWindow,
  showMainWindow,
  type PetManifest,
  type PetWindowSettingsData,
} from "../../lib/native";
import {
  tickMovement,
  pickNextPhase,
  type PetMovementState,
  type PetSpeed,
} from "./petMovement";
import { PetTooltip } from "./PetTooltip";
import "./DesktopPet.css";

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
  const [settings, setSettings] = useState<PetWindowSettingsData>({
    visible: true,
    character: "",
    speed: "normal",
    opacity: 1.0,
    auto_sleep: true,
    size_preset: "medium",
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

  const phaseTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const idleSince = useRef(Date.now());
  const dragging = useRef(false);
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
    const config = {
      petWidth: view.w,
      screenW: bounds.w,
      screenH: bounds.h,
      speed: settingsRef.current.speed as PetSpeed,
      autoSleep: settingsRef.current.auto_sleep,
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

  // Push the window position to the backend whenever it changes (physical).
  useEffect(() => {
    void movePetWindow(
      Math.round(pos.x * scale),
      Math.round(pos.y * scale),
    ).catch(() => {});
  }, [pos, scale]);

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
        autoSleep: settingsRef.current.auto_sleep,
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

  // Drag: press + move picks the pet up. Pointer capture keeps the drag
  // alive even when the cursor runs ahead of the small window.
  const onPointerDown = useCallback((e: React.PointerEvent) => {
    if (e.button !== 0) return;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    dragging.current = true;
    dragStart.current = {
      winX: posRef.current.x,
      winY: posRef.current.y,
      clientX: e.clientX,
      clientY: e.clientY,
    };
  }, []);

  const onPointerMove = useCallback((e: React.PointerEvent) => {
    if (!dragging.current) return;
    const start = dragStart.current;
    const dx = e.clientX - start.clientX;
    const dy = e.clientY - start.clientY;
    if (Math.abs(dx) < DRAG_THRESHOLD_PX && Math.abs(dy) < DRAG_THRESHOLD_PX) {
      return;
    }
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

  const onPointerUp = useCallback(() => {
    dragging.current = false;
    idleSince.current = Date.now();
  }, []);

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
          : "pet-state-idle";

  // Sprite size follows the size preset, shrinking only when the window
  // viewport is too small for it.
  const base = SPRITE_BASE[settings.size_preset] ?? SPRITE_BASE.medium;
  const spriteW = Math.max(48, Math.min(base, viewSize.w - 12));
  const spriteH = Math.round(spriteW * SPRITE_ASPECT);

  return (
    <div
      className="pet-window"
      style={{ opacity: settings.opacity }}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onMouseLeave={() => setHovering(false)}
      onClick={closeMenu}
      onContextMenu={(e) => e.preventDefault()}
    >
      <div className="pet-stage">
        <div
          className="pet-sprite"
          onMouseEnter={() => setHovering(true)}
          onMouseLeave={() => setHovering(false)}
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

        {hovering && !menu && activePet && (
          <div className="pet-tooltip-anchor">
            <PetTooltip visible={hovering} />
          </div>
        )}
      </div>

      {menu && (
        <div
          className="pet-menu"
          style={{ left: menu.x, top: menu.y }}
          role="menu"
          aria-label="Pet options"
          onClick={(e) => e.stopPropagation()}
        >
          <button
            type="button"
            className="pet-menu-item"
            role="menuitem"
            onClick={handleOpenSettings}
          >
            Pet settings
          </button>
          <button
            type="button"
            className="pet-menu-item pet-menu-item-danger"
            role="menuitem"
            onClick={handleClosePet}
          >
            Close pet
          </button>
        </div>
      )}
    </div>
  );
}
