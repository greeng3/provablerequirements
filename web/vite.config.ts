import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  build: { outDir: "dist" },
  // During `vite dev`, forward API calls to a locally running `provreq serve`.
  // 17869 is that command's default port — keep the two in step.
  server: {
    proxy: {
      "/health": "http://127.0.0.1:17869",
      "/api": "http://127.0.0.1:17869",
    },
  },
  test: {
    globals: true,
    environment: "jsdom",
    setupFiles: "./src/test-setup.ts",
  },
});
