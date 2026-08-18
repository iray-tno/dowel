// The React Native entry point for `@hozo/runtime`.
//
// Metro resolves a platform extension ahead of the plain file, so an app
// importing `@hozo/runtime` gets this on device and `./index.ts` on Web.
// That split is what lets the parts needing `react`/`react-native` be
// exported at all: `./index.ts` is imported by the Web build too, where
// those modules don't exist.
//
// Generated code imports from the package root -- `import { useHozoDark,
// HozoSpaced } from '@hozo/runtime'` -- so everything the compiler can
// emit as a runtime import has to be reachable from here. See
// `hozo_native::LowerOutput::runtime_imports` for that list.

export * from './index.ts'
export { useHozoDark, useHozoBreakpoint, useHozoViewport, useHozoSpin } from './hooks.native.ts'
export { HozoSpaced } from './spacing.native.tsx'
export { HozoGrid, HozoGridItem } from './grid.native.tsx'
export { HozoLink, type HozoLinkProps } from './link.native.tsx'
export { hozoImageSource } from './image-source.native.ts'
export type { GridTrack } from './grid.ts'
export {
  HozoPressable,
  HozoText,
  type HozoPressableProps,
  type HozoPressableState,
  type HozoTextProps,
  type HozoTransition,
} from './pressable.native.tsx'
// Re-exported rather than left in `@hozo/a11y`: generated code should
// depend on one package, not on how the compiler divides its own. The
// implementation stays there, where its tests and its reasoning are.
export { HozoDialog, type HozoDialogProps } from '@hozo/a11y'
export type { BreakpointName, Viewport } from './ambient.ts'
