#![allow(dead_code)] // TODO: This is fine while we're iterating, but should be removed later.
use apollo_compiler::ast::DirectiveList;
use apollo_compiler::schema::Name;
use apollo_compiler::Schema;
use schema::FederationSchema;

use crate::error::FederationError;
use crate::merge::merge_subgraphs;
use crate::merge::MergeFailure;
use crate::schema::position;
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
        let links = api_schema
            .metadata()
            .as_ref()
            .map_or(vec![], |metadata| metadata.all_links().to_vec());

        let is_fed_directive_name = |name: &Name| -> bool {
            name == "core"
                || links
                    .iter()
                    .any(|link| link.is_feature_directive_definition(name))
        };

        let is_fed_type_name = |name: &Name| -> bool {
            links
                .iter()
                .any(|link| link.is_feature_type_definition(name))
        };

        // Return whether a directive application should be kept in the schema.
        // Federation directives should be removed.

        // remove known internal types
        let types_for_removal = api_schema
            .get_types()
            .filter(|position| {
                is_fed_type_name(position.type_name())
                    || position
                        .get(api_schema.schema())
                        .ok()
                        .is_some_and(|ty| ty.directives().has("inaccessible"))
            })
            .collect::<Vec<_>>();
        let directives_for_removal = api_schema
            .get_directive_definitions()
            .filter(|position| is_fed_directive_name(&position.directive_name))
            .collect::<Vec<_>>();

        for position in types_for_removal {
            println!("remove {}", position.type_name());
            use position::TypeDefinitionPosition as P;
            match position {
                P::Object(object) => object.remove_recursive(&mut api_schema).unwrap(),
                P::Scalar(scalar) => scalar.remove_recursive(&mut api_schema).unwrap(),
                P::Interface(interface) => interface.remove_recursive(&mut api_schema).unwrap(),
                P::Union(union_) => union_.remove_recursive(&mut api_schema).unwrap(),
                P::Enum(enum_) => enum_.remove_recursive(&mut api_schema).unwrap(),
                P::InputObject(input_object) => {
                    input_object.remove_recursive(&mut api_schema).unwrap()
                }
            }
        }

        for position in directives_for_removal {
            println!("remove @{}", position.directive_name);
            position.remove(&mut api_schema).unwrap();
        }

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

fn is_inaccessible_applied(directives: &DirectiveList) -> bool {
    directives.has("inaccessible")
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
