import { presetUno, transformerCompileClass, transformerDirectives, transformerVariantGroup } from "unocss";

import UnoCSS from '@unocss/svelte-scoped/preprocess'
import adapter from '@sveltejs/adapter-static';
import extractorSvelte from "@unocss/extractor-svelte";
import { presetScrollbar } from "unocss-preset-scrollbar";
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

const prod = process.env.NODE_ENV !== "development";


/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: [
    vitePreprocess(),
    UnoCSS({
      combine: prod,
      presets: [presetUno(), presetScrollbar()],
      extractors: [extractorSvelte()],
      transformers: [
        transformerVariantGroup(),
        transformerDirectives(),
        transformerCompileClass({
          classPrefix: "supercss-",
        }),
      ],
      safelist: [
        "ml-auto",
        "mt-auto",
        "items-center",
        "mb-10",
        "ml-4",
        "top-48",
        "top-60",
        "top-72",
        "text-nowrap",
        "text-5xl",
      ],
      details: true,
    })
  ],

  kit: {
    adapter: adapter({
      // fallback: "200.html",
      // precompress: false,
    }),
    alias: {
      $assets: "./src/assets",
      $src: "./src",
      $wasm: "./src/lib/wasmdev/",
    },
    // prerender: {
    //   crawl: true,
    //   entries: ["*"],
    //   origin: "https://knucklebones.fyi",
    // }
  },
  compilerOptions: {
    warningFilter: () => false
  }
};

export default config;
