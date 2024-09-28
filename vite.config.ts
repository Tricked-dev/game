import type { Plugin } from "vite";
import UnoCSS from "@unocss/svelte-scoped/vite";
import { defineConfig } from "vite";
import { enhancedImages } from "@sveltejs/enhanced-img";
import { sveltekit } from "@sveltejs/kit/vite";
import topLevelAwaitPlugin from "vite-plugin-top-level-await";
import wasm from "vite-plugin-wasm";

function wasmFunnies(): Plugin {
  return {
    name: "wasm-funnies",
    config: (userConfig, { mode }) => {
      // Check if the environment is production
      // if (mode === "production") {
      //   return {
      //     resolve: {
      //       alias: {
      //         $wasm: "./wasmprd/",
      //       },
      //     },
      //   };
      // } else {
      //   return {
      //     resolve: {
      //       alias: {
      //         $wasm: "./wasmdev/",
      //       },
      //     },
      //   };
      // }
    },
  };
}

export default defineConfig({
  plugins: [
    topLevelAwaitPlugin(),
    enhancedImages(),
    UnoCSS({
      injectReset: "@unocss/reset/tailwind.css",
    }),
    wasm(),
    wasmFunnies(),
    sveltekit(),
  ],
});
