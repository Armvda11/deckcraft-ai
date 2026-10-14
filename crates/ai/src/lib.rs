//! AI presentation plans compile to native editable objects, never executable model code.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

pub mod types;
pub use types::*;

#[cfg(not(target_arch = "wasm32"))]
pub mod compiler;
#[cfg(not(target_arch = "wasm32"))]
pub mod equation;
#[cfg(not(target_arch = "wasm32"))]
pub mod jobs;
#[cfg(not(target_arch = "wasm32"))]
pub mod provider;
#[cfg(not(target_arch = "wasm32"))]
pub mod settings;
#[cfg(not(target_arch = "wasm32"))]
pub mod sources;
