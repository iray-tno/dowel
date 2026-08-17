// The `TextInput` half of `@dowel/core`'s fallback primitives, kept in its
// own file because it carries an accessibility rule the others don't.
//
// See `./index.tsx` for why these fallbacks exist at all: the compiler's
// job is to make invoking them unnecessary where it can, not to make them
// required.

import type { ReactNode } from 'react'

export interface TextInputProps {
  className?: string
  value?: string
  placeholder?: string
  /**
   * The field's accessible name.
   *
   * Spelled the React Native way and mapped to `aria-label` here, so one
   * source spelling works on both platforms -- the same arrangement
   * `accessibilityRole` already has on `Pressable`.
   *
   * A `placeholder` is not a substitute (proposal §10.2): it may not be
   * announced as the field's name, and it disappears on the first
   * keystroke, which is exactly when someone would want to check what the
   * field was for. The compiler warns when this is missing.
   */
  accessibilityLabel?: string
  /** Additional guidance announced after the field's accessible name. */
  accessibilityHint?: string
  onChangeText?: (text: string) => void
  disabled?: boolean
  children?: ReactNode
}

export function TextInput({
  className,
  value,
  placeholder,
  accessibilityLabel,
  accessibilityHint,
  onChangeText,
  disabled,
}: TextInputProps) {
  return (
    <input
      className={className}
      value={value}
      placeholder={placeholder}
      aria-label={accessibilityLabel}
      aria-description={accessibilityHint}
      disabled={disabled}
      onChange={(event) => onChangeText?.(event.target.value)}
    />
  )
}
