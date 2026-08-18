import assert from 'node:assert/strict'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { test } from 'node:test'

import { compile, compileNative } from '@hozo/compiler'
import { loadTheme, toHex } from './theme.ts'

/** Writes a stylesheet somewhere Tailwind can resolve imports from. */
async function themeFrom(css: string) {
  const dir = mkdtempSync(path.join(import.meta.dirname, '.theme-test-'))
  try {
    writeFileSync(path.join(dir, 'app.css'), css)
    return await loadTheme(css, dir)
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
}

test('a project token is read alongside the default palette', async () => {
  const theme = await themeFrom(`@import "tailwindcss";
    @theme { --color-brand: oklch(62% 0.19 259); }`)

  const brand = theme.colors.find((c) => c.token === 'brand')
  assert.equal(brand?.oklch, 'oklch(62% 0.19 259)')
  assert.equal(brand?.hex, '#3581f6')
  // The defaults are still there: a `@theme` block adds to the palette
  // rather than replacing it, and reading only the custom ones would make
  // `bg-red-500` stop resolving the moment a project defined anything.
  assert.ok(theme.colors.some((c) => c.token === 'red-500'))
})

test('a token the project redefines wins over the built-in copy', async () => {
  // Tailwind lets a `@theme` redefine `--color-blue-500`. A compiler that
  // preferred its own bundled palette would render a colour the project
  // had explicitly changed.
  const theme = await themeFrom(`@import "tailwindcss";
    @theme { --color-blue-500: oklch(50% 0.2 200); }`)
  const blue = theme.colors.find((c) => c.token === 'blue-500')
  assert.equal(blue?.oklch, 'oklch(50% 0.2 200)')

  const source =
    `import { View } from '@hozo/core'\n` +
    `export function C() { return <View className="bg-blue-500" /> }\n`
  assert.match(compile(source, theme)[0].css, /background-color: oklch\(50% 0\.2 200\)/)
})

test('a custom colour reaches both backends in the spelling each needs', async () => {
  const theme = await themeFrom(`@import "tailwindcss";
    @theme { --color-brand: oklch(62% 0.19 259); }`)
  const source =
    `import { View } from '@hozo/core'\n` +
    `export function C() { return <View className="bg-brand" /> }\n`

  // Web keeps the oklch Tailwind itself would emit; React Native has no
  // `oklch()`, so it takes the hex.
  assert.match(compile(source, theme)[0].css, /background-color: oklch\(62% 0\.19 259\)/)
  assert.match(compileNative(source, theme)[0].styles, /backgroundColor: '#3581f6'/)
})

test('without a theme the same source is unresolved, not wrong', async () => {
  // What every project got before this existed, and what a project still
  // gets for a token nothing defines. Both backends say "unresolved" in
  // their own way rather than inventing a colour.
  const source =
    `import { View } from '@hozo/core'\n` +
    `export function C() { return <View className="bg-brand" /> }\n`
  assert.match(compile(source)[0].css, /background-color: var\(--hozo-color-brand\)/)
  assert.match(compileNative(source)[0].styles, /hozo-unresolved:brand/)
})

test('a colour that will not convert is left out rather than guessed', () => {
  assert.equal(toHex('oklch(62% 0.19 259)'), '#3581f6')
  assert.equal(toHex('not a colour'), null)
  // The backends have a defined answer for a token they can't resolve, and
  // that beats a colour that is nearly right.
  assert.equal(toHex(''), null)
})

test('a project spacing scale reaches every spacing utility', async () => {
  // The failure this fixes was silent, unlike the colour one: a project
  // setting `--spacing` got the right number of steps at the wrong size,
  // and the output was an ordinary padding.
  const theme = await themeFrom(`@import "tailwindcss";
    @theme { --spacing: 0.2rem; }`)
  assert.equal(theme.spacingPx, 3.2)

  const source =
    `import { View } from '@hozo/core'\n` +
    `export function C() { return <View className="p-4 -mt-2 gap-3 border-2" /> }\n`
  const css = compile(source, theme)[0].css

  assert.match(css, /padding-top: 12\.8px/)
  // Negation happens in steps, so the sign survives the scale.
  assert.match(css, /margin-top: -6\.4px/)
  // Resolved, not left as arithmetic -- and rounded, since the product is
  // written into the stylesheet as a literal.
  assert.match(css, /gap: 9\.6px/)
  // A border width is absolute whatever the spacing scale is, which is why
  // the two are different kinds of length rather than one number.
  assert.match(css, /border-top-width: 2px/)
})

test('a spacing scale Hozo cannot read leaves the default alone', async () => {
  // Guessing here would rescale every padding, margin and gap in the
  // project by a number nobody chose.
  const theme = await themeFrom(`@import "tailwindcss";
    @theme { --spacing: calc(1rem / 3); }`)
  assert.equal(theme.spacingPx, undefined)
})

test('p-px stays one physical pixel', async () => {
  // Tailwind means a hairline by it, not a step of anything.
  const theme = await themeFrom(`@import "tailwindcss";
    @theme { --spacing: 0.2rem; }`)
  const source =
    `import { View } from '@hozo/core'\n` +
    `export function C() { return <View className="p-px" /> }\n`
  assert.match(compile(source, theme)[0].css, /padding-top: 1px/)
})
