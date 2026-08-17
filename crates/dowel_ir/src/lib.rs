//! Platform-independent IR shared across the Dowel compiler pipeline.
//!
//! `Node` (semantic tree) plus `StyleDeclaration` (per-node style, each
//! gated by a `Condition`) together form the Dowel IR described in the
//! proposal's architecture section. Values here are the compiler's output
//! shape, not its parsing shape -- `dowel_parser` builds this from
//! TSX/Tailwind source.

mod colors;
pub use colors::{resolve_color_token, ResolvedColor};

mod theme;
pub use theme::{Theme, ThemeColor};

// ---------------------------------------------------------------------------
// Source spans / diagnostics
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSpan {
    pub start: u32,
    pub end: u32,
}

/// Reference to a source expression Dowel does not evaluate or interpret --
/// only re-emits verbatim into generated output (an event handler, a prop
/// value the compiler doesn't model, or the leaf of a `ConditionExpr`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExprRef(pub SourceSpan);

#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub severity: Severity,
    pub message: String,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Build-stopping. Reserved for cases where continuing would ship
    /// something silently wrong -- e.g. a Web-only utility reaching the
    /// Native backend, where dropping it would leave a layout that looks
    /// right on Web and is broken on device with no signal.
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticCode {
    /// Interactive Pressable/Button with no accessible role (proposal §10.2).
    A11yInteractiveWithoutRole,
    /// A `Dialog` that can't be dismissed (proposal §10.3).
    ///
    /// Escape on Web and the hardware back button on Android both arrive
    /// as a request to close, and a modal that ignores them is a trap --
    /// the one failure in §10.3's quality bar that a compiler *can* see,
    /// since it is a missing prop rather than a runtime behaviour.
    A11yDialogWithoutDismiss,
    /// An element that needs an accessible name and has none (proposal
    /// §10.2): a `TextInput`, a `Dialog`.
    ///
    /// One code rather than one per element, because the fix is the same
    /// sentence every time -- give it a name -- and the message says which
    /// element it is.
    ///
    /// Separate from the role diagnostic because the fix is different and
    /// the wrong fix is so common: a `placeholder` looks like a label,
    /// and is not one. Screen readers may not announce it, and it
    /// vanishes the moment the user types -- which is exactly when they
    /// would want to check what the field was for.
    A11yMissingAccessibleName,
    /// A class carries square brackets or a `(--var)` -- Tailwind's
    /// arbitrary syntax -- and Dowel couldn't read it.
    ///
    /// This exists because the alternative is so much worse than silence.
    /// Dowel's utility parsing ends in a colour catch-all: whatever the
    /// specific forms decline becomes a palette token. Before this code
    /// existed, `text-[14px]` -- a font size -- came out the other end as
    /// `color: var(--dowel-color-[14px])`, and `w-[32px]` was dropped with
    /// no trace at all. Neither failed; both produced a page that was
    /// simply wrong somewhere the author would have to find by eye.
    ///
    /// Brackets are what makes this detectable. A bare unknown class is
    /// ordinary -- projects have their own CSS and Dowel must leave it
    /// alone -- but a bracket is unambiguously Tailwind asking for
    /// something, so failing to read one is worth saying out loud.
    UnreadableArbitraryValue,
    /// A prop spread appears after a statically compiled className/style and
    /// could silently override it at runtime; that node's className is not
    /// compiled and falls back instead of failing silently.
    UnsafePropSpreadAfterStyle,
    /// A utility with no React Native equivalent reached the Native
    /// backend. Verified against Yoga (RN's layout engine), whose `display`
    /// is only Flex/None/Contents and which has no grid implementation at
    /// all. Dowel maps `block` to Yoga's visible flex mode, but inline and
    /// grid formatting contexts cannot be approximated and are refused.
    WebOnlyPropertyOnNative,
    /// Part of a `className` couldn't be decomposed statically (proposal
    /// §7's third tier). The expression is preserved so its classes still
    /// reach the DOM, but Dowel generates no CSS for whatever they turn out
    /// to be -- only for classes it could read at build time.
    DynamicClassNameNotResolved,
    /// A utility React Native could express, that Dowel doesn't lower yet.
    ///
    /// Distinct from `WebOnlyPropertyOnNative`, and the distinction is the
    /// point: those utilities are impossible on this platform (Yoga has no
    /// grid), whereas these are merely unbuilt. Keeping them apart is what
    /// stops "not built yet" from hardening into "can't be built" -- and
    /// the refusal audit (`@dowel/tailwind-conformance`) leans on it, since
    /// only the Web-only claims are ones React Native's types can contradict.
    ///
    /// Covers a variant with nothing to drive it (`hover:`/`focus:`, which
    /// are real on tablets with a pointer and on the desktop/visionOS
    /// targets; `disabled:` with no `disabled` prop), a metric that needs a
    /// font size the element doesn't set (`leading-tight` alone), and a
    /// utility whose target primitive Dowel doesn't have yet
    /// (`placeholder-*`, which React Native carries as `TextInput`'s
    /// `placeholderTextColor`).
    NotWiredOnNative,
    /// A Dowel primitive sits inside something the compiler carries but
    /// doesn't read -- an expression container, or an unmodeled component's
    /// children -- so it reaches output as source rather than as compiled
    /// markup.
    ///
    /// Not an error: on Web `@dowel/core`'s real components render it and
    /// the candidate stylesheet supplies its CSS, and on Native `View`/
    /// `Text`/`Pressable` resolve to the same react-native components Dowel
    /// lowers to. It costs the compile-time benefit for that element, which
    /// is worth saying out loud rather than leaving the user to wonder why
    /// one element behaves differently.
    PrimitiveNotLowered,
}

// ---------------------------------------------------------------------------
// Node tree (semantic IR)
// ---------------------------------------------------------------------------

/// Phase 0 primitive set (proposal §13). Image/Link land in a later phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Primitive {
    View,
    Text,
    Pressable,
    Button,
    /// A modal dialog (proposal §10.3, v1's first hard primitive).
    ///
    /// A primitive rather than a component the compiler walks past,
    /// because otherwise its `className` never compiles and its missing
    /// accessible name is never noticed -- both backends lower it to
    /// `/a11y`'s `DowelDialog`, which owns the runtime behaviour.
    Dialog,
    /// A single-line text field. `<input>` on Web, `TextInput` on React
    /// Native -- and the reason `placeholder-*` can lower at all, since
    /// React Native carries that colour as a prop on this component
    /// rather than as a style on anything.
    TextInput,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub primitive: Primitive,
    pub style: Vec<StyleDeclaration>,
    pub props: PropSet,
    /// Everything between this element's tags, in source order.
    ///
    /// Ordered and total: order matters (`<Text>Hello {name}</Text>` is not
    /// `<Text>{name} Hello</Text>`), and every JSX child is represented,
    /// including the ones the compiler doesn't understand. Before
    /// 2026-08-15 this was a `Vec<Node>` holding only Dowel primitives,
    /// with a separate `text` field -- which meant an unmodeled component,
    /// an expression, or a fragment had nowhere to go and was silently
    /// deleted from the output.
    pub children: Vec<Child>,
    /// Parts of a `className` expression that couldn't be statically
    /// decomposed into `style` (proposal §7's "truly dynamic" tier) --
    /// threaded through to `@dowel/runtime`'s `cx()` at render time.
    /// Populated per-leaf, not per-node: a `cn(...)` call can contribute
    /// some declarations to `style` and some entries here in the same call.
    pub class_name_fallback: Vec<ExprRef>,
    pub span: SourceSpan,
}

/// One thing between an element's tags.
#[derive(Debug, Clone, PartialEq)]
pub enum Child {
    /// A Dowel primitive, lowered like any other element.
    Node(Node),
    /// Literal text, already trimmed of surrounding JSX whitespace.
    Text(String),
    /// Everything else that renders: a component Dowel doesn't model, an
    /// expression container (`{name}`, `{cond && <A/>}`, `{items.map(..)}`),
    /// a fragment, a child spread.
    ///
    /// Re-emitted from the original source -- the same treatment
    /// `PropSet::passthrough` and `class_name_fallback` already give the
    /// parts of a component the compiler doesn't claim to understand. Not
    /// understanding something is a reason to leave it alone, not a reason
    /// to drop it.
    ///
    /// "Doesn't understand" applies to the *expression*, not to what's
    /// inside it. A Dowel primitive nested in there is perfectly readable,
    /// so each one is lowered and spliced back into the re-emitted text at
    /// `nested` -- `{show && <Text className="p-4">hi</Text>}` compiles its
    /// `Text` exactly as a top-level one, while `show &&` stays untouched.
    Verbatim { source: ExprRef, nested: Vec<NestedNode> },
}

/// A Dowel primitive found inside a `Child::Verbatim`, with the source
/// range its lowered output replaces.
#[derive(Debug, Clone, PartialEq)]
pub struct NestedNode {
    pub span: SourceSpan,
    pub node: Node,
}

impl Child {
    /// Whether this occupies a position `:first-child` would count. CSS
    /// counts elements only, so literal text doesn't shift anything.
    ///
    /// `Verbatim` is the interesting case: it may render nothing, one
    /// element, or a hundred (`{items.map(..)}`), so a sibling after one
    /// has no compile-time position at all.
    pub fn is_element_position(&self) -> bool {
        matches!(self, Child::Node(_) | Child::Verbatim { .. })
    }
}

