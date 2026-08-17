// Reads the bundle back.
//
// Building is not enough on its own: Metro will happily bundle a module
// that refers to an identifier nothing imported, because that is only an
// error when it runs. Exactly that shipped -- a compiled `TextInput` with
// no import behind it -- and the build was green. So the bundle is read.

import { readFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const bundle = readFileSync(
  path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', 'dist', 'index.bundle.js'),
  'utf8',
)

// The compiled App, located by a string only it contains.
const marker = bundle.indexOf('you@example.com')
if (marker === -1) throw new Error('the example App is not in the bundle')
const app = bundle.slice(marker - 4000, marker + 2000)

const failures = []
const expect = (condition, description) => {
  if (!condition) failures.push(description)
}

// Every primitive the example uses reaches the bundle bound to something.
for (const component of ['View', 'Text', 'TextInput', 'Image', 'ScrollView', 'FlatList']) {
  expect(app.includes(`_reactNative.${component}`), `${component} is imported from react-native`)
}
expect(/DowelSpaced/.test(bundle), 'DowelSpaced is bundled')
expect(/DowelDialog/.test(bundle), 'DowelDialog is bundled')
expect(/smoke-grid/.test(bundle), 'the device acceptance grid is bundled')
expect(/smoke-horizontal-scroll/.test(bundle), 'the horizontal ScrollView fixture is bundled')
expect(/smoke-row-/.test(bundle), 'the virtualized renderItem fixture is bundled')

// The utilities became styles and props, and no className survived.
expect(app.includes('placeholderTextColor'), 'placeholder-* became a TextInput prop')
expect(app.includes('accessibilityLabel'), 'the accessible name reached the field')
expect(!/className/.test(app), 'no className is left in the compiled output')
expect(/style: styles\./.test(app), 'elements reference the generated StyleSheet')

// Text styles set on the View were carried down rather than left behind.
expect(/fontSize:/.test(bundle), 'text styles reached the StyleSheet')

// The project's own theme, not Tailwind's defaults:  sets
// --spacing to 0.2rem, so  is 19.2 rather than 24, and --color-brand
// resolves to a real hex rather than the not-a-colour marker.
expect(/paddingTop: 19.2/.test(app), 'the project spacing scale reached the styles')
expect(bundle.includes('#3581f6'), 'the project colour resolved')
expect(!/dowel-unresolved/.test(bundle), 'no colour was left unresolved')

// A coarse dependency/runtime regression guard. This is an unminified dev
// bundle, so the margin is intentionally broad; crossing it means a feature
// likely pulled a second platform layer or another large dependency into
// every Native app and deserves inspection.
expect(bundle.length < 4_500_000, `Native dev bundle stays below 4.5 MB (was ${bundle.length} bytes)`)

if (failures.length > 0) {
  console.error('bundle check failed:')
  for (const failure of failures) console.error(`  - ${failure}`)
  process.exit(1)
}

console.log(`bundle check passed (${bundle.length} bytes)`)

// The project's own theme rather than Tailwind's defaults. `global.css`
// sets `--spacing` to 0.2rem, so `p-6` is 19.2px and not 24; and
// `--color-brand` resolves to a real hex rather than the marker the
// compiler emits for a token it can't resolve.
const themed = []
if (!/paddingTop: 19\.2/.test(app)) themed.push('the project spacing scale reached the styles')
if (!bundle.includes('#3581f6')) themed.push('the project colour resolved')
if (/dowel-unresolved/.test(bundle)) themed.push('no colour was left unresolved')

if (themed.length > 0) {
  console.error('theme check failed:')
  for (const failure of themed) console.error(`  - ${failure}`)
  process.exit(1)
}
console.log('theme check passed')
