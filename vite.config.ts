import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// dev server 只綁 127.0.0.1、固定埠：Tauri 的 devUrl 與 devCsp 都寫死這個位址（ADR-0007）。
export default defineConfig({
  root: "ui",
  plugins: [vue()],
  clearScreen: false,
  server: {
    host: "127.0.0.1",
    port: 5173,
    strictPort: true,
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    target: "es2022",
    // CSP 不允許 data: —— 小檔案也不要內嵌成 data URI。
    assetsInlineLimit: 0,
  },
});
