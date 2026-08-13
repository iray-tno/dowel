// The utilities the report measures against. Not all of Tailwind -- which
// is effectively unbounded once arbitrary values count -- but a
// representative slice of what real app code uses, grouped so the report
// can show where the gaps actually are rather than one flat number.

export const CANDIDATE_GROUPS: Record<string, string[]> = {
  layout: ['flex', 'inline-flex', 'block', 'hidden', 'grid', 'flex-1', 'flex-auto', 'flex-none', 'flex-row', 'flex-col', 'grid-cols-3'],
  align: ['items-center', 'items-start', 'items-end', 'items-stretch', 'justify-center', 'justify-between', 'justify-start', 'justify-end', 'self-center', 'content-center'],
  spacing: ['p-4', 'px-4', 'py-2', 'pt-1', 'pb-8', 'm-4', 'mx-auto', 'mt-2', 'mb-0', 'gap-4', 'gap-x-2', 'gap-y-2', 'space-x-2'],
  sizing: ['w-full', 'w-4', 'w-1/2', 'w-auto', 'h-full', 'h-4', 'h-screen', 'size-4', 'min-w-0', 'max-w-md'],
  typography: ['text-xs', 'text-sm', 'text-base', 'text-lg', 'text-xl', 'text-2xl', 'font-normal', 'font-medium', 'font-bold', 'text-left', 'text-center', 'leading-6', 'leading-tight', 'tracking-wide', 'truncate', 'uppercase'],
  color: ['bg-blue-500', 'bg-white', 'text-gray-900', 'text-black', 'border-red-500', 'bg-transparent'],
  border: ['border', 'border-2', 'border-t', 'border-b-4', 'border-solid', 'border-dashed', 'rounded', 'rounded-sm', 'rounded-md', 'rounded-lg', 'rounded-xl', 'rounded-full', 'rounded-none'],
  effects: ['opacity-0', 'opacity-50', 'opacity-100', 'shadow', 'shadow-lg', 'blur-sm'],
  position: ['relative', 'absolute', 'top-0', 'top-4', 'bottom-0', 'left-2', 'right-2', 'inset-0', 'z-10'],
  transform: ['rotate-45', 'scale-95', 'translate-x-2'],
  transition: ['transition', 'duration-200', 'ease-in-out', 'animate-spin'],
  variants: ['hover:bg-blue-600', 'focus:font-bold', 'disabled:opacity-50', 'md:flex-row', 'lg:p-8', 'sm:text-sm', 'dark:bg-black', 'first:mt-0'],
}

export const ALL_CANDIDATES = Object.values(CANDIDATE_GROUPS).flat()

/// Variant-prefixed candidates lower to a conditional rule on both sides
/// (`.x:hover`, `@media`), so the report compares the *base* utility's
/// declarations and separately checks the condition survived.
export function stripVariant(candidate: string): { variant: string | null; base: string } {
  const idx = candidate.indexOf(':')
  if (idx === -1) return { variant: null, base: candidate }
  return { variant: candidate.slice(0, idx), base: candidate.slice(idx + 1) }
}
