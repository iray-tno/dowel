const { render } = await import('../dist-ssr/ssr.js')

const first = render()
const second = render()
if (first !== second) throw new Error('SSR output is not deterministic across identical renders')

for (const expected of [
  'data-testid="compatibility-root"',
  'id="compatibility"',
  'data-hozo-pointer-events="box-none"',
  'aria-live="polite"',
  'Compatibility fixture',
  'Universal Web adapter',
]) {
  if (!first.includes(expected)) throw new Error(`SSR output is missing ${expected}: ${first}`)
}
for (const leaked of ['testID=', 'nativeID=', 'onLayout=', 'onScroll=']) {
  if (first.includes(leaked)) throw new Error(`Native-only prop leaked into SSR markup: ${leaked}`)
}

console.log('Web SSR determinism and universal-prop check passed')
