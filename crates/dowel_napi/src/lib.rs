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
    }
}

fn to_js_diagnostic(diagnostic: Diagnostic) -> CompileDiagnostic {
    CompileDiagnostic {
        code: diagnostic_code_str(diagnostic.code).to_string(),
        severity: match diagnostic.severity {
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

/// Parses `source` as TSX and lowers every top-level JSX element found (one
/// per component's returned JSX, see `dowel_parser::parse_tsx`) to Web
/// output. Returns one `CompiledComponent` per root found, in source order.
#[napi]
pub fn compile(source: String) -> Vec<CompiledComponent> {
    let parsed = dowel_parser::parse_tsx(&source);
    parsed
        .roots
        .iter()
        .map(|root| {
            let output = dowel_web::lower(root);
            CompiledComponent {
                jsx: output.jsx,
                css: output.css,
                diagnostics: output.diagnostics.into_iter().map(to_js_diagnostic).collect(),
                span_start: root.span.start,
                span_end: root.span.end,
            }
        })
        .collect()
}
