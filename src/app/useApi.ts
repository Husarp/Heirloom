import { useEffect, useRef, useState } from "react";
import { call, ApiError } from "../api/transport";
import { useStore } from "./store";

/** Fetches `method` and fetches again whenever the archive changes or `args` change. Keeps the previous data
 *  while reloading, so screens don't flash. */
export function useApi<T>(method: string | null, args?: object) {
  const dataVersion = useStore((s) => s.dataVersion);
  const key = method ? `${method}|${JSON.stringify(args ?? null)}` : null;
  const [state, setState] = useState<{ key: string | null; data: T | null; error: ApiError | null; loading: boolean }>({
    key: null,
    data: null,
    error: null,
    loading: method != null,
  });
  const latest = useRef(0);

  useEffect(() => {
    if (!method) return;
    const request = ++latest.current;
    setState((s) => ({ ...s, loading: true }));
    call<T>(method, args)
      .then((data) => {
        if (request === latest.current) setState({ key, data, error: null, loading: false });
      })
      .catch((error: ApiError) => {
        if (request === latest.current) setState({ key, data: null, error, loading: false });
      });
    // `key` covers method and args.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, dataVersion]);

  return {
    data: state.data,
    error: state.error,
    loading: state.loading,
    /** True when the data belongs to the current arguments (not left over from the previous ones). */
    fresh: state.key === key,
  };
}
