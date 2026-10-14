# DeckCraft AI acceptance checklist

This checklist distinguishes implemented behavior from release gates that still require a suitable environment or provider. Passing a unit test does not claim visual, provider, or platform acceptance.

## Automated checks

- [x] Existing `.deckcraft` model fields use serde defaults for backward compatibility.
- [x] AI plans compile into editable native text, shapes, diagrams, charts, tables, code text, images, equations, connectors, and animations.
- [x] Chart values must match values found in the cited source text.
- [x] Selected-object edits preserve unselected shapes and reject out-of-scope commands.
- [x] Candidate acceptance checks document identity and content revision and creates one undoable edit.
- [x] PDF and DOCX source extraction preserves page or paragraph locations; PDF rendering provides page image assets.
- [x] Equation source is retained and rendered through a closed Typst world.
- [x] PDF export supports animation click states, retains the first state as the internal-link target, and reports timelines above its stated limit.
- [x] Request inputs are bounded before provider calls; attached sources and DOCX expanded media enforce aggregate size and asset-count limits.
- [x] A loopback OpenAI-compatible mock verifies model discovery, bearer auth and structured chat output.
- [x] Rust formatting, Clippy, asset attribution, dependency layering, WASM checks and tests excluding the `deckcraft-ui-egui` test target passed on macOS.
- [ ] The full `cargo xtask ci` passed: this sandbox run stopped on two CJK font lookup tests and one UI harness test without a native wgpu adapter.
- [x] A development Apple Silicon `.app`, ad-hoc signed DMG and CLI archive were built; the app launched and reopened the generated `.deckcraft` file.
- [ ] A Linux package has been built and opened on a Linux host.

## Manual acceptance still required

- [ ] Generate a ten-slide talk from a prompt, PDF, DOCX, and images; save, close, reopen, and verify sources and equation LaTeX. The live Codex check covered two slides without attached sources.
- [ ] Inspect PDF and PPTX exports, animation order and an external display. The two-slide `.deckcraft` was visually inspected in the macOS editor at 82% zoom.
- [ ] Edit selected objects, accept once, undo once, redo, and verify slide/deck scope behavior in the UI.
- [ ] Cancel during network generation and during source import; verify partial drafts remain recoverable and the editor remains responsive.
- [x] Complete live generation with Codex CLI. The last live run preceded the stricter single-code-block repair rule; repeat it to check that rule in a provider response.
- [ ] Complete live generation with a separately configured compatible local or hosted endpoint.
- [ ] Verify Keychain persistence and Linux Secret Service behavior.
- [ ] Run Linux graphics and packaging smoke tests. The current developer environment has not yet completed these checks.

## Known boundaries

- Scanned-document OCR, web research, image generation, collaboration, and new animation effects are outside this release.
- Exported equations are rendered images; `.deckcraft` retains the editable LaTeX source.
- PPTX preservation of DeckCraft-specific animations and equations is best effort.
- A provider's factual accuracy and visual quality still require review.
