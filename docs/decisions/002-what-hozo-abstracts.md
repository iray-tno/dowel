# 2. What Hozo abstracts, and what it carries

**Status:** provisional direction
**Date:** 2026-08-21

Settled far enough to build against and to say no with. The tier names and
the growth list are firm; the order things get built in is not.

## The rule

> **Hozo abstracts what is *declared*. What is *executed* is carried, and
> Hozo takes responsibility for the accessibility envelope around it
> instead.**

This decides cases that otherwise get argued one at a time.

| | | |
| --- | --- | --- |
| SVG | declared markup | abstract it |
| `<table>` and its family | declared markup | abstract it |
| Landmarks, text-level semantics | declared markup | abstract it |
| Canvas 2D | an imperative drawing API | carry it |
| Maps, WebGL | imperative SDKs | carry it |

A compiler can lower `<path d="…" />`. It cannot lower `ctx.arc(…)`,
because what a canvas contains lives in a pile of JavaScript reached
through a ref — there is no declaration to read.

## SVG is the most abstractable of the three, not the least

Worth stating because the intuition runs the other way.

`react-native-svg` deliberately mirrors SVG's element names — `<Svg>`,
`<Path>`, `<Circle>` — and Expo ships it, so it is the de facto standard.
The two platforms' APIs already agree. Lowering `<Svg>/<Path>/<Circle>` to
`<svg>/<path>/<circle>` on Web and to `react-native-svg` on Native is
ordinary work, and it takes `react-native-svg` out of the Web bundle
entirely, which is the same thing Hozo does for React Native for Web.

SVG also carries ARIA semantics of its own (`role="img"`, `<title>`,
`<desc>`), so it is a Tier 2 citizen rather than an escape hatch.

## The tiers

Mostly a name for what already exists.

```
Tier 1  Platform primitives
        View / Text / Pressable / Image / TextInput / ScrollView / FlatList
        React Native shaped. Mechanism only; no meaning React Native does
        not already have.

Tier 2  Semantic primitives                            <- where growth goes
        Section / Article / Nav / List / ListItem / Heading / Paragraph
        + the table family, form fields, landmarks, SVG, figure
        Meaning stated rather than inferred. Real elements on Web, an
        ARIA-annotated View on Native. No runtime.

Tier 3  Patterns that need behaviour (@hozo/a11y)
        Dialog / Tabs / Menu / Listbox / Disclosure / Combobox
        State, focus management, keyboard. Headless, unstyled. This is
        where roving tabindex lives -- see 001.

Tier 4  Styled components (outside core)
        @hozo/base-ui or a third party. Swappable.

Escape  Carried verbatim
        Canvas, Skia, maps, WebGL. Hozo does not model them and does not
        get in the way of them.
```

The split between app authors and library authors falls out of this rather
than needing its own mechanism: Tier 2 and 3 are the sensible defaults,
and the escape hatch plus `sources`, refs, spreads and `onLayout` are the
way down. What was checked, on real compiler output: `ref`, `{...spread}`,
`style`, `data-*` and `onLayout` all survive lowering, and an unmodelled
tag is carried with the tree around it still compiled.

## Growing Tier 2 is a finite list, and it is derived

`aria-query`'s `elementRoles` — already a dependency, already the source
for `crates/hozo_parser/src/aria.rs` — says which HTML elements carry a
role of their own. Those are the ones worth naming; the role-less ones
(`div`, `span`) are `View` and `Text` already.

54 elements carry a role. Hozo models 14. The remaining 40 fall into five
blocks:

| block | |
| --- | --- |
| **Table** (8) | `table` `thead` `tbody` `tfoot` `tr` `th` `td` `caption` |
| **Landmarks** (4) | `main` `header`→banner `footer`→contentinfo `aside`→complementary |
| **Form** (8) | `fieldset` `textarea` `option` `optgroup` `datalist` `progress` `meter` `output` |
| **Text-level** (11) | `em` `strong` `code` `mark` `sub` `sup` `del` `ins` `dfn` `time` `blockquote` |
| **Structure** (7) | `figure` `hr`→separator `dd` `dt` `details` `address` `menu` |

**The table family is the first block to build.** It is the largest gap
between what HTML means and what React Native can say, and Hozo already
has the machinery to check it: the generated role table carries
`required_context` (a `row` must be inside a `table`, `grid` or
`rowgroup`) and `required_owned` (a `table` must own `row`s). A library
author building a data grid would get "this `row` is not inside a
`rowgroup`" at compile time. Nothing else in this space does that.

## Rejected: a generic element escape

Something like `<Element as="aside">` or a `Box` taking a tag name.

**A tag name cannot travel.** `<Element as="aside">` is an `<aside>` on
Web and nothing in particular on Native — the meaning evaporates at the
platform boundary, silently, in code that looks correct. `<Aside>` is an
`<aside>` on Web and a `View` with `role="complementary"` on Native.

Naming a thing *is* the work of making it cross-platform. A generic escape
lets an author skip that work and get a Web-only result without being told.

The replacement is the list above: grow the named set deliberately, from a
denominator someone else maintains.

## What Hozo owes the things it carries

Not modelling something is not the same as ignoring it. A chart Hozo
cannot draw still needs a name and an alternative:

```tsx
<Figure accessibilityLabel="Monthly revenue">
  <Canvas ref={surface} />   {/* carried; Hozo knows nothing about it */}
  <Table>…</Table>           {/* the same data, said another way */}
</Figure>
```

Hozo cannot check what is inside the canvas. It can check that the figure
around it has a name — which is Tier 2's job, and the reason the escape
hatch is not the end of the conversation.

## Not settled

- Whether `Figure` requires an accessible name in its type, or reports a
  missing one as a diagnostic.
- Whether the SVG lowering also validates SVG's own accessibility rules
  or only translates the markup.
- The order the five blocks get built in, beyond table being first.
