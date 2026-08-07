export const ALERTS_UPDATED_EVENT = "lnwdeck:alerts-updated";

export function emitAlertsUpdated(): void {
  window.dispatchEvent(new Event(ALERTS_UPDATED_EVENT));
}
