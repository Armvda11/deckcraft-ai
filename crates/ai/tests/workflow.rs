#![cfg(not(target_arch = "wasm32"))]
use deckcraft_ai::*;
use deckcraft_model::SourceReference;
use serde_json::json;
use std::io::Write;

fn plan() -> DeckPlan {
    serde_json::from_value(json!({"title":"Research","theme":"Harbor","slides":[
        {"title":"Method","layout":"diagram","elements":[{"id":"pipeline","kind":"diagram","items":["Input","Model","Output"]}],"animations":[{"target":"pipeline","effect":"fade","start":"onClick"}]},
        {"title":"Implementation","layout":"code","elements":[{"id":"code","kind":"code","text":"def predict(x):\n    return model(x)"}]},
        {"title":"Evidence","layout":"chart","elements":[{"id":"plot","kind":"chart","chart":{"kind":"column","categories":["A","B"],"series":[{"name":"Score","values":[72,84]}]},"sourceRefs":["s1"]}],"sourceRefs":["s1"]},
        {"title":"Equation","layout":"equation","elements":[{"id":"eq","kind":"equation","latex":"\\frac{1}{2}+x^2"}]}
    ]})).unwrap()
}
fn sources() -> SourceBundle {
    SourceBundle {
        chunks: vec![SourceChunk {
            reference: SourceReference { id: "s1".into(), name: "results.txt".into(), locator: "section 1".into() },
            text: "A: 72, B: 84".into(),
        }],
        ..Default::default()
    }
}

#[test]
fn research_plan_builds_native_objects_and_survives_save_reopen() {
    let candidate = compiler::compile_plan(&plan(), &sources()).unwrap();
    let doc = &candidate.document;
    assert_eq!(doc.slides.len(), 4);
    assert!(doc.validate().is_empty());
    assert!(doc.slides[0].shapes.iter().any(|s| matches!(s.kind, deckcraft_model::ShapeKind::Connector { .. })));
    assert!(doc.slides[2].shapes.iter().any(|s| matches!(s.kind, deckcraft_model::ShapeKind::Chart(_))));
    assert!(doc.slides[3].shapes.iter().any(|s| s.equation_source.as_deref() == Some(r"\frac{1}{2}+x^2")));
    assert!(doc.slides[0].animations.len() >= 3, "the diagram stages receive ordered entrance effects");
    let reopened = deckcraft_format::load(&deckcraft_format::save(doc).unwrap()).unwrap();
    assert_eq!(&reopened, doc.as_ref());
    assert_eq!(reopened.sources.len(), 1);
}

