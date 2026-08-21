//! napi-rs bindings exposing the Hozo compiler to Node-based build tooling.
//!
//! First pass: proves the Rust<->JS bridge itself works (the actual
//! foundational bet from the design discussion, untested until now) by
//! exposing the same `hozo_parser::parse_tsx` -> `hozo_web::lower`
//! pipeline already validated in `hozo_web`'s tests/example, as a
//! synchronous Node-callable function. This is not yet the shape
//! `@hozo/vite` will actually want (full rendered HTML rather than
//! source-rewrite instructions) -- that comes once the plugin itself is
//! being wired up and its real requirements are known.

use hozo_ir::{Diagnostic, DiagnosticCode, Severity};
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
        DiagnosticCode::RoleHasNoWebEquivalent => "ROLE_HAS_NO_WEB_EQUIVALENT",
        DiagnosticCode::AriaIncompletePattern => "ARIA_INCOMPLETE_PATTERN",
        DiagnosticCode::AriaPropNotAllowed => "ARIA_PROP_NOT_ALLOWED",
        DiagnosticCode::AriaNameProhibited => "ARIA_NAME_PROHIBITED",
        DiagnosticCode::FocusableDisabledUnsupported => "FOCUSABLE_DISABLED_UNSUPPORTED",
        DiagnosticCode::TailwindVariantNotSupported => "TAILWIND_VARIANT_NOT_SUPPORTED",
        DiagnosticCode::A11yMissingAccessibleName => "A11Y_MISSING_ACCESSIBLE_NAME",
        DiagnosticCode::A11yDialogWithoutDismiss => "A11Y_DIALOG_WITHOUT_DISMISS",
        DiagnosticCode::InvalidSemanticNesting => "INVALID_SEMANTIC_NESTING",
        DiagnosticCode::UnsafePropSpreadAfterStyle => "UNSAFE_PROP_SPREAD_AFTER_STYLE",
        DiagnosticCode::WebOnlyPropertyOnNative => "WEB_ONLY_PROPERTY_ON_NATIVE",
        DiagnosticCode::DynamicClassNameNotResolved => "DYNAMIC_CLASS_NAME_NOT_RESOLVED",
        DiagnosticCode::NotWiredOnNative => "NOT_WIRED_ON_NATIVE",
        DiagnosticCode::PrimitiveNotLowered => "PRIMITIVE_NOT_LOWERED",
        DiagnosticCode::UnreadableArbitraryValue => "UNREADABLE_ARBITRARY_VALUE",
    }
}

