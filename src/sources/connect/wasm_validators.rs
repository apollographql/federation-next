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
    let combined_schema = format!(
        "{}\n{}\n{}",
        subgraph_schema,
        constants::TEMP_FEDERATION_DEFINITIONS,
        constants::TEMP_SOURCE_DEFINITIONS
    );
    let schema = apollo_compiler::Schema::parse(combined_schema, "")
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

/// Needed directive definitions until federation-next has support for custom directives/versions when parsing
mod constants {
    pub static TEMP_FEDERATION_DEFINITIONS: &str = r#"
        directive @link(url: String, as: String, for: link__Purpose, import: [link__Import]) repeatable on SCHEMA
        scalar link__Import

        enum link__Purpose {
        """
        \`SECURITY\` features provide metadata necessary to securely resolve fields.
        """
        SECURITY

        """
        \`EXECUTION\` features provide metadata necessary for operation execution.
        """
        EXECUTION
        }
    "#;

    pub static TEMP_SOURCE_DEFINITIONS: &str = r#"
        """
        Defines a connector as the implementation of a field.

        Exactly one of {http} must be present.
        """
        directive @connect(
        """
        Optionally connects a @source directive for shared connector configuration.
        Must match the `name:` argument of a @source directive in this schema.
        """
        source: String

        """
        Defines HTTP configuration for this connector.
        """
        http: ConnectHTTP

        """
        Uses the JSONSelection syntax to define a mapping of connector response
        to GraphQL schema.
        """
        selection: JSONSelection

        """
        Marks this connector as a canonical resolver for an entity (uniquely
        identified domain model.) If true, the connector must be defined on a
        field of the Query type.
        """
        entity: Boolean = false
        ) on FIELD_DEFINITION

        """
        HTTP configuration for a connector.

        Exactly one of {GET,POST,PATCH,PUT,DELETE} must be present.
        """
        input ConnectHTTP {
        """
        URL template for GET requests to an HTTP endpoint.

        Can be a full URL or a partial path. If it's a partial path, it will
        be appended to an associated `baseURL` from the related @source.
        """
        GET: URLPathTemplate

        "Same as GET but for POST requests"
        POST: URLPathTemplate

        "Same as GET but for PATCH requests"
        PATCH: URLPathTemplate

        "Same as GET but for PUT requests"
        PUT: URLPathTemplate

        "Same as GET but for DELETE requests"
        DELETE: URLPathTemplate

        """
        Define a request body using JSONSelection. Selections can include
        values from field arguments using `$args.argName` and from fields on the
        parent type using `$this.fieldName`.
        """
        body: JSONSelection

        """
        Configuration for headers to attach to the request.

        Takes precedence over headers defined on the associated @source.
        """
        headers: [HTTPHeaderMapping!]
        }

        """
        At most one of {as,value} can be present.
        """
        input HTTPHeaderMapping {
        "The name of the incoming HTTP header to propagate to the endpoint"
        name: String!

        "If present, this defines the name of the header in the endpoint request"
        as: String

        "If present, this defines values for the headers in the endpoint request"
        value: [String]
        }

        """
        Defines connector configuration for reuse across multiple connectors.

        Exactly one of {http} must be present.
        """
        directive @source(
        name: String!

        http: SourceHTTP
        ) on SCHEMA

        """
        Common HTTP configuration for connectors.
        """
        input SourceHTTP {
        """
        If the URL path template in a connector is not a valid URL, it will be appended
        to this URL. Must be a valid URL.
        """
        baseURL: String!

        """
        Common headers from related connectors.
        """
        headers: [HTTPHeaderMapping!]
        }

        """
        A string containing a "JSON Selection", which defines a mapping from one JSON-like
        shape to another JSON-like shape.

        Example: ".data { id: user_id name account: { id: account_id } }"
        """
        scalar JSONSelection @specifiedBy(url: "...")

        """
        A string that declares a URL path with values interpolated inside `{}`.

        Example: "/product/{$this.id}/reviews?count={$args.count}"
        """
        scalar URLPathTemplate @specifiedBy(url: "...")
    "#;
}
