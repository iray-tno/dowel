// Keyboard activation for elements Hozo made interactive itself.
//
// A `<button>` gets Enter and Space from the browser. A
// `<div role="button" tabindex="0">` gets nothing, and a control a keyboard
// user can reach but cannot operate fails WCAG 2.1.1. Hozo lowers
// `Pressable` to a `<div>` -- no HTML element carries its semantics, and
// `<button>` cannot hold one because its content model is phrasing content
// while a `View` is a `<div>` -- so this is the part that has to be script.
//
// The behaviour mirrors a native button exactly, which is more than one
// keydown branch: Enter activates on key *down*, Space activates on key
// *up* and only suppresses scrolling on the way down. Half of that is the
// kind of nearly-right that reads as working and is not.
//
// `currentTarget.click()` rather than calling the author's handler: it
// dispatches a real click, so the handler that the compiler wired to
// `onClick` receives a genuine MouseEvent rather than a keyboard one it
// was never typed for. It also means these two functions close over
// nothing, so they are module-level constants -- one stable reference per
// module, no allocation per render, and no dependency array to get wrong.

/** The shape both handlers need, structural so React's event type fits. */
interface ActivationEvent {
  key: string
  preventDefault(): void
  currentTarget: { click(): void }
}

/** Enter activates; Space is swallowed here so the page does not scroll. */
export function hozoActivateKeyDown(event: ActivationEvent): void {
  if (event.key === 'Enter') {
    event.preventDefault()
    event.currentTarget.click()
  } else if (event.key === ' ') {
    event.preventDefault()
  }
}

/** Space activates on release, the way a real button does. */
export function hozoActivateKeyUp(event: ActivationEvent): void {
  if (event.key === ' ') {
    event.preventDefault()
    event.currentTarget.click()
  }
}
