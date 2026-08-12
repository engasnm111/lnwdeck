import { describe, expect, it } from "vitest";
import { renderHook } from "@testing-library/react";
import { useLatestRequestGuard } from "./use-latest-request-guard";

describe("useLatestRequestGuard", () => {
  it("returns true for the latest request in the same context", () => {
    const { result } = renderHook(() => useLatestRequestGuard(["a"]));
    const isCurrent = result.current();
    expect(isCurrent()).toBe(true);
  });

  it("invalidates a request when dependencies change", async () => {
    let resolveFirst: (() => void) | undefined;
    const firstDone = new Promise<void>((resolve) => {
      resolveFirst = resolve;
    });

    const { result, rerender } = renderHook(
      ({ dep }) => useLatestRequestGuard([dep]),
      { initialProps: { dep: "a" } },
    );

    const firstIsCurrent = result.current();
    rerender({ dep: "b" });
    resolveFirst?.();
    await firstDone;

    expect(firstIsCurrent()).toBe(false);
    expect(result.current()()).toBe(true);
  });

  it("invalidates a request on unmount", () => {
    const { result, unmount } = renderHook(() => useLatestRequestGuard(["a"]));
    const isCurrent = result.current();
    unmount();
    expect(isCurrent()).toBe(false);
  });
});

describe("useLatestRequestGuard concurrent requests", () => {
  it("invalidates an older in-flight request when a newer one starts", () => {
    const { result } = renderHook(() => useLatestRequestGuard(["a"]));
    const first = result.current();
    const second = result.current();
    expect(first()).toBe(false);
    expect(second()).toBe(true);
  });
});
