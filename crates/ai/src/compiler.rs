//! Deterministic native object composition and scoped edit validation.
use crate::*;
use deckcraft_engine::{DocState, Session};
use deckcraft_model::{Presentation, Shape, ShapeId, ShapeKind};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};

fn run(s: &mut Session, id: &str, p: Value) -> Result<Value, String> {
    s.execute(id, &p).map_err(|e| e.to_string())
}
fn object_id(v: &Value) -> Result<ShapeId, String> {
    v.get("id")
        .and_then(Value::as_u64)
        .and_then(|i| u32::try_from(i).ok())
        .map(ShapeId)
        .ok_or_else(|| "Object creation returned no identifier".into())
}
fn select(s: &mut Session, id: ShapeId) -> Result<(), String> {
    run(s, "text.exit", json!({}))?;
    run(s, "edit.select", json!({"ids":[id]}))?;
    Ok(())
}
fn style(s: &mut Session, id: ShapeId, size: f64, color: &str, mono: bool) -> Result<(), String> {
    select(s, id)?;
    run(s, "format.size", json!({"size":size}))?;
    run(s, "format.color", json!({"color":color}))?;
    run(s, "format.font", json!({"family":if mono{"Liberation Mono"}else{"Inter"}}))?;
    run(s, "format.align", json!({"align":"left"}))?;
    Ok(())
}
fn text(s: &mut Session, t: &str, rect: [f64; 4], size: f64, color: &str) -> Result<ShapeId, String> {
    let id = object_id(&run(s, "insert.textBox", json!({"rect":rect,"text":t}))?)?;
    style(s, id, size, color, false)?;
    run(s, "format.autofit", json!({"mode":"none"}))?;
    run(s, "shape.resize", json!({"ids":[id],"w":rect[2],"h":rect[3]}))?;
    Ok(id)
}
fn box_shape(s: &mut Session, t: &str, rect: [f64; 4], preset: &str, fill: &str, size: f64) -> Result<ShapeId, String> {
    let id = object_id(&run(s, "shape.insert", json!({"preset":preset,"rect":rect,"text":t}))?)?;
    run(s, "shape.fill", json!({"id":id,"color":fill}))?;
    run(s, "shape.line", json!({"id":id,"none":true}))?;
    style(s, id, size, "#ffffff", false)?;
    run(s, "format.autofit", json!({"mode":"none"}))?;
    run(s, "format.align", json!({"align":"center"}))?;
    Ok(id)
}

fn text_overflows(s: &Session, id: ShapeId) -> Result<bool, String> {
    let state = s.doc().map_err(|e| e.to_string())?;
    let slide = state.current_slide().ok_or("No current slide")?;
    let shape = slide.shapes.iter().find(|shape| shape.id == id).ok_or("Missing text shape")?;
    let Some(body) = &shape.text else { return Ok(false) };
    if body.text().trim().is_empty() {
        return Ok(false);
    }
    let Some(ctx) = deckcraft_model::resolve::Ctx::for_slide(&state.doc, slide) else { return Ok(false) };
    let bounds = shape.xfrm.ok_or("Text shape has no rectangle")?;
    let rect = deckcraft_render::shape_geometry(shape, bounds.w, bounds.h).text_rect;
    let layout = deckcraft_text::layout(
        &ctx,
        shape,
        body,
        &deckcraft_text::Opts { no_shrink: true, rect, fields: &deckcraft_text::NoFields, prompt_color: None },
    );
    Ok(layout.overflow || (body.text().split_whitespace().count() == 1 && layout.lines.len() > 1))
}

