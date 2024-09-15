import { defineConfig } from 'astro/config';
import svelte from "@astrojs/svelte";
import tailwind from "@astrojs/tailwind";
import wasm from "vite-plugin-wasm"

// https://astro.build/config
export default defineConfig({
  integrations: [svelte(), tailwind(),
  {
    name: "wasm-funnies",
    hooks: {
      "astro:config:setup": (config) => {
        if (process.env.NODE_ENV === "production") {
          config.updateConfig(
            {
              vite: {
                resolve: {
                  alias: {
                    "./lib/wasmdev/": "./lib/wasmprd/"
                  }
                }
              }
            }
          )
        }

      }
    }
  }],
  vite: {
    server: {
      host: "127.0.0.1",
    },
    plugins: [wasm()]
  }
});