// One policy for what a diagnostic does, shared by every integration.
//
// It was four policies. Metro threw on an error-severity diagnostic and
// warned otherwise; Vite and Next warned on everything, including errors.
// That difference read as deliberate -- Native has cases with no correct
// output, Web mostly doesn't -- but it was not: every error-severity
// diagnostic Hozo can emit today happens to come from the Native backend,
// so the Web integrations had simply never been handed one. The first Web
// error added would have been printed as a warning and shipped.
//
// So the policy lives here instead, once, and says the same thing whichever
// bundler is asking. Today it changes no behaviour, which is the point of
// doing it now rather than after something depends on the gap.

import type { CompileDiagnostic } from './index.ts'

/** Where an integration sends a warning. */
export type WarnFn = (message: string) => void

export function formatDiagnostic(diagnostic: CompileDiagnostic): string {
  return `[hozo] ${diagnostic.code}: ${diagnostic.message}`
}

/**
 * Reports every diagnostic, throwing if any of them is an error.
 *
 * Throwing rather than returning a flag: an integration that forgot to
 * check the flag would compile the module anyway, which is the failure this
 * replaces. The message names the file, because a bundler that catches this
 * shows the stack and not the source.
 */
export function reportDiagnostics(
  diagnostics: CompileDiagnostic[],
  filename: string,
  warn: WarnFn,
): void {
  const errors = diagnostics.filter((diagnostic) => diagnostic.severity === 'error')
  if (errors.length > 0) {
    const detail = errors.map((diagnostic) => `  ${diagnostic.code}: ${diagnostic.message}`).join('\n')
    throw new Error(`[hozo] ${filename} cannot be compiled:\n${detail}`)
  }
  for (const diagnostic of diagnostics) {
    warn(formatDiagnostic(diagnostic))
  }
}
