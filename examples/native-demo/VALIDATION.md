# Native device acceptance

The automated suite proves that this source passes the real Metro transformer, that generated modules evaluate against the Native component contract, and that measured Grid state settles under synthetic `onLayout` events. A simulator or device is still required for the platform behaviors below.

Use a React Native 0.87 host that registers `DowelNativeDemo` from `index.js`, run Metro with this directory as the project root, then inspect the acceptance screen on both iOS and Android.

## Visual and interaction pass

- `smoke-image`: the remote logo loads, is 80×80, cropped, and rounded.
- `smoke-horizontal-scroll`: a horizontal gesture reaches cards three and four without vertical-axis capture.
- `smoke-grid`: the tall tile spans both measured rows; the displayed width is non-zero and updates after rotation.
- `smoke-list`: rows scroll smoothly and retain their content after recycling.
- `smoke-interaction`: pointer hover changes color where supported; keyboard focus has a visible color; pressing opens the dialog.
- Dialog: Android Back requests close and focus does not escape the modal.

## Screen-reader pass

- VoiceOver and TalkBack announce the acceptance screen as a list and each virtual row once.
- The logo is announced as “React Native logo”.
- The input is announced as “Email address”, followed by its hint; the placeholder is not used as its name.
- Continue is announced as a button named “Review email address”.
- Opening the dialog announces “Confirm your address”; dismissing it returns focus to Continue.

Record OS, device/simulator, screen reader, result, and any failing `testID`. Do not mark the device gate complete from the Metro bundle alone.
