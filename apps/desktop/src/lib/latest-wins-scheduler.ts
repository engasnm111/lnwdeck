export type LatestWinsResult<T> =
  | { status: "completed"; value: T }
  | { status: "superseded" };

export type LatestWinsOwner = symbol;

export interface LatestWinsScheduler {
  run<T>(
    task: () => Promise<T>,
    owner?: LatestWinsOwner,
  ): Promise<LatestWinsResult<T>>;
  cancelPending(owner: LatestWinsOwner): void;
}

type PendingTask = {
  task: () => Promise<unknown>;
  owner?: LatestWinsOwner;
  resolve: (result: LatestWinsResult<unknown>) => void;
  reject: (error: unknown) => void;
};

/**
 * Bounds asynchronous work to one active task plus one replaceable trailing
 * task. While the active task is running, every new request supersedes the
 * previous pending request. This is intentionally latest-wins: navigation to
 * an intermediate page must not build an unbounded native request backlog.
 */
export function createLatestWinsScheduler(): LatestWinsScheduler {
  let active = false;
  let pending: PendingTask | null = null;

  const start = (entry: PendingTask) => {
    active = true;
    void entry
      .task()
      .then((value) => {
        entry.resolve({ status: "completed", value });
      })
      .catch((error) => {
        entry.reject(error);
      })
      .finally(() => {
        active = false;
        const next = pending;
        pending = null;
        if (next) {
          start(next);
        }
      });
  };

  return {
    run<T>(
      task: () => Promise<T>,
      owner?: LatestWinsOwner,
    ): Promise<LatestWinsResult<T>> {
      return new Promise<LatestWinsResult<T>>((resolve, reject) => {
        const entry: PendingTask = {
          task,
          owner,
          resolve: resolve as (result: LatestWinsResult<unknown>) => void,
          reject,
        };

        if (!active) {
          start(entry);
          return;
        }

        pending?.resolve({ status: "superseded" });
        pending = entry;
      });
    },

    cancelPending(owner: LatestWinsOwner) {
      if (pending?.owner !== owner) return;
      pending.resolve({ status: "superseded" });
      pending = null;
    },
  };
}
