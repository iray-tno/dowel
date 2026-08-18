// Deliberately opaque to the compiler: this is proposal §7's third tier.
//
// Hozo reads `className` from the AST, which is what lets it compile the
// other two tiers away entirely -- but it can't read *this*, because the
// class only exists as the return value of a function in another module.
// The byte scan of the project finds `bg-emerald-500` and `bg-slate-500`
// here and emits a rule for each under its real Tailwind name, so whichever
// string this returns at runtime matches by itself.
//
// Under Next.js the scan runs while `next.config.ts` is being evaluated,
// because Turbopack has no build-start hook to put it in.

export function accentFor(enabled: boolean): string {
  return enabled ? 'bg-emerald-500' : 'bg-slate-500'
}
