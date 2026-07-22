import type { Backlog, Detail } from "./types";

/**
 * Fetch the read-only backlog + coverage from the local backend (REQ034).
 * A non-2xx response carries a JSON `{ error }` the backend wrote (e.g. an
 * unadopted subject → 409 naming `init`); surface that message rather than a
 * bare status code so the operator sees the actionable cause.
 */
export async function fetchBacklog(signal?: AbortSignal): Promise<Backlog> {
  const res = await fetch("/api/requirements", { signal });
  if (!res.ok) {
    const message = await errorMessage(res);
    throw new Error(message);
  }
  return (await res.json()) as Backlog;
}

/**
 * Fetch one requirement's read-only formalization detail (REQ035). A 404 (unknown id) or 409
 * (unadopted subject) carries the backend's `{ error }` message; surface it verbatim.
 */
export async function fetchDetail(id: string, signal?: AbortSignal): Promise<Detail> {
  const res = await fetch(`/api/requirements/${encodeURIComponent(id)}`, { signal });
  if (!res.ok) {
    throw new Error(await errorMessage(res));
  }
  return (await res.json()) as Detail;
}

async function errorMessage(res: Response): Promise<string> {
  try {
    const body = (await res.json()) as { error?: string };
    if (body.error) return body.error;
  } catch {
    // Fall through to the status line when the body is not the JSON error shape.
  }
  return `HTTP ${res.status}`;
}
