import { useCallback, useEffect, useRef, useState } from "react";
import { fetchOverview, OverviewData } from "../../lib/native";

type LockMode = "unlocked" | "locked";

interface WidgetState {
  opacity: number;
  lockMode: LockMode;
}

const STORAGE_KEY = "lnwdeck_widget_state";

function loadWidgetState(): WidgetState {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored) {
      return JSON.parse(stored);
    }
  } catch {
    // ignore
  }
  return { opacity: 1.0, lockMode: "unlocked" };
}

function saveWidgetState(state: WidgetState) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch {
    // ignore
  }
}

export function FloatingWidget() {
  const [data, setData] = useState<OverviewData | null>(null);
  const [state, setState] = useState<WidgetState>(loadWidgetState);
  const dragRef = useRef<HTMLDivElement>(null);

  const load = useCallback(async () => {
    try {
      const result = await fetchOverview();
      setData(result);
    } catch {
      // silently handle
    }
  }, []);

  useEffect(() => {
    load();
    const interval = setInterval(load, 30_000);
    return () => clearInterval(interval);
  }, [load]);

  const toggleLock = () => {
    const newLock = state.lockMode === "locked" ? "unlocked" : "locked";
    const newState = { ...state, lockMode: newLock as LockMode };
    setState(newState);
    saveWidgetState(newState);
  };

  const changeOpacity = (delta: number) => {
    const newOpacity = Math.max(0.1, Math.min(1.0, state.opacity + delta));
    const newState = { ...state, opacity: newOpacity };
    setState(newState);
    saveWidgetState(newState);
  };

  return (
    <div
      ref={dragRef}
      data-tauri-drag-region={state.lockMode === "unlocked" ? "" : undefined}
      style={{
        height: "100vh",
        opacity: state.opacity,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        userSelect: "none",
        padding: "0.5rem",
      }}
    >
      <div style={{ display: "flex", gap: 4, marginBottom: 4 }}>
        <button
          onClick={toggleLock}
          aria-label={
            state.lockMode === "locked" ? "Unlock widget" : "Lock widget"
          }
          style={{ fontSize: "0.75rem" }}
        >
          {state.lockMode === "locked" ? "🔒" : "🔓"}
        </button>
        <button
          onClick={() => changeOpacity(-0.1)}
          aria-label="Decrease opacity"
          style={{ fontSize: "0.75rem" }}
        >
          −
        </button>
        <button
          onClick={() => changeOpacity(0.1)}
          aria-label="Increase opacity"
          style={{ fontSize: "0.75rem" }}
        >
          +
        </button>
      </div>

      {data ? (
        <div style={{ textAlign: "center", fontSize: "0.8rem" }}>
          <p>
            <strong>{data.total_events.toLocaleString()}</strong> events
          </p>
          <p>
            <strong>
              {(
                data.total_tokens_input + data.total_tokens_output
              ).toLocaleString()}
            </strong>{" "}
            tokens
          </p>
        </div>
      ) : (
        <p style={{ fontSize: "0.8rem" }}>lnwdeck</p>
      )}
    </div>
  );
}
