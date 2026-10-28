import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import path from 'path'

// 홍보물 프로토타입 전용 빌드. 앱 본체(../vite.config.ts)와 분리되어 있다.
export default defineConfig({
  root: __dirname,
  base: './',
  plugins: [react()],
  resolve: {
    alias: { '@': path.resolve(__dirname, '../src') },
  },
  server: { port: 5174, strictPort: true, fs: { allow: ['..'] } },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    target: 'chrome105',
    rollupOptions: {
      input: {
        index: path.resolve(__dirname, 'index.html'),
        app: path.resolve(__dirname, 'app.html'),
      },
    },
  },
})