fn to_js_diagnostic(diagnostic: Diagnostic) -> CompileDiagnostic {
    CompileDiagnostic {
        code: diagnostic_code_str(diagnostic.code).to_string(),
        severity: match diagnostic.severity {
            // Build-stopping; callers are expected to fail on this rather
            // than print it (see @hozo/metro).
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
    /// Named imports `jsx` needs from `@hozo/runtime`, which the caller
    /// splices at the top of the module. Same contract as the Native
    /// backend's field of this name.
    pub runtime_imports: Vec<String>,
    pub diagnostics: Vec<CompileDiagnostic>,
    pub span_start: u32,
    pub span_end: u32,
}

/// Parser diagnostics are file-level (they're about the source as written,
/// not about any one lowering), but this binding reports per-component --
/// so each is attributed to whichever root's span contains it.
fn parser_diagnostics_for(
    parsed: &hozo_parser::ParseOutput,
    root: &hozo_ir::Node,
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
/// per component's returned JSX, see `hozo_parser::parse_tsx`) to Web
/// output. Returns one `CompiledComponent` per root found, in source order.
#[napi]
pub fn compile(
    source: String,
    theme: Option<JsTheme>,
    sources: Option<Vec<String>>,
) -> Vec<CompiledComponent> {
    let theme = to_theme(theme);
    let parsed = hozo_parser::parse_tsx_with(&source, sources.as_deref());
    parsed
        .roots
        .iter()
        .map(|root| {
            let output = hozo_web::lower(&root.node, &source, &theme);
            let mut diagnostics = parser_diagnostics_for(&parsed, &root.node);
            diagnostics.extend(output.diagnostics.into_iter().map(to_js_diagnostic));
            CompiledComponent {
                jsx: output.jsx,
                css: output.css,
                runtime_imports: output
                    .runtime_imports
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
                diagnostics,
                span_start: root.node.span.start,
                span_end: root.node.span.end,
            }
        })
        .collect()
}

/// The build-side half of proposal §7's third tier.
///
/// A `className` the compiler couldn't read is passed through to runtime,
/// so something has to have already emitted CSS for whatever string it
/// evaluates to. That set can't come from the file being transformed --
/// a class written in one module can be produced by an expression in
/// another -- so it's accumulated across the whole project here, and the
/// stylesheet is generated once from the union.
///
/// Lives on the JS side rather than inside `compile()` because the
/// bundler owns the project walk and knows about file deletions; this
/// only owns the scanning, the staleness rule, and persistence.
#[napi]
pub struct CandidateCache {
    inner: hozo_cache::CandidateCache,
}

#[napi]
impl CandidateCache {
    /// Opens the cache at `path` (JSON on disk today -- the format is the
    /// Rust side's business). Pass no path for a build that has nothing to
    /// resume from, e.g. a one-shot production build.
    #[napi(constructor)]
    pub fn new(path: Option<String>) -> Self {
        let store: Box<dyn hozo_cache::SnapshotStore> = match path {
            Some(path) => Box::new(hozo_cache::JsonFileStore::new(path)),
            None => Box::new(hozo_cache::MemoryStore::new()),
        };
        CandidateCache { inner: hozo_cache::CandidateCache::open(store) }
    }

    /// Whether `path` was already scanned at this exact `modifiedMs`, i.e.
    /// the caller can skip reading and scanning it entirely.
    #[napi]
    pub fn is_current(&self, path: String, modified_ms: f64) -> bool {
        self.inner.is_current(&path, modified_ms as u64)
    }

    /// Scans `source` and records the result under `path`.
    ///
    /// Returns whether the candidate set changed -- saving a file without
    /// touching its classes returns `false`, so callers can leave the
    /// generated stylesheet alone instead of rewriting identical bytes and
    /// triggering a pointless HMR round.
    #[napi]
    pub fn scan_file(&mut self, path: String, source: String, modified_ms: f64) -> bool {
        let class_names = hozo_parser::scan_class_candidates(&source);
        self.inner.record(&path, modified_ms as u64, class_names)
    }

    /// Drops a deleted file's contribution. Returns whether it was tracked.
    #[napi]
    pub fn forget(&mut self, path: String) -> bool {
        self.inner.forget(&path)
    }

    /// Reconciles the persistent cache with one complete project walk and
    /// returns how many files disappeared since the previous walk.
    #[napi]
    pub fn retain_files(&mut self, paths: Vec<String>) -> u32 {
        self.inner.retain_files(paths) as u32
    }

    /// The stylesheet for every candidate in the project, written under the
    /// classes' real Tailwind names so a runtime-produced string matches by
    /// itself -- no runtime resolution code involved.
    #[napi]
    pub fn render_css(&self, theme: Option<JsTheme>) -> String {
        hozo_web::render_candidate_stylesheet(&self.inner.union(), &to_theme(theme))
    }

    /// The React Native counterpart of `renderCss`: a JS module holding a
    /// class-name -> style-object map plus a resolver bound to it.
    ///
    /// A separate call rather than a second field on one result because a
    /// project builds for one platform at a time -- generating the module
    /// a Web build will never import would be wasted work.
    #[napi]
    pub fn render_native_module(&self, theme: Option<JsTheme>) -> String {
        hozo_native::render_candidate_module(&self.inner.union(), &to_theme(theme))
    }

    /// Number of files tracked.
    #[napi(getter)]
    pub fn size(&self) -> u32 {
        self.inner.len() as u32
    }

    /// Writes the cache back, if anything changed since it was opened.
    #[napi]
    pub fn persist(&mut self) -> napi::Result<()> {
        self.inner.persist().map_err(|err| napi::Error::from_reason(err.to_string()))
    }
}

#[napi(object)]
pub struct CompiledNativeComponent {
    /// Compiled JSX to splice into the original source, same convention as
    /// `CompiledComponent.jsx`.
    pub jsx: String,
    /// `StyleSheet.create({ ... })`-ready object literal text (without the
    /// wrapper -- see `hozo_native::LowerOutput`).
    pub styles: String,
    /// Statements to splice at `hook_slot` for `jsx` to work. Empty unless
    /// a condition needed a React hook.
    pub prelude: Vec<String>,
    /// Named imports `prelude` needs from `@hozo/runtime`.
    pub runtime_imports: Vec<String>,
    /// Byte offset just inside the enclosing function's `{`, the only safe
    /// place for `prelude`. `None` when this JSX isn't inside a function
    /// body a statement can go in -- module scope, or a concise arrow.
    pub hook_slot: Option<u32>,
    pub diagnostics: Vec<CompileDiagnostic>,
    pub span_start: u32,
    pub span_end: u32,
}

/// Same shape as `compile`, but lowers to React Native (Pressable/View/Text
/// + a StyleSheet object) instead of DOM/CSS. See `hozo_native`'s module
/// docs for the current Phase 0 scope/limitations (non-Always conditions
/// aren't wired into the rendered `style` prop yet).
#[napi]
pub fn compile_native(
    source: String,
    theme: Option<JsTheme>,
    sources: Option<Vec<String>>,
) -> Vec<CompiledNativeComponent> {
    let theme = to_theme(theme);
    let parsed = hozo_parser::parse_tsx_with(&source, sources.as_deref());
    parsed
        .roots
        .iter()
        .map(|root| {
            let output = hozo_native::lower(&root.node, &source, &theme);
            let mut diagnostics = parser_diagnostics_for(&parsed, &root.node);
            diagnostics.extend(output.diagnostics.into_iter().map(to_js_diagnostic));
            CompiledNativeComponent {
                jsx: output.jsx,
                styles: output.styles,
                prelude: output.prelude,
                runtime_imports: output
                    .runtime_imports
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
                hook_slot: root.hook_slot,
                diagnostics,
                span_start: root.node.span.start,
                span_end: root.node.span.end,
            }
        })
        .collect()
}

/// A project's design tokens, as `@hozo/tailwind` extracts them.
///
/// Colours only for now. The rest of the scales -- spacing, containers,
/// fonts, breakpoints -- are resolved at parse time rather than at
/// lowering, so they need the theme threaded through a different path and
/// are a separate piece of work. Colours are the one that actually breaks
/// a project today: a `--color-brand` in a `@theme` compiled to a CSS
/// variable nothing defined.
#[napi(object)]
pub struct JsTheme {
    /// One spacing step in pixels (Tailwind's `--spacing`, 0.25rem by
    /// default). Absent means the default, which is what every project
    /// that does not change it has.
    pub spacing_px: Option<f64>,
    /// Token name (`"brand"`, `"blue-500"`) to its two spellings. Web takes
    /// the `oklch`, React Native the `hex`, which is why both are carried
    /// rather than converting at the boundary.
    pub colors: Vec<JsThemeColor>,
}

#[napi(object)]
pub struct JsThemeColor {
    pub token: String,
    pub oklch: String,
    pub hex: String,
}

fn to_theme(theme: Option<JsTheme>) -> hozo_ir::Theme {
    let Some(theme) = theme else {
        return hozo_ir::Theme::default();
    };
    let spacing_px = theme.spacing_px;
    hozo_ir::Theme::new(
        theme
            .colors
            .into_iter()
            .map(|color| {
                (color.token, hozo_ir::ThemeColor { oklch: color.oklch, hex: color.hex })
            })
            .collect(),
        spacing_px,
    )
}

/// Every binding a source file imports from one module, by local name.
///
/// The Native backend prepends `import { View, StyleSheet } from
/// 'react-native'`, and a React Native file already has its own -- so it
/// needs to know which names are taken before adding to them.
#[napi]
pub fn module_imports(source: String, module: String) -> Vec<String> {
    hozo_parser::module_imports(&source, &module)
}

/// Primitive-named bindings a file must not have lowered.
///
/// One implementation of the rule, shared with the backends: the Native
/// transform has its own guards that need to tell a component Hozo
/// declined from one it failed to lower.
#[napi]
pub fn foreign_primitives(source: String, sources: Vec<String>) -> Vec<String> {
    let mut names: Vec<String> = hozo_parser::foreign_primitives(&source, &sources).into_iter().collect();
    names.sort();
    names
}
