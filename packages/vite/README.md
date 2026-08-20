# @hozo/vite

Vite integration for Hozo's Web backend. Lowers `View`/`Text`/`Pressable`/`Button` into semantic DOM and a real stylesheet, rewriting each component in place.

## Setup

```ts
// vite.config.ts
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'
import { hozo } from '@hozo/vite'

export default defineConfig({
  plugins: [hozo({ css: 'src/theme.css' }), react()],
})
```

`hozo()` must come before `react()`. The plugin runs at `enforce: 'pre'` so it sees the file as written — the JSX the compiler was built to read, not `@vitejs/plugin-react`'s output.

## Options

Every Hozo integration takes the same options.

| Option | Meaning |
| --- | --- |
| `css` | The project's Tailwind entry stylesheet. Read once per build by running Tailwind's own resolver. |
| `root` | Project root. Defaults to Vite's. |
| `content` | Which files the project-wide scan walks. |
| `sources` | Modules whose primitives may be lowered. Defaults to `@hozo/core` and `react-native`. |
| `debug` | Report what the scan found. |

`css` is the one worth setting. Left out, Hozo looks for the usual filenames and falls back to Tailwind's defaults if it finds none, reporting what it looked for. That fallback is right until the project defines its own tokens under a name Hozo did not guess — and then `bg-brand` compiles to a CSS variable nothing defines and `p-4` to the wrong number of pixels, neither of which is an error.

## Two stylesheets

Each lowered module gets a `<file>.hozo.css` companion written next to it and imported normally, rather than served through a virtual module.

Alongside it, one project-wide stylesheet under `node_modules/.hozo/` covers the classes the compiler *couldn't* read — a `className` that only a runtime expression produces. Those come from a byte scan of every source file rather than from the AST, so this plugin owns the project walk and the file-deletion signal while the cache in Rust owns scanning, staleness and persistence.

## Dev mode

A style-only edit reaches the browser in two rounds: the `.tsx` change triggers the first, that transform writes the CSS, and the watcher seeing the new CSS triggers the second. It converges because identical bytes are never rewritten — without that, each transform would invalidate the stylesheet it had just written and the two would take turns forever.

The project-wide stylesheet lives under `node_modules`, which Vite's watcher ignores, so its invalidation is explicit rather than left to the watcher. Deleting a file drops its classes; creating one adds them, if the project's `content` globs would have included it.
