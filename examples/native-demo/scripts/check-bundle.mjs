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
for (const component of ['View', 'Text', 'TextInput']) {
  expect(app.includes(`_reactNative.${component}`), `${component} is imported from react-native`)
}
expect(/DowelSpaced/.test(bundle), 'DowelSpaced is bundled')
expect(/DowelDialog/.test(bundle), 'DowelDialog is bundled')

// The utilities became styles and props, and no className survived.
expect(app.includes('placeholderTextColor'), 'placeholder-* became a TextInput prop')
expect(app.includes('accessibilityLabel'), 'the accessible name reached the field')
expect(!/className/.test(app), 'no className is left in the compiled output')
expect(/style: styles\./.test(app), 'elements reference the generated StyleSheet')

// Text styles set on the View were carried down rather than left behind.
expect(/fontSize:/.test(bundle), 'text styles reached the StyleSheet')

if (failures.length > 0) {
  console.error('bundle check failed:')
  for (const failure of failures) console.error(`  - ${failure}`)
  process.exit(1)
}
console.log(`bundle check passed (${bundle.length} bytes)`)
