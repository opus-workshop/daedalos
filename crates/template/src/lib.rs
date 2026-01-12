//! template - Project scaffolding for Daedalos
//!
//! "Never start from scratch. Build on proven foundations."
//!
//! The template tool exists because project scaffolding is both tedious and critical.
//! Every new project needs the same boilerplate: directory structure, config files,
//! CI setup, gitignore, license, README. Doing this manually wastes time and introduces
//! inconsistency.
//!
//! Template is language-agnostic. It's literally "copy directory, replace placeholders."
//! Simple enough to understand in 5 minutes, flexible enough to template anything.

pub mod store;
pub mod variables;

pub use store::{Template, TemplateMetadata, TemplateStore};
pub use variables::Variables;
