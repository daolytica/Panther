import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [react()],

  build: {
    rollupOptions: {
      output: {
        manualChunks: (id) => {
          // Put Monaco editor in its own chunk so it loads only when Coder/Cline/AgentRuns open
          if (id.includes("monaco-editor") || id.includes("@monaco-editor")) {
            return "monaco";
          }
        },
      },
    },
    chunkSizeWarningLimit: 1200,
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. server config - use different ports for browser mode vs Tauri mode
  server: {
    port: 1420,
    strictPort: false, // Allow fallback to another port if 1420 is in use
    host: "0.0.0.0", // Bind to all interfaces for browser access
    hmr: {
      protocol: "ws",
      host: "localhost",
      // Don't specify port - let Vite auto-select an available one
    },
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
