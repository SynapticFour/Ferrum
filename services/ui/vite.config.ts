import path from 'path';
import { fileURLToPath } from 'url';
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
const __dirname = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  server: {
    port: 5173,
    proxy: {
      '/api': { target: 'http://localhost:8080', changeOrigin: true },
      '/ga4gh': { target: 'http://localhost:8080', changeOrigin: true },
      '/workspaces': { target: 'http://localhost:8080', changeOrigin: true },
      '/cohorts': { target: 'http://localhost:8080', changeOrigin: true },
      '/passports': { target: 'http://localhost:8080', changeOrigin: true },
      '/health': { target: 'http://localhost:8080', changeOrigin: true },
      '/admin': { target: 'http://localhost:8080', changeOrigin: true },
    },
  },
});
