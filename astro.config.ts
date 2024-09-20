import {
  presetUno,
  transformerCompileClass,
  transformerDirectives,
  transformerVariantGroup,
} from "unocss";

import UnoCSS from "unocss/astro";
import { defineConfig } from "astro/config";
import extractorSvelte from "@unocss/extractor-svelte";
import { presetScrollbar } from "unocss-preset-scrollbar";
import svelte from "@astrojs/svelte";
import wasm from "vite-plugin-wasm";

// https://astro.build/config
export default defineConfig({
  site: "https://knucklebones.fyi",
  integrations: [
    svelte(),
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
      injectReset: true,
    }),
    {
      name: "wasm-funnies",
      hooks: {
        "astro:config:setup": (config) => {
          // @ts-ignore
          if (process.env.NODE_ENV === "production") {
            config.updateConfig({
              vite: {
                resolve: {
                  alias: {
                    "./lib/wasmdev/": "./lib/wasmprd/",
                  },
                },
              },
            });
          }
        },
      },
    },
  ],
  vite: {
    plugins: [wasm()],
  },
});
