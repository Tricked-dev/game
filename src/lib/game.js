export * from "./wasmdev/lib_knuckle_bg";

import { __wbg_set_wasm } from "./wasmdev/lib_knuckle_bg.js";

let inited = false;
let initing = false;

export async function init() {
    // if(import.meta.env.SSR)return;
    if (inited) return;
    if (initing) {
        while (initing) {
            await new Promise(r => setTimeout(r, 100));
        }
        return
    }
    initing = true;
    if (import.meta.env.DEV) {
        __wbg_set_wasm(await import("./wasmdev/lib_knuckle_bg.wasm"));
    } else {
        __wbg_set_wasm(await import("./wasmprd/lib_knuckle_bg.wasm"));
    }
    inited = true;
    initing = false;
}