/// A JSX attribute Dowel doesn't model, carried through to output
/// untouched. Stored as the span of the *whole* attribute rather than a
/// name/value pair, because that one representation covers every form
/// uniformly -- `testID="row"`, `onLayout={fn}`, bare `autoFocus`, and
/// `{...rest}` (which has no name at all, so a name/value pair couldn't
/// represent it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PassthroughProp {
    pub span: ExprRef,
    /// True for `{...expr}`. Tracked separately because a spread's
    /// *position* matters: JSX resolves duplicate props last-wins, so a
    /// spread after Dowel's compiled className can silently override it at
    /// runtime (see `DiagnosticCode::UnsafePropSpreadAfterStyle`).
    pub is_spread: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PropSet {
    pub on_press: Option<ExprRef>,
    pub disabled: Option<ConditionExpr>,
    /// Explicit override; `None` means derive the role from `Primitive`
    /// (e.g. `Button` -> `AccessibilityRole::Button`).
    pub accessibility_role: Option<AccessibilityRole>,
    /// A `TextInput`'s accessible name, from `aria-label`/
    /// `accessibilityLabel`. Modelled rather than passed through because
    /// its *absence* is the diagnosis (proposal §10.2): a field with no
    /// name is unusable with a screen reader, and `placeholder` is the
    /// classic thing people reach for instead -- it disappears on first
    /// keystroke and is not announced as a label.
    pub accessibility_label: Option<ExprRef>,
    /// A `Dialog`'s `open` guard, re-emitted verbatim like `disabled`.
    pub open: Option<ConditionExpr>,
    /// Whether a `Dialog` was given an `onClose`. A modal with no way to
    /// dismiss it reads as a trap, and that is worth naming at build time
    /// rather than discovering with a screen reader.
    pub has_on_close: bool,
    /// Whether a `TextInput` was given a `placeholder`. Only whether, not
    /// what: the value is passed through untouched, and all the compiler
    /// needs to know is that a name-less field has one, which is the case
    /// worth naming.
    pub has_placeholder: bool,
    /// Props Dowel doesn't model explicitly -- re-emitted unchanged, in
    /// source order (which JSX's last-wins duplicate resolution depends on).
    pub passthrough: Vec<PassthroughProp>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessibilityRole {
    Button,
    Link,
}

// ---------------------------------------------------------------------------
// Style IR
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct StyleDeclaration {
    pub property: StyleProperty,
    pub condition: Condition,
}

/// Universal Style Subset (proposal §6.3), Phase 0 scope only.
///
/// `display: flex` is not a variant here -- it's part of every `View`'s
/// shared base style (proposal §8.1), not a per-declaration property.
#[derive(Debug, Clone, PartialEq)]
pub enum StyleProperty {
    // Layout
    /// Originally left out on the grounds that `display: flex` is part of
    /// every `View`'s base style (proposal §8.1) rather than something a
    /// user sets. That reasoning didn't cover `hidden`, which is common and
    /// has a direct equivalent on both platforms.
    Display(Display),
    FlexDirection(FlexDirection),
    Flex(FlexShorthand),
    AlignItems(Align),
    AlignSelf(AlignSelf),
    /// Reuses `Justify` -- CSS's `align-content` takes the same keyword set
    /// as `justify-content`, and both platforms accept them there.
    AlignContent(Justify),
    JustifyContent(Justify),
    Gap(Length),
    RowGap(Length),
    ColumnGap(Length),
    // Margin/padding/inset are per-side longhand properties, not a single
    // EdgeInsets-bundling property: Tailwind utilities like `px-4`/`py-2`
    // set disjoint sides independently, and only per-side variants let
    // "last declaration for a property wins" flattening compose them
    // correctly instead of one clobbering the other.
    MarginTop(Dimension),
    MarginRight(Dimension),
    MarginBottom(Dimension),
    MarginLeft(Dimension),
    PaddingTop(Length),
    PaddingRight(Length),
    PaddingBottom(Length),
    PaddingLeft(Length),
    // Writing-direction-relative counterparts, kept as their own variants
    // rather than resolved to a physical side: which side "start" means
    // isn't known until runtime (the document's direction on Web,
    // `I18nManager.isRTL` on Native), so collapsing them here would bake in
    // LTR and silently break RTL layouts. Both platforms have real
    // equivalents to lower onto -- CSS `*-inline-start/end`, RN
    // `paddingStart`/`marginEnd`/etc.
    MarginInlineStart(Dimension),
    MarginInlineEnd(Dimension),
    PaddingInlineStart(Length),
    PaddingInlineEnd(Length),
    Width(Dimension),
    Height(Dimension),
    MinWidth(Dimension),
    MinHeight(Dimension),
    MaxWidth(Dimension),
    MaxHeight(Dimension),
    /// `None` is `z-auto`. One variant for both, not a separate `ZIndexAuto`
    /// or a `Keyword`: they are the same CSS property, and splitting them
    /// would stop them overriding each other.
    ZIndex(Option<i32>),
    /// `order-*`. Flex ordering, which Yoga doesn't implement -- React
    /// Native lays children out in tree order and has no way to say
    /// otherwise.
    Order(i32),
    /// `cursor-*`. Held as the CSS keyword rather than an enum: these are
    /// pass-through names with no structure to model, and thirty-odd
    /// variants would only restate the CSS spec. React Native accepts two
    /// of them (`auto`, `pointer`), which `unsupported_on_native` checks by
    /// name.
    Cursor(String),
    /// `columns-*`. CSS multi-column layout, which React Native has no
    /// equivalent for at all.
    Columns(ColumnCount),
    /// `rotate-none`/`scale-none`/`translate-none`/`transform-none`: the
    /// standalone transform properties turned off.
    ///
    /// Separate from the axes rather than a value of them, because each
    /// clears the whole property -- and separate from each other, because
    /// CSS has four properties here and turning off one leaves the rest.
    RotateNone,
    ScaleNone,
    TranslateNone,
    TransformNone,
    /// `transform`/`transform-cpu`: the composed `transform` with nothing
    /// in it. Tailwind writes the register chain here, which resolves to
    /// empty; the declaration exists so a later utility has something to
    /// override.
    TransformEmpty,
    /// `transform-gpu`: the same chain with a null 3D translation in
    /// front, the long-standing trick for forcing GPU compositing.
    TransformGpu,
    /// `line-clamp-*`. `None` is `line-clamp-none`.
    ///
    /// One property standing for four CSS declarations, composed at emit
    /// time like the ring and mask slots -- the `-webkit-box` display and
    /// the `overflow` it needs are part of the mechanism, not separate
    /// intentions, and routing them through `Display`/`Overflow` would
    /// make `line-clamp-2` silently fight an `overflow-visible` written
    /// beside it.
    ///
    /// React Native expresses the whole thing as `numberOfLines` on Text,
    /// which is why this is a property rather than a pile of keywords.
    LineClamp(Option<Clamp>),
    /// `grow`/`grow-0`/`shrink`/`shrink-0` and `aspect-*`. All three are
    /// real React Native styles, which is why they are properties rather
    /// than `Keyword`s -- the refusal audit caught them being refused as
    /// part of the keyword tail.
    FlexGrow(f64),
    FlexShrink(f64),
    AspectRatio(&'static str),
    /// `border-s/e/x/y/bs/be-*`: a border width and its style on a
    /// *logical* edge.
    ///
    /// Edge-keyed rather than one variant per edge, unlike the physical
    /// sides above -- `dedupe_key` already knows how to key on an `Edge`,
    /// and six more edges is twelve more variants for no gain. Restricted
    /// to the logical edges precisely so the two encodings can't describe
    /// the same declaration: `Edge::Top` here would be a second spelling
    /// of `BorderTopWidth` and the pair would stop overriding each other.
    BorderLogicalWidth(Edge, Length),
    BorderLogicalStyle(Edge, BorderStyle),
    /// `overflow-x-*` / `overflow-y-*`. Separate properties from
    /// `Overflow`, matching CSS -- React Native has only the combined one,
    /// so the per-axis forms are refused there.
    OverflowX(Overflow),
    OverflowY(Overflow),
    /// `object-*`. React Native has `objectFit` and takes the same five
    /// keywords, so the text is shared.
    ObjectFit(&'static str),
    /// `select-*`. Two declarations on Web (the -webkit- prefix is still
    /// load-bearing in Safari), one `userSelect` on React Native.
    UserSelect(&'static str),
    /// `underline`/`overline`/`line-through`/`no-underline`. React Native
    /// has `textDecorationLine` and takes underline, line-through and their
    /// combination; `overline` is Web-only.
    TextDecorationLine(&'static str),
    /// `underline-offset-*`. A length rather than a keyword, so it takes a
    /// real property: the negative forms have to go through `negated`,
    /// which has nothing sensible to do with a `Keyword`.
    TextUnderlineOffset(Dimension),
    /// `mix-blend-*` / `bg-blend-*`, as the CSS keyword. React Native has
    /// `mixBlendMode` and takes the same names, so the text is shared;
    /// `background-blend-mode` has no equivalent there.
    MixBlendMode(&'static str),
    BackgroundBlendMode(&'static str),
    /// A pair of declarations that are the same value under two property
    /// names -- `-webkit-hyphens` and `hyphens`, `-webkit-font-smoothing`
    /// and `-moz-osx-font-smoothing`. Held as one property because they
    /// are one intention, and because a `Keyword` can only carry one.
    KeywordPair(&'static str, &'static str, &'static str, &'static str),
    /// One CSS declaration with a fixed value, as `(property, value)`.
    ///
    /// The deliberate escape hatch for the long tail: `touch-action`,
    /// `break-after`, `contain`, `color-scheme` and eighty-odd others are
    /// each a closed list of keywords that Dowel neither computes nor
    /// reinterprets, and giving every one its own variant would be ninety
    /// enum arms restating the CSS spec.
    ///
    /// Strictly for properties nothing else models. A property that *does*
    /// have a variant must not also arrive here: `dedupe_key` tells two
    /// `Keyword`s apart by property name but cannot see that
    /// `Keyword("align-items", ..)` and `AlignItems(..)` are the same
    /// declaration, so the two spellings would stop overriding each other.
    /// `keyword_utility` in `dowel_parser` is where that rule is kept.
    Keyword(&'static str, &'static str),
    /// A declaration the source wrote out in full: `[color:red]`,
    /// `[mask-type:luminance]`, `[--my-var:4px]`.
    ///
    /// Where `Keyword` is a closed list Dowel curates, this is whatever
    /// CSS the author reached for -- so both halves are runtime strings and
    /// neither is checked against anything. That is the deal an arbitrary
    /// property makes: it leaves the design system, and Dowel's job shrinks
    /// to putting it through unchanged.
    ///
    /// Native refuses every one of these. A CSS property name means nothing
    /// to React Native's style system, and guessing a camelCase equivalent
    /// would turn "this doesn't exist on Native" into "this exists and does
    /// nothing".
    ///
    /// Two of these collapse against each other by property name, the same
    /// way `Keyword` does. Neither collapses against a *typed* variant for
    /// the same property -- `[color:red]` and `text-blue-500` both survive
    /// into the rule, and CSS's own last-wins settles it. Correct, but it
    /// does mean the emitted rule can carry a declaration Tailwind would
    /// have dropped.
    Arbitrary(String, String),
    /// Column count for `grid-cols-<n>`. Web-only: React Native's layout
    /// engine has no grid implementation at all.
    GridTemplateColumns(GridTracks),
    GridTemplateRows(GridTracks),
    /// `col-start-*`/`col-end-*`/`row-start-*`/`row-end-*`: one edge of a
    /// grid item's placement.
    GridColumnStart(GridLine),
    GridColumnEnd(GridLine),
    GridRowStart(GridLine),
    GridRowEnd(GridLine),
    /// `col-span-*`/`col-auto`/`col-span-full`: both edges at once, which
    /// CSS spells as the `grid-column` shorthand rather than as a pair.
    GridColumn(GridSpan),
    GridRow(GridSpan),
    Position(Position),
    /// `Dimension` rather than `Length`: Tailwind offers `top-1/2` and
    /// `inset-x-auto` alongside the spacing scale, so a plain pixel length
    /// cannot represent the family.
    InsetTop(Dimension),
    InsetRight(Dimension),
    InsetBottom(Dimension),
    InsetLeft(Dimension),
    InsetInlineStart(Dimension),
    InsetInlineEnd(Dimension),
    /// The axis shorthands (`inset-x-*`/`inset-y-*`) and the block-logical
    /// pair, kept as their own properties for the same discriminant reason
    /// as the per-side border colours below.
    InsetInline(Dimension),
    InsetBlock(Dimension),
    InsetBlockStart(Dimension),
    InsetBlockEnd(Dimension),

    // Visual
    BackgroundColor(Color),
    Opacity(f64),
    BorderColor(Color),
    /// Per-side border colours, one variant per CSS longhand.
    ///
    /// Separate variants rather than one `BorderSideColor(Side, Color)`
    /// because `dedupe_last_wins` identifies a property by its enum
    /// discriminant -- a single variant would make `border-t-red-500` and
    /// `border-b-blue-500` collapse into one. Same reason the per-side
    /// widths above are spelled out.
    ///
    /// `Inline`/`Block` are CSS shorthands (start+end of that axis) and are
    /// emitted as such on Web to match Tailwind exactly; the Native backend
    /// expands them, since React Native has no shorthand form.
    BorderTopColor(Color),
    BorderRightColor(Color),
    BorderBottomColor(Color),
    BorderLeftColor(Color),
    BorderInlineColor(Color),
    BorderBlockColor(Color),
    BorderInlineStartColor(Color),
    BorderInlineEndColor(Color),
    BorderBlockStartColor(Color),
    BorderBlockEndColor(Color),
    // Per-side, for the same reason margin/padding are (see above):
    // `border-t-2` and `border-b-4` set disjoint sides and must compose.
    BorderTopWidth(Length),
    BorderRightWidth(Length),
    BorderBottomWidth(Length),
    BorderLeftWidth(Length),
    /// Needed for border widths to render at all on Web: CSS defaults
    /// `border-style` to `none`, so a width alone shows nothing. Tailwind
    /// emits a style declaration alongside every border-width utility for
    /// exactly this reason, and Dowel has no preflight/reset of its own to
    /// lean on instead.
    ///
    /// Per-side, and that matters more than it looks: an all-sides
    /// `border-style: solid` makes the three sides *without* an explicit
    /// width fall back to `border-width`'s initial value (`medium`) and
    /// render, so `border-t-2` would draw a full box instead of one edge.
    /// React Native has no per-side border style; its backend collapses
    /// these into its single `borderStyle` (harmless there, since RN
    /// defaults every border width to 0 rather than `medium`).
    BorderTopStyle(BorderStyle),
    BorderRightStyle(BorderStyle),
    BorderBottomStyle(BorderStyle),
    BorderLeftStyle(BorderStyle),
    BorderRadius(Radius),
    /// Per-corner radii. Tailwind's side forms (`rounded-t-*`) and logical
    /// side forms (`rounded-s-*`) each expand to the two corners on that
    /// edge, which is what Tailwind itself emits -- there is no CSS
    /// shorthand for one edge's pair.
    BorderTopLeftRadius(Radius),
    BorderTopRightRadius(Radius),
    BorderBottomRightRadius(Radius),
    BorderBottomLeftRadius(Radius),
    BorderStartStartRadius(Radius),
    BorderStartEndRadius(Radius),
    BorderEndStartRadius(Radius),
    BorderEndEndRadius(Radius),

    /// CSS states these as standalone properties (`rotate: 45deg`), which
    /// is also how Tailwind v4 emits them. React Native has no standalone
    /// equivalents -- only a combined `transform` -- so the Native backend
    /// composes whichever of these are present into one entry, in CSS's
    /// defined application order (translate, then rotate, then scale).
    Rotate(Angle),

    /// Per-axis scale, as a *percentage* the way Tailwind writes it
    /// (`scale-110` -> 110).
    ///
    /// Held in the authored unit rather than as a ratio so the Web lowering
    /// is exact: converting to a ratio at parse and back at emit made
    /// `scale-110` come out as `110.00000000000001%`. React Native wants
    /// the ratio and divides once, where the same rounding is invisible --
    /// it takes a number, not a string.
    ///
    /// Bare `scale-50` sets all three axes rather than being a fourth
    /// property, which is what makes `scale-50 scale-x-75` resolve the way
    /// Tailwind does: `dedupe_last_wins` keys on the property, so the axes
    /// have to be separate properties to override one another.
    ScaleX(Scale),
    ScaleY(Scale),
    ScaleZ(Scale),
    /// Whether the `scale` declaration writes its third component.
    ///
    /// Not the same question as "is the z register set": `scale-50` sets all
    /// three registers and still writes only two components, while
    /// `scale-3d` writes three without setting anything. Tailwind decides
    /// this by which utility was written, so the marker has to be its own
    /// property rather than inferred from `ScaleZ` being present.
    Scale3d,
    /// The 3D rotations and the skews. Separate from `Rotate` because CSS
    /// puts them in `transform` rather than in the standalone `rotate`
    /// property, and they compose into one declaration the same way the
    /// translate axes do.
    RotateX(Angle),
    RotateY(Angle),
    RotateZ(Angle),
    SkewX(Angle),
    SkewY(Angle),
    /// CSS's `translate` is one property taking up to three values, so
    /// these compose into a single declaration rather than one each --
    /// before 2026-08-15 `translate-x-4 translate-y-8` emitted two
    /// `translate:` declarations and the x was lost to last-wins.
    TranslateX(Dimension),
    TranslateY(Dimension),
    TranslateZ(Dimension),
    /// Kept as the already-composed CSS value rather than a structured
    /// list. React Native accepts a string for `boxShadow`/`filter` too, so
    /// both backends emit the same text and there's nothing for a
    /// structured form to buy here.
    BoxShadow(String),
    /// Ring layers, kept apart from `BoxShadow` because they *compose* with
    /// it rather than replace it: `shadow-lg ring-2` renders both, and a
    /// single property would make the later one win under
    /// `dedupe_last_wins`.
    ///
    /// Width and colour are separate for the same reason -- `ring-2` and
    /// `ring-blue-500` are two utilities that must combine, which is
    /// exactly what Tailwind uses its `--tw-ring-*` registers for. Dowel
    /// resolves the composition at compile time instead, so no custom
    /// properties reach the output.
    RingWidth(Length),
    RingColor(Color),
    /// `inset-shadow-*`: an inner shadow, a fourth layer beside the two
    /// rings and the outer shadow. Held as its composed CSS text for the
    /// same reason `BoxShadow` is.
    InsetShadow(String),
    InsetRingWidth(Length),
    InsetRingColor(Color),
    /// `ring-offset-*`: a gap between the element and its ring, drawn as a
    /// fifth layer in the same `box-shadow` rather than as a property of
    /// the ring. It also widens the ring's own spread by the same amount,
    /// which is why the two are composed together in
    /// `dowel_web::css::box_shadow_value` rather than each emitting text.
    RingOffsetWidth(Length),
    RingOffsetColor(Color),
    /// `shadow-<colour>` / `inset-shadow-<colour>`: the colour every layer
    /// of the matching shadow is painted in.
    ///
    /// Separate from `BoxShadow` because they are separate utilities that
    /// must combine -- `shadow-lg shadow-blue-500` is one blue shadow, not
    /// a shadow and a colour. Which means the shadow's own text has to
    /// keep its layers' default colours *replaceable*, and it does: see
    /// `dowel_web::css::repaint_shadow`.
    ShadowColor(Color),
    InsetShadowColor(Color),
    /// One function of the `filter` chain, and its already-formatted CSS
    /// argument (`blur(12px)` is `Blur` + `"blur(12px)"`).
    ///
    /// Per-function rather than one string, for the same reason the ring
    /// and mask slots are: Tailwind builds `filter` out of eight registers
    /// in a fixed order, and holding the whole chain as one value would
    /// make `blur-md grayscale` last-wins instead of composing. The
    /// `FilterFunction` is also the dedupe key, so `blur-sm blur-lg`
    /// replaces rather than stacks.
    Filter(FilterFunction, String),
    /// The same chain applied to what's *behind* the element. A separate
    /// property, not a flag on `Filter`: an element can carry both, and
    /// they compose independently.
    BackdropFilter(FilterFunction, String),

    // Typography
    FontSize(Length),
    FontWeight(FontWeight),
    LineHeight(LineHeight),
    /// Letter spacing, always in `em` -- Tailwind's `tracking-*` scale is
    /// defined relative to the element's own font size. Web-only: React
    /// Native's `letterSpacing` is an absolute number, and the font size to
    /// resolve against isn't known at compile time.
    LetterSpacing(LetterSpacing),
    /// `overflow`/`text-overflow`/`white-space`, the three declarations
    /// `truncate` expands to.
    Overflow(Overflow),
    TextOverflow(TextOverflow),
    WhiteSpace(WhiteSpace),
    /// CSS transitions, kept as already-composed values. Web-only: React
    /// Native has no declarative transition in its StyleSheet -- animation
    /// there is imperative (Animated/Reanimated), which is a runtime
    /// dependency rather than a lowering.
    TransitionProperty(String),
    TransitionDuration(u32),
    TransitionTimingFunction(String),
    /// Web-only, same reason as the transition properties. Carries the
    /// named animation rather than its shorthand text so the backend can
    /// also emit the matching `@keyframes`, which the shorthand alone
    /// wouldn't tell it to do.
    Animation(Animation),
    /// The odd one out: this styles the element's *children*, not the
    /// element. Tailwind's `space-x-*`/`space-y-*` are defined that way --
    /// a gap between siblings applied as a margin on all but the last --
    /// and there's no way to express it as a declaration on the parent.
    /// The Web backend emits a child-scoped rule for it; React Native has
    /// no selector engine, so it's refused there.
    /// The `mask-*` utilities that are one CSS property set to one keyword.
    ///
    /// These carry the CSS keyword as a string rather than getting an enum
    /// each, which is the opposite of how the rest of this file works. The
    /// reason is that nothing ever *reads* the value: masks don't exist in
    /// React Native at all, so there is no second lowering to map onto and
    /// no value transformation to perform. A typed enum here would be eight
    /// enums whose only use is to be turned straight back into the string
    /// they came from.
    ///
    /// One variant per CSS property, though, so `dedupe_last_wins` still
    /// resolves `mask-clip-border mask-clip-content` correctly.
    /// `bg-none`: no background image.
    ///
    /// A property rather than a `Keyword("background-image", "none")`,
    /// because the gradient utilities now write that same property from
    /// their own variant -- and a `Keyword` sharing a property with a
    /// variant is exactly the collision that stopped `z-auto` and `z-10`
    /// overriding each other. `dedupe_last_wins` keys on the variant, so
    /// the two have to be the same one to resolve.
    BackgroundImageNone,
    /// A background gradient: which function, and everything before the
    /// stops.
    ///
    /// The prelude is one string rather than a direction plus an
    /// interpolation space, because Tailwind writes it as one
    /// (`--tw-gradient-position: to right in oklab`) and one utility sets
    /// all of it -- the space is a *modifier* on the same class,
    /// `bg-linear-to-r/srgb`. Splitting it would be two properties that
    /// can never be written apart.
    ///
    /// Paints nothing without stops, exactly as in Tailwind: this is the
    /// gradient's shape and `GradientStopColor` supplies its colours.
    Gradient(GradientKind, String),
    /// `from-*` / `via-*` / `to-*`. Colour and position are separate
    /// properties for the same reason ring width and colour are: they are
    /// separate utilities that must combine, and `from-red-500 from-20%`
    /// sets both halves of one stop.
    GradientStopColor(GradientStop, Color),
    GradientStopPosition(GradientStop, Dimension),
    MaskClip(&'static str),
    MaskOrigin(&'static str),
    MaskMode(&'static str),
    MaskType(&'static str),
    MaskSize(&'static str),
    MaskPosition(&'static str),
    MaskRepeat(&'static str),
    MaskImageNone,
    /// The gradient half of `mask-*`.
    ///
    /// Tailwind builds these from a fixed list of `--tw-mask-*` registers
    /// spliced into one `mask-image`, with every unset slot defaulting to an
    /// opaque `linear-gradient(#fff, #fff)` that the `intersect` composite
    /// then ignores. Dowel knows the whole set at compile time, so
    /// `dowel_web::css::mask_declarations` resolves the list directly.
    ///
    /// Each utility contributes one piece, which is why these are so
    /// granular: `mask-t-from-red-500 mask-t-to-80%` are two utilities that
    /// must combine into one gradient.
    MaskStopColor(MaskSlot, MaskStop, Color),
    MaskStopPosition(MaskSlot, MaskStop, Dimension),
    /// The argument a mask gradient slot carries on its own: an angle for
    /// `mask-linear-*` and `mask-conic-*`, a size for `mask-radial-[…]`.
    ///
    /// One variant for all three because they occupy the same position in
    /// Tailwind's output -- the fallback inside
    /// `var(--tw-mask-<slot>-stops, …)`, which is what the gradient becomes
    /// when no stop utility was written. `Angle` carries it because that is
    /// what it is for two of the three, and because the arbitrary case is
    /// unresolvable text in any of them.
    ///
    /// Only the *arbitrary* radial size lands here. `mask-radial-closest-side`
    /// is `MaskRadialSize` instead and paints nothing alone, which is
    /// Tailwind's own split: it emits no rule at all for the named form and
    /// a whole gradient for the bracketed one.
    MaskSlotArgument(MaskSlot, Angle),
    /// `mask-circle`/`mask-ellipse`, `mask-radial-closest-side`, and
    /// `mask-radial-at-*`. Each paints nothing alone -- they only shape a
    /// radial gradient some other utility supplies.
    MaskRadialShape(&'static str),
    MaskRadialSize(String),
    MaskRadialPosition(&'static str),
    MaskComposite(&'static str),
    ScrollbarWidth(&'static str),
    ScrollbarGutter(&'static str),
    /// `scrollbar-thumb-*` / `scrollbar-track-*`. Two utilities that write
    /// one `scrollbar-color: <thumb> <track>`, so they compose in the
    /// backend the way ring and mask layers do. An unset half is
    /// transparent, matching Tailwind's register default.
    ScrollbarThumbColor(Color),
    ScrollbarTrackColor(Color),
    /// The remaining one-property spacing families. Nothing subtle in any
    /// of them -- they were simply unimplemented.
    FlexBasis(Dimension),
    BlockSize(Dimension),
    InlineSize(Dimension),
    MaxBlockSize(Dimension),
    MaxInlineSize(Dimension),
    MinBlockSize(Dimension),
    MinInlineSize(Dimension),
    TextIndent(Dimension),
    /// `border-spacing` is one property taking a horizontal and a vertical
    /// value, so `border-spacing-x-*` and `-y-*` compose into it.
    BorderSpacingX(Dimension),
    BorderSpacingY(Dimension),
    /// Block-axis logical margins and paddings. Emitted as the logical CSS
    /// longhands on Web to match Tailwind exactly; the Native backend folds
    /// them to top/bottom, which is the horizontal-writing-mode assumption
    /// it already makes for `py-*`.
    MarginBlockStart(Dimension),
    MarginBlockEnd(Dimension),
    PaddingBlockStart(Length),
    PaddingBlockEnd(Length),
    /// `scroll-m-*` / `scroll-p-*`.
    ///
    /// These carry their edge rather than getting a variant each, unlike
    /// the per-side padding/margin/border-colour properties above: there
    /// are eleven edges and two families, so spelling them out would be 22
    /// variants and 66 match arms for one fairly niche corner of CSS.
    /// `dedupe_key` is what makes that safe -- see its doc comment.
    ScrollMargin(Edge, Length),
    ScrollPadding(Edge, Length),
    /// `scroll-smooth`/`scroll-auto`. One variant covering both values, not
    /// a `Smooth` marker: the two are the same CSS property, and splitting
    /// them across a variant and a `Keyword` would stop them overriding
    /// each other -- see `StyleProperty::Keyword`.
    ScrollBehavior(&'static str),
    /// SVG paint, plus the handful of colour properties that are neither
    /// text nor background. All plain declarations -- the work here was
    /// recognising the utilities, not lowering them.
    Fill(Color),
    Stroke(Color),
    StrokeWidth(f64),
    AccentColor(Color),
    CaretColor(Color),
    TextDecorationColor(Color),
    TextDecorationStyle(DecorationStyle),
    TextDecorationThickness(Length),
    /// `placeholder-*`. Scoped to the `::placeholder` pseudo-element, not
    /// the element itself -- so it gets its own rule, the same way
    /// `divide-*`/`space-*` do. Emitting it as a plain `color` would tint
    /// the real text instead, and would still *compare* equal to Tailwind,
    /// since the difference lives in the selector rather than the
    /// declaration.
    PlaceholderColor(Color),
    OutlineWidth(Length),
    OutlineStyle(BorderStyle),
    OutlineColor(Color),
    OutlineOffset(Length),
    /// `divide-*`: like `space-*`, these style the element's *children*
    /// rather than the element itself. Web emits a second rule with a
    /// child-scoped selector; Native has no selector engine and instead
    /// hands the style to `DowelSpaced`, which distributes it over the
    /// children at render time. Both target every child but the last.
    DivideX(Dimension),
    DivideY(Dimension),
    DivideColor(Color),
    DivideStyle(BorderStyle),
    SpaceX(Dimension),
    SpaceY(Dimension),
    TextAlign(TextAlign),
    TextTransform(TextTransform),
    TextColor(Color),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexDirection {
    Row,
    Column,
    RowReverse,
    ColumnReverse,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FlexShorthand {
    Initial,
    Auto,
    None,
    Grow(f64),
    /// `flex-1/2`. Kept as the authored fraction, not folded to a
    /// percentage: Tailwind emits `calc(1/2 * 100%)` and the division is
    /// what keeps thirds exact rather than 33.33333333333333%.
    Fraction(u32, u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
    Stretch,
    Baseline,
    /// The values React Native's alignment unions don't have: `normal`,
    /// and the `safe` overflow-alignment forms.
    Css(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Justify {
    Start,
    Center,
    End,
    Between,
    Around,
    Evenly,
    /// `align-content` takes these and `justify-content` doesn't, on both
    /// platforms -- the two share this enum, so which one is legal is
    /// decided per property in `unsupported_on_native`.
    Stretch,
    Baseline,
    /// See `Align::Css`.
    Css(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    Relative,
    Absolute,
    /// React Native has this too, and means what CSS means by it: the
    /// element is not a containing block for absolutely positioned
    /// descendants. Refusing it was wrong, and the refusal audit said so.
    Static,
    /// `fixed` and `sticky`, which React Native has no equivalent for.
    Css(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
    Flex,
    None,
    Contents,
    // A visible Yoga node is necessarily a flex container, so CSS block
    // lowers to display:flex. The remaining modes require inline or grid
    // formatting contexts Yoga does not implement.
    Block,
    InlineFlex,
    Grid,
    /// The rest of CSS's display keywords, as authored. Grouped rather than
    /// enumerated because Yoga implements none of them, so the only thing
    /// the Native backend does with any of them is name it in a refusal --
    /// there is no per-keyword behaviour to model.
    Css(&'static str),
}

impl Display {
    /// Whether React Native can express this at all -- see the variants'
    /// own note.
    pub fn is_supported_on_native(self) -> bool {
        matches!(self, Display::Flex | Display::Block | Display::None | Display::Contents)
    }
}

impl StyleProperty {
    /// `Some(description)` when React Native has no way to express this, so
    /// the Native backend can refuse it by name instead of dropping it. Kept
    /// here rather than in that backend so every such case is listed in one
    /// place as more are found.
    /// `Some(reason)` when React Native *could* express this and Dowel
    /// doesn't lower it yet -- `DiagnosticCode::NotWiredOnNative` rather
    /// than `WebOnlyPropertyOnNative`.
    ///
    /// Checked before `unsupported_on_native`, and separate from it because
    /// the two make different claims. Conflating them is not harmless: the
    /// refusal audit checks the Web-only claims against React Native's own
    /// types, and a gap filed as impossible is a gap nobody revisits.
    ///
    /// Only the cases decidable from the property alone live here. A gap
    /// that depends on the rest of the element -- a `leading-*` ratio with
    /// no font size beside it -- is decided in `dowel_native`, which can see
    /// the node.
    pub fn not_wired_on_native(&self) -> Option<String> {
        match self {
            // `placeholder-*` used to live here. It lowers now: React Native
            // carries that colour as `placeholderTextColor` on `TextInput`,
            // and `TextInput` exists. What remains is the case where it is
            // written on something that isn't one, which `dowel_native`
            // decides because it can see the node.
            _ => None,
        }
    }

    /// The `Dimension` this property carries, if it carries one.
    ///
    /// Exists so `unsupported_on_native` can ask about a *value* once
    /// instead of per property. Every size utility takes the same value
    /// vocabulary, so a value React Native can't hold (`fit-content`,
    /// `100dvh`) is unusable on all of them -- and listing the properties
    /// one at a time is how the last batch of those came to be dropped in
    /// silence rather than refused.
    pub fn dimension(&self) -> Option<&Dimension> {
        match self {
            StyleProperty::Width(d)
            | StyleProperty::Height(d)
            | StyleProperty::MinWidth(d)
            | StyleProperty::MinHeight(d)
            | StyleProperty::MaxWidth(d)
            | StyleProperty::MaxHeight(d)
            | StyleProperty::BlockSize(d)
            | StyleProperty::InlineSize(d)
            | StyleProperty::MinBlockSize(d)
            | StyleProperty::MinInlineSize(d)
            | StyleProperty::MaxBlockSize(d)
            | StyleProperty::MaxInlineSize(d)
            | StyleProperty::FlexBasis(d)
            | StyleProperty::TextIndent(d)
            | StyleProperty::TextUnderlineOffset(d)
            | StyleProperty::TranslateX(d)
            | StyleProperty::TranslateY(d) => Some(d),
            _ => None,
        }
    }

    pub fn unsupported_on_native(&self) -> Option<String> {
        // Asked before the property-by-property match: a value React Native
        // has no way to hold is unusable whichever size carries it.
        if let Some(Dimension::Css(value)) = self.dimension() {
            return Some(format!(
                "`{value}`: React Native has no intrinsic sizing and no viewport unit that tracks \
                 browser chrome -- there is nothing to resolve it against"
            ));
        }
        // Viewport-relative sizes were refused here until 2026-08-15. They
        // are not impossible on React Native, only impossible *statically*:
        // the value changes when the device rotates, so it can't live in a
        // `StyleSheet.create` object. The Native backend lowers them to an
        // inline style read from a hook -- see
        // `dowel_native::viewport_object`.
        match self {
            StyleProperty::Display(d) if !d.is_supported_on_native() => Some(format!(
                "`display: {}`: React Native's layout engine supports only flex, none and contents",
                match d {
                    Display::Block => "block",
                    Display::InlineFlex => "inline-flex",
                    Display::Grid => "grid",
                    Display::Css(keyword) => keyword,
                    _ => unreachable!("guarded by is_supported_on_native"),
                }
            )),
            StyleProperty::GridTemplateColumns(_)
            | StyleProperty::GridTemplateRows(_)
            | StyleProperty::GridColumnStart(_)
            | StyleProperty::GridColumnEnd(_)
            | StyleProperty::GridRowStart(_)
            | StyleProperty::GridRowEnd(_)
            | StyleProperty::GridColumn(_)
            | StyleProperty::GridRow(_) => {
                Some("grid placement: React Native has no grid layout".to_string())
            }
            StyleProperty::Order(_) => Some(
                "`order-*`: Yoga lays children out in tree order and has no flex `order`, so the \
                 only way to reorder on React Native is to reorder the elements"
                    .to_string(),
            ),
            StyleProperty::Position(Position::Css(value)) => Some(format!(
                "`position: {value}`: React Native's position is relative or absolute"
            )),
            StyleProperty::RotateNone
            | StyleProperty::ScaleNone
            | StyleProperty::TranslateNone
            | StyleProperty::TransformNone
            | StyleProperty::TransformEmpty
            | StyleProperty::TransformGpu => Some(
                "`rotate-none`/`scale-none`/`translate-none`/`transform-none`: React Native builds 
                 one transform array, so there is no property to switch off -- omit the transform 
                 instead"
                    .to_string(),
            ),
            StyleProperty::Flex(FlexShorthand::Fraction(n, d)) => Some(format!(
                "`flex-{n}/{d}`: React Native's `flex` is a grow factor, not a fraction of the 
                 parent -- use a percentage width instead"
            )),
            // Scale3d is metadata selecting the three-axis form. Native
            // lowers its Z component through RN's supported 4x4 matrix.
            StyleProperty::Scale3d => None,
            // The arbitrary transform cases. React Native's transform
            // array takes numbers -- a rotation is a number of degrees and
            // a scale is a ratio -- so a value that stayed CSS text has
            // nothing to become. Named here rather than left out of the
            // composed `transform`, which is where it would otherwise
            // vanish without a word.
            StyleProperty::Rotate(Angle::Css(value))
            | StyleProperty::RotateX(Angle::Css(value))
            | StyleProperty::RotateY(Angle::Css(value))
            | StyleProperty::RotateZ(Angle::Css(value))
            | StyleProperty::SkewX(Angle::Css(value))
            | StyleProperty::SkewY(Angle::Css(value)) => Some(format!(
                "`[{value}]`: React Native's transform takes a number of degrees, and this is CSS \
                 text that only a browser can resolve to one"
            )),
            StyleProperty::ScaleX(Scale::Css(value))
            | StyleProperty::ScaleY(Scale::Css(value))
            | StyleProperty::ScaleZ(Scale::Css(value)) => {
                Some(format!(
                    "`[{value}]`: React Native's scale is a ratio, and this is CSS text that only \
                     a browser can resolve to one"
                ))
            }
            StyleProperty::LineClamp(Some(Clamp::Css(value))) => Some(format!(
                "`line-clamp-[{value}]`: React Native's numberOfLines is a line count, and this \
                 isn't one"
            )),
            // React Native's `backgroundImage` parses CSS gradient syntax,
            // but only the linear and radial functions -- `BackgroundImageValue`
            // is `LinearGradientValue | RadialGradientValue` and there is no
            // conic one.
            StyleProperty::Gradient(GradientKind::Conic, _) => Some(
                "`bg-conic-*`: React Native's backgroundImage has linear and radial gradients \
                 and no conic one"
                    .to_string(),
            ),
            StyleProperty::ZIndex(None) => Some(
                "`z-auto`: React Native's zIndex is a number and has no auto".to_string(),
            ),
            StyleProperty::AspectRatio("auto") => Some(
                "`aspect-auto`: React Native's aspectRatio is a number and has no auto".to_string(),
            ),
            StyleProperty::Columns(_) => {
                Some("`columns-*`: React Native has no multi-column layout".to_string())
            }
            // A CSS value React Native's union for that property doesn't
            // have. Each one is a genuine keyword there is no equivalent
            // for -- `overflow: clip`, `justify-content: safe center` --
            // rather than a spelling difference, so approximating would
            // change the layout rather than the text.
            StyleProperty::Overflow(Overflow::Css(value)) => Some(format!(
                "`overflow: {value}`: React Native's overflow is visible, hidden or scroll"
            )),
            StyleProperty::AlignItems(Align::Css(value))
            | StyleProperty::AlignContent(Justify::Css(value))
            | StyleProperty::JustifyContent(Justify::Css(value)) => Some(format!(
                "`{value}` alignment: React Native's alignment values don't include it"
            )),
            // RN's alignContent has stretch but not baseline; its
            // justifyContent has neither. The two share `Justify`, so the
            // legality is per property rather than per value.
            StyleProperty::AlignContent(Justify::Baseline) => Some(
                "`content-baseline`: React Native's alignContent has no baseline".to_string(),
            ),
            StyleProperty::JustifyContent(Justify::Stretch | Justify::Baseline) => Some(
                "`justify-stretch`/`justify-baseline`: React Native's justifyContent has neither"
                    .to_string(),
            ),
            StyleProperty::AlignSelf(AlignSelf::Css(value)) => Some(format!(
                "`self-{value}`: React Native's alignSelf values don't include it"
            )),
            StyleProperty::WhiteSpace(WhiteSpace::Css(value)) => Some(format!(
                "`white-space: {value}`: React Native's Text has no white-space control beyond \
                 wrapping"
            )),
            // React Native has only the combined `overflow`.
            StyleProperty::OverflowX(_) | StyleProperty::OverflowY(_) => Some(
                "`overflow-x-*`/`overflow-y-*`: React Native has one `overflow` for both axes"
                    .to_string(),
            ),
            StyleProperty::TextDecorationLine("overline") => Some(
                "`overline`: React Native's textDecorationLine has underline and line-through, \
                 not overline"
                    .to_string(),
            ),
            // The long tail. React Native has three of these properties and
            // none of the rest -- checked against its own StyleSheet types
            // rather than assumed, the same way the refusal audit checks
            // everything else here.
            StyleProperty::KeywordPair(_, _, property, _) => {
                Some(format!("`{property}`: React Native has no such style"))
            }
            // Two keyword properties React Native has but narrower than CSS.
            // Found by type-checking the emitted styles against RN rather
            // than by reading its docs -- the allowlist above only asked
            // whether the *property* exists.
            StyleProperty::Keyword("vertical-align", value)
                if !matches!(*value, "auto" | "top" | "bottom" | "middle") =>
            {
                Some(format!(
                    "`align-{value}`: React Native's verticalAlign is auto, top, bottom or middle"
                ))
            }
            StyleProperty::MixBlendMode("plus-darker") => Some(
                "`mix-blend-plus-darker`: React Native's blend modes have plus-lighter and not \n                 plus-darker"
                    .to_string(),
            ),
            StyleProperty::Keyword(property, _)
                if !matches!(*property, 
                    "user-select"
                        | "vertical-align"
                        | "transform-origin"
                        | "backface-visibility"
                        | "box-sizing"
                        | "isolation"
                        | "pointer-events"
                        | "font-style"
                        | "font-family"
                        | "flex-wrap"
                ) =>
            {
                Some(format!("`{property}`: React Native has no such style"))
            }
            StyleProperty::BackgroundBlendMode(_) => Some(
                "`bg-blend-*`: React Native has `mixBlendMode` but no background-blend-mode -- \n                 there is no separate background layer there to blend against"
                    .to_string(),
            ),
            // `filter` is real on React Native; `backdrop-filter` is not --
            // there is no such style key, and blurring what is *behind* a
            // view needs a native blur component (`@react-native-community/
            // blur`, Expo's BlurView) rather than a style.
            StyleProperty::BackdropFilter(..) => Some(
                "`backdrop-*`: React Native has no backdrop-filter style -- blurring what's \
                 behind a view needs a native blur component"
                    .to_string(),
            ),
            // React Native's `cursor` is real but narrow: `auto` and
            // `pointer` only, which is what a pointer-capable target
            // (tablet with a trackpad, macOS/Windows/visionOS) can act on.
            // The rest are refused by name rather than silently flattened
            // to `pointer`, which would claim an affordance the app doesn't
            // have.
            StyleProperty::Cursor(keyword) if keyword != "auto" && keyword != "pointer" => {
                Some(format!(
                    "`cursor-{keyword}`: React Native's cursor accepts only `auto` and `pointer`"
                ))
            }
            // `letter-spacing` in em and a unitless `line-height` are
            // deliberately absent here even though React Native has neither
            // form as a style. Both are relative to the font size, and when
            // a font size is set on the same element the compiler can
            // resolve them -- so whether they're expressible depends on the
            // node, which this method can't see. `dowel_native`'s
            // `font_relative_reason` answers it for the cases it can't fold.
            // `text-overflow` and `white-space: nowrap` are deliberately
            // absent here even though React Native has neither as a style.
            // Together they describe truncation, which RN expresses as
            // `numberOfLines`/`ellipsizeMode` *props* on `Text` -- so
            // whether they're supportable depends on the node they're on,
            // which this function can't see. `dowel_native` decides,
            // absorbing them into props where it can and refusing them
            // where it can't.
            StyleProperty::TransitionProperty(_)
            | StyleProperty::TransitionDuration(_)
            | StyleProperty::TransitionTimingFunction(_) => Some(
                "CSS transitions: React Native has no declarative transition in its StyleSheet"
                    .to_string(),
            ),
            StyleProperty::Animation(_) => Some(
                "CSS animations: React Native animates imperatively (Animated/Reanimated), which \
                 is a runtime dependency rather than a lowering"
                    .to_string(),
            ),
            StyleProperty::Fill(_) | StyleProperty::Stroke(_) | StyleProperty::StrokeWidth(_) => {
                Some(
                    "SVG paint: React Native has no SVG in core -- `react-native-svg` is a \
                     separate dependency with its own props, not a style Dowel can lower to"
                        .to_string(),
                )
            }
            StyleProperty::AccentColor(_) => Some(
                "`accent-*`: it tints native form controls, which React Native doesn't have"
                    .to_string(),
            ),
            StyleProperty::CaretColor(_) => Some(
                "`caret-*`: React Native carries this on `TextInput`'s `cursorColor` prop, not \
                 in a StyleSheet; a runtime-resolved class has no target primitive to put it on"
                    .to_string(),
            ),
            StyleProperty::MaskClip(_)
            | StyleProperty::MaskOrigin(_)
            | StyleProperty::MaskMode(_)
            | StyleProperty::MaskType(_)
            | StyleProperty::MaskSize(_)
            | StyleProperty::MaskPosition(_)
            | StyleProperty::MaskRepeat(_)
            | StyleProperty::MaskImageNone
            | StyleProperty::MaskStopColor(..)
            | StyleProperty::MaskStopPosition(..)
            | StyleProperty::MaskSlotArgument(..)
            | StyleProperty::MaskRadialShape(_)
            | StyleProperty::MaskRadialSize(_)
            | StyleProperty::MaskRadialPosition(_)
            | StyleProperty::MaskComposite(_) => Some(
                "`mask-*`: React Native has no masking of any kind -- no mask-image, no \
                 mask-clip, nothing to approximate it with"
                    .to_string(),
            ),
            StyleProperty::TranslateZ(_) => Some(
                "`translate-z-*`: React Native's transform has no 3D translation".to_string(),
            ),
            StyleProperty::TextIndent(_) => {
                Some("`indent-*`: React Native has no text-indent".to_string())
            }
            StyleProperty::BorderSpacingX(_) | StyleProperty::BorderSpacingY(_) => Some(
                "`border-spacing-*`: it applies to a separated-border table, which React Native \
                 has no equivalent of"
                    .to_string(),
            ),
            StyleProperty::ScrollbarWidth(_)
            | StyleProperty::ScrollbarGutter(_)
            | StyleProperty::ScrollbarThumbColor(_)
            | StyleProperty::ScrollbarTrackColor(_) => Some(
                "`scrollbar-*`: React Native's scroll indicators are configured with props on \
                 ScrollView (`indicatorStyle`, `showsVerticalScrollIndicator`), not styled"
                    .to_string(),
            ),
            StyleProperty::ScrollMargin(..)
            | StyleProperty::ScrollPadding(..)
            | StyleProperty::ScrollBehavior(_) => Some(
                "`scroll-m-*`/`scroll-p-*`/`scroll-smooth`: these tune CSS scroll-snap and \
                 smooth scrolling, neither of which React Native's ScrollView exposes as a style"
                    .to_string(),
            ),
            StyleProperty::TextDecorationThickness(_) | StyleProperty::TextUnderlineOffset(_) => {
                Some(
                    "`decoration-<n>`/`underline-offset-*`: React Native draws its text \
                     decorations at a fixed thickness and offset, and exposes neither"
                        .to_string(),
                )
            }
            StyleProperty::BorderTopStyle(BorderStyle::Double | BorderStyle::Hidden)
            | StyleProperty::BorderRightStyle(BorderStyle::Double | BorderStyle::Hidden)
            | StyleProperty::BorderBottomStyle(BorderStyle::Double | BorderStyle::Hidden)
            | StyleProperty::BorderLeftStyle(BorderStyle::Double | BorderStyle::Hidden)
            | StyleProperty::OutlineStyle(BorderStyle::Double | BorderStyle::Hidden) => Some(
                "`double`/`hidden` border styles: React Native accepts only solid, dotted and \
                 dashed"
                    .to_string(),
            ),
            StyleProperty::DivideStyle(BorderStyle::Double | BorderStyle::Hidden) => Some(
                "`divide-double`/`divide-hidden`: React Native's borderStyle accepts only solid, \
                 dotted and dashed"
                    .to_string(),
            ),
            _ => None,
        }
    }
}

/// `text-decoration-style`. Its own type rather than `BorderStyle`: the two
/// sets only look alike. Decorations add `double` and `wavy` and have no
/// `none` (that's `text-decoration-line`), and React Native accepts a
/// different subset again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecorationStyle {
    Solid,
    Double,
    Dotted,
    Dashed,
    Wavy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderStyle {
    Solid,
    Dashed,
    Dotted,
    /// Real CSS styles React Native has no counterpart for -- its
    /// `borderStyle` accepts solid/dotted/dashed only, so both are refused
    /// there by `unsupported_on_native`.
    Double,
    Hidden,
    None,
}

/// Corner radius. `Full` ("pill shape", Tailwind's `rounded-full`) is its
/// own variant rather than a large `Length`, because it's a distinct
/// intent and the platforms express it differently: CSS has a literal
/// `infinity`, React Native does not and needs a finite stand-in. Baking
/// the finite value into the IR would force the Web backend to emit an
/// approximation of something it can state exactly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Radius {
    Length(Length),
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Length {
    Px(f64),
    /// A count of Tailwind spacing steps: `p-4` is `Spacing(4.0)`.
    ///
    /// Held as the step count rather than the pixels it works out to,
    /// because the step is what the source said and the multiplier is the
    /// project's (`--spacing`, 0.25rem by default). Resolving at parse time
    /// baked the default in, so a project that changed it got the right
    /// number of steps at the wrong size -- silently, since the output was
    /// a perfectly ordinary padding.
    ///
    /// The same reasoning `Color` already followed: keep what was written,
    /// resolve where the theme is.
    Spacing(f64),
    /// A length written with an explicit unit in an arbitrary value:
    /// `w-[2rem]`, `p-[3ch]`, `mt-[1.5em]`.
    ///
    /// The unit is kept rather than converted because Web has to match
    /// Tailwind byte for byte -- `w-[2rem]` is `width: 2rem` there, not
    /// `width: 32px`, and the difference is visible the moment a user
    /// changes their browser's font size. Native converts what it can and
    /// refuses the rest by name; see `LengthUnit::px`.
    Unit(f64, LengthUnit),
}

/// The CSS length units an arbitrary value may carry.
///
/// A closed set rather than a kept string, so that every consumer is a
/// `match` the compiler checks. The alternative -- holding the unit as
/// text -- pushes the decision to whoever formats the value, and the
/// decision that matters (can React Native express this?) would then be
/// made by string comparison in the one place least able to explain
/// itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LengthUnit {
    /// Root-relative. Converted at 16px on Native, the same root size
    /// `@dowel/tailwind` already assumes when it reads `--spacing` out of
    /// a project's `@theme`. Two different answers for one unit inside one
    /// compiler would be worse than one documented assumption.
    Rem,
    /// Font-relative to the element. Folded against a known `font-size` on
    /// Native and refused where there isn't one.
    Em,
    /// Physically fixed ratios to the pixel, defined by CSS itself
    /// (1in = 96px, 1pc = 16px, 1pt = 96/72px, and the metric ones from
    /// 1in = 2.54cm). Converting these is exact arithmetic, not an
    /// assumption, so Native takes them.
    Cm,
    Mm,
    Q,
    In,
    Pt,
    Pc,
    /// Viewport-relative. Web resolves them; Native can, but only through
    /// the runtime viewport hook, so they're handled where that hook is.
    Vw,
    Vh,
    Vmin,
    Vmax,
    /// Resolved from font metrics the compiler doesn't have -- the width of
    /// a `0`, the height of an `x`, the computed line box. Only a browser
    /// knows these, so Native refuses them.
    Ch,
    Ex,
    Cap,
    Ic,
    Lh,
    Rlh,
    /// The viewport units that track browser chrome as it scrolls away.
    /// There is no such chrome on a device, and no static value that would
    /// be right, so these are Web-only.
    Dvh,
    Dvw,
    Lvh,
    Lvw,
    Svh,
    Svw,
}

impl LengthUnit {
    /// The unit as CSS spells it.
    pub fn css(self) -> &'static str {
        match self {
            LengthUnit::Rem => "rem",
            LengthUnit::Em => "em",
            LengthUnit::Cm => "cm",
            LengthUnit::Mm => "mm",
            LengthUnit::Q => "q",
            LengthUnit::In => "in",
            LengthUnit::Pt => "pt",
            LengthUnit::Pc => "pc",
            LengthUnit::Vw => "vw",
            LengthUnit::Vh => "vh",
            LengthUnit::Vmin => "vmin",
            LengthUnit::Vmax => "vmax",
            LengthUnit::Ch => "ch",
            LengthUnit::Ex => "ex",
            LengthUnit::Cap => "cap",
            LengthUnit::Ic => "ic",
            LengthUnit::Lh => "lh",
            LengthUnit::Rlh => "rlh",
            LengthUnit::Dvh => "dvh",
            LengthUnit::Dvw => "dvw",
            LengthUnit::Lvh => "lvh",
            LengthUnit::Lvw => "lvw",
            LengthUnit::Svh => "svh",
            LengthUnit::Svw => "svw",
        }
    }

    /// The unit this text names, or `None` if it isn't a CSS length unit.
    pub fn parse(text: &str) -> Option<LengthUnit> {
        Some(match text.to_ascii_lowercase().as_str() {
            "rem" => LengthUnit::Rem,
            "em" => LengthUnit::Em,
            "cm" => LengthUnit::Cm,
            "mm" => LengthUnit::Mm,
            "q" => LengthUnit::Q,
            "in" => LengthUnit::In,
            "pt" => LengthUnit::Pt,
            "pc" => LengthUnit::Pc,
            "vw" => LengthUnit::Vw,
            "vh" => LengthUnit::Vh,
            "vmin" => LengthUnit::Vmin,
            "vmax" => LengthUnit::Vmax,
            "ch" => LengthUnit::Ch,
            "ex" => LengthUnit::Ex,
            "cap" => LengthUnit::Cap,
            "ic" => LengthUnit::Ic,
            "lh" => LengthUnit::Lh,
            "rlh" => LengthUnit::Rlh,
            "dvh" => LengthUnit::Dvh,
            "dvw" => LengthUnit::Dvw,
            "lvh" => LengthUnit::Lvh,
            "lvw" => LengthUnit::Lvw,
            "svh" => LengthUnit::Svh,
            "svw" => LengthUnit::Svw,
            _ => return None,
        })
    }

    /// How many pixels one of this unit is, where that is knowable without
    /// asking a browser.
    ///
    /// `None` means the unit needs something the compiler doesn't have --
    /// the element's font metrics, or the size of a screen it can't see.
    /// Callers turn that into a refusal that names the unit rather than a
    /// number that would be wrong.
    pub fn px(self) -> Option<f64> {
        Some(match self {
            LengthUnit::Rem => ROOT_FONT_SIZE_PX,
            LengthUnit::In => 96.0,
            LengthUnit::Pc => 16.0,
            LengthUnit::Pt => 96.0 / 72.0,
            LengthUnit::Cm => 96.0 / 2.54,
            LengthUnit::Mm => 96.0 / 25.4,
            LengthUnit::Q => 96.0 / 101.6,
            _ => return None,
        })
    }
}

/// The root font size Dowel resolves `rem` against.
///
/// Browsers default to this and every major one lets a user change it, so
/// Web never converts -- it prints `rem` and lets the browser decide. This
/// constant exists for Native, which has no such concept and no browser to
/// ask.
pub const ROOT_FONT_SIZE_PX: f64 = 16.0;

fn round(value: f64) -> f64 {
    (value * 1e6).round() / 1e6
}

impl Length {
    /// The pixel value, against a project's spacing scale.
    ///
    /// `Px` is already absolute -- a `border-2` is two pixels whatever the
    /// spacing scale is, which is exactly why the two are different
    /// variants rather than one number with a flag.
    ///
    /// A `Unit` this can't convert reports zero, which is why no backend
    /// should reach here without having asked `resolvable` first. Web never
    /// does (it prints the unit); Native refuses first.
    pub fn px(self, theme: &Theme) -> f64 {
        match self {
            Length::Px(value) => value,
            // Rounded, because the product is written into the output as a
            // literal. Tailwind emits `calc(var(--spacing) * 3)` and lets
            // the browser do the arithmetic; Dowel resolves it, so binary
            // floating point surfaces as `9.600000000000001px` in a
            // stylesheet. Six decimals is far past any real length and
            // short of where the noise starts.
            Length::Spacing(steps) => round(steps * theme.spacing_px()),
            Length::Unit(value, unit) => round(value * unit.px().unwrap_or(0.0)),
        }
    }

    /// The unit standing between this length and a pixel value, if one is.
    ///
    /// `Px` and `Spacing` always resolve. A `Unit` resolves when the unit
    /// has a fixed ratio to the pixel; otherwise this names it, and the
    /// caller decides whether it can supply what's missing (an `em` against
    /// a known font size, a `vh` against the viewport hook) or has to
    /// refuse.
    pub fn unresolved_unit(self) -> Option<LengthUnit> {
        match self {
            Length::Px(_) | Length::Spacing(_) => None,
            Length::Unit(_, unit) => unit.px().map(|_| ()).map_or(Some(unit), |()| None),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Dimension {
    Length(Length),
    Percent(f64),
    Auto,
    /// A percentage of the viewport (`h-screen` is `ViewportHeight(100.0)`).
    /// React Native has no viewport unit -- screen size there is a runtime
    /// value from `useWindowDimensions()`, which a static `StyleSheet`
    /// can't hold -- so these are Web-only and the Native backend refuses
    /// them rather than freezing a launch-time size that would go stale on
    /// rotation.
    ViewportWidth(f64),
    ViewportHeight(f64),
    /// A size CSS can state and React Native cannot compute: an intrinsic
    /// keyword (`fit-content`, `max-content`), or one of the viewport units
    /// that track browser chrome (`100dvh`, `100lvh`, `100svh`) and the
    /// line-height unit (`1lh`).
    ///
    /// Kept as the exact CSS text because there is nothing to compute --
    /// every one of them is resolved by the browser against state Dowel
    /// doesn't have, and React Native can express none of them. Unlike
    /// `ViewportWidth`/`ViewportHeight`, which the Native backend *can*
    /// answer from `Dimensions`, these have no runtime equivalent to read:
    /// `dvh` tracks a URL bar that doesn't exist there, and `fit-content`
    /// is a layout mode Yoga doesn't implement.
    Css(String),
}

impl Dimension {
    pub fn is_supported_on_native(self) -> bool {
        !matches!(
            self,
            Dimension::ViewportWidth(_) | Dimension::ViewportHeight(_) | Dimension::Css(_)
        )
    }
}

/// Kept as an unresolved Tailwind token (e.g. `"blue-500"`), not RGBA --
/// token resolution is a separate lowering/optimization pass (proposal §16)
/// that needs the token identity preserved this far.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Color {
    Token(String),
    /// A CSS keyword that stands where a colour goes but isn't one:
    /// `accent-color: auto`, `fill: none`. Kept apart from `Token` because
    /// the palette resolver can't resolve these and its fallback --
    /// `var(--dowel-color-none)` -- would be a plausible-looking wrong
    /// answer rather than an unresolved one.
    Keyword(&'static str),
    /// A colour the source wrote out instead of naming: `bg-[#ff0000]`,
    /// `text-[rgb(0_0_0)]`, `bg-(--brand)`.
    ///
    /// Apart from `Token` because there is nothing to look up -- sending
    /// this through the palette resolver would produce
    /// `var(--dowel-color-#ff0000)`, which is not a colour and not an
    /// error either. Held as the CSS text because that is exactly what the
    /// author asked for; the whole point of an arbitrary value is that it
    /// escapes the design system.
    Css(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontWeight(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
    /// `start`, `end`, `justify`. React Native has all three.
    Css(&'static str),
}

/// An angle, in degrees where Dowel can resolve one.
///
/// `Css` is the arbitrary case, and it is not an edge case: Tailwind's
/// arbitrary values are unvalidated, so `rotate-x-[1.5em]` compiles to
/// `rotateX(1.5em)` there rather than being refused. The text has to live
/// inside this type rather than escaping to a raw declaration, because
/// transform functions *compose* -- `rotate-x-[…] skew-y-3` is one
/// `transform`, and a raw declaration beside a typed one would be two of
/// them, with last-wins silently dropping whichever came first.
#[derive(Debug, Clone, PartialEq)]
pub enum Angle {
    Deg(f64),
    Css(String),
}

impl Angle {
    /// The angle as CSS, which is where the two cases stop differing.
    pub fn css(&self) -> String {
        match self {
            Angle::Deg(degrees) => format!("{degrees}deg"),
            Angle::Css(text) => text.clone(),
        }
    }

    /// The angle in degrees, for the backends that need a number rather
    /// than a declaration. `None` is React Native's answer: it takes a
    /// numeric rotation, and there is nothing to hand it here.
    pub fn degrees(&self) -> Option<f64> {
        match self {
            Angle::Deg(degrees) => Some(*degrees),
            Angle::Css(_) => None,
        }
    }
}

/// A per-axis scale, as the percentage Tailwind writes (`scale-110` -> 110).
///
/// `Css` exists for the same reason `Angle::Css` does, with one extra
/// wrinkle: an arbitrary scale is a *ratio*, not a percentage.
/// `scale-x-[1.5]` is `scale: 1.5`, and putting that 1.5 in the numeric
/// case would print `1.5%` -- a scale of one sixty-sixth, which looks
/// deliberate and renders as nothing.
#[derive(Debug, Clone, PartialEq)]
pub enum Scale {
    Percent(f64),
    Css(String),
}

impl Scale {
    pub fn css(&self) -> String {
        match self {
            Scale::Percent(value) => format!("{value}%"),
            Scale::Css(text) => text.clone(),
        }
    }

    /// The scale as the ratio React Native's `transform` wants.
    pub fn ratio(&self) -> Option<f64> {
        match self {
            Scale::Percent(value) => Some(value / 100.0),
            Scale::Css(_) => None,
        }
    }
}

/// A line clamp: a count, or text Dowel couldn't read as one.
///
/// `line-clamp-[1.5]` is neither a refusal nor a clamp of one. Tailwind
/// writes `-webkit-line-clamp: 1.5` and lets the browser discard it, and
/// `as u32` used to turn it into a clamp of a single line -- a number that
/// looks deliberate and isn't the one written.
#[derive(Debug, Clone, PartialEq)]
pub enum Clamp {
    Lines(u32),
    Css(String),
}

impl Clamp {
    pub fn css(&self) -> String {
        match self {
            Clamp::Lines(lines) => lines.to_string(),
            Clamp::Css(text) => text.clone(),
        }
    }

    /// The count React Native's `numberOfLines` takes.
    pub fn lines(&self) -> Option<u32> {
        match self {
            Clamp::Lines(lines) => Some(*lines),
            Clamp::Css(_) => None,
        }
    }
}

/// A length in `em` -- relative to the element's own font size, so it can't
/// be resolved to pixels at compile time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Em(pub f64);

/// The functions of a `filter`/`backdrop-filter` chain.
///
/// Declared in the order Tailwind writes them, and that order is the
/// discriminant order deliberately: filter functions don't commute --
/// `grayscale(100%) invert(100%)` and `invert(100%) grayscale(100%)` render
/// differently -- so the chain is sorted by this enum rather than by the
/// order the utilities happened to appear in the class string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FilterFunction {
    Blur,
    Brightness,
    Contrast,
    Grayscale,
    HueRotate,
    Invert,
    Saturate,
    Sepia,
    DropShadow,
    /// `backdrop-opacity-*` only; there is no `opacity-*` filter utility,
    /// since bare `opacity-*` is the CSS property rather than the function.
    Opacity,
    /// `filter-none`/`backdrop-filter-none`, which clear the whole chain
    /// rather than contributing to it.
    None,
}

/// What `grid-template-columns`/`grid-template-rows` are set to.
///
/// `Count` is Tailwind's equal-track form, which it writes as
/// `repeat(n, minmax(0, 1fr))` rather than `repeat(n, 1fr)` -- the `minmax`
/// floor is what stops an oversized item from widening its track, and
/// dropping it would produce a grid that behaves differently under
/// overflow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GridTracks {
    Count(u32),
    None,
    Subgrid,
    /// A track list written out: `grid-cols-[1fr_auto]`,
    /// `grid-cols-[repeat(3,minmax(0,1fr))]`.
    ///
    /// Tailwind's own scale only counts equal columns, which covers the
    /// common case and none of the interesting ones -- a sidebar beside a
    /// fluid main column is two unequal tracks and cannot be said any
    /// other way.
    Css(String),
}

/// One edge of a grid item's placement. A negative line counts back from
/// the end of the explicit grid, which is why this is signed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridLine {
    Line(i32),
    Auto,
}

/// Both edges at once, as the `grid-column`/`grid-row` shorthand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridSpan {
    /// `span n / span n` -- n tracks from wherever the item lands.
    Span(u32),
    /// `1 / -1` -- the first line to the last, however many tracks there are.
    Full,
    Auto,
}

/// `columns` takes either a number of columns or an ideal column *width*,
/// and the two mean opposite things -- `columns-3` fixes the count and lets
/// the width follow, `columns-md` fixes the width and lets the count
/// follow. Tailwind spells both as a bare suffix, so the distinction has to
/// survive into the IR or the output would be a plausible wrong answer.
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnCount {
    Count(u32),
    Width(Dimension),
    Auto,
}

/// CSS lets a letter spacing be relative to the font size or absolute.
/// Tailwind writes the named scale in `em` (`tracking-wide` is `0.025em`);
/// the absolute form exists because React Native's `letterSpacing` is
/// absolute, so the Native backend resolves the `em` against a font size on
/// the same element and stores the result here (see
/// `dowel_native::fold_font_relative`).
///
/// One variant rather than a second `StyleProperty`, so that the two forms
/// still dedupe against each other -- `dedupe_last_wins` keys on the
/// property's discriminant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LetterSpacing {
    Em(Em),
    Px(Length),
}

/// CSS allows a line height to be an absolute length or a unitless
/// multiplier of the font size. Tailwind uses both: `leading-6` is the
/// spacing scale, `leading-tight` is a ratio.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineHeight {
    Length(Length),
    /// React Native has no unitless line height -- its `lineHeight` is an
    /// absolute number -- and the font size to multiply by isn't known at
    /// compile time, so this form is Web-only.
    Ratio(f64),
}

/// Tailwind's built-in animations. Named rather than stored as shorthand
/// text because emitting `animation: spin 1s linear infinite` is only half
/// the job -- the matching `@keyframes` has to reach the stylesheet too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Animation {
    Spin,
    Ping,
    Pulse,
    Bounce,
    None,
}

impl Animation {
    /// The `animation` shorthand value.
    pub fn shorthand(self) -> &'static str {
        match self {
            Animation::Spin => "spin 1s linear infinite",
            Animation::Ping => "ping 1s cubic-bezier(0, 0, 0.2, 1) infinite",
            Animation::Pulse => "pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite",
            Animation::Bounce => "bounce 1s infinite",
            Animation::None => "none",
        }
    }

    /// The `@keyframes` block this animation needs, or `None` for `none`.
    /// Verbatim from Tailwind's own definitions.
    pub fn keyframes(self) -> Option<&'static str> {
        Some(match self {
            Animation::Spin => "@keyframes spin {\n  to {\n    transform: rotate(360deg);\n  }\n}",
            Animation::Ping => {
                "@keyframes ping {\n  75%, 100% {\n    transform: scale(2);\n    opacity: 0;\n  }\n}"
            }
            Animation::Pulse => "@keyframes pulse {\n  50% {\n    opacity: 0.5;\n  }\n}",
            Animation::Bounce => {
                "@keyframes bounce {\n  0%, 100% {\n    transform: translateY(-25%);\n    \
                 animation-timing-function: cubic-bezier(0.8, 0, 1, 1);\n  }\n  50% {\n    \
                 transform: none;\n    animation-timing-function: cubic-bezier(0, 0, 0.2, 1);\n  }\n}"
            }
            Animation::None => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overflow {
    Visible,
    Hidden,
    Scroll,
    /// `auto` and `clip`. React Native's overflow union has neither,
    /// so these are refused there rather than approximated.
    Css(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextOverflow {
    Clip,
    Ellipsis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhiteSpace {
    Normal,
    NoWrap,
    /// `pre`, `pre-line`, `pre-wrap`, `break-spaces`. React Native's Text
    /// has no white-space control beyond wrapping, so these are refused.
    Css(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextTransform {
    Uppercase,
    Lowercase,
    Capitalize,
    None,
}

/// `align-self` takes `Align`'s keywords plus `auto`, so it can't reuse
/// `Align` directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignSelf {
    Auto,
    Start,
    Center,
    End,
    Stretch,
    Baseline,
    /// See `Align::Css`.
    Css(&'static str),
}

// ---------------------------------------------------------------------------
// Conditions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum Condition {
    Always,
    /// Several conditions that must all hold: `md:hover:flex`.
    ///
    /// Written order, which is also the order the atoms nest in. Tailwind
    /// nests them the same way -- `md:hover:` is a width query around a
    /// hover query and `hover:md:` is the reverse -- and while the two
    /// render identically, matching the order keeps the output the same
    /// text rather than an equivalent one.
    ///
    /// Stacked variants compiled to *nothing* before this existed:
    /// `parse_variant_prefix` took `md:` and handed `hover:bg-blue-500` to
    /// the utility parser, which doesn't know what that is. No CSS, no
    /// diagnostic, for a class people write every day. It was invisible
    /// because Tailwind's `getClassList()` enumerates utilities and not
    /// the combinations of variants in front of them, so no denominator in
    /// this repository contained one.
    All(Vec<Condition>),
    Responsive(Breakpoint),
    /// Compiles straight to a real CSS pseudo-class on Web (zero runtime).
    Hover,
    Focus,
    Disabled,
    /// Tailwind's `pressed:` variant. Originally assumed this needed
    /// synthesized JS-tracked state (no CSS `:active` equivalent matches
    /// RN's touch semantics) and so should desugar into `Expr` -- wrong on
    /// both counts: Web has a perfectly good `:active` pseudo-class for
    /// this (same free-CSS treatment as Hover/Focus/Disabled), and RN's
    /// `Pressable` already tracks pressed state natively via its
    /// `style={({pressed}) => ...}` render-prop form. Neither platform
    /// needs anything synthesized; each just needs a different, still
    /// zero-extra-runtime, lowering.
    Pressed,
    /// `dark:`. Tailwind v4's default strategy is the
    /// `prefers-color-scheme` media query rather than a `.dark` class, and
    /// React Native's `useColorScheme()` reports the same OS-level
    /// preference -- so the two agree on meaning even though only Web can
    /// express it as a style condition.
    Dark,
    /// `first:`. A structural position, which only the DOM can match on its
    /// own; React Native has no selector engine. Dowel does see the whole
    /// JSX tree, so resolving this at compile time for statically-known
    /// children is possible -- but not for `.map()`-generated ones, and
    /// that's not built yet.
    FirstChild,
    /// `last:`. The same question from the other end, and resolvable on
    /// Native under the same condition: the compiler can see whether
    /// anything follows this element, unless what follows is a
    /// `Child::Verbatim` that may render nothing or a hundred elements.
    LastChild,
    /// `focus-visible:`. Distinct from `Focus` and not a nicety: it is the
    /// one that doesn't put a ring around a button someone clicked, which
    /// is why it exists at all.
    FocusVisible,
    /// `[&>*]:p-4`, `[&_a]:underline`, `[&:nth-child(3)]:font-bold`. The
    /// selector as written, with `&` still standing for the element.
    ///
    /// Web substitutes the generated class for `&` and emits the result.
    /// Nothing is validated: a selector Dowel doesn't understand is one
    /// the browser might, and the author asked for it by name.
    ///
    /// Native refuses all of these, and not for want of effort -- React
    /// Native has no selector engine at all. `[&>*]` asks a question about
    /// the rendered tree that only a matching engine can answer, and there
    /// isn't one on the other side.
    ArbitrarySelector(String),
    /// `[@media(print)]:hidden`, `[@supports(display:grid)]:grid`. The
    /// at-rule prelude as written, `@` included.
    ///
    /// Kept apart from `ArbitrarySelector` because the lowering differs in
    /// kind: a selector rewrites the rule's head, an at-rule wraps the
    /// whole rule in a block. Folding them together would mean deciding
    /// which one this is by looking for a leading `@` at emit time, in the
    /// one place that has the least context for it.
    ArbitraryAtRule(String),
    /// Arbitrary structurally-dynamic condition (proposal §7): a prop,
    /// local variable, or `useState` value used as a guard.
    Expr(ConditionExpr),
}

/// Which gradient function a `bg-*` gradient utility starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradientKind {
    Linear,
    Radial,
    Conic,
}

impl GradientKind {
    pub fn css(self) -> &'static str {
        match self {
            GradientKind::Linear => "linear-gradient",
            GradientKind::Radial => "radial-gradient",
            GradientKind::Conic => "conic-gradient",
        }
    }
}

/// Which stop of a gradient a `from-*`/`via-*`/`to-*` utility describes.
///
/// Declaration order is the order they appear in the value, which is what
/// `GradientStop as u32` is used for in `dedupe_key`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradientStop {
    From,
    Via,
    To,
}

impl GradientStop {
    /// The position CSS uses when no `from-<n>%` was written. These are
    /// the `initial-value`s of Tailwind's own `--tw-gradient-*-position`
    /// registers.
    pub fn default_position(self) -> &'static str {
        match self {
            GradientStop::From => "0%",
            GradientStop::Via => "50%",
            GradientStop::To => "100%",
        }
    }
}

/// One slot in Tailwind's `mask-image` layer list.
///
/// The four sides compose into the first slot as a nested list, which is
/// why a side utility produces six layers and a shape utility three.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MaskSlot {
    Left,
    Right,
    Bottom,
    Top,
    Linear,
    Radial,
    Conic,
}

/// Which end of a mask gradient a stop describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MaskStop {
    From,
    To,
}

/// Which edge (or edge pair) a per-side property targets, for the families
/// where the number of edges makes one variant each impractical.
///
/// Named to match the CSS longhand suffixes, so each backend's lookup table
/// reads as the property list it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Edge {
    /// No suffix -- the shorthand that sets every edge.
    All,
    Top,
    Right,
    Bottom,
    Left,
    Inline,
    Block,
    InlineStart,
    InlineEnd,
    BlockStart,
    BlockEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Breakpoint {
    Sm,
    Md,
    Lg,
    Xl,
    Xl2,
}

/// A condition's leaves are opaque source references, not parsed
/// identifiers/comparisons: the compiler never evaluates a condition, it
/// only needs to know where one guard ends and the next begins so it can
/// re-emit the expression verbatim in generated output.
#[derive(Debug, Clone, PartialEq)]
pub enum ConditionExpr {
    /// A JSX boolean shorthand such as `<Button disabled />` has no source
    /// expression span to preserve, but is still a real constant guard.
    Static(bool),
    Ref(ExprRef),
    Not(Box<ConditionExpr>),
    And(Box<ConditionExpr>, Box<ConditionExpr>),
    Or(Box<ConditionExpr>, Box<ConditionExpr>),
}

// ---------------------------------------------------------------------------
// Grouping/flattening (shared by every lowering backend)
// ---------------------------------------------------------------------------

/// Groups declarations by `Condition`, preserving first-occurrence order
/// (deterministic output, not hashmap-random) -- a linear scan is fine at
/// the sizes a single node's style list reaches in practice.
///
/// Flattening ("last declaration wins") only applies *within* a group of
/// declarations sharing the identical `Condition` -- declarations under
/// different conditions are separate output rules (a CSS rule on Web, a
/// separate style object on Native), not competing values to resolve at
/// compile time.
pub fn group_by_condition(declarations: &[StyleDeclaration]) -> Vec<(Condition, Vec<StyleProperty>)> {
    let mut groups: Vec<(Condition, Vec<StyleProperty>)> = Vec::new();
    for decl in declarations {
        match groups.iter_mut().find(|(condition, _)| *condition == decl.condition) {
            Some((_, props)) => props.push(decl.property.clone()),
            None => groups.push((decl.condition.clone(), vec![decl.property.clone()])),
        }
    }
    groups
}

/// Within one condition group, the last declaration for a given property
/// wins -- resolved by discriminant (the property's *kind*, ignoring its
/// value), keeping only the last occurrence of each while preserving
/// overall relative order.
pub fn dedupe_last_wins(props: Vec<StyleProperty>) -> Vec<StyleProperty> {
    let mut seen = std::collections::HashSet::new();
    let mut kept: Vec<StyleProperty> = Vec::new();
    for prop in props.into_iter().rev() {
        if seen.insert(prop.dedupe_key()) {
            kept.push(prop);
        }
    }
    kept.reverse();
    kept
}

impl StyleProperty {
    /// What makes two declarations "the same property" for last-wins
    /// flattening.
    ///
    /// The enum discriminant, almost always -- which is why nearly every
    /// per-side property here has its own variant rather than carrying a
    /// side. The exception is the variants that carry their own target: for
    /// those the target is part of the identity, or `scroll-mt-4
    /// scroll-mb-8` (and `mask-t-from-4 mask-b-from-8`) would collapse into
    /// one.
    ///
    /// The extra discriminator is a plain integer because it has to cover
    /// an `Edge`, a `(MaskSlot, MaskStop)` pair, and a bare `MaskSlot`
    /// without those needing a common type. Only the variants listed here
    /// use it; everything else passes 0.
    ///
    /// `Keyword` needs a third component: every one of those shares a
    /// single discriminant, so the CSS property name is what tells two of
    /// them apart. It is a string rather than a number on purpose --
    /// hashing into the integer would make a collision silently drop a
    /// declaration, which is the failure this key exists to prevent.
    fn dedupe_key(&self) -> (std::mem::Discriminant<Self>, u32, String) {
        if let StyleProperty::Keyword(property, _) = self {
            return (std::mem::discriminant(self), 0, (*property).to_string());
        }
        // Same reasoning as `Keyword`: one discriminant covers every
        // arbitrary property, so the name is the only thing that tells two
        // of them apart.
        if let StyleProperty::Arbitrary(property, _) = self {
            return (std::mem::discriminant(self), 0, property.clone());
        }
        let target = match self {
            StyleProperty::ScrollMargin(edge, _) | StyleProperty::ScrollPadding(edge, _) => {
                *edge as u32
            }
            StyleProperty::MaskStopColor(slot, stop, _)
            | StyleProperty::MaskStopPosition(slot, stop, _) => {
                (*slot as u32) * 2 + *stop as u32
            }
            StyleProperty::BorderLogicalWidth(edge, _) | StyleProperty::BorderLogicalStyle(edge, _) => {
                *edge as u32
            }
            StyleProperty::MaskSlotArgument(slot, _) => *slot as u32,
            StyleProperty::GradientStopColor(stop, _) | StyleProperty::GradientStopPosition(stop, _) => {
                *stop as u32
            }
            StyleProperty::Filter(function, _) | StyleProperty::BackdropFilter(function, _) => {
                *function as u32
            }
            _ => 0,
        };
        (std::mem::discriminant(self), target, String::new())
    }
}

#[cfg(test)]
mod grouping_tests {
    use super::*;

    #[test]
    fn dedupe_keeps_last_value_per_property_kind() {
        let props = vec![
            StyleProperty::PaddingLeft(Length::Px(4.0)),
            StyleProperty::PaddingTop(Length::Px(4.0)),
            StyleProperty::PaddingLeft(Length::Px(16.0)),
        ];
        let deduped = dedupe_last_wins(props);
        assert_eq!(
            deduped,
            vec![StyleProperty::PaddingTop(Length::Px(4.0)), StyleProperty::PaddingLeft(Length::Px(16.0))]
        );
    }

    #[test]
    fn an_edge_is_part_of_a_property_s_identity() {
        // `ScrollMargin` carries its edge instead of having eleven variants,
        // so the edge has to be in the dedupe key -- otherwise these two
        // would look like the same property and only the last would survive.
        let props = vec![
            StyleProperty::ScrollMargin(Edge::Top, Length::Px(16.0)),
            StyleProperty::ScrollMargin(Edge::Bottom, Length::Px(32.0)),
        ];
        assert_eq!(dedupe_last_wins(props).len(), 2);

        // ...while the same edge twice still resolves last-wins.
        let props = vec![
            StyleProperty::ScrollMargin(Edge::Top, Length::Px(16.0)),
            StyleProperty::ScrollMargin(Edge::Top, Length::Px(32.0)),
        ];
        assert_eq!(
            dedupe_last_wins(props),
            vec![StyleProperty::ScrollMargin(Edge::Top, Length::Px(32.0))]
        );
    }

    #[test]
    fn condition_groups_stay_separate() {
        let decls = vec![
            StyleDeclaration {
                property: StyleProperty::BackgroundColor(Color::Token("red-500".to_string())),
                condition: Condition::Always,
            },
            StyleDeclaration {
                property: StyleProperty::BackgroundColor(Color::Token("blue-500".to_string())),
                condition: Condition::Hover,
            },
        ];
        let groups = group_by_condition(&decls);
        assert_eq!(groups.len(), 2);
    }
}
