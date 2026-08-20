# @hozo/tailwind

Reads a project's design tokens out of its own Tailwind entry stylesheet and hands them to the Hozo compiler.

Used by every Hozo integration; a project does not normally import it directly.

## Why it runs Tailwind rather than parsing a config

Tailwind 4 defines a theme in CSS — `@theme { --color-brand: oklch(62% 0.19 258); }` — with `@import`, `@plugin` and cascade layers all in play. Working out what `bg-brand` means from the file text would mean reimplementing that resolution, and being subtly wrong about it in ways that look reasonable from both sides.

So this loads the stylesheet through Tailwind's own resolver and reads the answer. The colours come back in three forms — token name, `oklch()`, and a hex fallback — because Native has no `oklch()` and the compiler needs to pick per backend.

Spacing is carried as the project's *step*, not as pixels: `p-4` means "four spacing units", and a project that sets `--spacing` to something other than `0.25rem` moves every one of them together. Resolving to pixels at this layer would silently freeze the scale.

The theme is read once per build. It is a project-wide fact, and re-reading it per module would run Tailwind's resolver hundreds of times for one answer.

## API

```ts
import { loadProjectTheme } from '@hozo/tailwind'

const theme = await loadProjectTheme(root, {
  css: 'src/theme.css',
  warn: (message) => console.warn(message),
})
```

Returns `undefined` when there is no entry stylesheet to read, which is the signal to fall back to Tailwind's defaults. A token that fails to resolve is reported through `warn` rather than dropped.
