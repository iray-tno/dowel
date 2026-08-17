import assert from 'node:assert/strict'
import test from 'node:test'

import { A11Y_CONTEXTUAL_CASES, compareA11yContextual } from './a11y-contextual.ts'

for (const testCase of A11Y_CONTEXTUAL_CASES) {
  test(`${testCase.name} has the same accessibility contract on Web and Native`, () => {
    const result = compareA11yContextual(testCase)
    assert.equal(result.covered, true, result.detail)
  })
}
