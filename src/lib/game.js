export * from "$wasm/lib_knuckle_bg";

import { __wbg_set_wasm } from "$wasm/lib_knuckle_bg.js";

export async function init() {
    if (import.meta.env.DEV) {
        const wasm = await import("./wasmdev/lib_knuckle_bg.wasm");
        __wbg_set_wasm(wasm);
    } else {
        const wasm = await import("./wasmprd/lib_knuckle_bg.wasm");
        __wbg_set_wasm(wasm);
    }
}