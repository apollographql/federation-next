#![allow(dead_code)] // TODO: This is fine while we're iterating, but should be removed later.
use apollo_compiler::ast::DirectiveList;
use apollo_compiler::Schema;
use link::inaccessible_spec_definition::remove_inaccessible_elements;
use link::inaccessible_spec_definition::validate_inaccessible;
use schema::FederationSchema;

use crate::error::FederationError;
use crate::merge::merge_subgraphs;
use crate::merge::MergeFailure;
use crate::subgraph::ValidSubgraph;
use apollo_compiler::validation::Valid;

pub mod database;
pub mod error;
pub mod link;
pub mod merge;
pub mod query_graph;
pub mod query_plan;
pub mod schema;
pub mod subgraph;

pub struct Supergraph {
    pub schema: Valid<Schema>,
}

impl Supergraph {
    pub fn new(schema_str: &str) -> Result<Self, FederationError> {
        let schema = Schema::parse_and_validate(schema_str, "schema.graphql")?;
        // TODO: federation-specific validation
        Ok(Self { schema })
    }

    pub fn compose(subgraphs: Vec<&ValidSubgraph>) -> Result<Self, MergeFailure> {
        let schema = merge_subgraphs(subgraphs)?.schema;
        Ok(Self { schema })
    }

    /// Generates API schema from the supergraph schema.
    pub fn to_api_schema(&self) -> Result<Valid<Schema>, FederationError> {
        let mut api_schema = FederationSchema::new(self.schema.clone().into_inner())?;

        validate_inaccessible(&api_schema)?;
        remove_inaccessible_elements(&mut api_schema)?;

        /*
        let metadata = api_schema.metadata().unwrap();

        // Remove federation types and directives
        let types_for_removal = api_schema
            .get_types()
            .filter(|position| metadata.source_link_of_type(position.type_name()).is_some())
            .collect::<Vec<_>>();
        let directives_for_removal = api_schema
            .get_directive_definitions()
            .filter(|position| {
                metadata
                    .source_link_of_directive(&position.directive_name)
                    .is_some()
            })
            .collect::<Vec<_>>();
        */

        Ok(apollo_compiler::validation::Valid::assume_valid(
            api_schema.schema().clone(),
        ))
    }
}

impl From<Valid<Schema>> for Supergraph {
    fn from(schema: Valid<Schema>) -> Self {
        Self { schema }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_extract_subgraph() {
        // TODO: not actually implemented; just here to give a sense of the API.
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
        let _subgraphs = database::extract_subgraphs(&supergraph)
            .expect("Should have been able to extract subgraphs");
        // TODO: actual assertions on the subgraph once it's actually implemented.
    }
}
