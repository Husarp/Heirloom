import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri runs the dev server on a fixed port. In a normal browser (tests, design checks) the UI talks to
// `heirloom-bridge` instead of Tauri, so /api and /media are forwarded to it.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    proxy: {
      "/api": "http://127.0.0.1:1430",
      "/media": "http://127.0.0.1:1430",
    },
    watch: { ignored: ["**/src-tauri/**", "**/crates/**", "**/target/**", "**/test-archives/**", "**/build/**"] },
  },
  build: {
    target: "es2022",
    chunkSizeWarningLimit: 2000,
  },
  test: {
    include: ["src/**/*.test.ts"],
  },
});
