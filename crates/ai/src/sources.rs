//! Local, bounded extraction. Source documents are evidence, never instructions.
use crate::*;
use deckcraft_model::SourceReference;
use quick_xml::{Reader, events::Event};
use std::{
    hash::{Hash, Hasher},
    io::{Cursor, Read},
    path::Path,
};

const MAX_FILE: u64 = 25 << 20;
pub const MAX_BUNDLE_TEXT_BYTES: usize = 2 << 20;
pub const MAX_BUNDLE_ASSET_BYTES: usize = 12 << 20;
pub const MAX_BUNDLE_ASSETS: usize = 32;

pub fn extract(path: &Path) -> Result<SourceBundle, String> {
    if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > MAX_FILE {
        return Err("Source exceeds 25 MB".into());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    extract_bytes(&name, bytes)
}

pub fn extract_bytes(name: &str, bytes: Vec<u8>) -> Result<SourceBundle, String> {
    if bytes.len() as u64 > MAX_FILE {
        return Err("Source exceeds 25 MB".into());
    }
    let name = name.to_string();
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut hash);
    bytes.hash(&mut hash);
    let prefix = format!("s{:x}", hash.finish());
    let ext = Path::new(&name).extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let mut out = SourceBundle::default();
    match ext.as_str() {
        "pdf" => {
            let pages = pdf_extract::extract_text_from_mem_by_pages(&bytes).map_err(|e| format!("PDF extraction: {e}"))?;
            if pages.len() > 200 {
                return Err("PDF exceeds 200 pages; attach a shorter section".into());
            }
            for (i, text) in pages.into_iter().enumerate() {
                chunks(&mut out, &prefix, &name, &format!("page {}", i + 1), &text);
            }
            if out.chunks.is_empty() {
                return Err("This PDF has no extractable text. OCR is not available; attach text notes or page images.".into());
            }
            // Page images are reusable assets. Limit previews to keep long documents inexpensive.
            if let Ok(pdf) = hayro::hayro_syntax::Pdf::new(bytes) {
                let cache = hayro::RenderCache::new();
                for (i, page) in pdf.pages().iter().take(12).enumerate() {
                    let (w, h) = page.render_dimensions();
                    if !w.is_finite() || !h.is_finite() || w <= 0.0 || h <= 0.0 {
                        continue;
                    }
                    let scale = 1024.0 / w.max(h);
                    let pixmap = hayro::render(
                        page,
                        &cache,
                        &Default::default(),
                        &hayro::RenderSettings { x_scale: scale, y_scale: scale, ..Default::default() },
                    );
                    if let Ok(png) = pixmap.into_png() {
                        out.assets.push(Asset { id: format!("{prefix}-page-{}", i + 1), name: format!("{name} — page {}", i + 1), bytes: png });
                    }
                }
            }
        }
        "docx" => {
            let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
            let xml = read_entry(&mut archive, "word/document.xml", 4 << 20)?;
            let mut reader = Reader::from_reader(xml.as_slice());
            let mut paragraph = String::new();
            let mut index = 0usize;
            loop {
                match reader.read_event().map_err(|e| format!("DOCX XML: {e}"))? {
                    Event::Text(t) => paragraph.push_str(&t.decode().map_err(|e| e.to_string())?),
                    Event::GeneralRef(r) => {
                        let entity = r.decode().map_err(|e| e.to_string())?;
                        let encoded = format!("&{entity};");
                        paragraph.push_str(&quick_xml::escape::unescape(&encoded).map_err(|e| e.to_string())?);
                    }
                    Event::End(e) if e.local_name().as_ref() == b"p" => {
                        index += 1;
                        chunks(&mut out, &prefix, &name, &format!("paragraph {index}"), &paragraph);
                        paragraph.clear();
                    }
                    Event::Empty(e) if matches!(e.local_name().as_ref(), b"tab" | b"br") => paragraph.push(' '),
                    Event::Eof => break,
                    _ => {}
                }
            }
            let entries: Vec<String> =
                archive.file_names().filter(|n| n.starts_with("word/media/")).take(MAX_BUNDLE_ASSETS + 1).map(String::from).collect();
            if entries.len() > MAX_BUNDLE_ASSETS {
                return Err(format!("DOCX contains more than {MAX_BUNDLE_ASSETS} media assets"));
            }
            let mut expanded_media = 0usize;
            for (i, entry) in entries.iter().enumerate() {
                let size = archive.by_name(entry).map_err(|e| e.to_string())?.size() as usize;
                expanded_media = expanded_media.checked_add(size).ok_or("DOCX media size overflow")?;
                if expanded_media > MAX_BUNDLE_ASSET_BYTES {
                    return Err("DOCX expanded media exceeds 12 MB".into());
                }
                let data = read_entry(&mut archive, entry, 8 << 20)?;
                if valid_image(&data) {
                    out.assets.push(Asset { id: format!("{prefix}-image-{i}"), name: entry.clone(), bytes: data });
                }
            }
        }
        "txt" | "md" | "markdown" | "csv" => {
            let text = String::from_utf8(bytes).map_err(|_| "Source must be UTF-8 text".to_string())?;
            chunks(&mut out, &prefix, &name, "text", &text);
        }
        "png" | "jpg" | "jpeg" | "webp" | "gif" => {
            if !valid_image(&bytes) {
                return Err("Image is unreadable or exceeds 32 megapixels".into());
            }
            out.assets.push(Asset { id: format!("{prefix}-image"), name, bytes });
        }
        _ => return Err("Supported sources: PDF, DOCX, Markdown, text, CSV, PNG, JPEG, WebP and GIF".into()),
    }
    validate_bundle(&out)?;
    Ok(out)
}

