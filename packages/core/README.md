# @hozo/core

Hozo's canonical primitives. Import these instead of `react-native`, and the compiler lowers them to semantic DOM and CSS on Web, or to real React Native components and a `StyleSheet` on Native.

```tsx
import { Section, Text, Heading, Pressable } from '@hozo/core'

export function Card({ title, onOpen }) {
  return (
    <Section className="p-4 rounded-lg bg-white">
      <Heading level={2} className="text-xl">{title}</Heading>
      <Pressable accessibilityRole="button" onPress={onOpen}>
        <Text>Open</Text>
      </Pressable>
    </Section>
  )
}
```

On Web that `Section`/`Heading` pair becomes `<section>` and `<h2>` with a scoped class, not a pair of `<div>`s. The primitives with no React Native equivalent — `Section`, `Article`, `Nav`, `List`, `ListItem`, `Paragraph`, `Heading` — exist so that the semantics can be *stated* rather than inferred from a `View` that happens to look like a heading.

## These also run

Every component here is a working React implementation, not a marker the compiler consumes and discards. When Hozo isn't in the build, or can't fully lower a particular usage, the same code renders through these — with the accessibility props mapped to ARIA either way. That is what makes adoption incremental and what makes a Storybook story render before any of this is set up.

The Native counterparts are chosen the same way: `Image` maps to React Native's `Image`, `ScrollView` and `FlatList` to theirs, so the virtualized list stays virtualized.

## Accessibility props

The universal props follow React Native's names — `accessibilityLabel`, `accessibilityHint`, `accessibilityState`, `accessibilityValue`, `accessibilityLiveRegion`, `testID`, `nativeID` — and map onto ARIA on Web. `role` is accepted directly as well; React Native has supported it since 0.71, so it means the same thing on both sides.

`Pressable` takes them too. It did not until recently — its props extended only the responder set, so `testID` and `accessibilityState` were not part of its contract at all, which is a strange place for the gap to be: an interactive element is exactly where `aria-checked`, `aria-expanded` and `aria-selected` earn their keep.
