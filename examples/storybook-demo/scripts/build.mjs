// Runs the Storybook build, and survives a teardown crash that has nothing
// to do with the build.
//
// Three runs in eight, on Windows, this exits 0xC0000409 *after* writing
// every artifact correctly -- on a libuv assertion,
// `!(handle->flags & UV_HANDLE_CLOSING)` in `src\win\async.c`, which is
// `uv_async_send` reaching a handle the loop has already begun closing.
// The build is finished by then; `check-build.mjs` passes on the output of
// a crashed run. Nothing in Hozo is on that path -- the crash is between
// Node and the worker threads Storybook's bundler leaves behind.
//
// So the exit code stops being the judge and the artifacts start, which is
// the same rule `check-build.mjs` was written under: prove the build did
// its job rather than that it exited zero. A genuine failure still fails,
// because the output directory is removed first and the check runs
// against whatever is actually there afterwards.
//
// Narrow on purpose: only this exit code, only on Windows. Any other
// failure is reported as-is.

import { spawnSync } from 'node:child_process'
import { existsSync, rmSync } from 'node:fs'

/** 0xC0000409, as Node reports it either signed or unsigned. */
const STACK_BUFFER_OVERRUN = new Set([-1073740791, 3221226505])

// Removed rather than overwritten: a stale artifact from an earlier run
// would let a build that produced nothing look like one that worked.
rmSync('storybook-static', { recursive: true, force: true })

const result = spawnSync(
  'storybook',
  ['build', '--output-dir', 'storybook-static', '--quiet'],
  { stdio: 'inherit', shell: true },
)

if (result.status === 0) process.exit(0)

const teardownCrash = process.platform === 'win32' && STACK_BUFFER_OVERRUN.has(result.status)
if (teardownCrash && existsSync('storybook-static')) {
  console.warn(
    `[storybook-demo] build exited ${result.status} after writing its output -- ` +
      'the known libuv teardown crash. check-build.mjs decides from here.',
  )
  process.exit(0)
}

console.error(`[storybook-demo] storybook build failed with ${result.status ?? result.signal}`)
process.exit(1)
