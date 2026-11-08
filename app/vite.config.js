import { resolve } from "node:path";
import { defineConfig } from "vite";

// Three windows, three pages: the main window, the transparent subtitle
// overlay, and the full-screen region selector.
export default defineConfig({
  clearScreen: false,
  server: { host: "127.0.0.1", port: 1420, strictPort: true, watch: { ignored: ["**/src-tauri/**"] } },
  build: {
    target: "es2022",
    rollupOptions: {
      input: {
        main: resolve(import.meta.dirname, "index.html"),
        overlay: resolve(import.meta.dirname, "overlay.html"),
        selector: resolve(import.meta.dirname, "selector.html"),
      },
    },
  },
});
