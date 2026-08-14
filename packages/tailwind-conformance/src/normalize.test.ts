// The normalizer is the load-bearing part of the whole comparison: if it
// mis-resolves, every reported match and diff downstream is suspect. These
// pin its behavior directly, including the cases where it must refuse to
// resolve rather than guess.

import assert from 'node:assert/strict'
import { test } from 'node:test'
import { normalize } from './normalize.ts'

const vars = new Map<string, string>([
  ['--spacing', '0.25rem'],
  ['--color-blue-500', 'oklch(62.3% 0.214 259.815)'],
  ['--text-xl', '1.25rem'],
  ['--text-xl--line-height', 'calc(1.75 / 1.25)'],
  ['--radius-lg', '0.5rem'],
])

function decls(block: string) {
  return Object.fromEntries(normalize(block, vars).declarations)
}

test('resolves var() and calc() into pixels', () => {
  assert.deepEqual(decls('padding: calc(var(--spacing) * 4);'), {
    'padding-top': '16px',
    'padding-right': '16px',
    'padding-bottom': '16px',
    'padding-left': '16px',
  })
})

test('expands the flex shorthand so 1 and 1 1 0% agree', () => {
  assert.deepEqual(decls('flex: 1;'), decls('flex: 1 1 0%;'))
  assert.deepEqual(decls('flex: auto;'), { 'flex-grow': '1', 'flex-shrink': '1', 'flex-basis': 'auto' })
  assert.deepEqual(decls('flex: none;'), { 'flex-grow': '0', 'flex-shrink': '0', 'flex-basis': 'auto' })
})

test('expands box shorthands to longhands, including 2-value form', () => {
  assert.deepEqual(decls('margin: 4px 8px;'), {
    'margin-top': '4px',
    'margin-right': '8px',
    'margin-bottom': '4px',
    'margin-left': '8px',
  })
  assert.deepEqual(decls('inset: 0px;'), { top: '0', right: '0', bottom: '0', left: '0' })
})

test('a shorthand and its longhands normalize identically', () => {
  assert.deepEqual(
    decls('padding: 16px;'),
    decls('padding-top: 16px; padding-right: 16px; padding-bottom: 16px; padding-left: 16px;'),
  )
})

test('resolves color custom properties', () => {
  assert.deepEqual(decls('background-color: var(--color-blue-500);'), {
    'background-color': 'oklch(62.3% 0.214 259.815)',
  })
})

test('folds a unitless line-height ratio against the font size in the same rule', () => {
  // Tailwind's text-xl: font-size 1.25rem, line-height calc(1.75/1.25).
  assert.deepEqual(
    decls('font-size: var(--text-xl); line-height: var(--tw-leading, var(--text-xl--line-height));'),
    { 'font-size': '20px', 'line-height': '28px' },
  )
})

test('resolves --tw-border-style to its registered initial value', () => {
  assert.deepEqual(decls('border-style: var(--tw-border-style); border-width: 1px;'), {
    'border-top-style': 'solid',
    'border-right-style': 'solid',
    'border-bottom-style': 'solid',
    'border-left-style': 'solid',
    'border-top-width': '1px',
    'border-right-width': '1px',
    'border-bottom-width': '1px',
    'border-left-width': '1px',
  })
})

test('expands the border-style shorthand to per-side longhands', () => {
  // Tailwind writes the shorthand; Dowel emits per-side longhands so that
  // `border-t` can scope its style to one edge.
  assert.deepEqual(
    decls('border-style: solid;'),
    decls(
      'border-top-style: solid; border-right-style: solid; border-bottom-style: solid; border-left-style: solid;',
    ),
  )
})

test('treats zero as unit-agnostic', () => {
  assert.deepEqual(decls('top: 0px;'), decls('top: 0;'))
})

test('normalizes rem to px', () => {
  assert.deepEqual(decls('border-radius: var(--radius-lg);'), { 'border-radius': '8px' })
})

test('reports unresolvable values instead of guessing', () => {
  // An unknown custom property with no fallback must not silently vanish
  // or resolve to something invented.
  const result = normalize('box-shadow: var(--tw-shadow);', vars)
  assert.equal(result.declarations.size, 0)
  assert.equal(result.unresolved.length, 1)
})

test('declines to fold calc() mixing incompatible units', () => {
  const result = normalize('width: calc(100% - 4px);', vars)
  assert.equal(result.unresolved.length, 1)
})

test('resolves registers assigned in the same rule that references them', () => {
  // Tailwind sets `--tw-blur` and reads it back in the very next
  // declaration; the assignment isn't output to compare, but it has to be
  // in scope first.
  assert.deepEqual(
    decls('--tw-blur: blur(8px); filter: var(--tw-blur,) var(--tw-brightness,);'),
    { filter: 'blur(8px)' },
  )
})

test('resolves a long var() chain without truncating the tail', () => {
  // Regression: each pass resolves one var(), and Tailwind's `filter`
  // chains nine. A depth cap of 8 left the last one unresolved, which
  // showed up as a bogus SKIP rather than an obviously wrong value.
  const nine = Array.from({ length: 9 }, (_, i) => `var(--slot-${i},)`).join(' ')
  const withVars = new Map(vars)
  withVars.set('--slot-8', 'blur(8px)')
  const result = normalize(`filter: ${nine};`, withVars)
  assert.equal(result.unresolved.length, 0)
  assert.equal(result.declarations.get('filter'), 'blur(8px)')
})

test('drops fully transparent box-shadow layers', () => {
  // Tailwind always splices its ring/inset-ring registers into box-shadow;
  // unset they are `0 0 #0000`, which paints nothing.
  assert.deepEqual(decls('box-shadow: 0 0 #0000, 0 0 #0000, 0 1px 3px 0 rgb(0 0 0 / 0.1);'), {
    'box-shadow': '0 1px 3px 0 rgb(0 0 0 / 0.1)',
  })
})

test('skips Tailwind runtime register declarations', () => {
  // `--tw-*` assignments feed later declarations; they are not output to
  // compare on their own.
  assert.deepEqual(decls('--tw-leading: 1; line-height: 1;'), { 'line-height': '1' })
})
