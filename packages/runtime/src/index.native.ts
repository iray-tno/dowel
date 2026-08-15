// The React Native entry point for `@dowel/runtime`.
//
// Metro resolves a platform extension ahead of the plain file, so an app
// importing `@dowel/runtime` gets this on device and `./index.ts` on Web.
// That split is what lets the parts needing `react`/`react-native` be
// exported at all: `./index.ts` is imported by the Web build too, where
// those modules don't exist.
//
// Generated code imports from the package root -- `import { useDowelDark,
// DowelSpaced } from '@dowel/runtime'` -- so everything the compiler can
// emit as a runtime import has to be reachable from here. See
// `dowel_native::LowerOutput::runtime_imports` for that list.

export * from './index.ts'
export { useDowelDark, useDowelBreakpoint, useDowelViewport } from './hooks.native.ts'
export { DowelSpaced } from './spacing.native.tsx'
export type { BreakpointName, Viewport } from './ambient.ts'
