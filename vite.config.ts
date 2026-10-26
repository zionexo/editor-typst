import { defineConfig } from "vite";
import wasm from "vite-plugin-wasm";
import topLevelAwait from "vite-plugin-top-level-await";

// Configurazione Vite per Typster.
// La porta 5173 deve combaciare con devUrl in tauri.conf.json.
export default defineConfig({
  // `vite-plugin-wasm`: gestisce l'import ESM del .wasm di `codemirror-lang-typst`
  //   (build wasm-pack target `bundler`), non supportato da Vite nativamente.
  // `vite-plugin-top-level-await`: il glue WASM usa top-level await dopo la
  //   trasformazione del plugin precedente; questo lo rende compatibile col build.
  plugins: [wasm(), topLevelAwait()],
  // Evita che Vite ottimizzi gli import Tauri (già ottimizzati da Tauri)
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  // Necessario per far funzionare l'env TAURI_DEV_HOST in ambienti mobile
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    rollupOptions: {
      output: {
        manualChunks: undefined,
      },
    },
  },
});
