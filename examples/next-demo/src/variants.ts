// Deliberately opaque to the compiler: this is proposal §7's third tier.
//
// Hozo reads `className` from the AST, which is what lets it compile the
// other two tiers away entirely -- but it can't read *this*, because the
// class only exists as the return value of a function in another module.
// The byte scan of the project finds these names and emits a rule for each,
// so whichever string this returns at runtime matches by itself.
//
// Under Next.js the scan runs while `next.config.ts` is being evaluated,
// because Turbopack has no build-start hook to put it in.
//
// `bg-brand` is here on purpose. It is the one class in this file that
// cannot resolve without `src/theme.css`, so it is the only one that can
// tell a candidate stylesheet rendered *with* the project theme from one
// rendered without it. While every class here came from Tailwind's default
// palette, the check could not see that the theme was missing -- and it
// was, on any warm cache.
export function accentFor(enabled: boolean): string {
  return enabled ? 'bg-emerald-500' : 'bg-brand'
}
