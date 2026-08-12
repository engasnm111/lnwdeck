import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { RefreshProgressEvent } from "./native";
import { useDebouncedCallback } from "./use-debounced-reload";
import { useLatestRequestGuard } from "./use-latest-request-guard";
import { createLatestWinsScheduler } from "./latest-wins-scheduler";

export type RefreshEvent = "usage-updated" | "quota-updated";

// One shared lane per WebView for the first load of each mounted page. Route
// unmount/remount must not create an unbounded native-read backlog. Reloads
// within the same mounted page use a local lane for background work, while
// foreground filter/range changes bypass it so stale work cannot block input.
const pageReadScheduler = createLatestWinsScheduler();

export interface UsePageLoadOptions<T> {
  load: () => Promise<T>;
  deps?: readonly unknown[];
  refreshEvents?: RefreshEvent[];
  /** Reload in the background when refresh-progress completes. */
  listenRefreshProgress?: boolean;
  debounceMs?: number;
  enabled?: boolean;
}

export interface UsePageLoadResult<T> {
  data: T | null;
  loading: boolean;
  error: Error | null;
  reload: () => Promise<void>;
  reloadBackground: () => Promise<void>;
  setData: React.Dispatch<React.SetStateAction<T | null>>;
}

/**
 * Loads page data with stale-request cancellation and debounced reloads
 * after background sync events.
 */
