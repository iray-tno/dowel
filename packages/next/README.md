# @hozo/next

Wrap the config in `next.config.ts`; nothing else is needed:

```ts
import { withHozo } from '@hozo/next'

export default withHozo()
```

For a non-standard Tailwind entry, pass the same options as `@hozo/vite`:

```ts
export default withHozo({}, { css: 'src/theme.css' })
```

Any config you already have is preserved, including your own `webpack()`
and `turbopack.rules`.

## Both bundlers

Next 16 builds with Turbopack by default and still accepts
`next build --webpack`. Hozo registers the same loader with both — the two
differ in how they order it, not in what they ask of it:

- Turbopack takes it as a `turbopack.rules` entry.
- webpack takes it as a `module.rules` entry marked `enforce: 'pre'`.
  Without that mark it runs *after* SWC and is handed compiled JavaScript
  with no JSX left to lower.

## Where the project-wide work happens

The Vite plugin walks the project in `buildStart` and persists in
`buildEnd`. Turbopack has neither hook — a loader is the only place user
code runs — so the walk happens while `next.config.ts` is being evaluated
instead. That is once per build, before anything is compiled, which is what
`buildStart` means. The loader then rescans each module it is handed, so a
class written during `next dev` reaches the candidate stylesheet without a
restart.

## Tailwind's own pipeline is not needed

Hozo compiles the utilities. The Tailwind entry stylesheet is read for its
`@theme` tokens and never bundled, so a Hozo project has no
`@tailwindcss/postcss` in it and nothing to import into the app.

## The same options everywhere

`@hozo/vite`, `@hozo/next`, `@hozo/metro` and `@hozo/storybook` all take
exactly `HozoProjectOptions` and add nothing:

| option    | meaning                                                    |
| --------- | ---------------------------------------------------------- |
| `css`     | Tailwind entry stylesheet, read for `@theme` and not bundled |
| `content` | source globs and ignores for the project-wide scan          |
| `root`    | project root; defaults to whatever the bundler already knows |
| `debug`   | report scan work and timing through the bundler's logger     |

An error-severity diagnostic fails the build in all four, and prints the
same way.
