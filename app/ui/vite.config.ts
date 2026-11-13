import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vite'
import { fileURLToPath } from 'node:url'

const icons = fileURLToPath(new URL('../../crates/core/assets/icons', import.meta.url))

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 5173, strictPort: true, fs: { allow: ['.', icons] } },
  resolve: { alias: { $icons: icons } },
  build: { target: 'es2022' },
})
