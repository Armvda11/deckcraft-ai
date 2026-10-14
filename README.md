# DeckCraft AI

A native, editable presentation editor with AI generation and refinement. Based on [DeckCraft](https://github.com/storytold/deckcraft), built in Rust and egui. macOS Apple Silicon is the primary platform; Linux checks and WASM compatibility are maintained.

## Use the application

Open **DeckCraft AI.app**, then choose **Create with AI** or **AI → AI Studio**.

1. Expand **Connection**. The default **Codex login** uses the installed Codex CLI and your existing ChatGPT login. It never copies credentials. A model name is optional. [Codex noninteractive mode](https://developers.openai.com/codex/noninteractive) describes the adapter's CLI.
2. Alternatively select **Compatible API**, then enter a base URL and model. Ollama usually uses `http://localhost:11434/v1`; LM Studio usually uses `http://localhost:1234/v1`. Hosted endpoints require HTTPS. Keys can be session-only or saved in the system credential store.
3. Attach PDF, DOCX, Markdown, text, CSV, and images under **Sources**, or drop them onto the editor with AI Studio open.
4. Describe the talk and audience, then generate 2–30 slides (default 10), optionally reviewing the outline first.
5. Review previews and diagnostics, then **Open draft**. Text, diagrams, connectors, charts, tables, code, and images remain editable.
6. Select objects, a slide, or the deck and request an edit or animation. Review and **Accept edit**. Acceptance is one undo step. Outdated candidates cannot overwrite changes made while generation runs; open them separately or regenerate.
7. Save as `.deckcraft`, present through Slide Show, or export PDF and best-effort PPTX through File → Export. PDF can include successive animation click states as raster handouts.

Equations retain LaTeX source in `.deckcraft`. Select one and use **Editable LaTeX equation → Load selected equation**, edit, and preview. Invalid source preserves the previous equation. PDF/PPTX carry the rendered equation image.

Manual editing works offline. AI works offline with a compatible local model server. **Recover completed drafts** opens checkpoints after cancellation or failure. Original source documents remain outside the deck; source names and page/paragraph locators remain within it.

Inputs are bounded: 1 MB prompts, 4 MB of image inputs, 2 MB source text, 12 MB of attached image data, and 32 assets. DOCX expanded media shares the same 12 MB asset limit.

## Build

Rust **1.99.0**, Xcode command-line tools on macOS, and the open fonts are required. The application needs no Python companion.

```sh
git clone https://github.com/storytold/craft-fonts ../craft-fonts
git -C ../craft-fonts checkout 43913056ce82a3fdad0ef69f6b88d1dc922a8104
export CRAFT_FONTS_DIR="$(cd ../craft-fonts && pwd)"
sh scripts/fetch-math-font.sh
cargo run -p deckcraft
```

Linux needs the audio, X11/Wayland, and graphics development packages in `.github/workflows/ci.yml`. Persistent Linux keys use Secret Service; use session-only keys when unavailable.

```sh
cargo xtask ci
cargo build -p deckcraft -p deckcraft-cli
packaging/macos/package.sh --development
```

The development package is locally usable and ad-hoc signed. For an optimized release use the packaging script with `--arch aarch64`. Developer ID signing and notarization require your own Apple credentials. Linux scripts remain under `packaging/linux`; publish packages after Linux runtime smoke testing.

## Architecture and verification

`crates/ai` owns providers, local extraction, bounded generation, deterministic composition, and scoped candidate edits. `crates/ui-egui/src/ai.rs` supplies the desktop pane. The engine owns atomic acceptance and content revisions. HTTP, extraction, and generation run in background workers.

See the [implementation record](docs/ai-implementation.md) and [acceptance checklist](docs/ai-acceptance.md) for evidence and remaining gates. Synthetic examples are in `examples/ai`.

Development takes place in the [DeckCraft AI fork](https://github.com/Armvda11/deckcraft-ai). The
`main` branch is the reviewed version; work starts on focused branches. See
[CONTRIBUTING.md](CONTRIBUTING.md) for code organization, commit checks, and upstream updates.

The same worker can be exercised from a script:

```sh
cargo run -p deckcraft-ai --example generate -- output.deckcraft "Explain my research" examples/ai/research.md
```

`DECKCRAFT_AI_SLIDES`, `DECKCRAFT_AI_ENDPOINT`, and `DECKCRAFT_AI_MODEL` configure that example. `DECKCRAFT_AI_CONFIG_DIR` overrides settings/checkpoint storage. Never put keys in plans or source documents.

## Current boundaries

No scanned-document OCR, web research, generated imagery, collaboration, or new animation engine. PDF extraction preserves pages; DOCX preserves paragraph locations and table text. Up to twelve PDF page previews become image assets. Charts require values present in cited source data. Factual claims still need review. Overcrowded slides are recomposed through bounded repairs. Model capabilities and quality affect results.

## License and upstream

MIT OR Apache-2.0: [LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE), [NOTICE](NOTICE), [ATTRIBUTION.md](ATTRIBUTION.md). Original DeckCraft copyright and contributor credits remain. The fork uses its own application identity and does not ship ArtCraft trademark logos.
