mod selection_parser;
pub(crate) mod spec;
mod url_path_template;

pub use selection_parser::ApplyTo;
pub use selection_parser::ApplyToError;
pub use selection_parser::Selection;
pub use url_path_template::URLPathTemplate;

// For use with external WASM validation
#[deprecated(note = "will be superseded by composition in Rust")]
pub mod wasm_validators;
