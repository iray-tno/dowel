// The Tailwind-facing side of Hozo (proposal §6.2): Tailwind is the
// frontend, the Style IR is the internal representation, and this package
// is the boundary. The utility-to-IR translation lives in Rust; what
// belongs here is the part only a JavaScript package can do -- asking
// Tailwind what the project's theme is.

export { loadTheme, toHex, type Theme, type ThemeColor } from './theme.ts'
export {
  DEFAULT_CSS_FILES,
  loadProjectTheme,
  type ProjectThemeOptions,
} from './project.ts'
