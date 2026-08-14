//! napi-rs bindings exposing the Dowel compiler to Node-based build tooling.
//!
//! First pass: proves the Rust<->JS bridge itself works (the actual
//! foundational bet from the design discussion, untested until now) by
//! exposing the same `dowel_parser::parse_tsx` -> `dowel_web::lower`
//! pipeline already validated in `dowel_web`'s tests/example, as a
//! synchronous Node-callable function. This is not yet the shape
//! `@dowel/vite-plugin` will actually want (full rendered HTML rather than
//! source-rewrite instructions) -- that comes once the plugin itself is
//! being wired up and its real requirements are known.

use dowel_ir::{Diagnostic, DiagnosticCode, Severity};
use napi_derive::napi;

#[napi(object)]
pub struct CompileDiagnostic {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub span_start: u32,
    pub span_end: u32,
}

fn diagnostic_code_str(code: DiagnosticCode) -> &'static str {
    match code {
        DiagnosticCode::A11yInteractiveWithoutRole => "A11Y_INTERACTIVE_WITHOUT_ROLE",
        DiagnosticCode::UnsafePropSpreadAfterStyle => "UNSAFE_PROP_SPREAD_AFTER_STYLE",
        DiagnosticCode::WebOnlyPropertyOnNative => "WEB_ONLY_PROPERTY_ON_NATIVE",
        DiagnosticCode::DynamicClassNameNotResolved => "DYNAMIC_CLASS_NAME_NOT_RESOLVED",
    }
}

fn to_js_diagnostic(diagnostic: Diagnostic) -> CompileDiagnostic {
    CompileDiagnostic {
        code: diagnostic_code_str(diagnostic.code).to_string(),
        severity: match diagnostic.severity {
            // Build-stopping; callers are expected to fail on this rather
            // than print it (see @dowel/metro-transformer).
            Severity::Error => "error".to_string(),
            Severity::Warning => "warning".to_string(),
            Severity::Info => "info".to_string(),
        },
        message: diagnostic.message,
        span_start: diagnostic.span.start,
        span_end: diagnostic.span.end,
    }
}

#[napi(object)]
pub struct CompiledComponent {
    /// Compiled JSX to splice into the original source in place of the
    /// text at `[span_start, span_end)` -- callers (the Vite plugin) own
    /// the actual splicing, since this binding doesn't touch source text.
    pub jsx: String,
    pub css: String,
    pub diagnostics: Vec<CompileDiagnostic>,
    pub span_start: u32,
    pub span_end: u32,
}

/// Parser diagnostics are file-level (they're about the source as written,
/// not about any one lowering), but this binding reports per-component --
/// so each is attributed to whichever root's span contains it.
fn parser_diagnostics_for(
    parsed: &dowel_parser::ParseOutput,
    root: &dowel_ir::Node,
) -> Vec<CompileDiagnostic> {
    parsed
        .diagnostics
        .iter()
        .filter(|d| d.span.start >= root.span.start && d.span.end <= root.span.end)
        .cloned()
        .map(to_js_diagnostic)
        .collect()
}

/// Parses `source` as TSX and lowers every top-level JSX element found (one
/// per component's returned JSX, see `dowel_parser::parse_tsx`) to Web
/// output. Returns one `CompiledComponent` per root found, in source order.
#[napi]
pub fn compile(source: String) -> Vec<CompiledComponent> {
    let parsed = dowel_parser::parse_tsx(&source);
    // Candidate classes for the fallback path. Scanned once per file, and
    // only actually emitted by `lower` when some className in that root
    // couldn't be resolved statically.
    let scanned = dowel_parser::scan_class_candidates(&source);
    parsed
        .roots
        .iter()
        .map(|root| {
            let output = dowel_web::lower(root, &source, &scanned);
            let mut diagnostics = parser_diagnostics_for(&parsed, root);
            diagnostics.extend(output.diagnostics.into_iter().map(to_js_diagnostic));
            CompiledComponent {
                jsx: output.jsx,
                css: output.css,
                diagnostics,
                span_start: root.span.start,
                span_end: root.span.end,
            }
        })
        .collect()
}

#[napi(object)]
pub struct CompiledNativeComponent {
    /// Compiled JSX to splice into the original source, same convention as
    /// `CompiledComponent.jsx`.
    pub jsx: String,
    /// `StyleSheet.create({ ... })`-ready object literal text (without the
    /// wrapper -- see `dowel_native::LowerOutput`).
    pub styles: String,
    pub diagnostics: Vec<CompileDiagnostic>,
    pub span_start: u32,
    pub span_end: u32,
}

/// Same shape as `compile`, but lowers to React Native (Pressable/View/Text
/// + a StyleSheet object) instead of DOM/CSS. See `dowel_native`'s module
/// docs for the current Phase 0 scope/limitations (non-Always conditions
/// aren't wired into the rendered `style` prop yet).
#[napi]
pub fn compile_native(source: String) -> Vec<CompiledNativeComponent> {
    let parsed = dowel_parser::parse_tsx(&source);
    parsed
        .roots
        .iter()
        .map(|root| {
            let output = dowel_native::lower(root, &source);
            let mut diagnostics = parser_diagnostics_for(&parsed, root);
            diagnostics.extend(output.diagnostics.into_iter().map(to_js_diagnostic));
            CompiledNativeComponent {
                jsx: output.jsx,
                styles: output.styles,
                diagnostics,
                span_start: root.span.start,
                span_end: root.span.end,
            }
        })
        .collect()
}
