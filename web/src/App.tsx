import { BrowserRouter } from "react-router-dom";

import { useServerEvents } from "./api/useServerEvents";
import { QueryProvider } from "./providers/QueryProvider";
import { AppRoutes } from "./routes";

function AppInner() {
  // Subscribe to SSE change events from the backend and
  // invalidate react-query caches whenever the world shifts.
  useServerEvents();
  return (
    <BrowserRouter>
      <AppRoutes />
    </BrowserRouter>
  );
}

export default function App() {
  return (
    <QueryProvider>
      <AppInner />
    </QueryProvider>
  );
}
