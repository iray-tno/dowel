import assert from 'node:assert/strict'
import test from 'node:test'

import { viteFinal } from './preset.ts'

test('adds Hozo before Storybook framework plugins without replacing config', () => {
  const frameworkPlugin = { name: 'storybook:framework' }
  const config = {
    root: '/project',
    resolve: { alias: { '~': '/project/src' } },
    plugins: [frameworkPlugin],
  }

  const result = viteFinal(config, { css: 'src/theme.css', debug: true })

  assert.equal(result.root, '/project')
  assert.deepEqual(result.resolve, config.resolve)
  assert.equal(result.plugins?.[0]?.name, 'hozo')
  assert.equal(result.plugins?.[1], frameworkPlugin)
})
