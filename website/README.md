# jet.fish

Astro site for the Bebop console.

## Commands

```sh
pnpm install
pnpm dev       # dev server
pnpm build     # static build
pnpm preview   # preview build
```

## Structure

- `src/pages/index.astro` — page
- `src/styles` — styles, bundled by Astro/Vite
- `src/scene` — Bebop WebGL scene, bundled by Astro/Vite
- `public/assets` — fonts, icons, scripts, images
- `term` — Rust/WASM terminal bridge

## Licenses

- [Departure Mono](public/assets/fonts/departure-mono/LICENSE.txt)
- [Lilex](public/assets/fonts/lilex/LICENSE.txt)
- [Obel Mono](public/assets/fonts/obel-mono/LICENSE.txt)
- [Pixelarticons](public/assets/icons/LICENSE.txt)
