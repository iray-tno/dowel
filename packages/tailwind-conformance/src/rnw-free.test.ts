import assert from 'node:assert/strict'
import test from 'node:test'

import { compareRnwFree, RNW_FREE_CASES } from './rnw-free.ts'

for (const testCase of RNW_FREE_CASES) {
  test(`${testCase.primitive} lowers without React Native Web`, () => {
    const result = compareRnwFree(testCase)
    assert.equal(result.covered, true, result.detail)
  })
}
