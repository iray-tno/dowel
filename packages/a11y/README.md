# @hozo/a11y

The accessibility patterns that need real runtime behaviour.

Most of what Hozo does for accessibility is compile-time: roles, states and properties become ARIA attributes in the markup, and the compiler reports a role used without the properties it is meaningless without. Some patterns are not markup. A dialog has to trap focus, restore it on close, and respond to Escape. That is behaviour, and it lives here.

```tsx
import { Dialog } from '@hozo/core'   // re-exported from here

<Dialog open={open} onClose={close} accessibilityLabel="Settings">
  <Text>…</Text>
</Dialog>
```

## Dialog

On Web it is a real `<dialog>` element, so the browser supplies the modal semantics, the top layer and the backdrop rather than this package reimplementing them. Focus moves to the first sensible target on open and returns to whatever had it before, and Escape closes.

On Native it is React Native's `Modal` with `accessibilityViewIsModal`, which is what tells the platform screen reader not to reach the content behind it. `animationType` defaults to `fade` rather than `slide`: a dialog animating in from an edge reads as a screen transition, and nothing in the source says which the author meant.

The focus logic itself — which candidate to focus, whether to restore — is in `focus.ts`, free of both platforms' imports, so it is tested directly rather than through a rendered tree.

## Scope

This package grows as patterns are added. Everything in it is here because it needs state, keyboard handling or platform APIs; anything that can be a compile-time attribute is not here.
