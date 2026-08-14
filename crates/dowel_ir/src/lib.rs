//! Platform-independent IR shared across the Dowel compiler pipeline.
//!
//! `Node` (semantic tree) plus `StyleDeclaration` (per-node style, each
//! gated by a `Condition`) together form the Dowel IR described in the
//! proposal's architecture section. Values here are the compiler's output
//! shape, not its parsing shape -- `dowel_parser` builds this from
//! TSX/Tailwind source.

mod colors;
pub use colors::{resolve_color_token, ResolvedColor};

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
    /// A prop spread appears after a statically compiled className/style and
    /// could silently override it at runtime; that node's className is not
    /// compiled and falls back instead of failing silently.
    UnsafePropSpreadAfterStyle,
    /// A utility with no React Native equivalent reached the Native
    /// backend. Verified against Yoga (RN's layout engine), whose `display`
    /// is only Flex/None/Contents and which has no grid implementation at
    /// all -- so `block`, `inline-flex`, `grid` and friends can't be
    /// approximated, only refused.
    WebOnlyPropertyOnNative,
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub primitive: Primitive,
    pub style: Vec<StyleDeclaration>,
    pub props: PropSet,
    pub children: Vec<Node>,
    /// Present only on `Text` nodes.
    pub text: Option<TextContent>,
    /// Parts of a `className` expression that couldn't be statically
    /// decomposed into `style` (proposal §7's "truly dynamic" tier) --
    /// threaded through to `@dowel/runtime`'s `cx()` at render time.
    /// Populated per-leaf, not per-node: a `cn(...)` call can contribute
    /// some declarations to `style` and some entries here in the same call.
    pub class_name_fallback: Vec<ExprRef>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TextContent {
    Literal(String),
    /// e.g. `{user.name}` -- re-emitted verbatim, not interpreted.
    Dynamic(ExprRef),
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
    ZIndex(i32),
    /// Column count for `grid-cols-<n>`. Web-only: React Native's layout
    /// engine has no grid implementation at all.
    GridTemplateColumns(u32),
    Position(Position),
    InsetTop(Length),
    InsetRight(Length),
    InsetBottom(Length),
    InsetLeft(Length),
    InsetInlineStart(Length),
    InsetInlineEnd(Length),

    // Visual
    BackgroundColor(Color),
    Opacity(f32),
    BorderColor(Color),
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

    /// CSS states these as standalone properties (`rotate: 45deg`), which
    /// is also how Tailwind v4 emits them. React Native has no standalone
    /// equivalents -- only a combined `transform` -- so the Native backend
    /// composes whichever of these are present into one entry, in CSS's
    /// defined application order (translate, then rotate, then scale).
    Rotate(Angle),
    /// A ratio: `scale-95` is 0.95, not 95.
    Scale(f32),
    TranslateX(Length),
    TranslateY(Length),
    /// Kept as the already-composed CSS value rather than a structured
    /// list. React Native accepts a string for `boxShadow`/`filter` too, so
    /// both backends emit the same text and there's nothing for a
    /// structured form to buy here.
    BoxShadow(String),
    Filter(String),

    // Typography
    FontSize(Length),
    FontWeight(FontWeight),
    LineHeight(LineHeight),
    /// Letter spacing, always in `em` -- Tailwind's `tracking-*` scale is
    /// defined relative to the element's own font size. Web-only: React
    /// Native's `letterSpacing` is an absolute number, and the font size to
    /// resolve against isn't known at compile time.
    LetterSpacing(Em),
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
    SpaceX(Length),
    SpaceY(Length),
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
    Grow(f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
    Stretch,
    Baseline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Justify {
    Start,
    Center,
    End,
    Between,
    Around,
    Evenly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    Relative,
    Absolute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
    Flex,
    None,
    Contents,
    // No React Native equivalent: Yoga implements exactly Flex/None/
    // Contents and has no grid at all, so these can't be approximated.
    // `dowel_native` refuses them rather than dropping them silently.
    Block,
    InlineFlex,
    Grid,
}

impl Display {
    /// Whether React Native can express this at all -- see the variants'
    /// own note.
    pub fn is_supported_on_native(self) -> bool {
        matches!(self, Display::Flex | Display::None | Display::Contents)
    }
}

impl StyleProperty {
    /// `Some(description)` when React Native has no way to express this, so
    /// the Native backend can refuse it by name instead of dropping it. Kept
    /// here rather than in that backend so every such case is listed in one
    /// place as more are found.
    pub fn unsupported_on_native(&self) -> Option<String> {
        let viewport = |dim: &Dimension, property: &str| match dim {
            Dimension::ViewportWidth(pct) => {
                Some(format!("`{property}: {pct}vw`: React Native has no viewport unit"))
            }
            Dimension::ViewportHeight(pct) => {
                Some(format!("`{property}: {pct}vh`: React Native has no viewport unit"))
            }
            _ => None,
        };
        match self {
            StyleProperty::Display(d) if !d.is_supported_on_native() => Some(format!(
                "`display: {}`: React Native's layout engine supports only flex, none and contents",
                match d {
                    Display::Block => "block",
                    Display::InlineFlex => "inline-flex",
                    Display::Grid => "grid",
                    _ => unreachable!("guarded by is_supported_on_native"),
                }
            )),
            StyleProperty::Width(d) => viewport(d, "width"),
            StyleProperty::Height(d) => viewport(d, "height"),
            StyleProperty::MinWidth(d) => viewport(d, "min-width"),
            StyleProperty::MinHeight(d) => viewport(d, "min-height"),
            StyleProperty::MaxWidth(d) => viewport(d, "max-width"),
            StyleProperty::MaxHeight(d) => viewport(d, "max-height"),
            StyleProperty::GridTemplateColumns(_) => {
                Some("`grid-template-columns`: React Native has no grid layout".to_string())
            }
            StyleProperty::LetterSpacing(_) => Some(
                "`letter-spacing` in em: React Native's letterSpacing is absolute, and the font \
                 size to resolve against isn't known at compile time"
                    .to_string(),
            ),
            StyleProperty::LineHeight(LineHeight::Ratio(_)) => Some(
                "a unitless `line-height`: React Native's lineHeight is absolute, and the font \
                 size to multiply by isn't known at compile time"
                    .to_string(),
            ),
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
            StyleProperty::SpaceX(_) | StyleProperty::SpaceY(_) => Some(
                "`space-*`: it styles the element's children via a selector, and React Native has \
                 no selector engine"
                    .to_string(),
            ),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderStyle {
    Solid,
    Dashed,
    Dotted,
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
    Px(f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Dimension {
    Length(Length),
    Percent(f32),
    Auto,
    /// A percentage of the viewport (`h-screen` is `ViewportHeight(100.0)`).
    /// React Native has no viewport unit -- screen size there is a runtime
    /// value from `useWindowDimensions()`, which a static `StyleSheet`
    /// can't hold -- so these are Web-only and the Native backend refuses
    /// them rather than freezing a launch-time size that would go stale on
    /// rotation.
    ViewportWidth(f32),
    ViewportHeight(f32),
}

impl Dimension {
    pub fn is_supported_on_native(self) -> bool {
        !matches!(self, Dimension::ViewportWidth(_) | Dimension::ViewportHeight(_))
    }
}

/// Kept as an unresolved Tailwind token (e.g. `"blue-500"`), not RGBA --
/// token resolution is a separate lowering/optimization pass (proposal §16)
/// that needs the token identity preserved this far.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Color {
    Token(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontWeight(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Angle {
    pub degrees: f32,
}

/// A length in `em` -- relative to the element's own font size, so it can't
/// be resolved to pixels at compile time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Em(pub f32);

/// CSS allows a line height to be an absolute length or a unitless
/// multiplier of the font size. Tailwind uses both: `leading-6` is the
/// spacing scale, `leading-tight` is a ratio.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineHeight {
    Length(Length),
    /// React Native has no unitless line height -- its `lineHeight` is an
    /// absolute number -- and the font size to multiply by isn't known at
    /// compile time, so this form is Web-only.
    Ratio(f32),
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
}

// ---------------------------------------------------------------------------
// Conditions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum Condition {
    Always,
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
    /// Arbitrary structurally-dynamic condition (proposal §7): a prop,
    /// local variable, or `useState` value used as a guard.
    Expr(ConditionExpr),
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
        if seen.insert(std::mem::discriminant(&prop)) {
            kept.push(prop);
        }
    }
    kept.reverse();
    kept
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
