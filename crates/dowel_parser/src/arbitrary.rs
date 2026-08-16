//! Tailwind's arbitrary syntax: `w-[32px]`, `bg-(--brand)`, `[color:red]`,
//! `[&>*]:p-4`.
//!
//! Everything else in `tailwind.rs` maps a name Tailwind defined to a value
//! Dowel understands. This module is the opposite: the author has stepped
//! outside the design system and written CSS, and Dowel's job is to carry
//! it through without inventing meaning.
//!
//! It exists mostly because of what happened without it. Utility parsing
//! ends in a colour catch-all -- whatever the specific forms decline
//! becomes a palette token -- so a bracket that no arm recognised fell
//! through to it. `text-[14px]`, a font size, compiled to
//! `color: var(--dowel-color-[14px])`. `w-[32px]` matched nothing at all
//! and vanished. Neither raised a diagnostic, and neither is the kind of
//! wrong you notice in a diff.
//!
//! None of this is visible to the conformance harness either, which is
//! worth stating plainly: its candidate list comes from Tailwind's
//! `getClassList()`, and arbitrary values aren't in it because there are
//! infinitely many. The 100% Web figure was 100% of a catalogue that
//! contained none of this.

use dowel_ir::{
    BorderStyle, Color, Dimension, GridTracks, Length, LengthUnit, LetterSpacing, LineHeight,
    Edge, Radius,
    StyleProperty,
};

/// The functions inside which Tailwind spaces out math operators, copied
/// from its own list. `rem` is a math function here (the remainder
/// operation), not the length unit -- the two are told apart by the
/// following `(`.
const MATH_FUNCTIONS: &[&str] = &[
    "calc", "min", "max", "clamp", "mod", "rem", "sin", "cos", "tan", "asin", "acos", "atan",
    "atan2", "pow", "sqrt", "hypot", "log", "exp", "round",
];

/// Whether a class uses arbitrary syntax at all -- brackets, or v4's
/// `(--var)` shorthand.
///
/// Deliberately cheap and deliberately over-eager. Its only job is to
/// decide whether failing to parse something deserves a diagnostic, and a
/// bare unknown class must never get one: projects have their own CSS and
/// Dowel has no business complaining about `card-header`. A bracket is
/// different -- it is unambiguously Tailwind being asked for something.
pub fn is_arbitrary(token: &str) -> bool {
    token.contains('[') || token.contains("(--")
}

/// Turns the text between the brackets into the CSS it stands for.
///
/// Two transformations, in this order because Tailwind does them in this
/// order: underscores become spaces, then math operators get breathing
/// room. Doing it the other way round would leave `calc(100%_-_2rem)`
/// with the operator still glued to an underscore.
pub fn normalize(raw: &str) -> String {
    space_math_operators(&substitute_underscores(raw))
}

/// `_` means a space, because a CSS class can't contain one.
///
/// Two exceptions, both Tailwind's. A backslash escapes an underscore that
/// was meant literally, and `url(...)` is left entirely alone -- an
/// underscore is an ordinary character in a path, and rewriting it would
/// turn a working image reference into a 404 that looks like a typo.
fn substitute_underscores(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = String::with_capacity(raw.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\\' && index + 1 < bytes.len() {
            // The escape is consumed with what it escapes: `\_` is an
            // underscore, and the backslash itself doesn't reach the CSS.
            out.push(bytes[index + 1] as char);
            index += 2;
            continue;
        }
        if raw[index..].starts_with("url(") {
            match raw[index..].find(')') {
                Some(offset) => {
                    out.push_str(&raw[index..index + offset + 1]);
                    index += offset + 1;
                    continue;
                }
                // Unbalanced, so there's no region to protect. Falling
                // through treats it as ordinary text rather than
                // swallowing the rest of the value.
                None => {}
            }
        }
        let ch = raw[index..].chars().next().unwrap_or('\0');
        out.push(if ch == '_' { ' ' } else { ch });
        index += ch.len_utf8();
    }
    out
}

