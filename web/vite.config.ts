import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  build: { outDir: "dist" },
  // During `vite dev`, forward API calls to a locally running `provreq serve`.
  server: {
    proxy: { "/health": "http://127.0.0.1:8080" },
  },
  test: {
    globals: true,
    environment: "jsdom",
    setupFiles: "./src/test-setup.ts",
  },
});
