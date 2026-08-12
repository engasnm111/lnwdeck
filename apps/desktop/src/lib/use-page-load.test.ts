import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";
import { usePageLoad } from "./use-page-load";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe("usePageLoad", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("loads data on mount", async () => {
    const load = vi.fn(async () => ({ value: 42 }));
    const { result } = renderHook(() =>
      usePageLoad({ load, deps: ["a"] }),
    );

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(load).toHaveBeenCalledTimes(1);
    expect(result.current.data).toEqual({ value: 42 });
    expect(result.current.error).toBeNull();
  });

  it("keeps only the latest page load when routes unmount while a native read is in flight", async () => {
    const first = deferred<string>();
    const loads: string[] = [];

    const mountPage = (page: string) =>
      renderHook(() =>
        usePageLoad({
          load: async () => {
            loads.push(page);
            if (page === "a") return first.promise;
            return page;
          },
          deps: [page],
        }),
      );

    const pageA = mountPage("a");
    await waitFor(() => expect(loads).toEqual(["a"]));
    pageA.unmount();

    const pageB = mountPage("b");
    pageB.unmount();
    const pageC = mountPage("c");
    pageC.unmount();
    const pageD = mountPage("d");

    expect(loads).toEqual(["a"]);

    await act(async () => {
      first.resolve("a");
      await first.promise;
    });

    await waitFor(() => expect(loads).toEqual(["a", "d"]));
    await waitFor(() => expect(pageD.result.current.data).toBe("d"));
    expect(pageD.result.current.loading).toBe(false);
    pageD.unmount();
  });

  it("keeps only one trailing background reload while a load is active", async () => {
    const first = deferred<{ value: number }>();
    const load = vi
      .fn<() => Promise<{ value: number }>>()
      .mockImplementationOnce(() => first.promise)
      .mockResolvedValue({ value: 2 });

    const { result } = renderHook(() =>
      usePageLoad({ load, deps: ["a"] }),
    );

    await waitFor(() => expect(load).toHaveBeenCalledTimes(1));

    act(() => {
      void result.current.reloadBackground();
      void result.current.reloadBackground();
      void result.current.reloadBackground();
    });

    expect(load).toHaveBeenCalledTimes(1);

    await act(async () => {
      first.resolve({ value: 1 });
      await first.promise;
    });

    await waitFor(() => expect(load).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(result.current.data).toEqual({ value: 2 }));
  });

  it("keeps loading false during background reload", async () => {
    const load = vi
      .fn()
      .mockResolvedValueOnce({ value: 1 })
      .mockImplementation(
        () =>
          new Promise<{ value: number }>((resolve) => {
            setTimeout(() => resolve({ value: 2 }), 50);
          }),
      );

    const { result } = renderHook(() =>
      usePageLoad({ load, deps: ["a"] }),
    );

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.data).toEqual({ value: 1 });

    await act(async () => {
      void result.current.reloadBackground();
    });

    expect(result.current.loading).toBe(false);

    await waitFor(() => expect(result.current.data).toEqual({ value: 2 }));
  });
});

describe("usePageLoad refresh events", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("debounces usage-updated reloads", async () => {
    const handlers = new Map<string, () => void>();
    const { listen } = await import("@tauri-apps/api/event");
    vi.mocked(listen).mockImplementation(async (event, handler) => {
      handlers.set(event, handler as () => void);
      return () => handlers.delete(event);
    });

    const load = vi.fn(async () => ({ value: 1 }));
    const { result } = renderHook(() =>
      usePageLoad({
        load,
        deps: ["a"],
        refreshEvents: ["usage-updated"],
        debounceMs: 1200,
      }),
    );

    await waitFor(() => expect(result.current.loading).toBe(false));
    await waitFor(() => expect(handlers.has("usage-updated")).toBe(true));
    load.mockClear();

    act(() => {
      handlers.get("usage-updated")?.();
      handlers.get("usage-updated")?.();
      handlers.get("usage-updated")?.();
    });
    expect(load).not.toHaveBeenCalled();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1200);
    });
    expect(load).toHaveBeenCalledTimes(1);
  });
});
