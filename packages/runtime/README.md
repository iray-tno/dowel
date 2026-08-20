# @hozo/runtime

The small amount that genuinely has to happen at runtime.

Hozo compiles away everything it can. What is left is here: styles the compiler could not resolve statically, the interaction states CSS pseudo-classes give you for free on Web and nothing gives you on Native, and the accessibility behaviour that is behaviour rather than markup.

Nothing here is meant to be imported by hand. Generated components import what they need.

## What lives here

**Dynamic class resolution.** A `className` the compiler could not read — `<View className={getVariant()} />` — is resolved on device against a project-wide map that `@hozo/metro` generates at config time. React Native has no CSS engine, so this is the one place a class name becomes a style object at runtime.

**Contextual variants.** `dark:`, `sm:`/`md:`/`lg:`, and viewport-relative sizes are media queries on Web and subscriptions on Native. `useHozoDark`, `useHozoBreakpoint` and `useHozoViewport` are separate stores on purpose: a component using only `md:` must not re-render on every resize that does not cross a breakpoint.

**Interaction state.** `HozoPressable` tracks pressed, hovered, focused and focus-visible, and provides them to descendants — which is how a `Text` inside a pressed button changes colour without the button knowing what is inside it. Focus-visible follows the input modality, so a tap does not draw a focus ring and a Tab key does.

**Transitions.** `transition-*` utilities compile to `Animated` timings, including colour interpolation, with the blend point preserved when an interrupted transition restarts.

**Layout that CSS does for free.** The `HozoGrid` and `HozoSpaced` helpers reproduce the parts of grid and gap that React Native's layout engine does not have.

## Platform split

`index.ts` is the Web build and `index.native.ts` the React Native one, selected by the `react-native` export condition. The logic worth testing lives in platform-free modules — `ambient.ts`, `grid.ts`, `color-transition.ts` — so it can be tested without a device or a native module registry.
