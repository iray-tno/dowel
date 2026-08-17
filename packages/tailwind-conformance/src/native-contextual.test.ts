import assert from 'node:assert/strict'
import test from 'node:test'

import { compareNativeContextual, NATIVE_CONTEXTUAL_CASES } from './native-contextual.ts'

for (const testCase of NATIVE_CONTEXTUAL_CASES) {
  test(`${testCase.candidate} is covered in its Native interaction context`, () => {
    const result = compareNativeContextual(testCase)
    assert.equal(result.verdict, 'COVERED', result.detail)
  })
}