/// Transcribed from Tailwind's `addWhitespaceAroundMathOperators`.
///
/// Transcribed rather than reimplemented, and that is the point: the rules
/// are not derivable from CSS. `calc(100%-2rem)` is subtraction and
/// `w-[-1px]` is a negative number; `1e-3` is neither. Commas gain a space
/// inside `min()` and keep none inside `repeat()`. Every one of those is a
/// choice Tailwind made, and matching it is the whole requirement -- the
/// conformance harness compares emitted CSS byte for byte.
fn space_math_operators(input: &str) -> String {
    if !MATH_FUNCTIONS.iter().any(|name| input.contains(name)) {
        return input.to_string();
    }

    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    // Innermost first: whether each open paren belongs to a math function.
    let mut math_stack: Vec<bool> = Vec::new();
    // The run of digits/unit characters currently being read, and where the
    // previous one ended. The latter is what lets `2rem-4px` be read as a
    // subtraction: the `-` sits immediately after a completed number.
    let mut run_start: Option<usize> = None;
    let mut previous_run_end: Option<usize> = None;

    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        let in_math = *math_stack.first().unwrap_or(&false);

        let continues_run = byte.is_ascii_digit()
            || (run_start.is_some() && (byte == b'%' || byte.is_ascii_alphabetic()));
        if continues_run {
            run_start = Some(index);
        } else {
            previous_run_end = run_start;
            run_start = None;
        }

        if byte == b'(' {
            out.push(byte);
            // The function name is the run of letters and digits directly
            // before the paren.
            let mut start = index;
            for probe in (0..index).rev() {
                let c = bytes[probe];
                if c.is_ascii_digit() || c.is_ascii_lowercase() {
                    start = probe;
                } else {
                    break;
                }
            }
            let name = &input[start..index];
            // A bare `(` inside a math function is still math -- CSS lets
            // you group with parentheses there.
            let is_math = MATH_FUNCTIONS.contains(&name) || (in_math && name.is_empty());
            math_stack.insert(0, is_math);
            index += 1;
            continue;
        }

        if byte == b')' {
            out.push(byte);
            if !math_stack.is_empty() {
                math_stack.remove(0);
            }
            index += 1;
            continue;
        }

        if byte == b',' && in_math {
            out.extend_from_slice(b", ");
            index += 1;
            continue;
        }

        if byte == b' ' && in_math && out.last() == Some(&b' ') {
            index += 1;
            continue;
        }

        if matches!(byte, b'+' | b'*' | b'/' | b'-') && in_math {
            let trimmed = trim_end(&out);
            let last = trimmed.last().copied();
            let second_last = if trimmed.len() >= 2 { Some(trimmed[trimmed.len() - 2]) } else { None };
            let next = bytes.get(index + 1).copied();

            let after_exponent = matches!(last, Some(b'e') | Some(b'E'))
                && second_last.is_some_and(|c| c.is_ascii_digit());
            let after_operator = matches!(last, Some(b'+') | Some(b'*') | Some(b'/') | Some(b'-'));
            let after_open = matches!(last, Some(b'(') | Some(b','));

            if after_exponent || after_operator || after_open {
                // All three mean this isn't a binary operator: it's an
                // exponent's sign, a doubled operator, or the sign of the
                // first term.
                out.push(byte);
            } else if index > 0 && bytes[index - 1] == b' ' {
                out.push(byte);
                out.push(b' ');
            } else if last.is_some_and(|c| c.is_ascii_digit())
                || next.is_some_and(|c| c.is_ascii_digit())
                || last == Some(b')')
                || matches!(next, Some(b'(') | Some(b'+') | Some(b'*') | Some(b'/') | Some(b'-'))
                || previous_run_end.is_some_and(|end| end + 1 == index)
            {
                out.push(b' ');
                out.push(byte);
                out.push(b' ');
            } else {
                out.push(byte);
            }
            index += 1;
            continue;
        }

        out.push(byte);
        index += 1;
    }

    String::from_utf8(out).unwrap_or_else(|_| input.to_string())
}

fn trim_end(bytes: &[u8]) -> &[u8] {
    let mut end = bytes.len();
    while end > 0 && bytes[end - 1] == b' ' {
        end -= 1;
    }
    &bytes[..end]
}

// ---------------------------------------------------------------------------
// Reading a normalized value
// ---------------------------------------------------------------------------

/// A length, where the value is one.
///
/// `32px` becomes `Px` rather than `Unit(32, Px)` -- there is no pixel
/// variant, because a pixel is what every other length resolves *to*.
/// Unitless zero is a length too, which CSS allows and RN needs.
pub fn parse_length(css: &str) -> Option<Length> {
    if let Ok(zero) = css.parse::<f64>() {
        // Only zero: CSS requires a unit on every other length, and
        // accepting `w-[32]` would be accepting more than Tailwind does.
        return (zero == 0.0).then_some(Length::Px(0.0));
    }
    let split = css.find(|c: char| c.is_ascii_alphabetic() || c == '%')?;
    let (number, unit) = css.split_at(split);
    let value: f64 = number.parse().ok()?;
    if unit.eq_ignore_ascii_case("px") {
        return Some(Length::Px(value));
    }
    LengthUnit::parse(unit).map(|unit| Length::Unit(value, unit))
}

/// A value in a position that accepts a size.
///
/// Anything that isn't a length, a percentage or `auto` is kept as CSS
/// text. That is not a fallback so much as the honest answer: `calc()`,
/// `min()` and `fit-content` are resolved against layout state that only a
/// browser has, and `Dimension::Css` is exactly the variant that says so --
/// Web prints it, Native refuses it by name.
pub fn parse_dimension(css: &str) -> Dimension {
    if css == "auto" {
        return Dimension::Auto;
    }
    if let Some(percent) = css.strip_suffix('%') {
        if let Ok(value) = percent.parse::<f64>() {
            return Dimension::Percent(value);
        }
    }
    match parse_length(css) {
        Some(length) => Dimension::Length(length),
        None => Dimension::Css(css.to_string()),
    }
}

/// A colour from arbitrary text.
pub fn parse_color(css: &str) -> Color {
    Color::Css(css.to_string())
}

// ---------------------------------------------------------------------------
// Splitting a token
// ---------------------------------------------------------------------------

/// Tailwind's data-type hints: `text-[length:14px]` says "this is a size,
/// not a colour" where the value's shape alone would be ambiguous.
///
/// A closed list because position isn't enough to tell a hint from part of
/// the value -- `bg-[url(https://x)]` has a colon in it too, and reading
/// `url(https` as a hint would leave `//x)` as the value.
const HINTS: &[&str] = &[
    "length", "color", "image", "url", "percentage", "number", "integer", "angle", "ratio",
    "position", "bg-size", "line-width", "family-name", "generic-name", "absolute-size",
    "relative-size", "vector",
];

/// A token split into what it targets and what it says.
pub struct Arbitrary<'a> {
    /// The utility name: `w`, `bg`, `grid-cols`.
    pub prefix: &'a str,
    /// The value as CSS, normalized.
    pub value: String,
    /// The declared data type, where the source gave one.
    pub hint: Option<&'a str>,
}

