# jet.fish

Astro site for the Bebop console.

## Commands

```sh
npm install
npm run dev    # dev server
npm run build  # static build
npm run preview  # preview build
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
