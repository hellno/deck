<!-- Thanks for contributing to Deck! Keep PRs small and focused. -->

## What & why

<!-- One or two sentences: what does this change, and why? Link any issue. -->

## Definition of Done

<!-- Run `just ci` and paste the output (or confirm it's green). See CONTRIBUTING.md. -->

- [ ] `just ci` is green (fmt + clippy on default / `tray` / `overlay` / `tray,overlay` + tests).
- [ ] No new or changed dependencies in `Cargo.toml` / `Cargo.lock` (or maintainer approval is linked).
- [ ] GPUI pins changed only via `just bump-gpui` (not hand-edited), if applicable.
- [ ] Docs updated if behavior or setup changed (`README.md` / `docs/`).

## Platforms tested

<!-- The author daily-drives macOS; Linux is kept honest by CI. Tell us what you ran on. -->

- [ ] macOS
- [ ] Linux (note X11 / Wayland)
- Features exercised: <!-- default / tray / overlay / tray,overlay -->

## Notes for reviewers

<!-- Anything tricky, follow-ups, or screenshots/GIFs for UI changes. -->
