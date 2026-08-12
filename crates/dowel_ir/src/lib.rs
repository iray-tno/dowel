//! Platform-independent IR shared across the Dowel compiler pipeline.
//!
//! `Node` (semantic tree) plus `StyleDeclaration` (per-node style, each
//! gated by a `Condition`) together form the Dowel IR described in the
//! proposal's architecture section. Values here are the compiler's output
//! shape, not its parsing shape -- `dowel_parser` builds this from
//! TSX/Tailwind source.

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
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TextContent {
    Literal(String),
    /// e.g. `{user.name}` -- re-emitted verbatim, not interpreted.
    Dynamic(ExprRef),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PropSet {
    pub on_press: Option<ExprRef>,
    pub disabled: Option<ConditionExpr>,
    /// Explicit override; `None` means derive the role from `Primitive`
    /// (e.g. `Button` -> `AccessibilityRole::Button`).
    pub accessibility_role: Option<AccessibilityRole>,
    /// Props Dowel doesn't model explicitly -- re-emitted unchanged.
    pub passthrough: Vec<(String, ExprRef)>,
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
    FlexDirection(FlexDirection),
    Flex(FlexShorthand),
    AlignItems(Align),
    JustifyContent(Justify),
    Gap(Length),
    RowGap(Length),
    ColumnGap(Length),
    // Margin/padding/inset are per-side longhand properties, not a single
    // EdgeInsets-bundling property: Tailwind utilities like `px-4`/`py-2`
    // set disjoint sides independently, and only per-side variants let
    // "last declaration for a property wins" flattening compose them
    // correctly instead of one clobbering the other.
    MarginTop(Length),
    MarginRight(Length),
    MarginBottom(Length),
    MarginLeft(Length),
    PaddingTop(Length),
    PaddingRight(Length),
    PaddingBottom(Length),
    PaddingLeft(Length),
    Width(Dimension),
    Height(Dimension),
    Position(Position),
    InsetTop(Length),
    InsetRight(Length),
    InsetBottom(Length),
    InsetLeft(Length),

    // Visual
    BackgroundColor(Color),
    Opacity(f32),
    BorderColor(Color),
    BorderWidth(Length),
    BorderRadius(Length),

    // Typography
    FontSize(Length),
    FontWeight(FontWeight),
    LineHeight(Length),
    TextAlign(TextAlign),
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Length {
    Px(f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Dimension {
    Length(Length),
    Percent(f32),
    Auto,
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
    /// Arbitrary structurally-dynamic condition (proposal §7): a prop, local
    /// variable, or `useState` value used as a guard. Also the desugar
    /// target for Tailwind's `pressed:` variant, which has no CSS `:active`
    /// equivalent matching RN's touch semantics and so always needs
    /// JS-tracked state -- no reason to give it a separate variant when it
    /// lowers through the exact same path as any other `Expr`.
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
