import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { test } from 'node:test'

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..', '..')
const generated = path.join(repoRoot, 'crates', 'hozo_parser', 'src', 'aria.rs')

test('the checked-in ARIA table matches the specification it came from', () => {
  // The same hazard the Tailwind harness exists for, one layer down: a
  // hand-kept copy of somebody else's specification drifts, and both
  // halves go on looking reasonable while it does. `aria-query` is the
  // machine-readable ARIA spec -- the one eslint-plugin-jsx-a11y and
  // Testing Library read -- so the table is generated from it and this
  // checks the file on disk is what the generator produces today.
  const before = readFileSync(generated, 'utf8')
  execFileSync(process.execPath, [path.join(repoRoot, 'scripts', 'generate-aria.mjs')], {
    cwd: repoRoot,
    stdio: 'pipe',
  })
  const after = readFileSync(generated, 'utf8')
  assert.equal(
    after,
    before,
    'crates/hozo_parser/src/aria.rs is stale -- run `node scripts/generate-aria.mjs`',
  )
})

test('carries what a role needs to mean anything', () => {
  const table = readFileSync(generated, 'utf8')
  // Spot checks, because the generator is what this trusts and a table
  // that generated cleanly from the wrong fields would still be wrong.
  assert.match(table, /name: "combobox".*required_props: &\["aria-controls", "aria-expanded"\]/)
  assert.match(table, /name: "option".*required_props: &\["aria-selected"\]/)
  assert.match(table, /name: "tab".*required_context: &\["tablist"\]/)
  assert.match(table, /name: "listbox".*required_owned: &\["option"\]/)
  // Abstract roles are in the table and marked, so `role="widget"` can be
  // named as the mistake it is rather than reported as unknown.
  assert.match(table, /name: "widget", is_abstract: true/)
})
