import { useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";

/// Mounts an EventSource against `/api/events` and invalidates the
/// react-query cache on every `change` event. One consumer at the
/// app root is enough; call it inside QueryProvider so every
/// child query participates.
///
/// EventSource reconnects automatically on transport drops, so
/// there's nothing to reconnect-manage here. jsdom doesn't ship
/// EventSource, so tests skip this by providing
/// `import.meta.env.VITEST === "true"` (vitest sets that).
export function useServerEvents() {
  const qc = useQueryClient();

  useEffect(() => {
    if (typeof EventSource === "undefined") return;

    const es = new EventSource("/api/events");
    const handler = () => {
      qc.invalidateQueries();
    };
    es.addEventListener("change", handler);
    return () => {
      es.removeEventListener("change", handler);
      es.close();
    };
  }, [qc]);
}
