// The surface extractor is a regex over React Native's shipped `.d.ts`, and
// its failure mode is silent: a formatting change upstream makes the surface
// shrink, which turns suspect refusals into confirmed ones and makes the
// audit look cleaner than it is. So the properties the audit actually leans
// on are asserted here -- including absences, which are what a refusal is
// justified by.

import assert from 'node:assert/strict'
import { test } from 'node:test'
import { camelCase, reactNativeStyleKeys } from './native-surface.ts'

test('reads a plausible number of style keys', () => {
  const keys = reactNativeStyleKeys()
  // ~145 at RN 0.87. A loose range: the point is to catch the regex
  // matching nothing or matching the whole file, not to pin a version.
  assert.ok(keys.size > 100 && keys.size < 300, `got ${keys.size} keys`)
})

test('finds keys from every style interface', () => {
  const keys = reactNativeStyleKeys()
  for (const key of [
    'flexDirection', // FlexStyle
    'shadowRadius', // ShadowStyleIOS
    'backgroundColor', // ViewStyle
    'writingDirection', // TextStyleIOS
    'includeFontPadding', // TextStyleAndroid
    'fontWeight', // TextStyle
    'resizeMode', // ImageStyle
  ]) {
    assert.ok(keys.has(key), `missing ${key}`)
  }
})

test('excludes shapes that only appear inside a style value', () => {
  const keys = reactNativeStyleKeys()
  // `BoxShadowValue` has these, but they are not keys you can write at the
  // top level of a StyleSheet entry. Counting them would manufacture
  // expressibility the platform doesn't have.
  for (const key of ['blurRadius', 'spreadDistance', 'offsetX', 'colorStops']) {
    assert.ok(!keys.has(key), `${key} should not be a top-level style key`)
  }
})

test('reads closed unions as value constraints', () => {
  const keys = reactNativeStyleKeys()
  // The case the audit needs most: the key exists, so a property-level
  // check alone would call `display: grid` expressible.
  assert.deepEqual(keys.get('display')?.values, new Set(['none', 'flex', 'contents']))
  assert.deepEqual(keys.get('position')?.values, new Set(['absolute', 'relative', 'static']))
})

test('resolves a union declared through a type alias', () => {
  // `alignItems?: FlexAlignType` -- the literals live one indirection away.
  assert.deepEqual(
    reactNativeStyleKeys().get('alignItems')?.values,
    new Set(['flex-start', 'flex-end', 'center', 'stretch', 'baseline']),
  )
})

test('leaves open types unconstrained', () => {
  const keys = reactNativeStyleKeys()
  // Anything taking a number, colour or dimension has no useful value-level
  // check, and claiming one would refuse valid styles.
  for (const key of ['width', 'backgroundColor', 'zIndex', 'borderTopWidth']) {
    assert.equal(keys.get(key)?.values, undefined, `${key} should be unconstrained`)
  }
})

test('camelCase matches how React Native spells CSS properties', () => {
  assert.equal(camelCase('border-top-width'), 'borderTopWidth')
  assert.equal(camelCase('color'), 'color')
  assert.equal(camelCase('inset-inline-start'), 'insetInlineStart')
})

test('the keys the audit flagged as wrongly refused really are declared', () => {
  const keys = reactNativeStyleKeys()
  // Each of these backed a refusal the audit contradicted, so they are the
  // assertions most worth pinning: if RN ever drops one, the refusal
  // becomes correct again and this test should say so.
  assert.ok(keys.get('textDecorationStyle')?.values?.has('wavy'))
  assert.ok(keys.has('borderColor')) // divide-*
  assert.ok(keys.has('marginInlineStart')) // space-x-*
  assert.ok(keys.has('height')) // h-screen
})