export function usePageLoad<T>({
  load,
  deps = [],
  refreshEvents = [],
  listenRefreshProgress = false,
  debounceMs = 1200,
  enabled = true,
}: UsePageLoadOptions<T>): UsePageLoadResult<T> {
  const [data, setData] = useState<T | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);
  const loadRef = useRef(load);
  const firstLoadRef = useRef(true);
  const backgroundLoadSchedulerRef = useRef(createLatestWinsScheduler());
  const beginRequest = useLatestRequestGuard(deps);

  useEffect(() => {
    loadRef.current = load;
  }, [load]);

  const runLoad = useCallback(
    (background: boolean): Promise<void> => {
      if (!enabled) return Promise.resolve();

      const isCurrent = beginRequest();
      if (!background) {
        setLoading(true);
      }
      setError(null);

      return (async () => {
        try {
          const scheduled = firstLoadRef.current
            ? await (() => {
                firstLoadRef.current = false;
                return pageReadScheduler.run(async () => {
                  const local = await backgroundLoadSchedulerRef.current.run(() =>
                    loadRef.current(),
                  );
                  if (local.status !== "completed") {
                    throw new Error("initial page load was unexpectedly superseded");
                  }
                  return local.value;
                });
              })()
            : background
              ? await backgroundLoadSchedulerRef.current.run(() => loadRef.current())
              : { status: "completed" as const, value: await loadRef.current() };
          if (scheduled.status === "completed" && isCurrent()) {
            setData(scheduled.value);
          }
        } catch (loadError) {
          if (isCurrent()) {
            setError(
              loadError instanceof Error
                ? loadError
                : new Error(String(loadError)),
            );
          }
        } finally {
          if (isCurrent()) {
            setLoading(false);
          }
        }
      })();
    },
    [beginRequest, enabled],
  );

  const reload = useCallback(() => runLoad(false), [runLoad]);
  const reloadBackground = useCallback(() => runLoad(true), [runLoad]);

  const scheduleBackgroundReload = useDebouncedCallback(() => {
    void runLoad(true);
  }, debounceMs);

  const refreshEventsKey = refreshEvents.join("\0");

  useEffect(() => {
    if (!enabled) return;
    void runLoad(false);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- deps drive reloads explicitly
  }, [enabled, runLoad, ...deps]);

  useEffect(() => {
    if (!enabled || refreshEventsKey.length === 0) return;
    let cancelled = false;
    const unlisteners: Array<Promise<() => void>> = [];
    const events = refreshEventsKey.split("\0");

    for (const event of events) {
      unlisteners.push(
        listen(event, () => {
          scheduleBackgroundReload();
        }),
      );
    }

    void Promise.all(unlisteners).then((cleanups) => {
      if (cancelled) {
        for (const cleanup of cleanups) cleanup();
      }
    });

    return () => {
      cancelled = true;
      void Promise.all(unlisteners).then((cleanups) => {
        for (const cleanup of cleanups) cleanup();
      });
    };
  }, [enabled, refreshEventsKey, scheduleBackgroundReload]);

  useEffect(() => {
    if (!enabled || !listenRefreshProgress) return;
    let cancelled = false;
    let unlisten: (() => void) | null = null;

    void listen<RefreshProgressEvent>("refresh-progress", (event) => {
      const phase = event.payload.phase;
      if (phase === "completed" || phase === "partial" || phase === "failed") {
        scheduleBackgroundReload();
      }
    }).then((cleanup) => {
      if (cancelled) {
        cleanup();
      } else {
        unlisten = cleanup;
      }
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [enabled, listenRefreshProgress, scheduleBackgroundReload]);

  return {
    data,
    loading,
    error,
    reload,
    reloadBackground,
    setData,
  };
}

/**
 * Orchestrates a custom load function with stale-request cancellation and
 * in-flight deduplication. Use when a page manages multiple independent
 * state fields instead of a single `data` blob.
 */
export function useAsyncLoad(
  load: (background: boolean, isCurrent: () => boolean) => Promise<void>,
  deps: readonly unknown[],
  options?: {
    enabled?: boolean;
    refreshEvents?: RefreshEvent[];
    listenRefreshProgress?: boolean;
    debounceMs?: number;
  },
): { loading: boolean; error: Error | null; reload: () => Promise<void> } {
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);
  const loadRef = useRef(load);
  const firstLoadRef = useRef(true);
  const backgroundLoadSchedulerRef = useRef(createLatestWinsScheduler());
  const beginRequest = useLatestRequestGuard(deps);
  const enabled = options?.enabled ?? true;
  const debounceMs = options?.debounceMs ?? 1200;

  useEffect(() => {
    loadRef.current = load;
  }, [load]);

  const runLoad = useCallback(
    (background: boolean): Promise<void> => {
      if (!enabled) return Promise.resolve();

      const isCurrent = beginRequest();
      if (!background) {
        setLoading(true);
      }
      setError(null);

      return (async () => {
        try {
          if (firstLoadRef.current) {
            firstLoadRef.current = false;
            await pageReadScheduler.run(async () => {
              await backgroundLoadSchedulerRef.current.run(() =>
                loadRef.current(background, isCurrent),
              );
            });
          } else if (background) {
            await backgroundLoadSchedulerRef.current.run(() =>
              loadRef.current(background, isCurrent),
            );
          } else {
            await loadRef.current(background, isCurrent);
          }
        } catch (loadError) {
          if (isCurrent()) {
            setError(
              loadError instanceof Error
                ? loadError
                : new Error(String(loadError)),
            );
          }
        } finally {
          if (isCurrent()) {
            setLoading(false);
          }
        }
      })();
    },
    [beginRequest, enabled],
  );

  const reload = useCallback(() => runLoad(false), [runLoad]);

  const scheduleBackgroundReload = useDebouncedCallback(() => {
    void runLoad(true);
  }, debounceMs);

  const refreshEventsKey = (options?.refreshEvents ?? []).join("\0");

  useEffect(() => {
    if (!enabled) return;
    void runLoad(false);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- deps drive reloads explicitly
  }, [enabled, runLoad, ...deps]);

  useEffect(() => {
    if (!enabled || refreshEventsKey.length === 0) return;
    let cancelled = false;
    const unlisteners: Array<Promise<() => void>> = [];
    const events = refreshEventsKey.split("\0");

    for (const event of events) {
      unlisteners.push(
        listen(event, () => {
          scheduleBackgroundReload();
        }),
      );
    }

    void Promise.all(unlisteners).then((cleanups) => {
      if (cancelled) {
        for (const cleanup of cleanups) cleanup();
      }
    });

    return () => {
      cancelled = true;
      void Promise.all(unlisteners).then((cleanups) => {
        for (const cleanup of cleanups) cleanup();
      });
    };
  }, [enabled, refreshEventsKey, scheduleBackgroundReload]);

  useEffect(() => {
    if (!enabled || !options?.listenRefreshProgress) return;
    let cancelled = false;
    let unlisten: (() => void) | null = null;

    void listen<RefreshProgressEvent>("refresh-progress", (event) => {
      const phase = event.payload.phase;
      if (phase === "completed" || phase === "partial" || phase === "failed") {
        scheduleBackgroundReload();
      }
    }).then((cleanup) => {
      if (cancelled) {
        cleanup();
      } else {
        unlisten = cleanup;
      }
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [enabled, options?.listenRefreshProgress, scheduleBackgroundReload]);

  return { loading, error, reload };
}
