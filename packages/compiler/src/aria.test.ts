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

test('every requirement the table carries is one the checker reads', () => {
  // The coverage claim, stated as a measurement rather than a promise.
  //
  // There is no per-role code in `aria_check.rs` -- it reads
  // `required_props`, `required_context` and `required_owned` for whatever
  // role it finds, so coverage is structural: every role with a
  // requirement is checked, and a role gains checks the moment the
  // specification gives it one and the table is regenerated.
  const table = readFileSync(generated, 'utf8')
  const rows = [...table.matchAll(/AriaRole \{ name: "([^"]+)", is_abstract: (\w+), required_props: &\[([^\]]*)\], required_context: &\[([^\]]*)\], required_owned: &\[([^\]]*)\]/g)]
  const usable = rows.filter(([, , isAbstract]) => isAbstract === 'false')
  const withRequirement = usable.filter(([, , , props, context, owned]) => props || context || owned)

  assert.ok(usable.length > 70, `expected the ARIA role list, got ${usable.length}`)
  assert.ok(withRequirement.length > 10, `expected roles with requirements, got ${withRequirement.length}`)

  // What the checker reads is exactly the three fields the generator
  // writes. Anything ARIA says that is not in those fields -- prohibited
  // properties, name-from-content, allowed values -- is outside this and
  // stays outside until the table carries it.
  const checker = readFileSync(
    path.join(repoRoot, 'crates', 'hozo_parser', 'src', 'aria_check.rs'),
    'utf8',
  )
  for (const field of ['required_props', 'required_context', 'required_owned']) {
    assert.match(checker, new RegExp(`spec\.${field}`), `${field} is in the table and unread`)
  }
  console.log(
    `        ARIA: ${usable.length} usable roles, ${withRequirement.length} carrying a requirement`,
  )
})
