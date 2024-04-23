//! Validation helpers for WASM targets
//!
//! These utility methods are mostly used for allowing WASM contexts to validate
//! connect-enabled schemas. Since this must pass the WASM FFI boundary, they can
//! only operate on WASM primitives (for now) like strings.

use crate::{
    error::FederationError,
    schema::ValidFederationSchema,
    sources::connect::spec::schema::{
        ConnectDirectiveArguments, SourceDirectiveArguments, CONNECT_DIRECTIVE_NAME_IN_SPEC,
        SOURCE_DIRECTIVE_NAME_IN_SPEC,
    },
};

#[derive(Default)]
pub struct Location {
    pub line: usize,
    pub column: usize,
}
#[derive(Default)]
pub struct LocationRange {
    pub start: Location,
    pub end: Location,
}
pub struct ValidationError {
    pub range: LocationRange,
    pub reason: String,
    pub error: FederationError,
}

/// Validate a subgraph schema with connect directives
// Note: The error is boxed because it is large and clippy complains
pub fn validate_connect_enabled_schema(subgraph_schema: &str) -> Result<(), Box<ValidationError>> {
    // Helper for mapping into the correct error type
    fn error_with_phony_loc(reason: &str, e: impl Into<FederationError>) -> Box<ValidationError> {
        Box::new(ValidationError {
            range: Default::default(),
            reason: reason.to_string(),
            error: e.into(),
        })
    }

    // First we need to create an actul schema from the provided one
    let schema = apollo_compiler::Schema::parse(subgraph_schema, "")
        .map_err(|e| error_with_phony_loc("failed to parse schema", e))?;

    // Make sure that the parsed schema is actually valid
    let schema = schema
        .validate()
        .map_err(|e| error_with_phony_loc("failed to validate schema", e))?;
    let schema = ValidFederationSchema::new(schema)
        .map_err(|e| error_with_phony_loc("failed to parse as federation schema", e))?;

    // Process the @source directives
    let sources = schema
        .referencers()
        .get_directive(&SOURCE_DIRECTIVE_NAME_IN_SPEC)
        .map_err(|e| error_with_phony_loc("could not get @source directives from schema", e))?;

    // Extract the sources from the schema definition and map them to their `Source` equivalent
    let schema_directive_refs = sources.schema.as_ref().unwrap();
    let _sources = schema_directive_refs
        .get(schema.schema())
        .directives
        .iter()
        .filter(|directive| directive.name == SOURCE_DIRECTIVE_NAME_IN_SPEC)
        .map(SourceDirectiveArguments::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| error_with_phony_loc("could not process @source in spec", e))?;

    // Now process the @connect directives
    let connects = schema
        .referencers()
        .get_directive(&CONNECT_DIRECTIVE_NAME_IN_SPEC)
        .map_err(|e| error_with_phony_loc("could not get @connect directives from schema", e))?;

    // Extract the connects from the schema definition and map them to their `Connect` equivalent
    // TODO: We can safely assume that a connect can only be on object fields, right?
    let _connects = connects
        .object_fields
        .iter()
        .flat_map(|field| field.get(schema.schema()).unwrap().directives.iter())
        .map(ConnectDirectiveArguments::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| error_with_phony_loc("could not process @connect in spec", e))?;

    Ok(())
}
