/// <reference path="../.astro/types.d.ts" />

interface ImportMetaEnv {
	readonly DEV: boolean;
}

interface ImportMeta {
	readonly env: ImportMetaEnv;
}