/// Splits `prefix-[value]` or v4's `prefix-(--var)` shorthand.
///
/// The shorthand is sugar and treated as such: `bg-(--brand)` becomes the
/// value `var(--brand)` and then follows exactly the same path as
/// `bg-[var(--brand)]`, so there is one implementation of what a custom
/// property means rather than two that can drift.
pub fn split(token: &str) -> Option<Arbitrary<'_>> {
    let (prefix, inner, is_var) = if let Some(body) = token.strip_suffix(']') {
        let cut = body.find("-[")?;
        (&body[..cut], &body[cut + 2..], false)
    } else if let Some(body) = token.strip_suffix(')') {
        let cut = body.find("-(")?;
        (&body[..cut], &body[cut + 2..], true)
    } else {
        return None;
    };

    let (hint, rest) = match inner.split_once(':') {
        Some((candidate, rest)) if HINTS.contains(&candidate) => (Some(candidate), rest),
        _ => (None, inner),
    };

    // The shorthand only ever names a custom property. Anything else in
    // those parens is not this syntax, and guessing would mean compiling
    // something Tailwind doesn't.
    if is_var && !rest.starts_with("--") {
        return None;
    }
    let value = if is_var { format!("var({})", normalize(rest)) } else { normalize(rest) };
    Some(Arbitrary { prefix, value, hint })
}

/// A whole class that is one declaration: `[color:red]`, `[--gap:4px]`.
///
/// Returns the property and its value. The property name is checked only
/// for shape -- a CSS identifier, or a custom property -- because the set
/// of real CSS properties is both large and always growing, and a compiler
/// that rejected next year's property would be worse than one that passed
/// through a typo.
pub fn split_property(token: &str) -> Option<(String, String)> {
    let inner = token.strip_prefix('[')?.strip_suffix(']')?;
    let (property, value) = inner.split_once(':')?;
    let valid = property.starts_with("--")
        || (property.starts_with(|c: char| c.is_ascii_alphabetic())
            && property.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
    if !valid || value.is_empty() {
        return None;
    }
    Some((property.to_string(), normalize(value)))
}

/// Splits an arbitrary *variant*: `[&>*]:p-4`, `[@media(print)]:hidden`.
///
/// Bracket-aware rather than a split on the first colon, which would cut
/// `[&:hover]:p-4` in the middle of its own selector.
pub fn split_variant(token: &str) -> Option<(String, &str)> {
    if !token.starts_with('[') {
        return None;
    }
    let mut depth = 0usize;
    for (index, ch) in token.char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    let rest = token[index + 1..].strip_prefix(':')?;
                    // An empty rest is `[&>*]:` with nothing after it,
                    // which is not a utility.
                    if rest.is_empty() {
                        return None;
                    }
                    return Some((normalize(&token[1..index]), rest));
                }
            }
            _ => {}
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Turning a split token into properties
// ---------------------------------------------------------------------------

/// The properties an arbitrary class maps to, or `None` if Dowel can't
/// read it.
///
/// `None` is not "ignore this". The caller reports it, because a bracket
/// is the author asking for something specific and silence is the one
/// answer that helps nobody -- see `DiagnosticCode::UnreadableArbitraryValue`.
pub fn properties(token: &str) -> Option<Vec<StyleProperty>> {
    if let Some((property, value)) = split_property(token) {
        return Some(vec![StyleProperty::Arbitrary(property, value)]);
    }
    let parsed = split(token)?;
    from_prefix(parsed.prefix, &parsed.value, parsed.hint)
}

/// Whether a `text-`/`bg-`/`border-` value should be read as a colour.
///
/// The polarity here is Tailwind's and it is not the obvious one. The
/// question asked is "is this a length?", and *everything else* is a
/// colour -- so `text-[nonsense]` compiles to `color: nonsense`, not to a
/// refusal. Written the intuitive way round ("does this look like a
/// colour?") the same class became a font size of `nonsense`, which is a
/// different wrong answer from the one Tailwind gives.
///
/// The hint wins where there is one, which is the entire reason Tailwind
/// has hints: `text-[color:var(--c)]` and `text-[length:var(--c)]` are the
/// same text and different properties, and no amount of looking at
/// `var(--c)` will say which.
fn is_color(value: &str, hint: Option<&str>) -> bool {
    match hint {
        Some("color") => true,
        Some(_) => false,
        None => parse_length(value).is_none(),
    }
}

fn is_percentage(value: &str) -> bool {
    value.strip_suffix('%').is_some_and(|n| n.parse::<f64>().is_ok())
}

/// Whether a value is CSS's `line-width`: what a border or outline width
/// accepts.
///
/// Length or bare number, and **not** a percentage -- `border-[50%]` is a
/// border *colour* in Tailwind. Each of these families draws its type
/// boundary somewhere slightly different, and the differences are not
/// guessable: a decoration thickness takes a percentage and refuses a bare
/// number, a stroke width takes both, and this takes the other one. Every
/// boundary here was set by disagreeing with the oracle first.
fn is_line_width(value: &str) -> bool {
    parse_length(value).is_some() || value.parse::<f64>().is_ok()
}

/// `<length-percentage>`: what a font size or a decoration thickness takes.
fn is_length_percentage(value: &str) -> bool {
    parse_length(value).is_some() || is_percentage(value)
}

/// A typed property where the value fits one, and the raw declaration
/// where it doesn't.
///
/// The fallback is not a concession -- it is what matching Tailwind
/// requires. Tailwind validates arbitrary values not at all: `w-[nonsense]`
/// emits `width: nonsense` and lets the browser discard it. Refusing here
/// would mean Dowel generated *less* than the engine it is checked
/// against, and the class would look broken for the wrong reason.
/// A border width for one side, typed where the value is a length.
///
/// The untyped case is a real one: `border-[1.5]` is a valid width in CSS
/// and not a length, so it goes through as the declaration it is.
fn border_width(property: &str, value: &str) -> StyleProperty {
    match parse_length(value) {
        Some(l) => match property {
            "border-top-width" => StyleProperty::BorderTopWidth(l),
            "border-right-width" => StyleProperty::BorderRightWidth(l),
            "border-bottom-width" => StyleProperty::BorderBottomWidth(l),
            _ => StyleProperty::BorderLeftWidth(l),
        },
        None => StyleProperty::Arbitrary(property.to_string(), value.to_string()),
    }
}

fn or_raw(property: &str, value: &str, typed: Option<StyleProperty>) -> Vec<StyleProperty> {
    match typed {
        Some(property) => vec![property],
        None => vec![StyleProperty::Arbitrary(property.to_string(), value.to_string())],
    }
}

