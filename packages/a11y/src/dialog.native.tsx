// The React Native half of Dowel's Dialog (proposal §10.3).
//
// Same shape as the Web one and the same posture: delegate. React Native's
// `Modal` is the platform's modal, and `accessibilityViewIsModal` is what
// tells VoiceOver and TalkBack that everything behind it is out of scope --
// the equivalent of the top layer making the page inert. Reimplementing
// either would mean fighting the OS accessibility tree from JavaScript,
// which is the emulation the proposal's principle warns off.
//
// Two differences from Web that are the platform, not the port:
//
// - Escape is Android's hardware back button, which arrives as
//   `onRequestClose`. React Native requires that prop on Android and warns
//   without it, for the same reason the Web side treats `onClose` as
//   effectively required: a modal you cannot dismiss reads as a trap.
// - Focus restoration is the OS's. React Native has no
//   `document.activeElement` to capture and nothing to give focus back to,
//   so `./focus.ts`'s restore rule has no work here -- the screen reader
//   returns to where it was when the modal is dismissed.

import { Modal, View } from 'react-native'
import type { ReactNode } from 'react'

export interface DowelDialogProps {
  open?: boolean
  onClose?: () => void
  accessibilityLabel?: string
  accessibilityHint?: string
  style?: unknown
  children?: ReactNode
}

export function DowelDialog({
  open = false,
  onClose,
  accessibilityLabel,
  accessibilityHint,
  style,
  children,
}: DowelDialogProps) {
  return (
    <Modal
      visible={open}
      transparent
      // Not `animationType="slide"` by default: a dialog that animates in
      // from an edge reads as a screen transition, and the compiler has no
      // way to know which the author meant. Fade is the neutral one.
      animationType="fade"
      onRequestClose={onClose}
    >
      <View
        style={style}
        accessible
        accessibilityViewIsModal
        accessibilityRole="none"
        accessibilityLabel={accessibilityLabel}
        accessibilityHint={accessibilityHint}
      >
        {children}
      </View>
    </Modal>
  )
}
