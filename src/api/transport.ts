// Talks to the Rust side: through Tauri in the app window, or through `heirloom-bridge` (HTTP, via the Vite
// proxy) when the UI runs in a normal browser for tests and design checks.

import { invoke } from "@tauri-apps/api/core";

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export class ApiError extends Error {
  code: string;
  constructor(code: string, message: string) {
    super(message);
    this.code = code;
  }
}

function toApiError(e: unknown): ApiError {
  if (e instanceof ApiError) return e;
  if (e && typeof e === "object" && "code" in e && "message" in e) {
    return new ApiError(String((e as { code: unknown }).code), String((e as { message: unknown }).message));
  }
  return new ApiError("internal", e instanceof Error ? e.message : String(e));
}

export async function call<T>(method: string, args?: object): Promise<T> {
  try {
    if (inTauri) return await invoke<T>("call", { method, args: args ?? null });
    const response = await fetch(`/api/${method}`, { method: "POST", body: JSON.stringify(args ?? null) });
    const body = await response.json();
    if (body.error) throw toApiError(body.error);
    return body.ok as T;
  } catch (e) {
    throw toApiError(e);
  }
}

/** A URL for a file in the open archive (`path` is relative to the archive folder, with `/` separators). */
/** A preview of the n-th file of the import being reviewed (not yet in the archive); size 0 = the file itself. */
export function importFileUrl(index: number, size: number, loadId: number): string {
  const route = `import/${size}/${index}?d=${loadId}`;
  return inTauri ? `http://heirloom.localhost/${route}` : `/media/${route}`;
}

export function mediaUrl(path: string, kind: "file" | "view" | "thumb" = "view", size = 256): string {
  const encoded = path.split(/[\\/]/).filter(Boolean).map(encodeURIComponent).join("/");
  const route = kind === "thumb" ? `thumb/${size}/${encoded}` : `${kind}/${encoded}`;
  return inTauri ? `http://heirloom.localhost/${route}` : `/media/${route}`;
}
