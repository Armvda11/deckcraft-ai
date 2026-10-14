//! Desktop AI task pane. Workers prepare candidates; acceptance uses the engine command.
use crate::SlideApp;
use deckcraft_ai::{
    jobs::{Job, Task},
    settings, *,
};
use deckcraft_engine::{DocState, Session};
use egui::{TextureHandle, Ui};
use serde_json::{Value, json};

pub struct State {
    pub settings: settings::Settings,
    key: String,
    key_loaded: Option<usize>,
    remember: bool,
    pub brief: String,
    audience: String,
    slides: usize,
    theme: String,
    review_outline: bool,
    pub prompt: String,
    scope: EditScope,
    sources: SourceBundle,
    source_path: String,
    job: Option<Job>,
    status: String,
    error: String,
    usage: (u64, u64),
    models: Vec<String>,
    pub candidate: Option<Candidate>,
    previews: Vec<(usize, TextureHandle)>,
    outline: Option<Outline>,
    checkpoint: Option<(Outline, DeckPlan)>,
    request: Option<GenerationRequest>,
    latex: String,
    equation_context: Option<(EditRequest, Option<deckcraft_model::ShapeId>)>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            settings: settings::load(),
            key: String::new(),
            key_loaded: None,
            remember: false,
            brief: String::new(),
            audience: "Technical audience".into(),
            slides: 10,
            theme: "Harbor".into(),
            review_outline: false,
            prompt: String::new(),
            scope: EditScope::Slide,
            sources: SourceBundle::default(),
            source_path: String::new(),
            job: None,
            status: "Ready".into(),
            error: String::new(),
            usage: (0, 0),
            models: vec![],
            candidate: None,
            previews: vec![],
            outline: None,
            checkpoint: None,
            request: None,
            latex: String::new(),
            equation_context: None,
        }
    }
}