fn read_entry(archive: &mut zip::ZipArchive<Cursor<Vec<u8>>>, name: &str, max: u64) -> Result<Vec<u8>, String> {
    let entry = archive.by_name(name).map_err(|e| e.to_string())?;
    if entry.size() > max {
        return Err(format!("DOCX entry too large: {name}"));
    }
    let mut data = Vec::new();
    entry.take(max + 1).read_to_end(&mut data).map_err(|e| e.to_string())?;
    if data.len() as u64 > max {
        return Err("DOCX expanded entry exceeds limit".into());
    }
    Ok(data)
}
fn valid_image(bytes: &[u8]) -> bool {
    image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()
        .and_then(|r| r.into_dimensions().ok())
        .is_some_and(|(w, h)| u64::from(w) * u64::from(h) <= 32_000_000)
}
fn chunks(out: &mut SourceBundle, prefix: &str, name: &str, locator: &str, text: &str) {
    if out.chunks.len() >= 2000 {
        return;
    }
    let chars: Vec<char> = text.chars().collect();
    for (i, part) in chars.chunks(4000).enumerate() {
        let text = part.iter().collect::<String>();
        if text.trim().is_empty() {
            continue;
        }
        out.chunks.push(SourceChunk {
            reference: SourceReference { id: format!("{prefix}-{}-{i}", out.chunks.len()), name: name.into(), locator: locator.into() },
            text,
        });
        if out.chunks.len() >= 2000 {
            break;
        }
    }
}

pub fn merge(target: &mut SourceBundle, other: SourceBundle) -> Result<(), String> {
    let chunks: Vec<_> = other.chunks.into_iter().filter(|chunk| !target.chunks.iter().any(|c| c.reference.id == chunk.reference.id)).collect();
    let assets: Vec<_> = other.assets.into_iter().filter(|asset| !target.assets.iter().any(|a| a.id == asset.id)).collect();
    let text_bytes = target.chunks.iter().map(|c| c.text.len()).sum::<usize>() + chunks.iter().map(|c| c.text.len()).sum::<usize>();
    let asset_bytes = target.assets.iter().map(|a| a.bytes.len()).sum::<usize>() + assets.iter().map(|a| a.bytes.len()).sum::<usize>();
    if text_bytes <= MAX_BUNDLE_TEXT_BYTES && asset_bytes <= MAX_BUNDLE_ASSET_BYTES && target.assets.len() + assets.len() <= MAX_BUNDLE_ASSETS {
        target.chunks.extend(chunks);
        target.assets.extend(assets);
        Ok(())
    } else {
        Err("Attached sources exceed the 2 MB text, 12 MB image, or 32 image limit".into())
    }
}

pub fn validate_bundle(bundle: &SourceBundle) -> Result<(), String> {
    if bundle.chunks.iter().map(|c| c.text.len()).sum::<usize>() > MAX_BUNDLE_TEXT_BYTES {
        return Err("Attached source text exceeds 2 MB; remove sources or attach shorter excerpts".into());
    }
    if bundle.assets.len() > MAX_BUNDLE_ASSETS {
        return Err(format!("Attached sources exceed {MAX_BUNDLE_ASSETS} image assets"));
    }
    if bundle.assets.iter().map(|a| a.bytes.len()).sum::<usize>() > MAX_BUNDLE_ASSET_BYTES {
        return Err("Attached source images exceed 12 MB; remove sources or attach smaller images".into());
    }
    Ok(())
}

pub fn context(bundle: &SourceBundle, query: &str, budget: usize) -> String {
    let words: std::collections::HashSet<String> = query.split_whitespace().map(str::to_ascii_lowercase).collect();
    let mut ranked: Vec<_> = bundle
        .chunks
        .iter()
        .map(|c| {
            let lower = c.text.to_ascii_lowercase();
            (words.iter().filter(|w| w.len() > 3 && lower.contains(w.as_str())).count(), c)
        })
        .collect();
    ranked.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    let mut out = String::new();
    let budget = budget.clamp(2000, 200_000);
    for (_, chunk) in ranked {
        let label = format!("\n[{}] {} — {}\n", chunk.reference.id, chunk.reference.name, chunk.reference.locator);
        if out.chars().count() + label.chars().count() + chunk.text.chars().count() > budget {
            continue;
        }
        out.push_str(&label);
        out.push_str(&chunk.text);
    }
    out.push_str("\nAvailable images (refer to asset IDs, do not invent paths):\n");
    for asset in &bundle.assets {
        let label = format!("{}: {}\n", asset.id, asset.name.chars().take(160).collect::<String>());
        if out.len() + label.len() > budget.saturating_add(512) {
            break;
        }
        out.push_str(&label);
    }
    out
}

pub fn check_refs(refs: &[String], bundle: &SourceBundle) -> Result<(), String> {
    for r in refs {
        if !bundle.chunks.iter().any(|c| &c.reference.id == r) {
            return Err(format!("Unknown source reference: {r}"));
        }
    }
    Ok(())
}

pub fn verify_numbers(values: &[f64], refs: &[String], bundle: &SourceBundle) -> Result<(), String> {
    check_refs(refs, bundle)?;
    let numbers: Vec<f64> = bundle
        .chunks
        .iter()
        .filter(|c| refs.contains(&c.reference.id))
        .flat_map(|c| {
            c.text.split(|ch: char| !ch.is_ascii_digit() && !matches!(ch, '.' | '-')).filter_map(|v| v.parse::<f64>().ok()).collect::<Vec<_>>()
        })
        .collect();
    if refs.is_empty() || values.iter().any(|v| !v.is_finite() || !numbers.iter().any(|n| (n - v).abs() < 1e-9)) {
        return Err("Chart contains values not found in its cited source data".into());
    }
    Ok(())
}