fn fit_text(s: &mut Session, id: ShapeId, preferred: f64, minimum: f64) -> Result<(), String> {
    let mut size = preferred;
    while text_overflows(s, id)? && size > minimum {
        size = (size - 2.0).max(minimum);
        select(s, id)?;
        run(s, "format.size", json!({"size":size}))?;
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Composition {
    Title,
    Content,
    Comparison,
    Process,
    Timeline,
    Diagram,
    Code,
    Chart,
    Equation,
    Image,
}

impl Composition {
    fn for_slide(slide: &SlidePlan) -> Self {
        match slide.layout.trim().to_ascii_lowercase().as_str() {
            "title" | "cover" | "statement" => Self::Title,
            "comparison" => Self::Comparison,
            "process" => Self::Process,
            "timeline" => Self::Timeline,
            "diagram" | "network" => Self::Diagram,
            "code" => Self::Code,
            "chart" => Self::Chart,
            "equation" => Self::Equation,
            "image" => Self::Image,
            _ => {
                if slide.elements.iter().any(|e| matches!(e.kind, ElementKind::Code)) {
                    Self::Code
                } else if slide.elements.iter().any(|e| matches!(e.kind, ElementKind::Chart)) {
                    Self::Chart
                } else if slide.elements.iter().any(|e| matches!(e.kind, ElementKind::Equation)) {
                    Self::Equation
                } else if slide.elements.iter().any(|e| matches!(e.kind, ElementKind::Image)) {
                    Self::Image
                } else if slide.elements.iter().any(|e| matches!(e.kind, ElementKind::Diagram)) {
                    Self::Diagram
                } else {
                    Self::Content
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
struct Palette {
    paper: &'static str,
    ink: &'static str,
    muted: &'static str,
    primary: &'static str,
    secondary: &'static str,
    tertiary: &'static str,
    line: &'static str,
}

fn palette(theme: &str) -> Palette {
    match theme.to_ascii_lowercase().as_str() {
        "ember" => Palette {
            paper: "#f7f4ec",
            ink: "#192736",
            muted: "#53616b",
            primary: "#b45336",
            secondary: "#39746e",
            tertiary: "#c39947",
            line: "#d7cfc0",
        },
        "meadow" => Palette {
            paper: "#f4f7f1",
            ink: "#19332c",
            muted: "#52695f",
            primary: "#387665",
            secondary: "#a66b42",
            tertiary: "#7d86a8",
            line: "#cddbd1",
        },
        "nocturne" => Palette {
            paper: "#182439",
            ink: "#f5f4ee",
            muted: "#d2d9de",
            primary: "#78b5d8",
            secondary: "#e1a580",
            tertiary: "#a8c69f",
            line: "#4a5971",
        },
        _ => Palette {
            paper: "#f3f6f8",
            ink: "#14243d",
            muted: "#52677b",
            primary: "#3979a0",
            secondary: "#be714f",
            tertiary: "#6f897b",
            line: "#cbd8df",
        },
    }
}

fn mark(s: &mut Session, preset: &str, rect: [f64; 4], fill: &str) -> Result<ShapeId, String> {
    let id = object_id(&run(s, "shape.insert", json!({"preset":preset,"rect":rect}))?)?;
    run(s, "shape.fill", json!({"id":id,"color":fill}))?;
    run(s, "shape.line", json!({"id":id,"none":true}))?;
    Ok(id)
}

fn heading(s: &mut Session, slide: &SlidePlan, index: usize, composition: Composition, p: Palette) -> Result<(), String> {
    if composition == Composition::Title {
        mark(s, "ellipse", [735.0, 160.0, 174.0, 174.0], p.secondary)?;
        mark(s, "rect", [66.0, 58.0, 82.0, 6.0], p.primary)?;
        let size = if slide.title.chars().count() > 55 { 40.0 } else { 52.0 };
        let id = text(s, &slide.title, [66.0, 92.0, 660.0, 186.0], size, p.ink)?;
        select(s, id)?;
        run(s, "format.bold", json!({"on":true}))?;
    } else {
        let (x, y, w) = if composition == Composition::Comparison { (74.0, 34.0, 780.0) } else { (68.0, 35.0, 820.0) };
        mark(s, "rect", [x, 24.0, if composition == Composition::Timeline { 130.0 } else { 55.0 }, 5.0], p.primary)?;
        let size = if slide.title.chars().count() > 62 { 30.0 } else { 35.0 };
        let id = text(s, &slide.title, [x, y, w, 110.0], size, p.ink)?;
        select(s, id)?;
        run(s, "format.bold", json!({"on":true}))?;
        if composition == Composition::Comparison {
            mark(s, "rect", [476.0, 169.0, 3.0, 282.0], p.line)?;
            mark(s, "ellipse", [74.0, 154.0, 26.0, 26.0], p.primary)?;
            mark(s, "diamond", [514.0, 154.0, 26.0, 26.0], p.secondary)?;
        }
    }
    text(s, &format!("{:02}", index + 1), [869.0, 500.0, 42.0, 27.0], 12.0, p.muted)?;
    Ok(())
}

fn slot(composition: Composition, position: usize, caption_position: usize, count: usize, has_diagram: bool, kind: &ElementKind) -> [f64; 4] {
    if composition == Composition::Title {
        return if position == 0 { [72.0, 316.0, 650.0, 132.0] } else { [72.0, 454.0, 650.0, 40.0] };
    }
    if composition == Composition::Comparison && count == 2 {
        return if position == 0 { [74.0, 194.0, 370.0, 254.0] } else { [514.0, 194.0, 370.0, 254.0] };
    }
    if matches!(composition, Composition::Process | Composition::Timeline | Composition::Diagram) && has_diagram {
        return if count == 1 {
            [70.0, 173.0, 820.0, 292.0]
        } else if matches!(kind, ElementKind::Diagram) {
            [70.0, 185.0, 820.0, 218.0]
        } else {
            let captions = count.saturating_sub(1).max(1);
            let gap = 14.0;
            let width = (820.0 - gap * (captions - 1) as f64) / captions as f64;
            [70.0 + caption_position as f64 * (width + gap), 410.0, width, 72.0]
        };
    }
    if matches!(composition, Composition::Code | Composition::Chart | Composition::Equation | Composition::Image) {
        if count == 1 {
            return [102.0, 158.0, 756.0, 309.0];
        }
        return if matches!(kind, ElementKind::Code | ElementKind::Chart | ElementKind::Equation | ElementKind::Image) {
            [322.0, 162.0, 566.0, 302.0]
        } else {
            [68.0, 192.0, 222.0, 250.0]
        };
    }
    match count {
        1 => [76.0, 169.0, 810.0, 285.0],
        2 => {
            if position == 0 {
                [76.0, 183.0, 420.0, 265.0]
            } else {
                [537.0, 214.0, 343.0, 232.0]
            }
        }
        3 => match position {
            0 => [76.0, 166.0, 800.0, 116.0],
            1 => [76.0, 306.0, 366.0, 155.0],
            _ => [510.0, 306.0, 366.0, 155.0],
        },
        _ => {
            let col = position % 2;
            let row = position / 2;
            let rows = count.div_ceil(2);
            let height = (306.0 - 18.0 * (rows.saturating_sub(1)) as f64) / rows as f64;
            [76.0 + col as f64 * 410.0, 158.0 + row as f64 * (height + 18.0), 380.0, height]
        }
    }
}

pub fn compile_plan(plan: &DeckPlan, bundle: &SourceBundle) -> Result<Candidate, String> {
    if plan.slides.is_empty() || plan.slides.len() > 30 {
        return Err("A draft must contain 1 to 30 slides".into());
    }
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"blank":true,"theme":if plan.theme.is_empty(){"Harbor"}else{&plan.theme}}))?;
    run(&mut s, "file.properties", json!({"title":plan.title}))?;
    run(&mut s, "document.sources", json!({"sources":bundle.chunks.iter().map(|c|&c.reference).collect::<Vec<_>>()}))?;
    for (index, slide) in plan.slides.iter().enumerate() {
        compose(&mut s, slide, index, bundle, &plan.theme)?;
    }
    let document = s.doc().map_err(|e| e.to_string())?.doc.clone();
    let diagnostics = diagnostics(&document);
    Ok(Candidate { document, base: None, summary: format!("Created {} editable slides", plan.slides.len()), diagnostics })
}

fn compose(s: &mut Session, slide: &SlidePlan, index: usize, bundle: &SourceBundle, theme: &str) -> Result<(), String> {
    if slide.title.trim().is_empty() || slide.title.len() > 200 || slide.elements.len() > 12 {
        return Err(format!("Slide {} needs a short title and at most 12 elements", index + 1));
    }
    let code_count = slide.elements.iter().filter(|element| matches!(element.kind, ElementKind::Code)).count();
    if code_count > 1 {
        return Err("Use one editable code element per slide and combine the example into a single short block".into());
    }
    if let Some(code) = slide.elements.iter().find(|element| matches!(element.kind, ElementKind::Code))
        && (code.text.lines().count() > 8 || code.text.lines().any(|line| line.chars().count() > 72))
    {
        return Err("Keep the code block to at most 8 lines and 72 characters per line".into());
    }
    sources::check_refs(&slide.source_refs, bundle)?;
    let mut names = HashSet::new();
    for e in &slide.elements {
        if e.id.is_empty() || !names.insert(e.id.clone()) || e.text.len() > 12_000 || e.items.len() > 20 {
            return Err("Element IDs must be unique and content must fit a slide".into());
        }
        if e.stage.is_some_and(|stage| stage > 8) {
            return Err("Reveal stages must be between 1 and 8".into());
        }
        sources::check_refs(&e.source_refs, bundle)?;
        if e.rect.is_some_and(|r| r.iter().any(|v| !v.is_finite()) || r[2] <= 0.0 || r[3] <= 0.0 || r[2] > 960.0 || r[3] > 540.0) {
            return Err("Invalid element rectangle".into());
        }
    }
    run(s, "slide.new", json!({"layout":"blank"}))?;
    let composition = Composition::for_slide(slide);
    let palette = palette(theme);
    run(s, "design.background", json!({"color":palette.paper}))?;
    heading(s, slide, index, composition, palette)?;
    let n = slide.elements.iter().filter(|e| !matches!(e.kind, ElementKind::Connector)).count().max(1);
    let has_diagram = slide.elements.iter().any(|e| matches!(e.kind, ElementKind::Diagram));
    let mut ids: HashMap<String, Vec<ShapeId>> = HashMap::new();
    let mut pos = 0usize;
    let mut caption_pos = 0usize;
    for e in &slide.elements {
        if matches!(e.kind, ElementKind::Connector) {
            continue;
        }
        let rect = e.rect.unwrap_or_else(|| slot(composition, pos, caption_pos, n, has_diagram, &e.kind));
        pos += 1;
        if !matches!(e.kind, ElementKind::Diagram) {
            caption_pos += 1;
        }
        let made = element(s, e, rect, bundle, composition, palette, pos)?;
        for (part, id) in made.iter().enumerate() {
            let name = if made.len() == 1 { e.id.clone() } else { format!("{} part {}", e.id, part + 1) };
            run(s, "shape.rename", json!({"id":id,"name":name}))?;
            if !e.source_refs.is_empty() {
                run(s, "shape.annotate", json!({"id":id,"sourceRefs":e.source_refs}))?;
            }
        }
        ids.insert(e.id.clone(), made);
    }
    for e in &slide.elements {
        if matches!(e.kind, ElementKind::Connector) {
            let from = ids.get(&e.from).and_then(|v| v.first()).ok_or("Connector has no source element")?;
            let to = ids.get(&e.to).and_then(|v| v.first()).ok_or("Connector has no destination element")?;
            let id = object_id(&run(s, "shape.connect", json!({"from":from,"to":to,"preset":"bentConnector3"}))?)?;
            ids.insert(e.id.clone(), vec![id]);
        }
    }
    if !slide.transition.is_empty() {
        run(s, "transition.set", json!({"kind":slide.transition,"duration":450}))?;
    }
    for a in &slide.animations {
        let targets = ids.get(&a.target).ok_or_else(|| format!("Animation targets missing element {}", a.target))?;
        for (i, target) in targets.iter().enumerate() {
            run(
                s,
                "animation.add",
                json!({"ids":[target],"effect":if a.effect.is_empty(){"fade"}else{&a.effect},"class":if a.class.is_empty(){"entrance"}else{&a.class},"start":if i % 2 == 1 && targets.len() > 1 { "withPrevious" } else if a.start.is_empty(){"onClick"}else{&a.start},"duration":a.duration.clamp(100,10_000),"delay":a.delay.min(10_000),"option":a.option}),
            )?;
            if a.by_paragraph {
                run(
                    s,
                    "animation.options",
                    json!({"index":s.doc().map_err(|e|e.to_string())?.current_slide().map(|sl|sl.animations.len().saturating_sub(1)).unwrap_or(i),"textBuild":"byParagraph"}),
                )?;
            }
        }
    }
    let mut stages: BTreeMap<u8, Vec<ShapeId>> = BTreeMap::new();
    for e in &slide.elements {
        if let Some(stage) = e.stage.filter(|stage| *stage > 0)
            && !slide.animations.iter().any(|a| a.target == e.id)
            && let Some(targets) = ids.get(&e.id)
        {
            stages.entry(stage).or_default().extend(targets);
        }
    }
    for targets in stages.values() {
        for (i, target) in targets.iter().enumerate() {
            run(
                s,
                "animation.add",
                json!({"ids":[target],"effect":"fade","class":"entrance","start":if i == 0 { "onClick" } else { "withPrevious" },"duration":420}),
            )?;
        }
    }
    for e in &slide.elements {
        if matches!(e.kind, ElementKind::Diagram)
            && e.stage.is_none()
            && !slide.animations.iter().any(|a| a.target == e.id)
            && let Some(targets) = ids.get(&e.id)
        {
            for (i, target) in targets.iter().enumerate() {
                run(
                    s,
                    "animation.add",
                    json!({"ids":[target],"effect":"fade","class":"entrance","start":if i % 2 == 1 { "withPrevious" } else { "onClick" },"duration":420}),
                )?;
            }
        }
    }
    let mut refs = slide.source_refs.clone();
    for e in &slide.elements {
        for r in &e.source_refs {
            if !refs.contains(r) {
                refs.push(r.clone());
            }
        }
    }
    let citations: Vec<String> = refs
        .iter()
        .filter_map(|r| bundle.chunks.iter().find(|c| &c.reference.id == r))
        .map(|c| format!("{} ({})", c.reference.name, c.reference.locator))
        .collect();
    let notes = if citations.is_empty() {
        format!("{}\n\nAI draft: review factual claims before presenting.", slide.notes)
    } else {
        format!("{}\n\nSources:\n{}", slide.notes, citations.join("\n"))
    };
    run(s, "slide.notes", json!({"text":notes}))?;
    if !citations.is_empty() {
        let footer = citations.join(" · ");
        let short: String = footer.chars().take(120).collect();
        text(s, &short, [60.0, 501.0, 780.0, 24.0], 10.0, palette.muted)?;
    }
    Ok(())
}

fn diagram(s: &mut Session, e: &Element, r: [f64; 4], composition: Composition, p: Palette) -> Result<Vec<ShapeId>, String> {
    if e.items.is_empty() || e.items.len() > 6 {
        return Err("A diagram needs 1 to 6 concise node labels".into());
    }
    let n = e.items.len();
    let mode = match e.preset.to_ascii_lowercase().as_str() {
        "flow" | "radial" | "timeline" | "staircase" => e.preset.to_ascii_lowercase(),
        _ if composition == Composition::Timeline => "timeline".into(),
        _ if composition == Composition::Diagram && n >= 4 => "radial".into(),
        _ if composition == Composition::Diagram => "staircase".into(),
        _ => "flow".into(),
    };
    let mut nodes = Vec::with_capacity(n);
    let accents = [p.primary, p.secondary, p.tertiary, p.ink];
    for (i, label) in e.items.iter().enumerate() {
        let (rect, preset) = if mode == "radial" && (3..=5).contains(&n) {
            if i == 0 {
                ([r[0] + r[2] / 2.0 - 98.0, r[1] + r[3] / 2.0 - 47.0, 196.0, 94.0], "roundRect")
            } else {
                let positions = [
                    [r[0] + 18.0, r[1] + 20.0, 168.0, 80.0],
                    [r[0] + r[2] - 186.0, r[1] + 20.0, 168.0, 80.0],
                    [r[0] + 18.0, r[1] + r[3] - 100.0, 168.0, 80.0],
                    [r[0] + r[2] - 186.0, r[1] + r[3] - 100.0, 168.0, 80.0],
                ];
                (positions[i - 1], ["ellipse", "hexagon", "roundRect", "ellipse"][i - 1])
            }
        } else if mode == "timeline" && n <= 4 {
            let gap = 24.0;
            let w = ((r[2] - gap * (n.saturating_sub(1)) as f64) / n as f64).min(190.0);
            let start = r[0] + (r[2] - n as f64 * w - (n.saturating_sub(1)) as f64 * gap) / 2.0;
            let y = if i % 2 == 0 { r[1] + 26.0 } else { r[1] + r[3] - 110.0 };
            ([start + i as f64 * (w + gap), y, w, 84.0], ["ellipse", "roundRect", "hexagon", "rect"][i % 4])
        } else if mode == "staircase" && n <= 4 {
            let gap = 28.0;
            let w = ((r[2] - gap * (n.saturating_sub(1)) as f64) / n as f64).min(205.0);
            let start = r[0] + (r[2] - n as f64 * w - (n.saturating_sub(1)) as f64 * gap) / 2.0;
            ([start + i as f64 * (w + gap), r[1] + 24.0 + i as f64 * 33.0, w, 84.0], ["roundRect", "ellipse", "hexagon", "rect"][i % 4])
        } else if n <= 4 {
            let gap = 28.0;
            let w = ((r[2] - gap * (n.saturating_sub(1)) as f64) / n as f64).min(218.0);
            let start = r[0] + (r[2] - n as f64 * w - (n.saturating_sub(1)) as f64 * gap) / 2.0;
            ([start + i as f64 * (w + gap), r[1] + (r[3] - 98.0) / 2.0, w, 98.0], ["ellipse", "hexagon", "roundRect", "rect"][i % 4])
        } else {
            let gap = 9.0;
            let h = (r[3] - gap * (n - 1) as f64) / n as f64;
            ([r[0] + (r[2] - 565.0) / 2.0, r[1] + i as f64 * (h + gap), 565.0, h], ["rect", "roundRect", "hexagon"][i % 3])
        };
        let size = if label.chars().count() > 18 {
            15.0
        } else if label.chars().count() > 11 {
            17.0
        } else {
            19.0
        };
        nodes.push(box_shape(s, label, rect, preset, accents[i % accents.len()], size)?);
    }
    let mut result = Vec::with_capacity(n.saturating_mul(2).saturating_sub(1));
    if let Some(first) = nodes.first() {
        result.push(*first);
    }
    for i in 1..nodes.len() {
        let from = if mode == "radial" && (3..=5).contains(&n) { nodes[0] } else { nodes[i - 1] };
        let connector = object_id(&run(s, "shape.connect", json!({"from":from,"to":nodes[i],"preset":"straightConnector1"}))?)?;
        result.push(connector);
        result.push(nodes[i]);
    }
    Ok(result)
}

fn element(
    s: &mut Session,
    e: &Element,
    r: [f64; 4],
    bundle: &SourceBundle,
    composition: Composition,
    p: Palette,
    position: usize,
) -> Result<Vec<ShapeId>, String> {
    let color = if e.color.is_empty() { p.ink } else { &e.color };
    let default_size = if composition == Composition::Title {
        27.0
    } else if composition == Composition::Comparison {
        25.0
    } else {
        23.0
    };
    let font = e.font_size.unwrap_or(default_size).clamp(18.0, 48.0);
    let id = match e.kind {
        ElementKind::Text => {
            let id = text(s, &e.text, r, font, color)?;
            fit_text(s, id, font, if r[3] < 60.0 { 14.0 } else { 16.0 })?;
            id
        }
        ElementKind::Bullets => {
            let id = text(s, &e.items.join("\n"), r, font, color)?;
            run(s, "format.bullets", json!({"on":true}))?;
            fit_text(s, id, font, 16.0)?;
            id
        }
        ElementKind::Code => {
            let id = text(s, &e.text, r, e.font_size.unwrap_or(18.0).clamp(14.0, 24.0), "#e2e8f0")?;
            run(s, "shape.fill", json!({"id":id,"color":"#14243d"}))?;
            style(s, id, e.font_size.unwrap_or(18.0).clamp(14.0, 24.0), "#e2e8f0", true)?;
            id
        }
        ElementKind::Shape => {
            let id =
                object_id(&run(s, "shape.insert", json!({"preset":if e.preset.is_empty(){"roundRect"}else{&e.preset},"rect":r,"text":e.text}))?)?;
            let fill = if e.fill.is_empty() { [p.primary, p.secondary, p.tertiary][position % 3] } else { &e.fill };
            run(s, "shape.fill", json!({"id":id,"color":fill}))?;
            style(s, id, font, if e.color.is_empty() { "#ffffff" } else { color }, false)?;
            fit_text(s, id, font, 14.0)?;
            id
        }
        ElementKind::Diagram => return diagram(s, e, r, composition, p),
        ElementKind::Chart => {
            let c = e.chart.as_ref().ok_or("Chart has no data")?;
            if c.categories.is_empty()
                || c.categories.len() > 30
                || c.series.is_empty()
                || c.series.len() > 8
                || c.series.iter().any(|v| v.values.len() != c.categories.len())
            {
                return Err("Chart categories and series must have matching lengths".into());
            }
            sources::verify_numbers(&c.series.iter().flat_map(|v| v.values.iter().copied()).collect::<Vec<_>>(), &e.source_refs, bundle)?;
            object_id(&run(
                s,
                "insert.chart",
                json!({"type":if c.kind.is_empty(){"column"}else{&c.kind},"rect":r,"categories":c.categories,"series":c.series,"title":e.text}),
            )?)?
        }
        ElementKind::Table => {
            let rows = e.cells.len();
            let cols = e.cells.first().map(Vec::len).unwrap_or(0);
            if rows == 0 || rows > 20 || cols == 0 || cols > 8 || e.cells.iter().any(|row| row.len() != cols) {
                return Err("Table must be rectangular, at most 20 by 8 cells".into());
            }
            let id = object_id(&run(s, "insert.table", json!({"rows":rows,"cols":cols,"rect":r}))?)?;
            for (row, cells) in e.cells.iter().enumerate() {
                for (col, value) in cells.iter().enumerate() {
                    run(s, "text.set", json!({"id":id,"cell":[row,col],"text":value}))?;
                }
            }
            id
        }
        ElementKind::Image => {
            let asset = bundle.assets.iter().find(|a| a.id == e.asset).ok_or_else(|| format!("Unknown image asset: {}", e.asset))?;
            object_id(&run(s, "insert.picture", json!({"name":asset.name,"data":deckcraft_engine::cmd::base64_encode(&asset.bytes),"rect":r}))?)?
        }
        ElementKind::Equation => {
            let png = equation::render(&e.latex)?;
            let (w, h) = image::ImageReader::new(std::io::Cursor::new(&png))
                .with_guessed_format()
                .map_err(|e| e.to_string())?
                .into_dimensions()
                .map_err(|e| e.to_string())?;
            let scale = (r[2] / f64::from(w)).min(r[3] / f64::from(h));
            let rect = [r[0], r[1] + (r[3] - f64::from(h) * scale) / 2.0, f64::from(w) * scale, f64::from(h) * scale];
            object_id(&run(s, "equation.upsert", json!({"latex":e.latex,"data":deckcraft_engine::cmd::base64_encode(&png),"rect":rect}))?)?
        }
        ElementKind::Connector => return Err("Connectors must be compiled after their targets".into()),
    };
    Ok(vec![id])
}

pub fn diagnostics(doc: &Presentation) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for (i, slide) in doc.slides.iter().enumerate() {
        let ctx = deckcraft_model::resolve::Ctx::for_slide(doc, slide);
        for sh in &slide.shapes {
            if let Some(r) = sh.xfrm
                && (r.x < -0.5 || r.y < -0.5 || r.x + r.w > doc.slide_size.width + 0.5 || r.y + r.h > doc.slide_size.height + 0.5)
            {
                out.push(Diagnostic { slide: i, message: format!("{} extends outside the slide", sh.name), blocking: true });
            }
            if let (Some(ctx), Some(body)) = (&ctx, &sh.text)
                && !body.text().trim().is_empty()
            {
                let bounds = sh.xfrm.map(|r| (r.w, r.h)).unwrap_or((100.0, 100.0));
                let rect = deckcraft_render::shape_geometry(sh, bounds.0, bounds.1).text_rect;
                let layout = deckcraft_text::layout(
                    ctx,
                    sh,
                    body,
                    &deckcraft_text::Opts { no_shrink: true, rect, fields: &deckcraft_text::NoFields, prompt_color: None },
                );
                if layout.overflow || (body.text().split_whitespace().count() == 1 && layout.lines.len() > 1) {
                    let excerpt: String = body.text().chars().take(72).collect();
                    out.push(Diagnostic { slide: i, message: format!("Text overflows in element {}: {excerpt}", sh.name), blocking: true });
                }
            }
        }
        for a in &slide.animations {
            if !slide.shapes.iter().any(|s| s.id == a.shape) {
                out.push(Diagnostic { slide: i, message: "Animation target is missing".into(), blocking: true });
            }
        }
    }
    out
}

const EDIT_COMMANDS: &[&str] = &[
    "text.set",
    "shape.move",
    "shape.resize",
    "shape.rotate",
    "shape.fill",
    "shape.line",
    "shape.rename",
    "shape.visible",
    "shape.lock",
    "shape.insert",
    "shape.connect",
    "insert.textBox",
    "insert.table",
    "insert.chart",
    "chart.type",
    "chart.data",
    "chart.options",
    "table.style",
    "table.insertRowAbove",
    "table.insertRowBelow",
    "table.insertColumnLeft",
    "table.insertColumnRight",
    "table.deleteRow",
    "table.deleteColumn",
    "format.font",
    "format.size",
    "format.color",
    "format.bold",
    "format.italic",
    "format.align",
    "format.bullets",
    "format.paragraph",
    "format.autofit",
    "animation.add",
    "animation.set",
    "animation.timing",
    "animation.options",
    "animation.remove",
    "animation.move",
    "transition.set",
    "transition.timing",
    "transition.options",
    "slide.notes",
    "equation.upsert",
    "edit.select",
    "edit.delete",
    "arrange.align",
    "arrange.distribute",
    "arrange.group",
    "arrange.ungroup",
];
const DECK_COMMANDS: &[&str] = &["slide.new", "slide.delete", "slide.go", "slide.move", "design.theme", "design.background"];
const OBJECT_SCOPE_BLOCKED: &[&str] = &[
    "shape.insert",
    "shape.connect",
    "insert.textBox",
    "insert.table",
    "insert.chart",
    "arrange.group",
    "arrange.ungroup",
    "transition.set",
    "transition.options",
    "transition.timing",
    "slide.notes",
];

pub fn edit_command_allowed(command: &str, scope: EditScope) -> bool {
    (EDIT_COMMANDS.contains(&command) || (scope == EditScope::Deck && DECK_COMMANDS.contains(&command)))
        && (scope != EditScope::Objects || !OBJECT_SCOPE_BLOCKED.contains(&command))
}

pub fn apply_patch(request: &EditRequest, patch: &EditPatch) -> Result<Candidate, String> {
    if patch.commands.is_empty() || patch.commands.len() > 200 {
        return Err("An edit must contain 1 to 200 object commands".into());
    }
    if request.scope == EditScope::Objects && request.selected.is_empty() {
        return Err("Select objects to edit".into());
    }
    let mut s = Session::new();
    s.add_document(DocState::new((*request.document).clone(), None, "AI preview".into()));
    run(&mut s, "slide.go", json!({"index":request.slide}))?;
    if !request.selected.is_empty() {
        run(&mut s, "edit.select", json!({"ids":request.selected}))?;
    }
    for call in &patch.commands {
        if !edit_command_allowed(&call.command, request.scope) {
            return Err(format!("AI cannot execute {}", call.command));
        }
        if request.scope == EditScope::Objects && OBJECT_SCOPE_BLOCKED.contains(&call.command.as_str()) {
            return Err("This change needs slide scope rather than selected objects".into());
        }
        let mut params = call.params.clone();
        if call.command == "equation.upsert" {
            if request.scope == EditScope::Objects
                && !params.get("id").and_then(Value::as_u64).is_some_and(|id| request.selected.contains(&ShapeId(id as u32)))
            {
                return Err("Equation edit is outside the selected objects".into());
            }
            let latex = params.get("latex").and_then(Value::as_str).ok_or("Equation edit needs LaTeX source")?;
            let png = equation::render(latex)?;
            params["data"] = json!(deckcraft_engine::cmd::base64_encode(&png));
        }
        run(&mut s, &call.command, params)?;
    }
    let document = s.doc().map_err(|e| e.to_string())?.doc.clone();
    if request.scope != EditScope::Deck {
        if document.slides.len() != request.document.slides.len()
            || document.masters != request.document.masters
            || document.props != request.document.props
        {
            return Err("AI attempted a change outside the requested slide".into());
        }
        for (i, (old, new)) in request.document.slides.iter().zip(&document.slides).enumerate() {
            if i != request.slide && old != new {
                return Err("AI attempted to modify another slide".into());
            }
            if i == request.slide && request.scope == EditScope::Objects {
                if !objects_preserved(&old.shapes, &new.shapes, &request.selected) {
                    return Err("AI attempted to modify unselected objects".into());
                }
                let ids: HashSet<_> = request.selected.iter().copied().collect();
                if old.animations.iter().filter(|a| !ids.contains(&a.shape)).collect::<Vec<_>>()
                    != new.animations.iter().filter(|a| !ids.contains(&a.shape)).collect::<Vec<_>>()
                    || old.notes != new.notes
                    || old.transition != new.transition
                {
                    return Err("AI attempted to change content outside the selected objects".into());
                }
            }
        }
    }
    let problems = document.validate();
    if !problems.is_empty() {
        return Err(problems.join(", "));
    }
    Ok(Candidate { diagnostics: diagnostics(&document), document, base: Some(request.base.clone()), summary: patch.summary.clone() })
}

fn objects_preserved(old: &[Shape], new: &[Shape], selected: &[ShapeId]) -> bool {
    let old_ids: HashSet<_> = old.iter().map(|s| s.id).collect();
    if new.iter().any(|s| !old_ids.contains(&s.id)) {
        return false;
    }
    for a in old {
        if selected.contains(&a.id) {
            continue;
        }
        let Some(b) = new.iter().find(|s| s.id == a.id) else {
            return false;
        };
        if let (ShapeKind::Group { children: ac, .. }, ShapeKind::Group { children: bc, .. }) = (&a.kind, &b.kind) {
            let (mut aa, mut bb) = (a.clone(), b.clone());
            if let ShapeKind::Group { children, .. } = &mut aa.kind {
                children.clear();
            }
            if let ShapeKind::Group { children, .. } = &mut bb.kind {
                children.clear();
            }
            if aa != bb || !objects_preserved(ac, bc, selected) {
                return false;
            }
        } else if a != b {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod edit_command_tests {
    use super::*;

    #[test]
    fn edit_prompt_and_validator_share_scope_allowlist() {
        for command in ["slide.new", "slide.delete", "slide.go", "slide.move", "design.theme", "design.background"] {
            assert!(!edit_command_allowed(command, EditScope::Slide));
            assert!(edit_command_allowed(command, EditScope::Deck));
        }
        assert!(edit_command_allowed("insert.table", EditScope::Slide));
        assert!(edit_command_allowed("table.insertRowAbove", EditScope::Slide));
        assert!(edit_command_allowed("shape.move", EditScope::Objects));
        assert!(!edit_command_allowed("shape.insert", EditScope::Objects));
        assert!(!edit_command_allowed("file.open", EditScope::Deck));
    }
}