impl State {
    pub fn attach_bytes(&mut self, name: &str, bytes: &[u8]) -> Result<(), String> {
        self.start(Task::ImportBytes(vec![(name.into(), bytes.to_vec())]))
    }
    fn profile(&self) -> ProviderProfile {
        self.settings.profiles.get(self.settings.active).cloned().unwrap_or_default()
    }
    fn start(&mut self, task: Task) -> Result<(), String> {
        if self.job.is_some() {
            return Err("An AI job is already running".into());
        }
        if self.key_loaded.is_none() {
            self.key = settings::load_key(&self.profile()).unwrap_or_default();
            self.key_loaded = Some(self.settings.active);
        }
        self.error.clear();
        self.status = "Starting…".into();
        self.job = Some(Job::start(self.profile(), self.key.clone(), task)?);
        Ok(())
    }
    fn generate(&mut self, resume: Option<(Outline, DeckPlan)>) -> Result<(), String> {
        let request = if resume.is_some() {
            self.request.clone().ok_or("No draft to resume")?
        } else {
            let request = GenerationRequest {
                brief: self.brief.clone(),
                audience: self.audience.clone(),
                slide_count: self.slides,
                theme: self.theme.clone(),
                sources: self.sources.clone(),
                review_outline: self.review_outline,
            };
            self.request = Some(request.clone());
            self.checkpoint = None;
            self.candidate = None;
            self.outline = None;
            self.previews.clear();
            self.usage = (0, 0);
            request
        };
        self.start(Task::Generate { request, resume })
    }
    pub fn poll(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.job else { return };
        let mut events = Vec::new();
        let disconnected = loop {
            match job.receiver.try_recv() {
                Ok(event) => events.push(event),
                Err(std::sync::mpsc::TryRecvError::Empty) => break false,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => break true,
            }
        };
        let cancelled = events.iter().any(|event| matches!(event, AiJobEvent::Cancelled));
        for event in events {
            match event {
                AiJobEvent::Progress(text) => self.status = text,
                AiJobEvent::Usage { input, output } => {
                    self.usage.0 += input;
                    self.usage.1 += output;
                }
                AiJobEvent::Preview { slide, png } => self.preview(ctx, slide, &png),
                AiJobEvent::Ready(candidate) => {
                    self.status = "Draft ready for review".into();
                    self.candidate = Some(candidate);
                }
                AiJobEvent::Failed { message, partial } => {
                    self.status = "Job stopped".into();
                    self.error = message;
                    if partial.is_some() {
                        self.candidate = partial;
                    }
                }
                AiJobEvent::Cancelled => self.status = "Cancelled — completed slides remain in recovery".into(),
                AiJobEvent::Connected(models) => {
                    self.models = models;
                    self.status = "Connection successful".into();
                }
                AiJobEvent::Sources(bundle) => match deckcraft_ai::sources::merge(&mut self.sources, bundle) {
                    Ok(()) => self.status = "Sources attached".into(),
                    Err(message) => self.error = message,
                },
                AiJobEvent::Outline(outline) => {
                    self.outline = Some(outline);
                    self.status = "Review the outline below".into();
                }
                AiJobEvent::Checkpoint { outline, plan } => self.checkpoint = Some((outline, plan)),
                AiJobEvent::Equation { latex, png } => {
                    self.preview(ctx, 0, &png);
                    if let Some((request, id)) = self.equation_context.take() {
                        let mut session = Session::new();
                        session.add_document(DocState::new((*request.document).clone(), None, "Equation preview".into()));
                        let result = session.execute("slide.go", &json!({"index": request.slide})).and_then(|_| {
                            session.execute("equation.upsert", &json!({"id": id, "latex": latex, "data": deckcraft_engine::cmd::base64_encode(&png)}))
                        });
                        match result {
                            Ok(_) => {
                                if let Some(st) = session.active() {
                                    self.candidate = Some(Candidate {
                                        document: st.doc.clone(),
                                        base: Some(request.base),
                                        summary: "Updated editable equation".into(),
                                        diagnostics: compiler::diagnostics(&st.doc),
                                    });
                                    self.status = "Equation ready to accept".into();
                                }
                            }
                            Err(error) => self.error = error.to_string(),
                        }
                    }
                }
            }
        }
        if disconnected || cancelled {
            self.job = None;
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
    fn preview(&mut self, ctx: &egui::Context, slide: usize, png: &[u8]) {
        if let Some(image) = deckcraft_render::decode_image(png) {
            let texture = ctx.load_texture(
                format!("ai-preview-{slide}"),
                egui::ColorImage::from_rgba_unmultiplied([image.width() as usize, image.height() as usize], image.as_raw()),
                egui::TextureOptions::LINEAR,
            );
            self.previews.retain(|(i, _)| *i != slide);
            self.previews.push((slide, texture));
            self.previews.sort_by_key(|p| p.0);
        }
    }
}

fn edit_request(app: &SlideApp) -> Result<EditRequest, String> {
    let st = app.session.active().ok_or("Open a presentation first")?;
    Ok(EditRequest {
        prompt: app.ai.prompt.clone(),
        scope: app.ai.scope,
        base: EditBase { document_uid: st.uid, content_revision: st.content_revision },
        document: st.doc.clone(),
        slide: st.selection.slide,
        selected: st.selection.shapes.clone(),
        sources: app.ai.sources.clone(),
    })
}

pub fn command(app: &mut SlideApp, id: &str, p: &Value) -> Result<Value, String> {
    app.ui.pane = Some("ai".into());
    app.ui.pane_width = app.ui.pane_width.max(360.0);
    match id {
        "ai.open" => {
            if app.session.active().is_none() {
                app.session.execute("file.new", &json!({"blank":true})).map_err(|e| e.to_string())?;
            }
            app.close_backstage();
        }
        "ai.status" => {
            return Ok(json!({"running": app.ai.job.is_some(), "status": app.ai.status, "error": app.ai.error,
            "candidate": app.ai.candidate.as_ref().map(|c|json!({"slides":c.document.slides.len(),"summary":c.summary,"diagnostics":c.diagnostics})), "usage":app.ai.usage}));
        }
        "ai.generate" => {
            if let Some(v) = p["brief"].as_str() {
                app.ai.brief = v.into();
            }
            if let Some(v) = p["slides"].as_u64() {
                app.ai.slides = v as usize;
            }
            if let Some(v) = p["audience"].as_str() {
                app.ai.audience = v.into();
            }
            if let Some(v) = p["reviewOutline"].as_bool() {
                app.ai.review_outline = v;
            }
            app.ai.generate(None)?;
        }
        "ai.edit" => {
            if let Some(v) = p["prompt"].as_str() {
                app.ai.prompt = v.into();
            }
            if let Some(v) = p["scope"].as_str() {
                app.ai.scope = match v {
                    "objects" => EditScope::Objects,
                    "deck" => EditScope::Deck,
                    _ => EditScope::Slide,
                };
            }
            if app.ai.prompt.trim().is_empty() {
                return Err("Describe the change".into());
            }
            let request = edit_request(app)?;
            app.ai.previews.clear();
            app.ai.candidate = None;
            app.ai.start(Task::Edit(request))?;
        }
        "ai.cancel" => {
            if let Some(job) = &mut app.ai.job {
                job.cancel();
            }
        }
        "ai.accept" => {
            let candidate = app.ai.candidate.as_ref().ok_or("No candidate to accept")?;
            let data = deckcraft_engine::cmd::base64_encode(&deckcraft_format::save(&candidate.document).map_err(|e| e.to_string())?);
            if let Some(base) = &candidate.base
                && !p["separate"].as_bool().unwrap_or(false)
            {
                app.session
                    .execute("document.applyCandidate", &json!({"documentUid":base.document_uid,"contentRevision":base.content_revision,"data":data}))
                    .map_err(|e| e.to_string())?;
            } else {
                app.session.execute("file.openBytes", &json!({"name":"AI Draft.deckcraft","data":data})).map_err(|e| e.to_string())?;
            }
            app.ai.candidate = None;
            app.ai.status = "Accepted — save your presentation when ready".into();
            app.close_backstage();
        }
        _ => return Err(format!("Unknown AI command {id}")),
    }
    Ok(json!({}))
}

pub fn show(app: &mut SlideApp, ui: &mut Ui) {
    let mut action: Option<(&str, Value)> = None;
    let state = &mut app.ai;
    ui.label("Generate a complete draft, then refine it here or on the canvas.");
    ui.collapsing("Connection", |ui| {
        egui::ComboBox::from_id_salt("ai-profile").selected_text(state.profile().name).show_ui(ui, |ui| {
            for (i, p) in state.settings.profiles.iter().enumerate() {
                ui.selectable_value(&mut state.settings.active, i, &p.name);
            }
        });
        if state.key_loaded != Some(state.settings.active) {
            state.key = settings::load_key(&state.profile()).unwrap_or_default();
            state.key_loaded = Some(state.settings.active);
            state.models.clear();
        }
        let Some(profile) = state.settings.profiles.get_mut(state.settings.active) else { return };
        ui.text_edit_singleline(&mut profile.name);
        ui.horizontal(|ui| {
            ui.selectable_value(&mut profile.kind, ProviderKind::Codex, "Codex login");
            ui.selectable_value(&mut profile.kind, ProviderKind::Compatible, "Compatible API");
        });
        if profile.kind == ProviderKind::Codex {
            ui.small("Uses Codex CLI with your ChatGPT login.");
            ui.add(egui::TextEdit::singleline(&mut profile.codex_path).hint_text("Codex executable (automatic)"));
        } else {
            ui.add(egui::TextEdit::singleline(&mut profile.endpoint).hint_text("https://provider.example/v1"));
            ui.add(egui::TextEdit::singleline(&mut state.key).password(true).hint_text("API key (optional for local models)"));
            ui.checkbox(&mut state.remember, "Remember in system credential store");
        }
        ui.add(egui::TextEdit::singleline(&mut profile.model).hint_text("Model (Codex default when blank)"));
        if !state.models.is_empty() {
            egui::ComboBox::from_id_salt("ai-models").selected_text("Available models").show_ui(ui, |ui| {
                for model in &state.models {
                    ui.selectable_value(&mut profile.model, model.clone(), model);
                }
            });
        }
        ui.checkbox(&mut profile.vision, "Review each generated slide visually (extra AI calls)");
        ui.horizontal(|ui| {
            ui.label("Source context");
            ui.add(egui::DragValue::new(&mut profile.context_chars).range(2_000..=200_000).speed(100));
        });
        ui.horizontal(|ui| {
            if ui.button("Save settings").clicked() {
                let result = settings::save(&state.settings).and_then(|_| {
                    if state.remember && state.profile().kind == ProviderKind::Compatible {
                        settings::save_key(&state.profile(), &state.key)
                    } else {
                        Ok(())
                    }
                });
                match result {
                    Ok(()) => state.status = "Connection settings saved".into(),
                    Err(error) => state.error = error,
                }
            }
            if ui.add_enabled(state.job.is_none(), egui::Button::new("Test connection")).clicked()
                && let Err(error) = state.start(Task::Connect)
            {
                state.error = error;
            }
            if ui.button("Add profile").clicked() {
                state.settings.profiles.push(ProviderProfile { name: "New provider".into(), kind: ProviderKind::Compatible, ..Default::default() });
                state.settings.active = state.settings.profiles.len() - 1;
            }
        });
    });
    ui.collapsing(format!("Sources · {} passages · {} images", state.sources.chunks.len(), state.sources.assets.len()), |ui| {
        ui.small("Attach PDF, DOCX, Markdown, text, CSV, or images. Original files stay outside the deck.");
        ui.horizontal(|ui| {
            if ui.add_enabled(state.job.is_none(), egui::Button::new("Attach file…")).clicked()
                && let Some(path) = app.services.pick_open.as_mut().and_then(|pick| pick("ai-source"))
                && let Err(error) = state.start(Task::Import(vec![path.into()]))
            {
                state.error = error;
            }
            if ui.button("Clear").clicked() {
                state.sources = SourceBundle::default();
            }
        });
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut state.source_path).hint_text("File path"));
            if ui.add_enabled(state.job.is_none() && !state.source_path.is_empty(), egui::Button::new("Attach")).clicked()
                && let Err(error) = state.start(Task::Import(vec![state.source_path.clone().into()]))
            {
                state.error = error;
            }
        });
        for chunk in state.sources.chunks.iter().take(20) {
            ui.small(format!("{} · {}", chunk.reference.name, chunk.reference.locator));
        }
        for asset in &state.sources.assets {
            ui.small(format!("Image: {}", asset.name));
        }
    });
    ui.separator();
    ui.strong("Create a presentation");
    ui.add(egui::TextEdit::multiline(&mut state.brief).desired_rows(4).hint_text("Topic, purpose, key messages, and desired style…"));
    ui.add(egui::TextEdit::singleline(&mut state.audience).hint_text("Audience"));
    ui.horizontal(|ui| {
        ui.label("Slides");
        ui.add(egui::DragValue::new(&mut state.slides).range(2..=30));
        ui.label("Theme");
        ui.text_edit_singleline(&mut state.theme);
    });
    ui.checkbox(&mut state.review_outline, "Review outline before composing");
    if ui.add_enabled(state.job.is_none() && !state.brief.trim().is_empty(), egui::Button::new("Generate complete draft")).clicked() {
        action = Some(("ai.generate", json!({})));
    }
    if let Some(outline) = &mut state.outline {
        ui.text_edit_singleline(&mut outline.title);
        for (i, slide) in outline.slides.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("{}.", i + 1));
                ui.text_edit_singleline(&mut slide.title);
            });
        }
        if ui.add_enabled(state.job.is_none(), egui::Button::new("Compose this outline")).clicked() {
            let outline = outline.clone();
            let plan = DeckPlan { title: outline.title.clone(), theme: state.theme.clone(), slides: vec![] };
            state.outline = None;
            if let Err(error) = state.generate(Some((outline, plan))) {
                state.error = error;
            }
        }
    }
    ui.separator();
    ui.strong("Refine the open presentation");
    ui.horizontal(|ui| {
        ui.selectable_value(&mut state.scope, EditScope::Objects, "Selection");
        ui.selectable_value(&mut state.scope, EditScope::Slide, "Slide");
        ui.selectable_value(&mut state.scope, EditScope::Deck, "Deck");
    });
    ui.add(
        egui::TextEdit::multiline(&mut state.prompt)
            .desired_rows(3)
            .hint_text("Simplify the explanation, improve layout, reveal the diagram in order…"),
    );
    if ui.add_enabled(state.job.is_none() && app.session.active().is_some(), egui::Button::new("Preview AI edit")).clicked() {
        action = Some(("ai.edit", json!({})));
    }
    ui.collapsing("Editable LaTeX equation", |ui| {
        if ui.button("Load selected equation").clicked() {
            state.latex =
                app.session.active().and_then(|st| st.selected_shapes().first().and_then(|s| s.equation_source.clone())).unwrap_or_default();
        }
        ui.add(egui::TextEdit::multiline(&mut state.latex).desired_rows(3).hint_text("\\frac{1}{2} + x^2"));
        ui.small("Math subset: fractions, roots, sums, integrals, Greek letters, matrices, text. No external files or macros.");
        if ui.add_enabled(state.job.is_none() && !state.latex.is_empty(), egui::Button::new("Render equation preview")).clicked() {
            action = Some(("equation", json!({})));
        }
    });
    ui.separator();
    if state.job.is_some() {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(&state.status);
            if ui.button("Cancel").clicked() {
                action = Some(("ai.cancel", json!({})));
            }
        });
    } else {
        ui.label(&state.status);
    }
    if state.usage != (0, 0) {
        ui.small(format!("Reported tokens: {} input · {} output", state.usage.0, state.usage.1));
    }
    if !state.error.is_empty() {
        ui.colored_label(egui::Color32::from_rgb(190, 60, 50), &state.error);
    }
    if state.job.is_none()
        && let Some(resume) = state.checkpoint.clone()
        && state.request.as_ref().is_some_and(|r| resume.1.slides.len() < r.slide_count)
        && ui.button("Retry from completed slides").clicked()
        && let Err(error) = state.generate(Some(resume))
    {
        state.error = error;
    }
    if let Some(candidate) = &state.candidate {
        ui.strong(&candidate.summary);
        for d in &candidate.diagnostics {
            ui.small(format!("Slide {}: {}", d.slide + 1, d.message));
        }
        ui.horizontal(|ui| {
            if ui.button(if candidate.base.is_some() { "Accept edit" } else { "Open draft" }).clicked() {
                action = Some(("ai.accept", json!({})));
            }
            if candidate.base.is_some() && ui.button("Open separately").clicked() {
                action = Some(("ai.accept", json!({"separate":true})));
            }
            if ui.button("Reject").clicked() {
                action = Some(("reject", json!({})));
            }
        });
    }
    for (slide, texture) in &state.previews {
        ui.small(format!("Preview {}", slide + 1));
        ui.add(egui::Image::new(texture).fit_to_exact_size(egui::vec2(ui.available_width(), ui.available_width() * 9.0 / 16.0)));
    }
    ui.collapsing("Recover completed drafts", |ui| {
        for path in settings::drafts().iter().take(10) {
            if ui.button(path.file_name().unwrap_or_default().to_string_lossy()).clicked() {
                action = Some(("file.open", json!({"path":path.to_string_lossy()})));
            }
        }
    });
    if let Some((id, params)) = action {
        let result = match id {
            "reject" => {
                app.ai.candidate = None;
                app.ai.previews.clear();
                Ok(json!({}))
            }
            "equation" => edit_request(app).and_then(|request| {
                let id = request.selected.first().copied().filter(|id| {
                    deckcraft_model::find_shape(&request.document.slides[request.slide].shapes, *id).is_some_and(|s| s.equation_source.is_some())
                });
                app.ai.equation_context = Some((request, id));
                app.ai.previews.clear();
                app.ai.candidate = None;
                app.ai.start(Task::Equation(app.ai.latex.clone())).map(|_| json!({}))
            }),
            _ => app.run(id, params),
        };
        if let Err(error) = result {
            app.ai.error = error;
        }
    }
}
