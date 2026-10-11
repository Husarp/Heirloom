// Long jobs report how far they are (`job.progress`, answered even while the job runs): the window shows „412 z 1 230
// plików”. Only the backup zip can be cancelled.

import { call } from "../api/transport";
import type { JobProgress } from "../api/types";
import { useStore } from "./store";

const EVERY = 300;

/** Waits for `promise` and tells `on` how far the running job is, every 300 ms. */
export async function trackProgress<T>(promise: Promise<T>, on: (p: JobProgress) => void): Promise<T> {
  let over = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const poll = async () => {
    try {
      const p = await call<JobProgress>("job.progress");
      if (!over && p.running) on(p);
    } catch {
      // The numbers are a courtesy; the job itself decides.
    }
    if (!over) timer = setTimeout(poll, EVERY);
  };
  timer = setTimeout(poll, EVERY);
  try {
    return await promise;
  } finally {
    over = true;
    clearTimeout(timer);
  }
}

/** „Przerwij”: asks the running job to stop (it ends with the error `cancelled`). */
export function cancelJob(): Promise<unknown> {
  return call("job.cancel").catch(() => {});
}

/** Blocks the window with `label` and the numbers while `promise` runs. */
export async function withProgress<T>(label: string, promise: Promise<T>, options: { note?: string; cancellable?: boolean } = {}): Promise<T> {
  useStore.setState({ busy: { text: label, note: options.note, cancellable: options.cancellable } });
  try {
    return await trackProgress(promise, (p) => useStore.setState({ busy: { text: label, note: options.note, cancellable: options.cancellable, done: p.done, total: p.total, cancelling: p.cancelling } }));
  } finally {
    useStore.setState({ busy: null });
  }
}