fn from_prefix(prefix: &str, value: &str, hint: Option<&str>) -> Option<Vec<StyleProperty>> {
    let length = || parse_length(value);
    let dimension = || parse_dimension(value);
    let color = || Color::Css(value.to_string());
    let number = || value.parse::<f64>().ok();
    let integer = || value.parse::<i32>().ok();
    let one = |property: StyleProperty| Some(vec![property]);

    // The families where one prefix means one property and the value is a
    // size. Grouped rather than spelled out because the mapping is
    // mechanical: every one of them takes the same value vocabulary.
    if let Some(properties) = sizing(prefix, value) {
        return Some(properties);
    }
    if let Some(properties) = spacing(prefix, value) {
        return Some(properties);
    }

    match prefix {
        // The ambiguous one. `text-[14px]` is a font size and
        // `text-[#fff]` is a colour, and before this existed both compiled
        // to a colour -- so a font size became `color: var(--dowel-color-[14px])`.
        "text" if hint != Some("color") && is_length_percentage(value) => Some(or_raw(
            "font-size",
            value,
            length().map(StyleProperty::FontSize),
        )),
        "text" => one(StyleProperty::TextColor(color())),
        // A background is an image, a position, a size or a colour, and
        // only the value says which. The length case is the surprising
        // one: a bare `bg-[10px]` is a background *position*, not a size
        // and not a colour -- so the obvious reading (colour, like
        // `bg-red-500`) is wrong for every numeric value.
        "bg" => {
            if matches!(hint, Some("image") | Some("url")) || value.starts_with("url(") {
                one(StyleProperty::Arbitrary("background-image".to_string(), value.to_string()))
            } else if matches!(hint, Some("bg-size") | Some("length")) || value == "auto" {
                one(StyleProperty::Arbitrary("background-size".to_string(), value.to_string()))
            } else if hint == Some("position") || length().is_some() || value.ends_with('%') {
                one(StyleProperty::Arbitrary("background-position".to_string(), value.to_string()))
            } else {
                one(StyleProperty::BackgroundColor(color()))
            }
        }
        // A border is a width or a colour, decided the same way. Tailwind
        // also emits `border-style: var(--tw-border-style)` alongside a
        // width; Dowel resolves that at compile time instead, so the style
        // is set explicitly here.
        // A border with a width and no style draws nothing, so Tailwind
        // sets `solid` alongside every width. Dowel resolves that at
        // compile time rather than through `--tw-border-style`, which
        // means writing it here -- omitting it left every arbitrary border
        // width invisible in the browser while looking correct in the CSS.
        "border" if hint != Some("color") && is_line_width(value) => Some(vec![
            StyleProperty::BorderTopStyle(BorderStyle::Solid),
            StyleProperty::BorderRightStyle(BorderStyle::Solid),
            StyleProperty::BorderBottomStyle(BorderStyle::Solid),
            StyleProperty::BorderLeftStyle(BorderStyle::Solid),
            border_width("border-top-width", value),
            border_width("border-right-width", value),
            border_width("border-bottom-width", value),
            border_width("border-left-width", value),
        ]),
        "border" => one(StyleProperty::BorderColor(color())),
        "border-t" | "border-r" | "border-b" | "border-l"
            if hint != Some("color") && is_line_width(value) =>
        {
            let (style, width) = match prefix {
                "border-t" => (StyleProperty::BorderTopStyle(BorderStyle::Solid), "border-top-width"),
                "border-r" => {
                    (StyleProperty::BorderRightStyle(BorderStyle::Solid), "border-right-width")
                }
                "border-b" => {
                    (StyleProperty::BorderBottomStyle(BorderStyle::Solid), "border-bottom-width")
                }
                _ => (StyleProperty::BorderLeftStyle(BorderStyle::Solid), "border-left-width"),
            };
            Some(vec![style, border_width(width, value)])
        }
        "border-t" => one(StyleProperty::BorderTopColor(color())),
        "border-r" => one(StyleProperty::BorderRightColor(color())),
        "border-b" => one(StyleProperty::BorderBottomColor(color())),
        "border-l" => one(StyleProperty::BorderLeftColor(color())),
        // The axis forms are logical properties, not pairs of physical
        // ones -- `border-inline-width`, which follows writing direction,
        // rather than a left and a right that don't.
        "border-x" | "border-y" if hint != Some("color") && is_line_width(value) => {
            let edge = if prefix == "border-x" { Edge::Inline } else { Edge::Block };
            Some(vec![
                StyleProperty::BorderLogicalStyle(edge, BorderStyle::Solid),
                match length() {
                    Some(l) => StyleProperty::BorderLogicalWidth(edge, l),
                    None => StyleProperty::Arbitrary(
                        format!("border-{}-width", if edge == Edge::Inline { "inline" } else { "block" }),
                        value.to_string(),
                    ),
                },
            ])
        }
        "border-x" => one(StyleProperty::BorderInlineColor(color())),
        "border-y" => one(StyleProperty::BorderBlockColor(color())),

        // Colour-only families.
        // An outline width takes the `line-width` type, which allows a
        // bare number as well as a length -- so `outline-[1.5]` is a width
        // and not, as the colour fallback would have it, a colour. The
        // style comes with it: an outline with a width and no style draws
        // nothing, and Tailwind sets `solid` for exactly that reason.
        // And an outline width takes a percentage where a border width
        // does not. Not a rule, just what the engine does.
        "outline"
            if hint != Some("color") && (is_line_width(value) || is_percentage(value)) =>
        {
            Some(vec![
                StyleProperty::OutlineStyle(BorderStyle::Solid),
                match length() {
                    Some(l) => StyleProperty::OutlineWidth(l),
                    None => {
                        StyleProperty::Arbitrary("outline-width".to_string(), value.to_string())
                    }
                },
            ])
        }
        "outline" => one(StyleProperty::OutlineColor(color())),
        "ring" if is_color(value, hint) => one(StyleProperty::RingColor(color())),
        "ring" => one(StyleProperty::RingWidth(length()?)),
        "placeholder" => one(StyleProperty::PlaceholderColor(color())),
        "accent" => one(StyleProperty::AccentColor(color())),
        "caret" => one(StyleProperty::CaretColor(color())),
        "fill" => one(StyleProperty::Fill(color())),
        // A stroke width is the widest of the three: length, number *and*
        // percentage all count.
        "stroke" if hint != Some("color") && (is_line_width(value) || is_percentage(value)) => {
            Some(or_raw("stroke-width", value, number().map(StyleProperty::StrokeWidth)))
        }
        "stroke" => one(StyleProperty::Stroke(color())),
        "divide" => one(StyleProperty::DivideColor(color())),
        "decoration" if hint == Some("color") || !is_length_percentage(value) => {
            one(StyleProperty::TextDecorationColor(color()))
        }
        "decoration" => Some(or_raw(
            "text-decoration-thickness",
            value,
            length().map(StyleProperty::TextDecorationThickness),
        )),

        // Typography.
        "leading" => Some(or_raw(
            "line-height",
            value,
            match length() {
                Some(l) => Some(StyleProperty::LineHeight(LineHeight::Length(l))),
                // Unitless, which CSS reads as a multiple of the font size.
                None => number().map(|n| StyleProperty::LineHeight(LineHeight::Ratio(n))),
            },
        )),
        "tracking" => Some(or_raw(
            "letter-spacing",
            value,
            length().map(|l| StyleProperty::LetterSpacing(LetterSpacing::Px(l))),
        )),
        "indent" => one(StyleProperty::TextIndent(dimension())),
        "underline-offset" => one(StyleProperty::TextUnderlineOffset(dimension())),

        // Corners.
        "rounded" => Some(or_raw(
            "border-radius",
            value,
            length().map(|l| StyleProperty::BorderRadius(Radius::Length(l))),
        )),
        "rounded-t" => Some(vec![
            StyleProperty::BorderTopLeftRadius(Radius::Length(length()?)),
            StyleProperty::BorderTopRightRadius(Radius::Length(length()?)),
        ]),
        "rounded-b" => Some(vec![
            StyleProperty::BorderBottomLeftRadius(Radius::Length(length()?)),
            StyleProperty::BorderBottomRightRadius(Radius::Length(length()?)),
        ]),
        "rounded-l" => Some(vec![
            StyleProperty::BorderTopLeftRadius(Radius::Length(length()?)),
            StyleProperty::BorderBottomLeftRadius(Radius::Length(length()?)),
        ]),
        "rounded-r" => Some(vec![
            StyleProperty::BorderTopRightRadius(Radius::Length(length()?)),
            StyleProperty::BorderBottomRightRadius(Radius::Length(length()?)),
        ]),
        "rounded-tl" => one(StyleProperty::BorderTopLeftRadius(Radius::Length(length()?))),
        "rounded-tr" => one(StyleProperty::BorderTopRightRadius(Radius::Length(length()?))),
        "rounded-bl" => one(StyleProperty::BorderBottomLeftRadius(Radius::Length(length()?))),
        "rounded-br" => one(StyleProperty::BorderBottomRightRadius(Radius::Length(length()?))),

        // Plain numbers, each with the same escape: Tailwind emits
        // `z-index: abc` for `z-[abc]` rather than nothing, so refusing
        // would generate less than the engine being matched against.
        "opacity" => Some(or_raw("opacity", value, number().map(StyleProperty::Opacity))),
        // Integers only. `ZIndex` and `Order` hold an `i32`, so `z-[1.5]`
        // went in as `1` -- a plausible number that isn't the one written,
        // which is the failure mode this whole module exists to stop.
        "z" => Some(or_raw("z-index", value, integer().map(|n| StyleProperty::ZIndex(Some(n))))),
        "order" => Some(or_raw("order", value, integer().map(StyleProperty::Order))),
        "grow" => Some(or_raw("flex-grow", value, number().map(StyleProperty::FlexGrow))),
        "shrink" => Some(or_raw("flex-shrink", value, number().map(StyleProperty::FlexShrink))),
        "aspect" => one(StyleProperty::Arbitrary("aspect-ratio".to_string(), value.to_string())),
        "outline-offset" => Some(or_raw(
            "outline-offset",
            value,
            length().map(StyleProperty::OutlineOffset),
        )),

        // Track lists, which the named scale can't express at all: it
        // counts equal columns, and the interesting layouts are unequal.
        "grid-cols" => one(StyleProperty::GridTemplateColumns(GridTracks::Css(value.to_string()))),
        "grid-rows" => one(StyleProperty::GridTemplateRows(GridTracks::Css(value.to_string()))),

        // Two-axis families: one written value, two written out. CSS
        // defaults the second axis to the first for `translate` but *not*
        // for `border-spacing`, and Tailwind writes both in every case, so
        // matching means writing both here too.
        "translate" => one(StyleProperty::Arbitrary(
            "translate".to_string(),
            format!("{value} {value}"),
        )),
        "border-spacing" => one(StyleProperty::Arbitrary(
            "border-spacing".to_string(),
            format!("{value} {value}"),
        )),
        "skew" => one(StyleProperty::Arbitrary(
            "transform".to_string(),
            format!("skewX({value}) skewY({value})"),
        )),
        // A `font-[...]` is a weight when it reads as one and a family
        // otherwise -- the same value-shaped decision `text-` makes, with
        // the roles of the two answers swapped.
        "font" if is_line_width(value) || is_percentage(value) => {
            one(StyleProperty::Arbitrary("font-weight".to_string(), value.to_string()))
        }
        // Not just the line count: a clamp needs `display: -webkit-box`
        // and `-webkit-box-orient: vertical` to do anything at all, which
        // is why this is a variant rather than a pass-through.
        // Integers only. A fractional clamp is not a line count, and
        // `as u32` turned `line-clamp-[1.5]` into a clamp of one -- a
        // number that looks deliberate and isn't the one written. Refused
        // rather than rounded: emitting nothing is visible, emitting the
        // wrong count is not.
        "line-clamp" => one(StyleProperty::LineClamp(Some(
            value.parse::<u32>().ok()?,
        ))),

        // The long tail, where the prefix names one CSS property and the
        // value goes through untouched. Everything here is Web-only by
        // construction: `StyleProperty::Arbitrary` is refused on Native,
        // which is right for most of these (there is no `will-change` or
        // `perspective-origin` on a device) and a real gap for the few
        // React Native does have. Those are worth promoting to typed
        // variants one at a time; guessing a camelCase spelling for all of
        // them is how a build error becomes a style that silently does
        // nothing.
        _ => PASSTHROUGH
            .iter()
            .find(|(name, _)| *name == prefix)
            .map(|(_, property)| {
                vec![StyleProperty::Arbitrary(property.to_string(), value.to_string())]
            }),
    }
}

