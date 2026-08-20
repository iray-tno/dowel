# 1. `disabled` means one thing, and it is not focusable

**Status:** decided
**Date:** 2026-08-20

## Decision

`disabled` on a Hozo primitive means exactly this, on every platform:

> inoperable, announced as disabled, present in the accessibility tree, absent
> from the keyboard focus order.

Hozo does **not** offer the other reading — the ARIA APG "focusable disabled"
pattern, where a control stays in the tab order, announces itself as
unavailable, and does nothing when activated. `focusable` / `tabIndex` remain
real props (roving tabindex needs them) but cannot be combined with `disabled`;
that combination is a diagnostic, not a feature.

## Why not offer both

The test applied was: **can an `aria-disabled` equivalent — announcement with
no change in behaviour — be produced on both native platforms?**

| | announcement without behaviour change |
| --- | --- |
| Web | yes: emit `aria-disabled`, omit the `disabled` attribute |
| iOS | yes, it is already the default (see below) |
| **Android** | **no, not without native code** |

React Native routes one prop to two places:

```
accessibilityState.disabled  (and its alias aria-disabled)
  ├─ BaseViewManager.java:385        view.setEnabled(false)   ← kills input focus
  └─ ReactAccessibilityDelegate.kt:634  info.isEnabled = false ← the announcement
```

`View.setEnabled(false)` removes the view from input focus — Android's
`View.canTakeFocus()` requires `ENABLED`, checked when focus is requested, so
an explicit `focusable={true}` loses regardless of ordering. There is no way
from JavaScript to get the second line without the first.

Offering a `focusable` escape would therefore mean shipping a prop that works
on Web, works on iOS, and silently does nothing on Android — for a library
whose claim is that one source means one thing on both platforms.

## Two things that are *not* the reason

**`AccessibilityNodeInfo.setStateDescription()` is not the missing piece.**
React Native never writes it (the only mention is a read at
`ReactAccessibilityDelegate.kt:808`), so it is unreachable from JavaScript —
but even if it were exposed it would not help. It carries spoken *text*, not a
state: Switch Access, Voice Access and braille displays get nothing semantic
from it, and it needs hand localisation. The semantic channel is
`info.isEnabled`, which React Native already sets. And exposing
`stateDescription` would not stop `BaseViewManager` from calling
`view.setEnabled(false)`; dropping the prop to avoid that would lose
`info.isEnabled` too, leaving only text. That is worse than what we have.

**iOS is not the obstacle.** On both architectures `accessibilityState.disabled`
sets one announcement bit and changes nothing else:

```
React/Fabric/Mounting/ComponentViews/View/RCTViewComponentView.mm:482
  self.accessibilityTraits |= UIAccessibilityTraitNotEnabled;
React/Views/RCTViewManager.m:47
  @"disabled" : @(UIAccessibilityTraitNotEnabled)
```

`userInteractionEnabled` is only ever driven by `pointerEvents`
(`RCTViewComponentView.mm:347`, `RCTView.m:176`), `accessibilityElement`
returns `self`, and `RCTViewComponentView` has no `accessibilityTraits`
getter override. The press is blocked in Pressable's JavaScript, not by the
view. So iOS is natively the focusable-disabled reading, and Android is not —
**React Native does not agree with itself here**, which is why "match native"
could not settle the question and the reasoning above had to.

*(Not verified from source: that VoiceOver focuses `NotEnabled` elements, and
whether iOS Full Keyboard Access skips them. Both need a device.)*

## What users do instead

The APG pattern exists to let a keyboard user reach a control and learn why it
is unavailable. That outcome is reachable without `disabled` at all, on every
platform, with no special support from Hozo:

```tsx
<Pressable onPress={busy ? explainWhy : save} className="opacity-50">
  <Text>Save</Text>
</Pressable>
```

Arguably this is the more honest encoding. The control *is* operable — it
answers — so calling it disabled was the inaccurate part. The dimming is
styling.

## The cost we accept

`disabled` in the C reading loses focus when the focused element becomes
disabled: the browser moves focus to `<body>`, a screen reader user loses their
place, and the next Tab starts from the top of the document. `disabled={busy}`
is the common shape, so this is not hypothetical and belongs in the user
documentation.

It is accepted because the criterion for a default is *what happens when the
developer does nothing else*. The focusable-disabled reading's advantage — the
user learns why — requires the author to supply a reason; without it, it is a
tab stop that announces "dimmed" and then silently does nothing. The C
reading's drawback appears only in one specific pattern.

## Deferred: `@hozo/android-a11y`

This will come up once there are users. It is worth doing, and the mechanism is
known and supported — Android expects exactly this extension point.

**What to build.** Keep `View.setEnabled(true)` so focus works, and install an
`AccessibilityDelegate` whose `onInitializeAccessibilityNodeInfo` forces
`info.isEnabled = false` (optionally with a `stateDescription` for polish).
React Native's own entry point is public and static:

```
ReactAccessibilityDelegate.kt:587   public fun setDelegate(...)   @JvmStatic
                              :604    ViewCompat.setAccessibilityDelegate(...)
```

Reaching it needs a native module: either a Fabric ViewManager carrying a
Hozo-specific prop, or a TurboModule that resolves a view by tag and swaps the
delegate. A plain prop will not do — `BaseViewManager` re-applies
`setEnabled(false)` on every render that carries `accessibilityState.disabled`.

**What blocks it, and it is not the work.** Hozo has no native code today, and
this repository has no device, no emulator and no Android CI. Shipping
*unverified accessibility* native code is the worst version of this: the
failure mode is silence — the control is never announced as disabled and
nobody notices. Every bug this decision came out of had that shape. Device
validation comes first; it is already on the roadmap.

**Why this stays additive.** `disabled` keeps its single meaning either way.
Adding Android support later turns an existing diagnostic into a supported
opt-in. No source a user has written changes meaning, and no released
behaviour changes for anyone who did not opt in.

The diagnostic text should say "not supported" rather than "impossible", so it
stays true when this lands:

```
focusable has no effect on a disabled element.
React Native's disabled state removes keyboard focus on Android, and Hozo
cannot yet separate that from the announcement.
To let people reach the control and learn why it is unavailable, keep it
enabled and answer in the handler instead.
```

## Checking this against a newer React Native

Every line reference above is from `react-native@0.87.0`. If the routing in
`BaseViewManager` or `ReactAccessibilityDelegate` changes, or if React Native
gains a first-class way to announce a disabled state without disabling the
view, this decision should be revisited — that would be the whole reason it
was made.
