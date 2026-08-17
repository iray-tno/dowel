import assert from 'node:assert/strict'
import test from 'node:test'

import {
  compareNativeGridContextual,
  NATIVE_GRID_CONTEXTUAL_CASES,
} from './native-grid-contextual.ts'

for (const testCase of NATIVE_GRID_CONTEXTUAL_CASES) {
  test(`${testCase.name} is covered in its Native grid context`, () => {
    const result = compareNativeGridContextual(testCase)
    assert.equal(result.verdict, 'COVERED', result.detail)
  })
}
