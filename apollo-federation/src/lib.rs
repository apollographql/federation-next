use apollo_compiler::ast::Directives;
use apollo_compiler::schema::ExtendedType;
use apollo_compiler::Schema;

use crate::merge::merge_subgraphs;
use crate::subgraph::Subgraph;

pub mod database;
pub mod error;
pub mod link;
pub mod merge;
pub mod subgraph;

type MergeError = &'static str;

// TODO: Same remark as in other crates: we need to define this more cleanly, and probably need
// some "federation errors" crate.
#[derive(Debug)]
pub struct SupergraphError {
    pub msg: String,
}

pub struct Supergraph {
    pub schema: Schema,
}

impl Supergraph {
    pub fn new(schema_str: &str) -> Self {
        let schema = Schema::parse(schema_str, "schema.graphql");

        // TODO: like for subgraphs, it would nice if `Supergraph` was always representing
        // a valid supergraph (which is simpler than for subgraph, but still at least means
        // that it's valid graphQL in the first place, and that it has the `join` spec).

        Self { schema }
    }

    pub fn compose(subgraphs: Vec<&Subgraph>) -> Result<Self, MergeError> {
        let merge_result = match merge_subgraphs(subgraphs) {
            Ok(success) => Ok(Self::new(success.schema.to_string().as_str())),
            // TODO handle errors
            Err(_) => Err("failed to compose"),
        };
        merge_result
    }

    /// Generates API schema from the supergraph schema.
    pub fn to_api_schema(&self) -> Schema {
        let mut api_schema = self.schema.clone();

        // remove schema directives
        api_schema.schema_definition.make_mut().directives.clear();

        // remove known internal types
        api_schema.types.retain(|type_name, graphql_type| {
            !is_join_type(type_name.as_str())
                && !graphql_type
                    .directives()
                    .iter()
                    .any(|d| d.name.eq("inaccessible"))
        });
        // remove directive applications
        for (_, graphql_type) in api_schema.types.iter_mut() {
            match graphql_type {
                ExtendedType::Scalar(scalar) => {
                    scalar.make_mut().directives.clear();
                }
                ExtendedType::Object(object) => {
                    let object = object.make_mut();
                    object.directives.clear();
                    object
                        .fields
                        .retain(|_, field| !is_inaccessible_applied(&field.directives));
                    for (_, field) in object.fields.iter_mut() {
                        let field = field.make_mut();
                        field.directives.clear();
                        field
                            .arguments
                            .retain(|arg| !is_inaccessible_applied(&arg.directives));
                        for arg in field.arguments.iter_mut() {
                            arg.make_mut().directives.clear();
                        }
                    }
                }
                ExtendedType::Interface(intf) => {
                    let intf = intf.make_mut();
                    intf.directives.clear();
                    intf.fields
                        .retain(|_, field| !is_inaccessible_applied(&field.directives));
                    for (_, field) in intf.fields.iter_mut() {
                        let field = field.make_mut();
                        field.directives.clear();
                        for arg in field.arguments.iter_mut() {
                            arg.make_mut().directives.clear();
                        }
                    }
                }
                ExtendedType::Union(union) => {
                    union.make_mut().directives.clear();
                }
                ExtendedType::Enum(enum_type) => {
                    let enum_type = enum_type.make_mut();
                    enum_type.directives.clear();
                    enum_type
                        .values
                        .retain(|_, enum_value| !is_inaccessible_applied(&enum_value.directives));
                    for (_, enum_value) in enum_type.values.iter_mut() {
                        enum_value.make_mut().directives.clear();
                    }
                }
                ExtendedType::InputObject(input_object) => {
                    let input_object = input_object.make_mut();
                    input_object.directives.clear();
                    input_object
                        .fields
                        .retain(|_, input_field| !is_inaccessible_applied(&input_field.directives));
                    for (_, input_field) in input_object.fields.iter_mut() {
                        input_field.make_mut().directives.clear();
                    }
                }
            }
        }
        // remove directives
        api_schema.directive_definitions.clear();

        api_schema
    }
}

impl From<Schema> for Supergraph {
    fn from(schema: Schema) -> Self {
        Self { schema }
    }
}

const JOIN_TYPES: [&str; 4] = [
    "join__Graph",
    "link__Purpose",
    "join__FieldSet",
    "link__Import",
];
fn is_join_type(type_name: &str) -> bool {
    JOIN_TYPES.contains(&type_name)
}

fn is_inaccessible_applied(directives: &Directives) -> bool {
    directives.iter().any(|d| d.name.eq("inaccessible"))
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

        let supergraph = Supergraph::new(schema);
        let _subgraphs = database::extract_subgraphs(&supergraph)
            .expect("Should have been able to extract subgraphs");
        // TODO: actual assertions on the subgraph once it's actually implemented.
    }
}
