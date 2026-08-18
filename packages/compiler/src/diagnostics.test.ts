import assert from 'node:assert/strict'
import { test } from 'node:test'

import { formatDiagnostic, reportDiagnostics } from './diagnostics.ts'

const warning = { code: 'DYNAMIC_CLASS_NAME_NOT_RESOLVED', message: 'unreadable', severity: 'warning' as const, line: 1, column: 1 }
const error = { code: 'WEB_ONLY_PROPERTY_ON_NATIVE', message: 'no such style', severity: 'error' as const, line: 2, column: 3 }

test('warnings go to the caller, one line each', () => {
  const seen: string[] = []
  reportDiagnostics([warning, warning], 'Page.tsx', (message) => seen.push(message))
  assert.deepEqual(seen, [formatDiagnostic(warning), formatDiagnostic(warning)])
})

test('an error stops the build, whichever integration is asking', () => {
  // The reason this is shared rather than Metro's own: every
  // error-severity diagnostic Hozo emits today comes from the Native
  // backend, so the Web integrations had never been handed one and warned
  // on everything. The first Web error added would have shipped.
  assert.throws(
    () => reportDiagnostics([warning, error], 'Page.tsx', () => {}),
    (thrown: Error) => {
      assert.match(thrown.message, /Page\.tsx cannot be compiled/)
      assert.match(thrown.message, /WEB_ONLY_PROPERTY_ON_NATIVE/)
      // Only the errors are listed; the warning is not an error.
      assert.ok(!thrown.message.includes('DYNAMIC_CLASS_NAME_NOT_RESOLVED'))
      return true
    },
  )
})

test('nothing to report is not an error', () => {
  let called = false
  reportDiagnostics([], 'Page.tsx', () => {
    called = true
  })
  assert.ok(!called)
})
