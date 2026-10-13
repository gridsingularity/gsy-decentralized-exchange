import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  server: {
    proxy: {
      // Proxy /api/* to the gsy-offchain-storage backend, stripping the /api
      // prefix so the browser sees a same-origin API in dev (no CORS needed).
      // e.g. /api/trades -> http://localhost:8080/trades
      '/api': {
        target: 'http://localhost:8080',
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/api/, ''),
      },
    },
  },
})
