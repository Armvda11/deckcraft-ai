//! Atomic acceptance of a candidate prepared outside the active document.
use super::*;
use crate::{Result, Session};
use serde_json::{Value, json};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!("document.applyCandidate", "Accept AI Edit", [], None, "{documentUid, contentRevision, data: base64 .deckcraft}", has_doc, apply),
        cmd!("equation.upsert", "Insert or Update Equation", [], None, "{id?, latex, data: base64 PNG, rect?}", has_slide, equation),
        cmd!("document.sources", "Source References", [], None, "{sources: [{id,name,locator}]}", has_doc, sources),
        cmd!("shape.annotate", "Object Source References", [], None, "{id, sourceRefs: [source id]}", has_slide, annotate),
    ]
}

fn sources(s: &mut Session, p: &Value) -> Result<Value> {
    let refs: Vec<deckcraft_model::SourceReference> =
        serde_json::from_value(p.get("sources").cloned().unwrap_or_default()).map_err(|e| bad("document.sources", e.to_string()))?;
    if refs.len() > 2000 || refs.iter().any(|r| r.id.is_empty() || r.name.len() > 1024 || r.locator.len() > 1024) {
        return Err(bad("document.sources", "invalid source catalogue"));
    }
    s.edit(|doc, _| {
        doc.sources = refs;
        Ok(())
    })?;
    Ok(json!({}))
}

fn annotate(s: &mut Session, p: &Value) -> Result<Value> {
    let refs: Vec<String> =
        serde_json::from_value(p.get("sourceRefs").cloned().unwrap_or_default()).map_err(|e| bad("shape.annotate", e.to_string()))?;
    if refs.len() > 100 || refs.iter().any(|r| !s.doc().is_ok_and(|d| d.doc.sources.iter().any(|src| &src.id == r))) {
        return Err(bad("shape.annotate", "unknown source reference"));
    }
    edit_shapes(s, p, "shape.annotate", |sh| {
        sh.source_refs = refs.clone();
        Ok(())
    })
}

fn equation(s: &mut Session, p: &Value) -> Result<Value> {
    let latex = str_param(p, "latex")
        .filter(|v| !v.trim().is_empty() && v.len() <= 16_384)
        .ok_or_else(|| bad("equation.upsert", "missing or oversized LaTeX source"))?
        .to_string();
    let bytes = str_param(p, "data")
        .filter(|v| v.len() <= 8 << 20)
        .and_then(base64_decode)
        .ok_or_else(|| bad("equation.upsert", "missing equation render"))?;
    if !bytes.starts_with(b"\x89PNG") || deckcraft_render::decode_image(&bytes).is_none() {
        return Err(bad("equation.upsert", "equation render must be a valid PNG"));
    }
    let target = id_param(p, "id");
    let rect = target
        .and_then(|id| s.doc().ok().and_then(|st| st.shape(id).map(|sh| xfrm_of(&st.doc, &st.selection, sh))))
        .or_else(|| rect_param(p, "rect"));
    if target.is_some_and(|id| !s.doc().is_ok_and(|st| st.shape(id).is_some())) {
        return Err(bad("equation.upsert", "equation no longer exists"));
    }
    let result = s.execute("insert.picture", &json!({"data":p["data"],"name":"Equation.png","rect":rect.map(|r|[r.x,r.y,r.w,r.h])}))?;
    let created = id_param(&result, "id").ok_or_else(|| bad("equation.upsert", "picture was not created"))?;
    s.edit(|doc, sel| {
        let shapes = crate::shapes_mut(doc, sel).ok_or_else(|| bad("equation.upsert", "no slide"))?;
        let kind = deckcraft_model::find_shape(shapes, created)
            .map(|sh| sh.kind.clone())
            .ok_or_else(|| bad("equation.upsert", "missing rendered picture"))?;
        let id = target.unwrap_or(created);
        if let Some(sh) = deckcraft_model::find_shape_mut(shapes, id) {
            sh.kind = kind;
            sh.text = None;
            sh.equation_source = Some(latex.clone());
            sh.descr = latex.clone();
        }
        if target.is_some() && id != created {
            shapes.retain(|sh| sh.id != created);
        }
        sel.shapes = vec![id];
        Ok(json!({"id":id}))
    })
}

fn apply(s: &mut Session, p: &Value) -> Result<Value> {
    let st = s.doc()?;
    if p.get("documentUid").and_then(Value::as_u64) != Some(st.uid) || p.get("contentRevision").and_then(Value::as_u64) != Some(st.content_revision) {
        return Err(bad("document.applyCandidate", "presentation changed: regenerate or open the candidate separately"));
    }
    let data = str_param(p, "data")
        .filter(|v| v.len() <= 64 << 20)
        .and_then(base64_decode)
        .ok_or_else(|| bad("document.applyCandidate", "missing or oversized candidate"))?;
    let candidate = deckcraft_format::load(&data).map_err(|e| bad("document.applyCandidate", e.to_string()))?;
    let problems = candidate.validate();
    if !problems.is_empty() {
        return Err(bad("document.applyCandidate", problems.join(", ")));
    }
    let count = candidate.slides.len();
    s.edit(|doc, _| {
        *doc = candidate;
        Ok(())
    })?;
    Ok(json!({"slides":count}))
}
