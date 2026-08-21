import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { test } from 'node:test'

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..', '..')
const generated = path.join(repoRoot, 'crates', 'hozo_parser', 'src', 'tailwind_variants.rs')

test('the checked-in variant list matches the Tailwind it came from', () => {
  // The same hazard as the ARIA table and the conformance harness's class
  // list: a hand-kept copy of somebody else's list drifts, and both halves
  // go on looking reasonable while it does. Tailwind is asked for its own
  // variants -- `getVariants()`, the call the editor tooling makes -- so
  // this only has to check the file on disk is what the generator produces
  // against the installed version.
  const before = readFileSync(generated, 'utf8')
  execFileSync(
    process.execPath,
    [path.join(repoRoot, 'scripts', 'generate-tailwind-variants.mjs')],
    { cwd: repoRoot, stdio: 'pipe' },
  )
  assert.equal(
    readFileSync(generated, 'utf8'),
    before,
    'crates/hozo_parser/src/tailwind_variants.rs is stale -- run `node scripts/generate-tailwind-variants.mjs`',
  )
})

test('the list is what makes "Hozo failed" separable from "not Tailwind"', () => {
  const table = readFileSync(generated, 'utf8')
  const names = [...table.matchAll(/^ {4}"([^"]+)",$/gm)].map(([, name]) => name)

  assert.ok(names.length > 50, `expected Tailwind's variant list, got ${names.length}`)
  // The ones this exists for: real Tailwind, and Hozo compiles none of
  // them. Each is a class an author can write today and watch produce
  // nothing.
  for (const name of ['group', 'peer', 'open', 'checked', 'enabled', 'aria', 'data', 'has']) {
    assert.ok(names.includes(name), `${name} is missing from the variant list`)
  }
  // And the ones Hozo does compile are in it too, so implementing a
  // variant is what removes the diagnostic rather than editing this file.
  for (const name of ['hover', 'focus', 'disabled', 'dark', 'first', 'last']) {
    assert.ok(names.includes(name), `${name} is missing from the variant list`)
  }

  console.log(`        Tailwind: ${names.length} variants defined`)
})
