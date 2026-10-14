//! Bounded LaTeX math rendering with a closed, in-memory Typst world.
use std::sync::{Arc, OnceLock};
use typst::{
    Library, LibraryExt, World,
    diag::FileError,
    foundations::{Bytes, Datetime, Duration},
    syntax::{FileId, Source},
    text::{Font, FontBook},
    utils::LazyHash,
};
include!(concat!(env!("OUT_DIR"), "/math_font.rs"));

struct EquationWorld {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Arc<Vec<Font>>,
    source: Source,
}
impl World for EquationWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }
    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }
    fn main(&self) -> FileId {
        self.source.id()
    }
    fn source(&self, id: FileId) -> Result<Source, FileError> {
        if id == self.source.id() { Ok(self.source.clone()) } else { Err(FileError::AccessDenied) }
    }
    fn file(&self, _id: FileId) -> Result<Bytes, FileError> {
        Err(FileError::AccessDenied)
    }
    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }
    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        None
    }
}

pub fn render(latex: &str) -> Result<Vec<u8>, String> {
    validate(latex)?;
    if MATH_FONT.is_empty() {
        return Err("Equation font missing: run scripts/fetch-math-font.sh and rebuild".into());
    }
    let converted = mitex::convert_math(latex, None).map_err(|e| format!("LaTeX: {e}"))?;
    static FONTS: OnceLock<Arc<Vec<Font>>> = OnceLock::new();
    let fonts = FONTS
        .get_or_init(|| {
            let mut fonts: Vec<Font> = Font::iter(Bytes::new(MATH_FONT.to_vec())).collect();
            for font in deckcraft_fonts::CRAFT_FONTS.iter().filter(|f| f.family == "Inter" && f.style == "Regular") {
                fonts.extend(Font::iter(Bytes::new(font.bytes.to_vec())));
            }
            Arc::new(fonts)
        })
        .clone();
    let source = Source::detached(format!(
        "#set page(width: auto, height: auto, margin: 8pt, fill: none)\n#show math.equation: set text(font: \"Libertinus Math\", size: 28pt)\n$ {converted} $"
    ));
    let world = EquationWorld { library: LazyHash::new(Library::default()), book: LazyHash::new(FontBook::from_fonts(fonts.iter())), fonts, source };
    let document = typst::compile::<typst_layout::PagedDocument>(&world)
        .output
        .map_err(|errors| errors.iter().map(|e| e.message.to_string()).collect::<Vec<_>>().join("; "))?;
    let page = document.pages().first().ok_or("Equation produced no page")?;
    let size = page.frame.size();
    if size.x.to_pt() > 2000.0 || size.y.to_pt() > 1000.0 {
        return Err("Equation is too large for a slide".into());
    }
    typst_render::render(page, &Default::default()).encode_png().map_err(|e| e.to_string())
}

fn validate(latex: &str) -> Result<(), String> {
    if latex.trim().is_empty() || latex.len() > 4096 {
        return Err("Enter a LaTeX math expression of at most 4096 bytes".into());
    }
    if latex.contains(['#', '$']) {
        return Err("Enter math source without dollar delimiters or Typst code".into());
    }
    let allowed = "frac dfrac tfrac sqrt sum prod int iint iiint oint lim min max sin cos tan log ln exp alpha beta gamma delta epsilon varepsilon zeta eta theta vartheta iota kappa lambda mu nu xi pi rho sigma tau upsilon phi varphi chi psi omega Gamma Delta Theta Lambda Xi Pi Sigma Phi Psi Omega infty partial nabla times cdot div pm mp le leq ge geq neq approx equiv sim simeq propto in notin subset subseteq supset supseteq cup cap emptyset forall exists neg land lor to rightarrow leftarrow Rightarrow Leftarrow leftrightarrow mapsto vec hat bar overline underline dot ddot mathrm mathbf mathit mathbb mathcal text operatorname left right begin end matrix pmatrix bmatrix cases aligned quad qquad ldots cdots vdots ddots ell langle rangle vert Vert underbrace overbrace binom choose displaystyle limits nonumber";
    let mut chars = latex.chars().peekable();
    let mut depth = 0usize;
    while let Some(c) = chars.next() {
        match c {
            '{' => {
                depth += 1;
                if depth > 32 {
                    return Err("Equation nesting exceeds 32 levels".into());
                }
            }
            '}' => {
                depth = depth.checked_sub(1).ok_or("Unbalanced equation braces")?;
            }
            '\\' => {
                let name: String =
                    std::iter::from_fn(|| if chars.peek().is_some_and(|c| c.is_ascii_alphabetic()) { chars.next() } else { None }).collect();
                if !name.is_empty() && !allowed.split_whitespace().any(|a| a == name) {
                    return Err(format!("Unsupported LaTeX command: \\{name}"));
                }
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err("Unbalanced equation braces".into());
    }
    Ok(())
}
