// Deliberately opaque to the compiler: this is proposal §7's third tier.
//
// Dowel reads `className` from the AST, which is what lets it compile the
// other two tiers away entirely -- but it can't read *this*, because the
// class only exists as the return value of a function in another module.
// The byte scan of the project finds `bg-blue-500` and `bg-slate-500`
// here, and the plugin emits a rule for each under its real Tailwind name,
// so whichever string this returns at runtime matches by itself. No
// resolution code ships to the browser.

export function accentFor(enabled: boolean): string {
  return enabled ? 'bg-blue-500' : 'bg-slate-500'
}