/// Prefixes that name one CSS property and pass their value straight
/// through.
///
/// Each pair is checked against Tailwind's own output by the arbitrary
/// conformance report, which is why the list can be this blunt: a wrong
/// property name here is not a silent gap, it is an immediate mismatch
/// against the reference engine.
const PASSTHROUGH: &[(&str, &str)] = &[
    ("align", "vertical-align"),
    ("animate", "animation"),
    ("auto-cols", "grid-auto-columns"),
    ("auto-rows", "grid-auto-rows"),
    ("basis", "flex-basis"),
    ("border-spacing", "border-spacing"),
    ("col", "grid-column"),
    ("col-end", "grid-column-end"),
    ("col-start", "grid-column-start"),
    ("columns", "columns"),
    ("contain", "contain"),
    ("content", "content"),
    ("cursor", "cursor"),
    ("delay", "transition-delay"),
    ("duration", "transition-duration"),
    ("ease", "transition-timing-function"),
    ("flex", "flex"),
    ("font", "font-family"),
    ("font-stretch", "font-stretch"),
    ("inset-be", "inset-block-end"),
    ("inset-bs", "inset-block-start"),
    ("inset-e", "inset-inline-end"),
    ("inset-s", "inset-inline-start"),
    ("line-clamp", "-webkit-line-clamp"),
    ("list", "list-style-type"),
    ("list-image", "list-style-image"),
    ("max-block", "max-block-size"),
    ("max-inline", "max-inline-size"),
    ("mbe", "margin-block-end"),
    ("mbs", "margin-block-start"),
    ("min-block", "min-block-size"),
    ("min-inline", "min-inline-size"),
    ("object", "object-position"),
    ("origin", "transform-origin"),
    ("pbe", "padding-block-end"),
    ("pbs", "padding-block-start"),
    ("perspective", "perspective"),
    ("perspective-origin", "perspective-origin"),
    ("rotate", "rotate"),
    ("row", "grid-row"),
    ("row-end", "grid-row-end"),
    ("row-start", "grid-row-start"),
    ("scale", "scale"),
    ("skew", "transform"),
    ("tab", "tab-size"),
    ("transform", "transform"),
    ("translate", "translate"),
    ("will-change", "will-change"),
    ("zoom", "zoom"),
];

