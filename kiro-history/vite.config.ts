import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [react()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  clearScreen: false,
  // Code splitting & tree-shaking
  build: {
    rollupOptions: {
      output: {
        manualChunks: {
          // Heavy visualization libs → separate chunk (lazy-loaded)
          "vendor-charts": ["recharts", "react-calendar-heatmap"],
          "vendor-diff": ["react-diff-viewer-continued"],
          // Markdown rendering → separate chunk
          "vendor-markdown": ["react-markdown", "remark-gfm", "rehype-highlight"],
          // React core
          "vendor-react": ["react", "react-dom"],
          // Lucide icons
          "vendor-icons": ["lucide-react"],
        },
      },
    },
    // Increase warning threshold to 600KB per chunk
    chunkSizeWarningLimit: 600,
    // Enable source map for production debugging
    sourcemap: false,
    minify: "esbuild",
    target: "es2020",
  },
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? { protocol: "ws", host, port: 1421 }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
}));
