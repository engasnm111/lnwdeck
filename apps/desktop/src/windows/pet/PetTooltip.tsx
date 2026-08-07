/**
 * Hover tooltip for the desktop pet.
 *
 * Shows a glass-morphism speech bubble with every quota window the providers
 * actually published: real remaining-percent windows render as progress bars,
 * and usage-only windows (a limit was never published) render as compact
 * "used" rows. The list is scrollable when it outgrows the space above the
 * pet, so every window stays reachable.
 */

import { useEffect, useState, useCallback } from "react";
import { fetchQuotaDashboard, type QuotaDashboardData } from "../../lib/native";
import { useI18n } from "../../lib/i18n";

function formatCompact(n: number): string {
  if (n >= 1_000_000_000) return `${(n / 1_000_000_000).toFixed(1)}B`;
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

interface QuotaBar {
  provider: string;
  label: string;
  percent: number;
  tone: "ok" | "warn" | "danger";
}

interface UsageRow {
  provider: string;
  label: string;
  used: number;
  kind: string;
}

export function PetTooltip({ visible }: { visible: boolean }) {
  const { t } = useI18n();
  const [data, setData] = useState<QuotaDashboardData | null>(null);
  const [totalTokens, setTotalTokens] = useState(0);

  const load = useCallback(async () => {
    try {
      const result = await fetchQuotaDashboard();
      setData(result);
      // Sum the LARGEST window per provider: rolling windows overlap (a 30d
      // window contains the 7d window), so summing every window would count
      // the same tokens several times.
      const perProvider = new Map<string, number>();
      for (const p of result.providers) {
        let best = 0;
        for (const w of p.windows) {
          if (w.used > best) best = w.used;
        }
        if (best > 0) {
          const current = perProvider.get(p.display_name) ?? 0;
          perProvider.set(p.display_name, current + best);
        }
      }
      let total = 0;
      for (const value of perProvider.values()) {
        total += value;
      }
      setTotalTokens(total);
    } catch {
      // Silently ignore — tooltip shows what we have.
    }
  }, []);

  useEffect(() => {
    void load();
    const interval = setInterval(() => void load(), 30_000);
    return () => clearInterval(interval);
  }, [load]);

  if (!visible) return null;

  // Every window the providers published: percent windows become bars,
  // usage-only windows become compact rows.
  const bars: QuotaBar[] = [];
  const usageRows: UsageRow[] = [];
  if (data) {
    for (const p of data.providers) {
      for (const w of p.windows) {
        if (w.remaining_percent !== null && Number.isFinite(w.remaining_percent)) {
          const pct = Math.max(0, Math.min(100, w.remaining_percent));
          bars.push({
            provider: p.display_name,
            label: w.label,
            percent: pct,
            tone: pct < 20 ? "danger" : pct < 50 ? "warn" : "ok",
          });
        } else if (w.used > 0) {
          usageRows.push({
            provider: p.display_name,
            label: w.label,
            used: w.used,
            kind: w.kind,
          });
        }
      }
    }
  }

  const toneColor = (tone: QuotaBar["tone"]) =>
    tone === "danger" ? "#fb7185" : tone === "warn" ? "#fbbf24" : "#34d399";

  const hasRows = bars.length > 0 || usageRows.length > 0;

  return (
    <div className="pet-tooltip" role="status" aria-live="polite">
      <div className="pet-tooltip-inner">
        {totalTokens > 0 && (
          <div className="pet-tooltip-tokens">
            <span className="pet-tooltip-token-count">
              {t("petTooltip.tokens", { count: formatCompact(totalTokens) })}
            </span>
          </div>
        )}
        {hasRows && (
          <div className="pet-tooltip-bars">
            {bars.map((bar) => (
              <div key={`${bar.provider}-${bar.label}`} className="pet-tooltip-bar-row">
                <span className="pet-tooltip-bar-label">
                  {bar.provider} {bar.label}
                </span>
                <span className="pet-tooltip-bar-track">
                  <span
                    className="pet-tooltip-bar-fill"
                    style={{
                      width: `${bar.percent}%`,
                      background: toneColor(bar.tone),
                    }}
                  />
                </span>
                <span className="pet-tooltip-bar-pct">
                  {Math.round(bar.percent)}%
                </span>
              </div>
            ))}
            {usageRows.map((row) => (
              <div key={`${row.provider}-${row.label}`} className="pet-tooltip-bar-row">
                <span className="pet-tooltip-bar-label">
                  {row.provider} {row.label}
                </span>
                <span className="pet-tooltip-bar-track pet-tooltip-bar-track-unknown" />
                <span className="pet-tooltip-bar-pct">
                  {t("petTooltip.used", { used: formatCompact(row.used), kind: row.kind })}
                </span>
              </div>
            ))}
          </div>
        )}
        {!hasRows && (
          <span className="pet-tooltip-empty">No usage yet</span>
        )}
      </div>
      <span className="pet-tooltip-arrow" />
    </div>
  );
}