/// The size families: one prefix, one property, a `Dimension` value.
fn sizing(prefix: &str, value: &str) -> Option<Vec<StyleProperty>> {
    let d = parse_dimension(value);
    let property = match prefix {
        "w" => StyleProperty::Width(d),
        "h" => StyleProperty::Height(d),
        "min-w" => StyleProperty::MinWidth(d),
        "min-h" => StyleProperty::MinHeight(d),
        "max-w" => StyleProperty::MaxWidth(d),
        "max-h" => StyleProperty::MaxHeight(d),
        "basis" => StyleProperty::FlexBasis(d),
        "top" => StyleProperty::InsetTop(d),
        "right" => StyleProperty::InsetRight(d),
        "bottom" => StyleProperty::InsetBottom(d),
        "left" => StyleProperty::InsetLeft(d),
        "start" => StyleProperty::InsetInlineStart(d),
        "end" => StyleProperty::InsetInlineEnd(d),
        "translate-x" => StyleProperty::TranslateX(d),
        "translate-y" => StyleProperty::TranslateY(d),
        "size" => {
            return Some(vec![StyleProperty::Width(d.clone()), StyleProperty::Height(d)]);
        }
        // The axis forms are single logical properties, not pairs of
        // physical ones. Emitting `left`+`right` for `inset-x-*` rendered
        // identically and still disagreed with Tailwind on every candidate
        // -- and the disagreement is real, not cosmetic: `inset-inline`
        // follows writing direction and `left`/`right` don't.
        "inset-x" => return Some(vec![StyleProperty::InsetInline(d)]),
        "inset-y" => return Some(vec![StyleProperty::InsetBlock(d)]),
        "inset" => {
            // A value with a space in it is several values -- `inset` is a
            // shorthand and takes up to four. Splitting it across the four
            // longhands would give each edge the whole list.
            if value.contains(' ') {
                return Some(vec![StyleProperty::Arbitrary(
                    "inset".to_string(),
                    value.to_string(),
                )]);
            }
            return Some(vec![
                StyleProperty::InsetTop(d.clone()),
                StyleProperty::InsetRight(d.clone()),
                StyleProperty::InsetBottom(d.clone()),
                StyleProperty::InsetLeft(d),
            ]);
        }
        _ => return None,
    };
    Some(vec![property])
}

