# AI presentation editor: approved plan and execution record

Goal: generate complete technical presentations inside the native DeckCraft editor, then refine content, layout and animations with AI or manual editing. macOS Apple Silicon is the primary development/release target; Linux compatibility and WASM compilation are maintained.

## Approved milestones

1. Isolated branch, pinned Rust 1.99, open fonts, baseline tests and macOS launch.
2. Integrated AI pane, provider profiles, connection testing, cancellation and secure credentials.
3. Structured whole-deck generation, varied editable compositions, progressive previews and recovery.
4. PDF/DOCX/Markdown/text/image sources, native technical content and source references.
5. Editable LaTeX equations rendered locally through MiTeX and Typst.
6. Scoped AI edits with candidate previews, stale-content checks, single-step undo and animation sequences.
7. Layout/export checks, click-state PDF handouts, macOS packaging, Linux CI/packages and distribution branding.

Defaults: ten slides (2–30), full draft generation with optional outline review, at most two repairs per slide. Native .deckcraft is authoritative. No OCR, generated imagery, autonomous web research, collaboration or new motion-graphics engine in this release.

## Execution record

- Base: `457406a7a5b62425e915926c8ccb0984e61a198c`; branch `feat/ai-presentations` in `deckcraft-ai-worktree`.
- Native worktree creation tool could not resolve a repository from the chat's parent directory; used Git's worktree command in the repository.
- Shared `storytold/craftrules` repository returns repository-not-found. Available AGENTS.md/CLAUDE.md standards remain applicable; unavailable standards are not fabricated.
- User steering: use Codex for now. Local Codex CLI 0.159.0 is installed and signed in with ChatGPT. Add Codex CLI as the default connection, retaining compatible HTTP providers. Do not access or copy Codex credentials.
- Ruling: no pushes to upstream main. Work stays on the isolated local branch; upstream's push instruction does not authorize publishing user work into the upstream repository.
- Rust 1.99.0 is pinned. CI covers macOS 14 and Ubuntu and fetches Libertinus Math v7.051 from its OFL-1.1 release. Font bytes remain outside this repo in `craft-fonts` and are embedded at build time.
- AI Studio is part of the native egui app. Codex CLI uses the existing login and disables shell, apps and multi-agent tools. OpenAI-compatible endpoints support local HTTP and hosted HTTPS, model discovery, structured responses, and cancellation. Keys are stored via the OS credential service when available, with session-only use on fallback.
- Generation creates native text, diagrams, charts, tables, code, images and equations, with slide-by-slide previews, bounded repair, animation and `.deckcraft` recovery. Sources cover PDF, DOCX, Markdown, text, CSV and images. PDF page crops and page/paragraph locators are retained.
- Request and extraction boundaries cap prompts at 1 MB, provider bodies at 8 MB, image inputs at 4 MB, source text at 2 MB, imported assets at 12 MB/32 assets, DOCX expanded media at 12 MB, Codex stdout/stderr at 2 MB each, and full-deck edit context at 512 KB. Source merges reject over-limit additions without partially changing attached sources.
- Scoped edits run against a candidate copy. Command prompts and validators share a scope allowlist. The engine accepts a candidate only at the matching document identity and content revision, as one undoable edit.
- macOS manual launch and Codex connection check succeeded. A two-slide live Codex draft was generated, one invalid animation target was repaired, then the saved file was opened after an app restart. Visual inspection found the first code layout split into narrow columns. The compiler now rejects split code blocks and requests a single block of at most eight lines; the live generation has not yet been repeated with that final rule.
- Early macOS checks passed formatting, Clippy, asset attribution, layering, WASM, and the AI workflow tests. The first full `cargo xtask ci` run exposed CJK fallback tests and a graphics-adapter limitation in the restricted sandbox. The CJK fallback is fixed; the later full gate passed with local socket and graphics access. Linux CI and runtime checks have not run here.
- `packaging/macos/package.sh --development` produced an ad-hoc signed Apple Silicon `.app`/DMG and CLI archive. The app launched and reopened the saved deck. The DMG is not notarized; Linux packages, signing/notarization, ten-slide source-heavy acceptance, exports, keychain persistence, and external-display checks remain open.
- The AI provider integration test uses a loopback HTTP server and a dummy token. It passed when run with permission to bind the local test socket; no real API key was read or stored.
- Fork setup: `Armvda11/deckcraft-ai` is linked to `storytold/deckcraft`; this checkout uses `origin` for the fork and `upstream` for the original. The fork's CI now runs on topic-branch pushes and pull requests. `CONTRIBUTING.md` records branch, review, code ownership, and upstream-update conventions.
- On this Mac, the preferred installed Korean and Chinese fallback fonts exceeded the interface font size limit. Prioritizing AppleGothic and Hiragino Sans GB made the existing CJK interface regression tests pass. With local socket access, `cargo xtask ci` now passes all six gates: fmt, Clippy, tests, assets, layers, and WASM. Linux CI remains to be observed after the branch is pushed.

## Acceptance gates

- Ten-slide source-grounded deck, varied native technical objects, equation source, ordered animations, save/reopen and export. A two-slide Codex draft has been generated and reopened; the ten-slide, multi-source scenario remains open.
- Selected-object edits preserve unselected content; accepting is one undo step; content edits invalidate stale candidates while selection changes do not.
- Cancellation, provider failures and partial generation keep manual editing and recovery available.
- Real-engine tests, visual macOS checks, required workspace gates and WASM checks.
- Live Codex generation completed for a two-slide draft; compatible HTTP protocol integration test passed against a loopback mock. Live local/hosted model-server checks require an available user-configured endpoint.
- Linux runtime and external-display checks must be reported as unverified if no such environment is available.