#[test]
fn six_node_diagram_uses_a_readable_vertical_composition() {
    let plan = DeckPlan {
        title: "Pipeline".into(),
        slides: vec![SlidePlan {
            title: "Six steps in a retrieval pipeline".into(),
            elements: vec![Element {
                id: "pipeline".into(),
                kind: ElementKind::Diagram,
                items: vec!["Question".into(), "Retrieve".into(), "Rank".into(), "Context".into(), "Generate".into(), "Answer".into()],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    let candidate = compiler::compile_plan(&plan, &SourceBundle::default()).unwrap();
    assert!(candidate.diagnostics.iter().all(|diagnostic| !diagnostic.blocking), "{:?}", candidate.diagnostics);
    assert!(candidate.document.slides[0].shapes.iter().filter(|shape| matches!(shape.kind, deckcraft_model::ShapeKind::Shape)).count() >= 6);
}

#[test]
fn chosen_compositions_produce_distinct_editable_visuals() {
    let plan = DeckPlan {
        title: "Visual variety".into(),
        slides: vec![
            SlidePlan {
                title: "A new field".into(),
                layout: "title".into(),
                elements: vec![Element {
                    id: "intro".into(),
                    kind: ElementKind::Text,
                    text: "Several ideas converged.".into(),
                    ..Default::default()
                }],
                ..Default::default()
            },
            SlidePlan {
                title: "Two approaches".into(),
                layout: "comparison".into(),
                elements: vec![
                    Element { id: "left".into(), kind: ElementKind::Text, text: "Rules describe a procedure.".into(), ..Default::default() },
                    Element { id: "right".into(), kind: ElementKind::Text, text: "Learning adjusts from examples.".into(), ..Default::default() },
                ],
                ..Default::default()
            },
            SlidePlan {
                title: "From input to answer".into(),
                layout: "process".into(),
                elements: vec![Element {
                    id: "process".into(),
                    kind: ElementKind::Diagram,
                    items: vec!["Input".into(), "Model".into(), "Answer".into()],
                    ..Default::default()
                }],
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let candidate = compiler::compile_plan(&plan, &SourceBundle::default()).unwrap();
    let slides = &candidate.document.slides;
    let title = slides[0].shapes.iter().find(|shape| shape.text.as_ref().is_some_and(|body| body.text() == "A new field")).unwrap();
    let comparison = slides[1].shapes.iter().find(|shape| shape.text.as_ref().is_some_and(|body| body.text() == "Two approaches")).unwrap();
    assert_ne!(title.xfrm, comparison.xfrm, "layout selection must affect the composition");
    let mut diagram_presets = slides[2]
        .shapes
        .iter()
        .filter(|shape| shape.text.as_ref().is_some_and(|body| ["Input", "Model", "Answer"].contains(&body.text().as_str())))
        .map(|shape| format!("{:?}", shape.geom))
        .collect::<Vec<_>>();
    diagram_presets.sort();
    diagram_presets.dedup();
    assert!(diagram_presets.len() >= 2, "a process should use more than one shape language");
    assert!(slides[2].animations.len() >= 3, "the process should reveal in stages without model-supplied timing");
    assert!(candidate.diagnostics.iter().all(|diagnostic| !diagnostic.blocking), "{:?}", candidate.diagnostics);
}

#[test]
fn authored_geometry_and_reveal_stages_survive_compilation() {
    let plan = DeckPlan {
        title: "Visual scene".into(),
        slides: vec![SlidePlan {
            title: "Two ideas meet".into(),
            layout: "content".into(),
            elements: vec![
                Element {
                    id: "first".into(),
                    kind: ElementKind::Shape,
                    text: "Evidence".into(),
                    preset: "ellipse".into(),
                    rect: Some([118.0, 225.0, 206.0, 92.0]),
                    stage: Some(1),
                    ..Default::default()
                },
                Element {
                    id: "second".into(),
                    kind: ElementKind::Shape,
                    text: "Explanation".into(),
                    preset: "hexagon".into(),
                    rect: Some([574.0, 225.0, 206.0, 92.0]),
                    stage: Some(2),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    };
    let candidate = compiler::compile_plan(&plan, &SourceBundle::default()).unwrap();
    let slide = &candidate.document.slides[0];
    let first = slide.shapes.iter().find(|shape| shape.text.as_ref().is_some_and(|body| body.text() == "Evidence")).unwrap();
    let second = slide.shapes.iter().find(|shape| shape.text.as_ref().is_some_and(|body| body.text() == "Explanation")).unwrap();
    assert_eq!(first.xfrm.map(|placement| (placement.x, placement.y)), Some((118.0, 225.0)));
    assert_eq!(second.xfrm.map(|placement| (placement.x, placement.y)), Some((574.0, 225.0)));
    assert_eq!(slide.animations.iter().map(|animation| animation.shape).collect::<Vec<_>>(), vec![first.id, second.id]);
    assert!(candidate.diagnostics.iter().all(|diagnostic| !diagnostic.blocking), "{:?}", candidate.diagnostics);
}

#[test]
fn process_captions_have_separate_default_positions() {
    let plan = DeckPlan {
        slides: vec![SlidePlan {
            title: "A process with two explanations".into(),
            layout: "process".into(),
            elements: vec![
                Element { id: "flow".into(), kind: ElementKind::Diagram, items: vec!["Input".into(), "Result".into()], ..Default::default() },
                Element { id: "left".into(), text: "First explanation".into(), ..Default::default() },
                Element { id: "right".into(), text: "Second explanation".into(), ..Default::default() },
            ],
            ..Default::default()
        }],
        ..Default::default()
    };
    let candidate = compiler::compile_plan(&plan, &SourceBundle::default()).unwrap();
    let captions = ["First explanation", "Second explanation"]
        .iter()
        .map(|label| {
            candidate.document.slides[0]
                .shapes
                .iter()
                .find(|shape| shape.text.as_ref().is_some_and(|body| body.text() == *label))
                .and_then(|shape| shape.xfrm)
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert!(captions[0].x + captions[0].w < captions[1].x);
}

#[test]
fn editorial_heading_accommodates_a_two_line_title() {
    let plan = DeckPlan {
        slides: vec![SlidePlan {
            title: "Turing’s imitation game: judge the conversation".into(),
            layout: "diagram".into(),
            elements: vec![],
            ..Default::default()
        }],
        ..Default::default()
    };
    let candidate = compiler::compile_plan(&plan, &SourceBundle::default()).unwrap();
    assert!(candidate.diagnostics.iter().all(|diagnostic| !diagnostic.blocking), "{:?}", candidate.diagnostics);
}

#[test]
fn compact_visual_labels_fit_native_shapes() {
    let plan = DeckPlan {
        slides: vec![SlidePlan {
            title: "The 1956 workshop".into(),
            elements: vec![
                Element {
                    id: "year".into(),
                    kind: ElementKind::Shape,
                    text: "1956".into(),
                    preset: "ellipse".into(),
                    rect: Some([150.0, 210.0, 70.0, 42.0]),
                    ..Default::default()
                },
                Element {
                    id: "label".into(),
                    kind: ElementKind::Text,
                    text: "Dartmouth workshop".into(),
                    rect: Some([244.0, 210.0, 166.0, 44.0]),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    };
    let candidate = compiler::compile_plan(&plan, &SourceBundle::default()).unwrap();
    assert!(candidate.diagnostics.iter().all(|diagnostic| !diagnostic.blocking), "{:?}", candidate.diagnostics);
}

#[test]
fn empty_decorative_shape_does_not_report_text_overflow() {
    let plan = DeckPlan {
        slides: vec![SlidePlan {
            title: "Chronology".into(),
            elements: vec![Element {
                id: "chronology".into(),
                kind: ElementKind::Shape,
                preset: "rect".into(),
                rect: Some([120.0, 250.0, 8.0, 8.0]),
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    let candidate = compiler::compile_plan(&plan, &SourceBundle::default()).unwrap();
    assert!(candidate.diagnostics.iter().all(|diagnostic| !diagnostic.blocking), "{:?}", candidate.diagnostics);
}

#[test]
fn single_word_label_does_not_break_inside_ellipse() {
    let plan = DeckPlan {
        slides: vec![SlidePlan {
            title: "An exchange".into(),
            elements: vec![Element {
                id: "interrogator".into(),
                kind: ElementKind::Shape,
                text: "Interrogator".into(),
                preset: "ellipse".into(),
                rect: Some([75.0, 260.0, 190.0, 100.0]),
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    let candidate = compiler::compile_plan(&plan, &SourceBundle::default()).unwrap();
    let slide = &candidate.document.slides[0];
    let shape = slide.shapes.iter().find(|shape| shape.name == "interrogator").unwrap();
    let bounds = shape.xfrm.unwrap();
    let rect = deckcraft_render::shape_geometry(shape, bounds.w, bounds.h).text_rect;
    let ctx = deckcraft_model::resolve::Ctx::for_slide(&candidate.document, slide).unwrap();
    let layout = deckcraft_text::layout(
        &ctx,
        shape,
        shape.text.as_ref().unwrap(),
        &deckcraft_text::Opts { no_shrink: true, rect, fields: &deckcraft_text::NoFields, prompt_color: None },
    );
    assert_eq!(layout.lines.len(), 1);
}

#[test]
fn compiler_rejects_code_split_across_multiple_blocks() {
    let plan = DeckPlan {
        title: "Pipeline".into(),
        slides: vec![SlidePlan {
            title: "Python pipeline".into(),
            layout: "code".into(),
            elements: vec!["question = read()", "context = retrieve(question)"]
                .into_iter()
                .enumerate()
                .map(|(index, text)| Element { id: format!("code-{index}"), kind: ElementKind::Code, text: text.into(), ..Default::default() })
                .collect(),
            ..Default::default()
        }],
        ..Default::default()
    };
    assert!(compiler::compile_plan(&plan, &SourceBundle::default()).is_err());
}

#[test]
fn chart_rejects_invented_values_and_unknown_sources() {
    let mut p = plan();
    p.slides[2].elements[0].chart.as_mut().unwrap().series[0].values[0] = 999.0;
    assert!(compiler::compile_plan(&p, &sources()).is_err());
    let mut p = plan();
    p.slides[2].source_refs = vec!["missing".into()];
    assert!(compiler::compile_plan(&p, &sources()).is_err());
}

#[test]
fn selected_object_edits_preserve_other_objects_and_block_file_commands() {
    let candidate = compiler::compile_plan(&plan(), &sources()).unwrap();
    let id = candidate.document.slides[1].shapes.iter().find(|s| s.text.as_ref().is_some_and(|t| t.text().contains("def predict"))).unwrap().id;
    let request = EditRequest {
        prompt: "Move this".into(),
        scope: EditScope::Objects,
        base: EditBase { document_uid: 1, content_revision: 1 },
        document: candidate.document.clone(),
        slide: 1,
        selected: vec![id],
        sources: sources(),
    };
    let patch =
        EditPatch { summary: "Moved code".into(), commands: vec![CommandCall { command: "shape.move".into(), params: json!({"ids":[id],"dx":10}) }] };
    let edited = compiler::apply_patch(&request, &patch).unwrap();
    for old in &request.document.slides[1].shapes {
        if old.id != id {
            assert_eq!(old, edited.document.slides[1].shapes.iter().find(|s| s.id == old.id).unwrap());
        }
    }
    let forbidden = EditPatch {
        summary: "bad".into(),
        commands: vec![CommandCall { command: "file.open".into(), params: json!({"path":"/tmp/other.deckcraft"}) }],
    };
    assert!(compiler::apply_patch(&request, &forbidden).is_err());
}

#[test]
fn docx_extracts_paragraph_locations_and_tables() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("notes.docx");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    zip.start_file("word/document.xml", zip::write::SimpleFileOptions::default()).unwrap();
    zip.write_all(br#"<w:document xmlns:w="x"><w:body><w:p><w:r><w:t>Research &amp; evidence</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:p><w:r><w:t>A 72</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#).unwrap();
    zip.finish().unwrap();
    let extracted = sources::extract(&path).unwrap();
    assert!(extracted.chunks.iter().any(|c| c.text.contains("Research & evidence")));
    assert!(extracted.chunks.iter().any(|c| c.text.contains("A 72")));
    assert!(extracted.chunks.iter().all(|c| !c.reference.locator.is_empty()));
}

#[test]
fn source_bundle_limits_are_enforced_while_merging_and_extracting_docx_media() {
    let mut bundle = SourceBundle::default();
    let oversized = SourceBundle {
        chunks: vec![SourceChunk {
            reference: SourceReference { id: "large".into(), name: "large.txt".into(), locator: "text".into() },
            text: "x".repeat(sources::MAX_BUNDLE_TEXT_BYTES + 1),
        }],
        ..Default::default()
    };
    assert!(sources::merge(&mut bundle, oversized).is_err());
    assert!(bundle.chunks.is_empty(), "rejected imports must not partially change the attached sources");

    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("oversized.docx");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    zip.start_file("word/document.xml", zip::write::SimpleFileOptions::default()).unwrap();
    zip.write_all(br#"<w:document xmlns:w="x"><w:body/></w:document>"#).unwrap();
    zip.start_file("word/media/large.png", zip::write::SimpleFileOptions::default()).unwrap();
    zip.write_all(&vec![0u8; sources::MAX_BUNDLE_ASSET_BYTES + 1]).unwrap();
    zip.finish().unwrap();
    let error = sources::extract(&path).unwrap_err();
    assert!(error.contains("expanded media exceeds 12 MB"), "{error}");
}

#[tokio::test]
async fn provider_rejects_oversized_prompts_before_network_access() {
    let (events, _) = std::sync::mpsc::channel();
    let error = provider::ask(
        &ProviderProfile { kind: ProviderKind::Compatible, endpoint: "http://127.0.0.1:1/v1".into(), model: "unused".into(), ..Default::default() },
        "",
        "system",
        &"x".repeat((1 << 20) + 1),
        &[],
        &events,
    )
    .await
    .unwrap_err();
    assert!(error.contains("1 MB limit"));
}

#[test]
fn pdf_extracts_searchable_text_and_page_references() {
    let plan = DeckPlan {
        title: "Source test".into(),
        slides: vec![SlidePlan {
            title: "Retrieval pipeline".into(),
            elements: vec![Element {
                id: "summary".into(),
                kind: ElementKind::Text,
                text: "Retrieve passages before generating an answer.".into(),
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    let document = compiler::compile_plan(&plan, &SourceBundle::default()).unwrap().document;
    let bytes = deckcraft_pdf::export(&document, &Default::default()).unwrap();
    let extracted = sources::extract_bytes("sample.pdf", bytes).unwrap();
    assert!(extracted.chunks.iter().any(|chunk| chunk.reference.locator == "page 1" && chunk.text.contains("Retrieve passages")));
    assert_eq!(extracted.assets.len(), 1, "the PDF page is available as a reusable image asset");
}

#[test]
fn latex_renders_and_invalid_latex_is_rejected() {
    let png = equation::render(r"\frac{1}{2}+\sum_{i=1}^{n} x_i^2").unwrap();
    assert!(png.starts_with(b"\x89PNG"));
    assert!(equation::render(r"\unsupportedcommand{x}").is_err());
    assert!(equation::render(r"\input{/etc/passwd}").is_err());
}

#[test]
fn provider_accepts_fenced_json_and_rejects_prose() {
    let parsed: serde_json::Value = provider::parse_json("```json\n{\"ok\":true}\n```").unwrap();
    assert_eq!(parsed["ok"], true);
    assert!(provider::parse_json::<serde_json::Value>("here is a story").is_err());
}

#[tokio::test]
async fn compatible_provider_discovers_models_and_returns_structured_content() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let mut requests = Vec::new();
        for _ in 0..2 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = vec![0; 8192];
            let count = socket.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..count]).to_string();
            let body = if request.starts_with("GET /v1/models ") {
                r#"{"data":[{"id":"mock-model"}]}"#
            } else {
                r#"{"choices":[{"message":{"content":"{\"ok\":true}"}}],"usage":{"prompt_tokens":12,"completion_tokens":4}}"#
            };
            let response =
                format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            socket.write_all(response.as_bytes()).await.unwrap();
            requests.push(request);
        }
        requests
    });
    let profile = ProviderProfile { kind: ProviderKind::Compatible, endpoint, model: "mock-model".into(), ..Default::default() };
    assert_eq!(provider::connect(&profile, "test-token").await.unwrap(), ["mock-model"]);
    let (events, _) = std::sync::mpsc::channel();
    let result = provider::ask(&profile, "test-token", "system", "prompt", &[], &events).await.unwrap();
    assert_eq!(result["ok"], true);
    let requests = server.await.unwrap();
    assert!(requests[0].starts_with("GET /v1/models "));
    assert!(requests[1].starts_with("POST /v1/chat/completions "));
    assert!(requests.iter().all(|request| request.to_ascii_lowercase().contains("authorization: bearer test-token")));
}
