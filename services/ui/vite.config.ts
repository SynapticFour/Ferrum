import path from 'path';
import { fileURLToPath } from 'url';
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
const __dirname = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  base: process.env.VITE_BASE_PATH || '/',
  plugins: [react()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (id.includes('node_modules/cytoscape')) return 'cytoscape';
          if (id.includes('node_modules/recharts')) return 'recharts';
          if (id.includes('node_modules/@radix-ui')) return 'radix';
          if (id.includes('node_modules/lucide-react')) return 'icons';
        },
      },
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
