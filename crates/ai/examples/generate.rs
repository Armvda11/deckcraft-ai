//! Exercise the same worker used by AI Studio; no companion process or Python required.
use deckcraft_ai::{
    jobs::{Job, Task},
    *,
};
fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let output = args.next().ok_or("Usage: generate OUTPUT.deckcraft BRIEF [SOURCE…]")?;
    let brief = args.next().ok_or("Missing brief")?;
    let mut sources = SourceBundle::default();
    for path in args {
        sources::merge(&mut sources, sources::extract(std::path::Path::new(&path))?)?;
    }
    let profile = if let Ok(endpoint) = std::env::var("DECKCRAFT_AI_ENDPOINT") {
        ProviderProfile {
            kind: ProviderKind::Compatible,
            endpoint,
            model: std::env::var("DECKCRAFT_AI_MODEL").unwrap_or_default(),
            ..Default::default()
        }
    } else {
        ProviderProfile::default()
    };
    let slides = std::env::var("DECKCRAFT_AI_SLIDES").ok().and_then(|v| v.parse().ok()).unwrap_or(10);
    let job = Job::start(
        profile,
        String::new(),
        Task::Generate {
            request: GenerationRequest {
                brief,
                audience: "Technical audience".into(),
                slide_count: slides,
                theme: "Harbor".into(),
                sources,
                review_outline: false,
            },
            resume: None,
        },
    )?;
    while let Ok(event) = job.receiver.recv() {
        match event {
            AiJobEvent::Progress(message) => println!("{message}"),
            AiJobEvent::Usage { input, output } => println!("Reported tokens: {input}/{output}"),
            AiJobEvent::Ready(candidate) => {
                std::fs::write(&output, deckcraft_format::save(&candidate.document).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
                println!("Saved {} slides to {output}", candidate.document.slides.len());
                return Ok(());
            }
            AiJobEvent::Failed { message, .. } => return Err(message),
            _ => {}
        }
    }
    Err("Worker ended without a complete draft".into())
}
