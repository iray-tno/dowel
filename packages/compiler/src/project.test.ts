import assert from 'node:assert/strict'
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import test from 'node:test'

import { discoverSources, scanProject, writeFileIfChanged } from './project.ts'

function project(): string {
  return mkdtempSync(path.join(tmpdir(), 'hozo-project-'))
}

function source(root: string, relative: string, text = 'export const x = "p-4"'): string {
  const file = path.join(root, relative)
  mkdirSync(path.dirname(file), { recursive: true })
  writeFileSync(file, text)
  return path.resolve(file)
}

test('discovery excludes generated trees and respects gitignore', () => {
  const root = project()
  try {
    const kept = source(root, 'src/kept.ts')
    source(root, 'target/generated.ts')
    source(root, 'temp/checkout.tsx')
    source(root, 'ignored/hidden.ts')
    writeFileSync(path.join(root, '.gitignore'), 'ignored/\n')

    assert.deepEqual(discoverSources(root), [kept])
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
})

test('content include and exclude narrow a walk deterministically', () => {
  const root = project()
  try {
    const kept = source(root, 'app/kept.tsx')
    source(root, 'app/generated/no.tsx')
    source(root, 'src/no.tsx')

    assert.deepEqual(
      discoverSources(root, { include: ['app/**/*.tsx'], exclude: ['app/generated/**'] }),
      [kept],
    )
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
})

test('a complete scan skips unchanged files and sweeps deleted ones', () => {
  const root = project()
  try {
    const removed = source(root, 'src/removed.ts', 'export const old = "p-4"')
    source(root, 'src/kept.ts', 'export const current = "gap-2"')

    const first = scanProject(root)
    assert.equal(first.stats.scannedFiles, 2)
    assert.equal(first.stats.deletedFiles, 0)

    const warm = scanProject(root)
    assert.equal(warm.stats.scannedFiles, 0)
    assert.equal(warm.stats.skippedFiles, 2)

    rmSync(removed)
    const afterDelete = scanProject(root)
    assert.equal(afterDelete.stats.deletedFiles, 1)
    assert.equal(afterDelete.changed, true)
    assert.doesNotMatch(afterDelete.cache.renderCss(), /p-4/)
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
})

test('generated files are not rewritten when their bytes are unchanged', () => {
  const root = project()
  try {
    const file = path.join(root, 'artifact.css')
    assert.equal(writeFileIfChanged(file, '.p-4{}'), true)
    assert.equal(writeFileIfChanged(file, '.p-4{}'), false)
    assert.equal(writeFileIfChanged(file, '.p-8{}'), true)
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
})
