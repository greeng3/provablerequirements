import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

/// PROVREQ_PORT defaults to 17869 (`provreq serve`'s default). The
/// dev proxy forwards the API surface to the running provreq
/// server so the frontend can use relative URLs; in production the
/// Rust binary serves this SPA same-origin.
const backendPort = Number(process.env.PROVREQ_PORT ?? 17869);
const backendTarget = `http://127.0.0.1:${backendPort}`;

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    port: 5173,
    proxy: {
      "/api": { target: backendTarget, changeOrigin: true },
      "/health": { target: backendTarget, changeOrigin: true },
    },
  },
  test: {
    globals: true,
    environment: "jsdom",
    setupFiles: ["./src/setupTests.ts"],
    css: false,
    // tests/e2e/ runs under node:test via tsx, not vitest.
    exclude: ["node_modules/**", "dist/**", "tests/e2e/**"],
  },
});
