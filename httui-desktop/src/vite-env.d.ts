/// <reference types="vite/client" />

// `__APP_VERSION__` is injected at build time from `package.json`
// via `vite.config.ts` `define`. The StatusBar version pill reads
// it.
declare const __APP_VERSION__: string;