/// The padding/margin/gap families, which take a bare `Length`.
fn spacing(prefix: &str, value: &str) -> Option<Vec<StyleProperty>> {
    // Margins take `auto`, so they are dimensions; paddings and gaps are
    // lengths. The split follows CSS rather than convenience.
    // A margin shorthand takes up to four values, so a value with a space
    // in it cannot be distributed across the longhands -- each edge would
    // get the whole list.
    if value.contains(' ') {
        let shorthand = match prefix {
            "m" => "margin",
            "mx" => "margin-inline",
            "my" => "margin-block",
            "p" => "padding",
            "px" => "padding-inline",
            "py" => "padding-block",
            _ => "",
        };
        if !shorthand.is_empty() {
            return Some(vec![StyleProperty::Arbitrary(
                shorthand.to_string(),
                value.to_string(),
            )]);
        }
    }

    let margin = |d: Dimension| -> Option<Vec<StyleProperty>> {
        Some(match prefix {
            "m" => vec![
                StyleProperty::MarginTop(d.clone()),
                StyleProperty::MarginRight(d.clone()),
                StyleProperty::MarginBottom(d.clone()),
                StyleProperty::MarginLeft(d),
            ],
            "mx" => vec![
                StyleProperty::MarginInlineStart(d.clone()),
                StyleProperty::MarginInlineEnd(d),
            ],
            "my" => vec![StyleProperty::MarginTop(d.clone()), StyleProperty::MarginBottom(d)],
            "mt" => vec![StyleProperty::MarginTop(d)],
            "mr" => vec![StyleProperty::MarginRight(d)],
            "mb" => vec![StyleProperty::MarginBottom(d)],
            "ml" => vec![StyleProperty::MarginLeft(d)],
            "ms" => vec![StyleProperty::MarginInlineStart(d)],
            "me" => vec![StyleProperty::MarginInlineEnd(d)],
            _ => return None,
        })
    };
    if let Some(properties) = margin(parse_dimension(value)) {
        return Some(properties);
    }

    // The CSS property each prefix writes, so that a value too odd for
    // `Length` can still be emitted as the declaration Tailwind emits.
    // `Length` has no text variant on purpose -- it is `Copy` and sits in
    // dozens of properties -- so the escape lives here instead.
    let css_property = match prefix {
        "p" => "padding",
        "px" => "padding-inline",
        "py" => "padding-block",
        "pt" => "padding-top",
        "pr" => "padding-right",
        "pb" => "padding-bottom",
        "pl" => "padding-left",
        "ps" => "padding-inline-start",
        "pe" => "padding-inline-end",
        "gap" => "gap",
        "gap-x" => "column-gap",
        "gap-y" => "row-gap",
        // `space-*`/`divide-*` style the element's children through a
        // separate rule, so there is no single declaration to fall back
        // to. A value they can't read is refused instead.
        "space-x" | "space-y" | "divide-x" | "divide-y" => "",
        _ => return None,
    };

    let Some(l) = parse_length(value) else {
        if css_property.is_empty() {
            return None;
        }
        return Some(vec![StyleProperty::Arbitrary(
            css_property.to_string(),
            value.to_string(),
        )]);
    };

    Some(match prefix {
        "p" => vec![
            StyleProperty::PaddingTop(l),
            StyleProperty::PaddingRight(l),
            StyleProperty::PaddingBottom(l),
            StyleProperty::PaddingLeft(l),
        ],
        "px" => vec![StyleProperty::PaddingInlineStart(l), StyleProperty::PaddingInlineEnd(l)],
        "py" => vec![StyleProperty::PaddingTop(l), StyleProperty::PaddingBottom(l)],
        "pt" => vec![StyleProperty::PaddingTop(l)],
        "pr" => vec![StyleProperty::PaddingRight(l)],
        "pb" => vec![StyleProperty::PaddingBottom(l)],
        "pl" => vec![StyleProperty::PaddingLeft(l)],
        "ps" => vec![StyleProperty::PaddingInlineStart(l)],
        "pe" => vec![StyleProperty::PaddingInlineEnd(l)],
        "gap" => vec![StyleProperty::Gap(l)],
        "gap-x" => vec![StyleProperty::ColumnGap(l)],
        "gap-y" => vec![StyleProperty::RowGap(l)],
        "space-x" => vec![StyleProperty::SpaceX(l)],
        "space-y" => vec![StyleProperty::SpaceY(l)],
        "divide-x" => vec![StyleProperty::DivideX(l)],
        "divide-y" => vec![StyleProperty::DivideY(l)],
        _ => return None,
    })
}

#[cfg(test)]
mod split_tests {
    use super::*;

    #[test]
    fn a_prefix_and_a_bracketed_value() {
        let parsed = split("w-[32px]").unwrap();
        assert_eq!(parsed.prefix, "w");
        assert_eq!(parsed.value, "32px");
        assert_eq!(parsed.hint, None);
    }

    #[test]
    fn the_var_shorthand_desugars_to_a_var_call() {
        let parsed = split("bg-(--brand)").unwrap();
        assert_eq!(parsed.prefix, "bg");
        assert_eq!(parsed.value, "var(--brand)");
    }

    #[test]
    fn a_hint_is_read_only_when_it_is_one() {
        assert_eq!(split("text-[length:14px]").unwrap().hint, Some("length"));
        // The colon here belongs to a URL. Reading `url(https` as a hint
        // would leave `//x)` as the value -- a broken declaration that
        // still looks like it parsed.
        let parsed = split("bg-[url(https://x)]").unwrap();
        assert_eq!(parsed.hint, None);
        assert_eq!(parsed.value, "url(https://x)");
    }

    #[test]
    fn a_whole_class_can_be_a_declaration() {
        assert_eq!(
            split_property("[color:red]"),
            Some(("color".to_string(), "red".to_string()))
        );
        assert_eq!(
            split_property("[--my-var:4px]"),
            Some(("--my-var".to_string(), "4px".to_string()))
        );
    }

