import { translate } from "../../lib/i18n";
import { formatCompactTokenCount } from "../../lib/token-format";

/** Time and presentation helpers shared by every widget layout. */
export type WidgetTranslator = (
  key: string,
  vars?: Record<string, string>,
) => string;

const defaultTranslator: WidgetTranslator = (key, vars) =>
  translate("en", key, vars);

/** English fallback values retained for callers and regression tests. */
export const RESET_UNAVAILABLE = "Reset time unavailable";
export const REMAINING_UNAVAILABLE = "Unavailable";

const MINUTE_MS = 60_000;
const HOUR_MS = 60 * MINUTE_MS;
const DAY_MS = 24 * HOUR_MS;

function localize(
  t: WidgetTranslator,
  key: string,
  vars?: Record<string, string>,
): string {
  return t(key, vars);
}

export function formatResetLabel(
  resetAt: string | null | undefined,
  now: number = Date.now(),
  t: WidgetTranslator = defaultTranslator,
): string {
  if (!resetAt) {
    return localize(t, "widget.time.resetUnavailable");
  }
  const target = new Date(resetAt).getTime();
  if (Number.isNaN(target)) {
    return localize(t, "widget.time.resetUnavailable");
  }
  const remaining = target - now;
  if (remaining <= 0) {
    return localize(t, "widget.time.resetNow");
  }
  if (remaining < HOUR_MS) {
    const minutes = Math.max(1, Math.round(remaining / MINUTE_MS));
    return localize(t, "widget.time.resetInMinutes", { minutes: String(minutes) });
  }
  if (remaining < DAY_MS) {
    const hours = Math.floor(remaining / HOUR_MS);
    const minutes = Math.floor((remaining % HOUR_MS) / MINUTE_MS);
    return localize(t, "widget.time.resetInHoursMinutes", {
      hours: String(hours),
      minutes: String(minutes),
    });
  }
  if (calendarDaysBetween(now, target) === 1) {
    return localize(t, "widget.time.resetTomorrow");
  }
  const days = Math.floor(remaining / DAY_MS);
  const hours = Math.floor((remaining % DAY_MS) / HOUR_MS);
  return localize(t, "widget.time.resetInDaysHours", {
    days: String(days),
    hours: String(hours),
  });
}

function calendarDaysBetween(from: number, to: number): number {
  const start = new Date(from);
  const end = new Date(to);
  const startDay = new Date(
    start.getFullYear(),
    start.getMonth(),
    start.getDate(),
  ).getTime();
  const endDay = new Date(
    end.getFullYear(),
    end.getMonth(),
    end.getDate(),
  ).getTime();
  return Math.round((endDay - startDay) / DAY_MS);
}

export function formatResetShort(
  resetAt: string | null | undefined,
  now: number = Date.now(),
  t: WidgetTranslator = defaultTranslator,
  locale = "en",
): string {
  if (!resetAt) {
    return localize(t, "widget.time.shortUnavailable");
  }
  const target = new Date(resetAt).getTime();
  if (Number.isNaN(target)) {
    return localize(t, "widget.time.shortUnavailable");
  }
  const remaining = target - now;
  if (remaining <= 0) {
    return localize(t, "widget.time.now");
  }
  if (remaining < HOUR_MS) {
    return localize(t, "widget.time.minutes", {
      minutes: String(Math.max(1, Math.round(remaining / MINUTE_MS))),
    });
  }
  if (remaining < DAY_MS) {
    const hours = Math.floor(remaining / HOUR_MS);
    const minutes = Math.floor((remaining % HOUR_MS) / MINUTE_MS);
    return localize(t, "widget.time.hoursMinutes", {
      hours: String(hours),
      minutes: String(minutes).padStart(2, "0"),
    });
  }
  const date = new Date(target);
  const weekday = date.toLocaleDateString(locale, { weekday: "short" });
  const time = date.toLocaleTimeString(locale, {
    hour: "2-digit",
    minute: "2-digit",
  });
  return `${weekday} ${time}`;
}

