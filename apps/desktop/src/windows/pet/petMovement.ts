/**
 * Movement and pose state machine for the desktop pet.
 *
 * The pet cycles through idle, walking and sleeping, and while idle it
 * randomly plays ambient poses from the spritesheet (waving, jumping,
 * looking around, waiting, reviewing). Walk direction and duration are
 * randomized; the pet bounces off screen edges. Pose states never move the
 * pet — only walk states advance the window.
 */

export type PetMovementState =
  | "idle"
  | "walk-left"
  | "walk-right"
  | "sleep"
  | "wave"
  | "jump"
  | "look-left"
  | "look-right"
  | "waiting"
  | "review";
export type PetSpeed = "slow" | "normal" | "fast";

/** Ambient poses the pet plays at random while idle (not moving). */
export const AMBIENT_POSES: readonly PetMovementState[] = [
  "wave",
  "jump",
  "look-left",
  "look-right",
  "waiting",
  "review",
];

/** Pixels per frame at each speed tier. */
const SPEED_PX: Record<PetSpeed, number> = { slow: 1.2, normal: 2.4, fast: 4.0 };

/** Duration ranges in ms. */
const IDLE_MS: [number, number] = [3000, 12000];
const WALK_MS: [number, number] = [2000, 8000];
const SLEEP_MS: [number, number] = [15000, 45000];
const POSE_MS: [number, number] = [1800, 4000];
/** How long until the pet decides to sleep (ms of continuous idle). */
const AUTO_SLEEP_THRESHOLD_MS = 30000;
/** Chance that an idle phase turns into an ambient pose instead of a walk. */
const AMBIENT_CHANCE = 0.45;

function rand(min: number, max: number): number {
  return min + Math.random() * (max - min);
}

function pick<T>(items: readonly T[]): T {
  return items[Math.floor(Math.random() * items.length)];
}

export interface MovementConfig {
  /** Width of the pet sprite in px. */
  petWidth: number;
  /** Screen width in px. */
  screenW: number;
  /** Screen height in px. */
  screenH: number;
  /** Current speed setting. */
  speed: PetSpeed;
  /** Whether auto-sleep is enabled. */
  autoSleep: boolean;
}

export interface MovementResult {
  x: number;
  y: number;
  state: PetMovementState;
  direction: "left" | "right";
  /** Whether to transition to a new phase on next tick. */
  nextPhase: boolean;
}

/**
 * Advances the pet position by one frame.
 *
 * Returns the new position, the movement state, and whether the current
 * phase is over (so the caller should pick a new phase).
 */
export function tickMovement(
  x: number,
  y: number,
  state: PetMovementState,
  direction: "left" | "right",
  config: MovementConfig,
): MovementResult {
  const { petWidth, screenW, speed } = config;
  const px = SPEED_PX[speed];

  if (state === "walk-left" || state === "walk-right") {
    const dx = state === "walk-left" ? -px : px;
    let nx = x + dx;
    let nd = direction;

    // Bounce off edges.
    if (nx <= 0) {
      nx = 0;
      nd = "right";
      return { x: nx, y, state: "walk-right", direction: nd, nextPhase: true };
    }
    if (nx >= screenW - petWidth) {
      nx = screenW - petWidth;
      nd = "left";
      return { x: nx, y, state: "walk-left", direction: nd, nextPhase: true };
    }

    return { x: nx, y, state, direction: nd, nextPhase: false };
  }

  // Idle, sleep and ambient poses stay in place.
  return { x, y, state, direction, nextPhase: false };
}

/** Picks the next phase and its duration. */
export function pickNextPhase(
  current: PetMovementState,
  idleContinuity: number,
  config: MovementConfig,
): { state: PetMovementState; direction: "left" | "right"; duration: number } {
  // If we've been idle long enough and auto-sleep is on, go to sleep.
  if (
    config.autoSleep &&
    current === "idle" &&
    idleContinuity >= AUTO_SLEEP_THRESHOLD_MS
  ) {
    return {
      state: "sleep",
      direction: Math.random() > 0.5 ? "left" : "right",
      duration: rand(...SLEEP_MS),
    };
  }

  // After sleeping, always wake to idle.
  if (current === "sleep") {
    return {
      state: "idle",
      direction: Math.random() > 0.5 ? "left" : "right",
      duration: rand(...IDLE_MS),
    };
  }

  // After walking or an ambient pose, rest.
  if (current === "walk-left" || current === "walk-right" || AMBIENT_POSES.includes(current)) {
    return {
      state: "idle",
      direction: Math.random() > 0.5 ? "left" : "right",
      duration: rand(...IDLE_MS),
    };
  }

  // After idle, sometimes play an ambient pose, otherwise walk.
  if (Math.random() < AMBIENT_CHANCE) {
    const pose = pick(AMBIENT_POSES);
    return {
      state: pose,
      direction:
        pose === "look-left" ? "left" : pose === "look-right" ? "right" : Math.random() > 0.5 ? "left" : "right",
      duration: rand(...POSE_MS),
    };
  }
  const dir: "left" | "right" = Math.random() > 0.5 ? "left" : "right";
  return {
    state: dir === "left" ? "walk-left" : "walk-right",
    direction: dir,
    duration: rand(...WALK_MS),
  };
}
