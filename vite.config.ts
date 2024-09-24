import { presetUno, transformerCompileClass, transformerDirectives, transformerVariantGroup } from "unocss";

import type { Plugin } from "vite";
import UnoCSS from "unocss/vite";
import { defineConfig } from "vite";
import { enhancedImages } from "@sveltejs/enhanced-img";
import extractorSvelte from "@unocss/extractor-svelte";
import { presetScrollbar } from "unocss-preset-scrollbar";
import { sveltekit } from "@sveltejs/kit/vite";
import topLevelAwaitPlugin from "vite-plugin-top-level-await";
import wasm from "vite-plugin-wasm";

function wasmFunnies(): Plugin {
  return {
    name: "wasm-funnies",
    config: (userConfig, { mode }) => {
      // Check if the environment is production
      if (mode === "production") {
        return {
          resolve: {
            alias: {
              "./lib/wasmdev/": "./lib/wasmprd/",
            },
          },
        };
      }
    },
  };
}

export default defineConfig({
  plugins: [
    topLevelAwaitPlugin(),
    enhancedImages(),
    UnoCSS({
      presets: [presetUno(), presetScrollbar()],
      extractors: [extractorSvelte()],
      transformers: [
        transformerVariantGroup(),
        transformerDirectives(),
        transformerCompileClass({
          classPrefix: "supercss-",
        }),
      ],
      safelist: ["ml-auto"],
      details: true,
    }),
    wasm(),
    wasmFunnies(),
    sveltekit(),
  ],
});
