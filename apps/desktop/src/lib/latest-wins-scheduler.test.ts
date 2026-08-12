import { describe, expect, it, vi } from "vitest";
import { createLatestWinsScheduler } from "./latest-wins-scheduler";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe("createLatestWinsScheduler", () => {
  it("runs the first task immediately", async () => {
    const scheduler = createLatestWinsScheduler();
    const task = vi.fn(async () => 42);

    await expect(scheduler.run(task)).resolves.toEqual({
      status: "completed",
      value: 42,
    });
    expect(task).toHaveBeenCalledTimes(1);
  });

  it("keeps only the latest pending task while one task is active", async () => {
    const scheduler = createLatestWinsScheduler();
    const first = deferred<string>();
    const calls: string[] = [];

    const a = scheduler.run(async () => {
      calls.push("A");
      return first.promise;
    });
    const b = scheduler.run(async () => {
      calls.push("B");
      return "B";
    });
    const c = scheduler.run(async () => {
      calls.push("C");
      return "C";
    });
    const d = scheduler.run(async () => {
      calls.push("D");
      return "D";
    });

    expect(calls).toEqual(["A"]);
    await expect(b).resolves.toEqual({ status: "superseded" });
    await expect(c).resolves.toEqual({ status: "superseded" });

    first.resolve("A");

    await expect(a).resolves.toEqual({ status: "completed", value: "A" });
    await expect(d).resolves.toEqual({ status: "completed", value: "D" });
    expect(calls).toEqual(["A", "D"]);
  });

  it("starts the latest pending task even when the active task fails", async () => {
    const scheduler = createLatestWinsScheduler();
    const first = deferred<string>();
    const calls: string[] = [];

    const a = scheduler.run(async () => {
      calls.push("A");
      return first.promise;
    });
    const b = scheduler.run(async () => {
      calls.push("B");
      return "B";
    });

    first.reject(new Error("boom"));

    await expect(a).rejects.toThrow("boom");
    await expect(b).resolves.toEqual({ status: "completed", value: "B" });
    expect(calls).toEqual(["A", "B"]);
  });
});
