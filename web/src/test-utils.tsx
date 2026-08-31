/// Shared test helpers: a fresh QueryClient with retries off so
/// `isError` surfaces immediately instead of waiting out the
/// default three-retry backoff schedule.

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";

export function TestQueryProvider({ children }: { children: ReactNode }) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}
