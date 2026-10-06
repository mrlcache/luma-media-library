# Android player presentation

The mobile player requests fixed landscape orientation while mounted and restores fixed portrait orientation on exit. Android status and navigation bars follow the player controls: visible controls show the bars; hidden controls or a locked player hide them. Android edge swipes can reveal the bars temporarily.

Window updates are serialized and pending updates are coalesced. A delayed enter command cannot override the exit command. The player keeps the display awake only while mounted.

Mobile playback disables backdrop filters throughout the app, including the underlying navigation layer, and restores them on exit. Desktop player filters remain unchanged.

Validation: `node --test scripts/player-presentation.test.mjs` covers command ordering, recovery after a failed command, and preservation of the mobile blur reset in production CSS. The Android build also checks that the packaged interface renders without a paired PC. Rotation, system bars, and hardware video compositing still require confirmation on a physical Android phone.

Native window changes require installing the updated APK; frontend hot reload alone does not update the Android activity.
