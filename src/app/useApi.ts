import { useEffect, useRef, useState } from "react";
import { call, ApiError } from "../api/transport";
import { useStore } from "./store";

/** Fetches `method` and fetches again whenever the archive changes or `args` change. Keeps the previous data
 *  while reloading, so screens don't flash. `enabled` false (a hidden screen kept mounted) pauses it and keeps the
 *  data: enabled again, it fetches only if the archive or the arguments changed meanwhile. */
export function useApi<T>(method: string | null, args?: object, enabled = true) {
  const dataVersion = useStore((s) => s.dataVersion);
  const key = method ? `${method}|${JSON.stringify(args ?? null)}` : null;
  const [state, setState] = useState<{ key: string | null; data: T | null; error: ApiError | null; loading: boolean }>({
    key: null,
    data: null,
    error: null,
    loading: method != null,
  });
  const latest = useRef(0);
  // What the data in hand was fetched for.
  const fetched = useRef<{ key: string; version: number } | null>(null);

  useEffect(() => {
    if (!method) {
      fetched.current = null;
      return;
    }
    if (!enabled || (fetched.current?.key === key && fetched.current.version === dataVersion)) return;
    fetched.current = { key: key!, version: dataVersion };
    const request = ++latest.current;
    setState((s) => ({ ...s, loading: true }));
    call<T>(method, args)
      .then((data) => {
        if (request === latest.current) setState({ key, data, error: null, loading: false });
      })
      .catch((error: ApiError) => {
        if (request !== latest.current) return;
        // Tried again the next time it's enabled.
        fetched.current = null;
        setState({ key, data: null, error, loading: false });
      });
    // `key` covers method and args.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, dataVersion, enabled]);

  return {
    data: state.data,
    error: state.error,
    loading: state.loading,
    /** True when the data belongs to the current arguments (not left over from the previous ones). */
    fresh: state.key === key,
  };
}
