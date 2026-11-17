import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// @ts-expect-error process é um global do Node
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [react()],

  // Opções do Vite para o Tauri (só valem em `tauri dev` e `tauri build`):
  // 1. não esconder os erros do Rust
  clearScreen: false,
  // 2. o Tauri espera uma porta fixa; falhar se ela estiver ocupada
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: {
      // 3. não observar o `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
