import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';
import { viteStaticCopy } from 'vite-plugin-static-copy';

export default defineConfig({
  plugins: [
    react(),
    viteStaticCopy({
      targets: [
        {
          src: 'node_modules/pdfjs-dist/cmaps/*',
          dest: 'cmaps',
          rename: { stripBase: true },
        },
        {
          src: 'node_modules/pdfjs-dist/standard_fonts/*',
          dest: 'standard_fonts',
          rename: { stripBase: true },
        },
      ],
    }),
  ],

  // Tauri 的 CLI 会接管终端输出，别让 Vite 清屏
  clearScreen: false,

  server: {
    // Tauri 的 devUrl 写死了这个端口，所以端口被占用时要报错而不是自动换
    port: 5173,
    strictPort: true,
    // src-tauri 里的改动由 cargo 监听，别让 Vite 重复触发
    watch: { ignored: ['**/src-tauri/**'] },
  },

  build: {
    // 目标要对齐最弱的那个 WebView：
    // macOS WKWebView / Windows WebView2 / Linux WebKitGTK
    target: ['es2021', 'chrome100', 'safari15'],
  },
});
