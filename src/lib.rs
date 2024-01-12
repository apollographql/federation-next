#![allow(dead_code)] // TODO: This is fine while we're iterating, but should be removed later.
use crate::error::FederationError;
use crate::link::inaccessible_spec_definition::remove_inaccessible_elements;
use crate::link::inaccessible_spec_definition::validate_inaccessible;
use crate::merge::merge_subgraphs;
use crate::merge::MergeFailure;
use crate::schema::position;
use crate::schema::FederationSchema;
use crate::subgraph::ValidSubgraph;
use apollo_compiler::validation::Valid;
use apollo_compiler::Schema;

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

        remove_core_feature_elements(&mut api_schema)?;

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

fn remove_core_feature_elements(schema: &mut FederationSchema) -> Result<(), FederationError> {
    let metadata = schema.metadata().unwrap();

    // Remove federation types and directives
    let types_for_removal = schema
        .get_types()
        .filter(|position| metadata.source_link_of_type(position.type_name()).is_some())
        .collect::<Vec<_>>();

    let directives_for_removal = schema
        .get_directive_definitions()
        .filter(|position| {
            metadata
                .source_link_of_directive(&position.directive_name)
                .is_some()
        })
        .collect::<Vec<_>>();

    // First remove children of elements that need to be removed, so there won't be outgoing
    // references from the type.
    for position in &types_for_removal {
        match position {
            position::TypeDefinitionPosition::Object(position) => {
                let object = position.get(schema.schema())?;
                let remove_children = object
                    .fields
                    .keys()
                    .map(|field_name| position.field(field_name.clone()))
                    .collect::<Vec<_>>();
                for child in remove_children {
                    child.remove(schema)?;
                }
            }
            position::TypeDefinitionPosition::Interface(position) => {
                let interface = position.get(schema.schema())?;
                let remove_children = interface
                    .fields
                    .keys()
                    .map(|field_name| position.field(field_name.clone()))
                    .collect::<Vec<_>>();
                for child in remove_children {
                    child.remove(schema)?;
                }
            }
            position::TypeDefinitionPosition::InputObject(position) => {
                let input_object = position.get(schema.schema())?;
                let remove_children = input_object
                    .fields
                    .keys()
                    .map(|field_name| position.field(field_name.clone()))
                    .collect::<Vec<_>>();
                for child in remove_children {
                    child.remove(schema)?;
                }
            }
            position::TypeDefinitionPosition::Enum(position) => {
                let enum_ = position.get(schema.schema())?;
                let remove_children = enum_
                    .values
                    .keys()
                    .map(|field_name| position.value(field_name.clone()))
                    .collect::<Vec<_>>();
                for child in remove_children {
                    child.remove(schema)?;
                }
            }
            _ => {}
        }
    }

    // TODO remove arguments first
    for position in &directives_for_removal {
        position.remove(schema)?;
    }

    for position in &types_for_removal {
        match position {
            position::TypeDefinitionPosition::Object(position) => {
                position.remove(schema)?;
            }
            position::TypeDefinitionPosition::Interface(position) => {
                position.remove(schema)?;
            }
            position::TypeDefinitionPosition::InputObject(position) => {
                position.remove(schema)?;
            }
            position::TypeDefinitionPosition::Enum(position) => {
                position.remove(schema)?;
            }
            position::TypeDefinitionPosition::Scalar(position) => {
                position.remove(schema)?;
            }
            position::TypeDefinitionPosition::Union(position) => {
                position.remove(schema)?;
            }
        }
    }

    Ok(())
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