export function windowSubtitle(
  scope: string,
  kind: string,
  label: string,
  t: WidgetTranslator = defaultTranslator,
): string {
  const unitKey =
    kind === "credits"
      ? "widget.time.unitCredits"
      : kind === "tokens"
        ? "widget.time.unitTokens"
        : kind === "parallel"
          ? "widget.time.unitParallel"
          : "widget.time.unitRequests";
  const unit = localize(t, unitKey);
  switch (scope) {
    case "rolling":
      return localize(t, "widget.time.windowSliding", { label: label.toLowerCase(), unit });
    case "session":
      return localize(t, "widget.time.windowSession", { unit });
    case "daily":
      return localize(t, "widget.time.windowDaily", { unit });
    case "weekly":
      return localize(t, "widget.time.windowWeekly", { unit });
    case "monthly":
      return localize(t, "widget.time.windowMonthly", { unit });
    default:
      return localize(t, "widget.time.windowLimit", { unit });
  }
}

export function formatCountdown(
  resetAt: string | null,
  now: number = Date.now(),
  t: WidgetTranslator = defaultTranslator,
): string | null {
  if (!resetAt || Number.isNaN(new Date(resetAt).getTime())) {
    return null;
  }
  const target = new Date(resetAt).getTime();
  const remaining = target - now;
  if (remaining <= 0) return localize(t, "widget.time.now");
  if (remaining < HOUR_MS) {
    return localize(t, "widget.time.minutes", {
      minutes: String(Math.max(1, Math.round(remaining / MINUTE_MS))),
    });
  }
  if (remaining < DAY_MS) {
    return localize(t, "widget.time.hoursMinutes", {
      hours: String(Math.floor(remaining / HOUR_MS)),
      minutes: String(Math.floor((remaining % HOUR_MS) / MINUTE_MS)),
    });
  }
  if (calendarDaysBetween(now, target) === 1) {
    return localize(t, "widget.time.tomorrow");
  }
  return localize(t, "widget.time.daysHours", {
    days: String(Math.floor(remaining / DAY_MS)),
    hours: String(Math.floor((remaining % DAY_MS) / HOUR_MS)),
  });
}

export function formatRefreshedAgo(
  collectedAt: string | null | undefined,
  now: number = Date.now(),
  t: WidgetTranslator = defaultTranslator,
): string {
  if (!collectedAt) return localize(t, "widget.time.never");
  const collected = new Date(collectedAt).getTime();
  if (Number.isNaN(collected)) return localize(t, "widget.time.unknown");
  const seconds = Math.max(0, Math.floor((now - collected) / 1000));
  if (seconds < 60) {
    return localize(t, "widget.time.agoSeconds", { value: String(seconds) });
  }
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) {
    return localize(t, "widget.time.agoMinutes", { value: String(minutes) });
  }
  const hours = Math.floor(minutes / 60);
  if (hours < 24) {
    return localize(t, "widget.time.agoHours", { value: String(hours) });
  }
  return localize(t, "widget.time.agoDays", { value: String(Math.floor(hours / 24)) });
}

export function formatCompact(value: number): string {
  return Number.isFinite(value)
    ? formatCompactTokenCount(value)
    : REMAINING_UNAVAILABLE;
}

export type QuotaLevel = "normal" | "warning" | "critical";

export function quotaLevel(remainingPercent: number): QuotaLevel {
  if (remainingPercent < 20) return "critical";
  if (remainingPercent <= 50) return "warning";
  return "normal";
}

export function formatRemaining(
  remainingPercent: number | null,
  t: WidgetTranslator = defaultTranslator,
): string {
  if (remainingPercent === null || !Number.isFinite(remainingPercent)) {
    return localize(t, "widget.time.unavailable");
  }
  const clamped = Math.max(0, Math.min(100, remainingPercent));
  return localize(t, "widget.time.remaining", {
    value: String(Math.round(clamped)),
  });
}
