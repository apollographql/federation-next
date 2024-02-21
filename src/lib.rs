#![allow(dead_code)] // TODO: This is fine while we're iterating, but should be removed later.

mod api_schema;
mod compat;
pub mod error;
pub mod link;
pub mod merge;
pub mod query_graph;
pub mod query_plan;
pub mod schema;
pub mod subgraph;

pub use crate::api_schema::ApiSchemaOptions;
use crate::error::FederationError;
use crate::error::SingleFederationError;
use crate::link::join_spec_definition::JoinSpecDefinition;
use crate::link::link_spec_definition::LinkSpecDefinition;
use crate::link::spec::Identity;
use crate::merge::merge_subgraphs;
use crate::merge::MergeFailure;
pub use crate::query_graph::extract_subgraphs_from_supergraph::ValidFederationSubgraph;
pub use crate::query_graph::extract_subgraphs_from_supergraph::ValidFederationSubgraphs;
use crate::schema::ValidFederationSchema;
use crate::subgraph::ValidSubgraph;
use apollo_compiler::validation::Valid;
use apollo_compiler::Schema;
use link::join_spec_definition::JOIN_VERSIONS;
use schema::FederationSchema;

pub(crate) type SupergraphSpecs = (&'static LinkSpecDefinition, &'static JoinSpecDefinition);

/// Checks that required supergraph directives are in the schema, and returns which ones were used.
pub(crate) fn validate_supergraph(
    supergraph_schema: &FederationSchema,
) -> Result<SupergraphSpecs, FederationError> {
    let Some(metadata) = supergraph_schema.metadata() else {
        return Err(SingleFederationError::InvalidFederationSupergraph {
            message: "Invalid supergraph: must be a core schema".to_owned(),
        }
        .into());
    };
    let link_spec_definition = metadata.link_spec_definition()?;
    let Some(join_link) = metadata.for_identity(&Identity::join_identity()) else {
        return Err(SingleFederationError::InvalidFederationSupergraph {
            message: "Invalid supergraph: must use the join spec".to_owned(),
        }
        .into());
    };
    let Some(join_spec_definition) = JOIN_VERSIONS.find(&join_link.url.version) else {
        return Err(SingleFederationError::InvalidFederationSupergraph {
            message: format!(
                "Invalid supergraph: uses unsupported join spec version {} (supported versions: {})",
                JOIN_VERSIONS.versions().map(|v| v.to_string()).collect::<Vec<_>>().join(", "),
                join_link.url.version,
            ),
        }.into());
    };
    Ok((link_spec_definition, join_spec_definition))
}

pub struct Supergraph {
    pub schema: ValidFederationSchema,
}

impl Supergraph {
    pub fn new(schema_str: &str) -> Result<Self, FederationError> {
        let schema = Schema::parse_and_validate(schema_str, "schema.graphql")?;
        Self::from_schema(schema)
    }

    pub fn from_schema(schema: Valid<Schema>) -> Result<Self, FederationError> {
        let schema = schema.into_inner();
        let schema = FederationSchema::new(schema)?;

        let _ = validate_supergraph(&schema)?;

        Ok(Self {
            // We know it's valid because the input was.
            schema: schema.assume_valid(),
        })
    }

    pub fn compose(subgraphs: Vec<&ValidSubgraph>) -> Result<Self, MergeFailure> {
        let schema = merge_subgraphs(subgraphs)?.schema;
        Ok(Self {
            schema: ValidFederationSchema::new(schema)
                .map_err(|err| todo!("missing error handling: {err}"))?,
        })
    }

    /// Generates an API Schema from this supergraph schema. The API Schema represents the combined
    /// API of the supergraph that's visible to end users.
    pub fn to_api_schema(
        &self,
        options: ApiSchemaOptions,
    ) -> Result<ValidFederationSchema, FederationError> {
        api_schema::to_api_schema(self.schema.clone(), options)
    }

    pub fn extract_subgraphs(&self) -> Result<ValidFederationSubgraphs, FederationError> {
        crate::query_graph::extract_subgraphs_from_supergraph::extract_subgraphs_from_supergraph(
            &self.schema,
            None,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_extract_subgraph() {
        let schema = r#"
          schema
            @link(url: "https://specs.apollo.dev/link/v1.0")
            @link(url: "https://specs.apollo.dev/join/v0.3", for: EXECUTION)
          {
            query: Query
          }

          directive @join__enumValue(graph: join__Graph!) repeatable on ENUM_VALUE

          directive @join__field(graph: join__Graph, requires: join__FieldSet, provides: join__FieldSet, type: String, external: Boolean, override: String, usedOverridden: Boolean) repeatable on FIELD_DEFINITION | INPUT_FIELD_DEFINITION

          directive @join__graph(name: String!, url: String!) on ENUM_VALUE

          directive @join__implements(graph: join__Graph!, interface: String!) repeatable on OBJECT | INTERFACE

          directive @join__type(graph: join__Graph!, key: join__FieldSet, extension: Boolean! = false, resolvable: Boolean! = true, isInterfaceObject: Boolean! = false) repeatable on OBJECT | INTERFACE | UNION | ENUM | INPUT_OBJECT | SCALAR

          directive @join__unionMember(graph: join__Graph!, member: String!) repeatable on UNION

          directive @link(url: String, as: String, for: link__Purpose, import: [link__Import]) repeatable on SCHEMA

          enum E
            @join__type(graph: SUBGRAPH2)
          {
            V1 @join__enumValue(graph: SUBGRAPH2)
            V2 @join__enumValue(graph: SUBGRAPH2)
          }

          scalar join__FieldSet

          enum join__Graph {
            SUBGRAPH1 @join__graph(name: "Subgraph1", url: "https://Subgraph1")
            SUBGRAPH2 @join__graph(name: "Subgraph2", url: "https://Subgraph2")
          }

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

          type Query
            @join__type(graph: SUBGRAPH1)
            @join__type(graph: SUBGRAPH2)
          {
            t: T @join__field(graph: SUBGRAPH1)
          }

          type S
            @join__type(graph: SUBGRAPH1)
          {
            x: Int
          }

          type T
            @join__type(graph: SUBGRAPH1, key: "k")
            @join__type(graph: SUBGRAPH2, key: "k")
          {
            k: ID
            a: Int @join__field(graph: SUBGRAPH2)
            b: String @join__field(graph: SUBGRAPH2)
          }

          union U
            @join__type(graph: SUBGRAPH1)
            @join__unionMember(graph: SUBGRAPH1, member: "S")
            @join__unionMember(graph: SUBGRAPH1, member: "T")
           = S | T
        "#;

        let supergraph = Supergraph::new(schema).unwrap();
        let _subgraphs = supergraph
            .extract_subgraphs()
            .expect("Should have been able to extract subgraphs");
        // TODO: actual assertions on the subgraph once it's actually implemented.
    }
}
