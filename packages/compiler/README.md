# @hozo/compiler

The JS entry point to Hozo's Rust compiler. Every bundler integration goes
through it; nothing else loads the native binding.

## How the binding gets here

Development builds a debug addon next to `src/`:

```sh
pnpm --filter @hozo/compiler build:native
```

A release builds one npm package per platform, each carrying a release
binary and declaring the `os`/`cpu`/`libc` it serves:

```sh
pnpm --filter @hozo/compiler pack:native                       # this machine
pnpm --filter @hozo/compiler pack:native -- --target <triple>  # a cross build
```

`@hozo/compiler` lists all eight as **optional** dependencies, which is
what makes an install ship one binary instead of eight: npm evaluates each
one against the machine, installs the match, and skips the rest without
complaint.

The loader tries them in order — an adjacent development addon first, then
the platform package — and `HOZO_NATIVE_BINDING` overrides both.

## The list that must not drift

`src/native-targets.ts` is the one table. Two independent things read it:
the packer, going from a Rust target triple, and `native-loader.ts`, going
from `process.platform`/`process.arch` at runtime. A disagreement between
them is invisible until someone on the platform nobody develops on installs
a published package and is told no addon could be loaded for it, so
`native-targets.test.ts` walks the table through both directions.

`scripts/check-artifacts.mjs` refuses a release that is missing any of the
eight, for the same reason: an optional dependency that does not exist
installs exactly as quietly as one that was skipped on purpose.

## Release

`.github/workflows/release.yml`, on a `v*` tag. **It has never run.** It
was written on a machine that can build one of the eight targets and cannot
execute a workflow, so the musl and cross-architecture jobs in particular
should be expected to need correcting on the first real attempt.
