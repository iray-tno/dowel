import assert from 'node:assert/strict'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import test from 'node:test'

import { loadProjectTheme } from './project.ts'

test('discovers the same conventional CSS entry for every bundler', async () => {
  const root = mkdtempSync(path.join(tmpdir(), 'hozo-project-theme-'))
  try {
    writeFileSync(
      path.join(root, 'global.css'),
      `@import "tailwindcss";\n@theme { --color-brand: oklch(62% 0.19 259); }\n`,
    )

    const theme = await loadProjectTheme(root)
    assert.equal(theme?.colors.find((color) => color.token === 'brand')?.hex, '#3581f6')
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
})

test('an explicitly missing CSS entry produces one actionable warning', async () => {
  const root = mkdtempSync(path.join(tmpdir(), 'hozo-missing-theme-'))
  try {
    const warnings: string[] = []
    const theme = await loadProjectTheme(root, {
      css: 'styles/tailwind.css',
      warn: (message) => warnings.push(message),
    })

    assert.equal(theme, undefined)
    assert.deepEqual(warnings, [
      '[hozo] no stylesheet at styles/tailwind.css, so utilities resolve against Tailwind\'s defaults',
    ])
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
})