    #[test]
    fn a_variant_splits_on_its_own_bracket_not_the_first_colon() {
        assert_eq!(split_variant("[&>*]:p-4"), Some(("&>*".to_string(), "p-4")));
        // The failure this prevents: cutting at the first colon yields the
        // selector `[&` and the utility `hover]:p-4`, both nonsense, and
        // neither obviously so.
        assert_eq!(split_variant("[&:hover]:p-4"), Some(("&:hover".to_string(), "p-4")));
    }

    #[test]
    fn a_size_and_a_colour_under_the_same_prefix_reach_different_properties() {
        // The bug this module was written for. Both are `text-[...]`, and
        // one of them used to compile to `color: var(--dowel-color-[14px])`.
        assert_eq!(
            properties("text-[14px]"),
            Some(vec![StyleProperty::FontSize(Length::Px(14.0))])
        );
        assert_eq!(
            properties("text-[#fff]"),
            Some(vec![StyleProperty::TextColor(Color::Css("#fff".to_string()))])
        );
    }

    #[test]
    fn a_length_keeps_its_unit_through_the_ir() {
        // Not converted to pixels at parse time: Web has to print `2rem`
        // to match Tailwind, and a reader who scaled up their font size
        // gets a different element from `32px`.
        assert_eq!(
            properties("mt-[2rem]"),
            Some(vec![StyleProperty::MarginTop(Dimension::Length(Length::Unit(
                2.0,
                LengthUnit::Rem
            )))])
        );
    }

    #[test]
    fn a_value_too_odd_to_model_still_reaches_the_output() {
        // Tailwind validates arbitrary values not at all, so refusing here
        // would generate less than the engine Dowel is measured against.
        assert_eq!(
            properties("p-[nonsense]"),
            Some(vec![StyleProperty::Arbitrary(
                "padding".to_string(),
                "nonsense".to_string()
            )])
        );
    }

    #[test]
    fn an_unreadable_bracket_is_refused_rather_than_guessed() {
        // No prefix table entry, so nothing is invented. The caller turns
        // this into `UnreadableArbitraryValue`.
        assert_eq!(properties("notautility-[10px]"), None);
    }

    #[test]
    fn an_underscore_in_a_variant_is_a_descendant_combinator() {
        assert_eq!(split_variant("[.dark_&]:text-white"), Some((".dark &".to_string(), "text-white")));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn underscores_become_spaces() {
        assert_eq!(normalize("1fr_auto"), "1fr auto");
        assert_eq!(normalize("0_0_4px_red"), "0 0 4px red");
    }

    #[test]
    fn an_escaped_underscore_stays_one() {
        assert_eq!(normalize("a\\_b"), "a_b");
    }

    #[test]
    fn a_url_keeps_its_underscores() {
        // The failure this prevents is a broken image path that looks like
        // a typo in the source rather than a compiler bug.
        assert_eq!(normalize("url(/a_b.png)"), "url(/a_b.png)");
    }

    #[test]
    fn math_operators_get_spaced() {
        assert_eq!(normalize("calc(100%-2rem)"), "calc(100% - 2rem)");
        assert_eq!(normalize("calc(100%*2)"), "calc(100% * 2)");
        assert_eq!(normalize("calc(100%/3)"), "calc(100% / 3)");
        assert_eq!(normalize("calc(1px+2px)"), "calc(1px + 2px)");
        assert_eq!(normalize("calc(var(--a)-var(--b))"), "calc(var(--a) - var(--b))");
    }

    #[test]
    fn already_spaced_math_is_left_alone() {
        assert_eq!(normalize("calc(100%_-_2rem)"), "calc(100% - 2rem)");
    }

    #[test]
    fn commas_space_out_inside_math_functions_only() {
        assert_eq!(normalize("min(100%,32rem)"), "min(100%, 32rem)");
        // `repeat` isn't a math function, so its commas stay tight -- and
        // Tailwind's output does too, which is the only reason this is a
        // rule rather than a preference.
        assert_eq!(normalize("repeat(3,minmax(0,1fr))"), "repeat(3,minmax(0,1fr))");
    }

    #[test]
    fn a_negative_number_is_not_a_subtraction() {
        assert_eq!(normalize("-4px"), "-4px");
        assert_eq!(normalize("calc(-4px)"), "calc(-4px)");
    }

    #[test]
    fn an_exponent_sign_is_not_an_operator() {
        assert_eq!(normalize("calc(1e-3px)"), "calc(1e-3px)");
    }

    #[test]
    fn lengths_keep_the_unit_they_were_written_in() {
        assert_eq!(parse_length("32px"), Some(Length::Px(32.0)));
        assert_eq!(parse_length("2rem"), Some(Length::Unit(2.0, LengthUnit::Rem)));
        assert_eq!(parse_length("1.5em"), Some(Length::Unit(1.5, LengthUnit::Em)));
        assert_eq!(parse_length("0"), Some(Length::Px(0.0)));
    }

    #[test]
    fn a_unitless_nonzero_is_not_a_length() {
        // `w-[32]` is not something Tailwind accepts, and accepting it here
        // would mean Dowel compiled a class the real engine drops.
        assert_eq!(parse_length("32"), None);
    }

    #[test]
    fn a_value_is_a_colour_unless_it_is_a_length() {
        // Tailwind's polarity, which is the opposite of the intuitive one:
        // the test is "is this a length?", and everything else -- including
        // outright nonsense -- is a colour. `text-[nonsense]` really does
        // compile to `color: nonsense` in Tailwind, and matching that is
        // the requirement.
        assert!(!is_color("14px", None));
        assert!(!is_color("2rem", None));
        assert!(is_color("#fff", None));
        assert!(is_color("rgb(0 0 0)", None));
        assert!(is_color("nonsense", None));
    }

    #[test]
    fn a_hint_overrides_the_shape() {
        // The one case shape cannot decide: identical text, two meanings.
        assert!(is_color("var(--c)", Some("color")));
        assert!(!is_color("var(--c)", Some("length")));
    }
}
