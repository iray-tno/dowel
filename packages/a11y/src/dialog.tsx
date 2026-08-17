// The Web half of Dowel's Dialog (proposal §10.3), which names it as v1's
// first hard primitive and sets the quality bar at: initial focus, focus
// trap, focus restoration, Escape, modal semantics, background inert,
// screen reader behaviour.
//
// Almost all of that is delegated rather than implemented, and that is the
// design rather than a shortcut. `<dialog>.showModal()` gives the trap, the
// top-layer inert background, Escape, and `aria-modal` semantics, from the
// browser -- the proposal's own principle is «prefer platform semantics
// over compatibility emulation», and a hand-rolled focus trap is the
// canonical example of emulation that is subtly wrong forever. What's left
// for Dowel to decide is where focus starts and where it goes back to,
// which is `./focus.ts`.

import { useEffect, useRef, type ReactNode } from 'react'

import { shouldRestoreFocus } from './focus.ts'

export interface DowelDialogProps {
  /** Whether the dialog is showing. Render is driven by this, not by mounting. */
  open?: boolean
  /**
   * Called when the browser asks for the dialog to close -- Escape, or the
   * form's own dismiss. Not optional in practice: without it Escape appears
   * to do nothing, which reads as a trap.
   */
  onClose?: () => void
  /** The dialog's accessible name. The compiler warns when it is absent. */
  accessibilityLabel?: string
  accessibilityHint?: string
  className?: string
  children?: ReactNode
}

export function DowelDialog({
  open = false,
  onClose,
  accessibilityLabel,
  accessibilityHint,
  className,
  children,
}: DowelDialogProps) {
  const ref = useRef<HTMLDialogElement | null>(null)
  // Captured on open rather than read on close: by the time the dialog
  // closes, the thing that opened it may be gone -- a confirm action that
  // removes the row its own trigger lived in is the ordinary case.
  const opener = useRef<Element | null>(null)

  useEffect(() => {
    const dialog = ref.current
    if (!dialog) return

    if (open && !dialog.open) {
      opener.current = document.activeElement
      // `showModal`, never `show`: only the modal form puts the dialog in
      // the top layer, makes the rest of the page inert, and traps focus.
      // `show` looks identical and does none of it.
      dialog.showModal()
      return
    }
    if (!open && dialog.open) {
      dialog.close()
      const previous = opener.current
      if (
        previous instanceof HTMLElement &&
        shouldRestoreFocus({ focusable: previous.isConnected && !hasAttribute(previous, 'disabled') })
      ) {
        previous.focus()
      }
      opener.current = null
    }
  }, [open])

  useEffect(() => {
    const dialog = ref.current
    if (!dialog) return
    // The browser fires `cancel` for Escape. Preventing the default close
    // and reporting it instead keeps the element's `open` in step with the
    // prop -- otherwise the DOM closes while React still thinks it is open,
    // and the next open is a no-op.
    const onCancel = (event: Event) => {
      event.preventDefault()
      onClose?.()
    }
    dialog.addEventListener('cancel', onCancel)
    return () => dialog.removeEventListener('cancel', onCancel)
  }, [onClose])

  return (
    <dialog
      ref={ref}
      className={className}
      aria-label={accessibilityLabel}
      aria-description={accessibilityHint}
    >
      {children}
    </dialog>
  )
}

function hasAttribute(element: HTMLElement, name: string): boolean {
  return element.hasAttribute(name)
}
