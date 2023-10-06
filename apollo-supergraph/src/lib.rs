use crate::composition::merge;
use apollo_compiler::schema::ExtendedType;
use apollo_compiler::Schema;
use apollo_subgraph::Subgraph;

pub mod composition;
pub mod database;

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
        let mergeResult = match merge(subgraphs) {
            Ok(success) => Ok(Self::new(success.schema.to_string().as_str())),
            // TODO handle errors
            Err(_) => Err("failed to compose"),
        };
        mergeResult
    }

    pub fn print_sdl(&self) -> String {
        let mut schema = self.schema.clone();
        schema.types.sort_by(|k1, v1, k2, v2| {
            let type_order = print_type_order(v1).cmp(&print_type_order(v2));
            if type_order.is_eq() {
                k1.cmp(k2)
            } else {
                type_order
            }
        });
        schema.directive_definitions.sort_keys();
        schema.to_string()
    }
}

fn print_type_order(extended_type: &ExtendedType) -> i8 {
    match extended_type {
        ExtendedType::Enum(_) => 1,
        ExtendedType::Interface(_) => 2,
        ExtendedType::Union(_) => 3,
        ExtendedType::Object(_) => 4,
        ExtendedType::InputObject(_) => 5,
        ExtendedType::Scalar(_) => 6,
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

        let supergraph = Supergraph::new(schema);
        let _subgraphs = database::extract_subgraphs(&supergraph)
            .expect("Should have been able to extract subgraphs");
        // TODO: actual assertions on the subgraph once it's actually implemented.
    }

    #[test]
    fn can_compose_supergraph() {
        let s1 = Subgraph::parse_and_expand(
            "Subgraph1",
            "https://subgraph1",
            r#"
                type Query {
                  t: T
                }
        
                type T @key(fields: "k") {
                  k: ID
                }
        
                type S {
                  x: Int
                }
        
                union U = S | T
            "#,
        )
        .unwrap();
        let s2 = Subgraph::parse_and_expand(
            "Subgraph2",
            "https://subgraph2",
            r#"
                type T @key(fields: "k") {
                  k: ID
                  a: Int
                  b: String
                }
                
                enum E {
                  V1
                  V2
                }
            "#,
        )
        .unwrap();

        let supergraph = Supergraph::compose(vec![&s1, &s2]).unwrap();
        let expected_supergraph_sdl = r#"schema @link(url: "https://specs.apollo.dev/link/v1.0") @link(url: "https://specs.apollo.dev/join/v0.3", for: EXECUTION) {
  query: Query
}

directive @join__enumValue(graph: join__Graph!) repeatable on ENUM_VALUE

directive @join__field(graph: join__Graph, requires: join__FieldSet, provides: join__FieldSet, type: String, external: Boolean, override: String, usedOverridden: Boolean) repeatable on FIELD_DEFINITION | INPUT_FIELD_DEFINITION

directive @join__graph(name: String!, url: String!) on ENUM_VALUE

directive @join__implements(graph: join__Graph!, interface: String!) repeatable on INTERFACE | OBJECT

directive @join__type(graph: join__Graph!, key: join__FieldSet, extension: Boolean! = false, resolvable: Boolean! = true, isInterfaceObject: Boolean! = false) repeatable on ENUM | INPUT_OBJECT | INTERFACE | OBJECT | SCALAR | UNION

directive @join__unionMember(graph: join__Graph!, member: String!) repeatable on UNION

directive @link(url: String, as: String, for: link__Purpose, import: [link__Import]) repeatable on SCHEMA

enum E @join__type(graph: SUBGRAPH2) {
  V1 @join__enumValue(graph: SUBGRAPH2)
  V2 @join__enumValue(graph: SUBGRAPH2)
}

enum join__Graph {
  SUBGRAPH1 @join__graph(name: "Subgraph1", url: "https://subgraph1")
  SUBGRAPH2 @join__graph(name: "Subgraph2", url: "https://subgraph2")
}

enum link__Purpose {
  "SECURITY features provide metadata necessary to securely resolve fields."
  SECURITY
  "EXECUTION features provide metadata necessary for operation execution."
  EXECUTION
}

union U @join__type(graph: SUBGRAPH1) @join__unionMember(graph: SUBGRAPH1, member: "S") @join__unionMember(graph: SUBGRAPH1, member: "T") = S | T

type Query @join__type(graph: SUBGRAPH1) @join__type(graph: SUBGRAPH2) {
  t: T @join__field(graph: SUBGRAPH1)
}

type S @join__type(graph: SUBGRAPH1) {
  x: Int
}

type T @join__type(graph: SUBGRAPH1, key: "k") @join__type(graph: SUBGRAPH2, key: "k") {
  k: ID
  a: Int @join__field(graph: SUBGRAPH2)
  b: String @join__field(graph: SUBGRAPH2)
}

scalar join__FieldSet

scalar link__Import
"#;
        assert_eq!(supergraph.print_sdl(), expected_supergraph_sdl);
    }

    #[test]
    fn can_compose_with_descriptions() {
        let s1 = Subgraph::parse_and_expand(
            "Subgraph1",
            "https://subgraph1",
            r#"
                "The foo directive description"
                directive @foo(url: String) on FIELD
    
                "A cool schema"
                schema {
                  query: Query
                }
    
                """
                Available queries
                Not much yet
                """
                type Query {
                  "Returns tea"
                  t(
                    "An argument that is very important"
                    x: String!
                  ): String
                }
            "#,
        )
        .unwrap();

        let s2 = Subgraph::parse_and_expand(
            "Subgraph2",
            "https://subgraph2",
            r#"
                "The foo directive description"
                directive @foo(url: String) on FIELD
    
                "An enum"
                enum E {
                  "The A value"
                  A
                  "The B value"
                  B
                }
            "#,
        )
        .unwrap();

        let expected_supergraph_sdl = r#""A cool schema"
schema @link(url: "https://specs.apollo.dev/link/v1.0") @link(url: "https://specs.apollo.dev/join/v0.3", for: EXECUTION) {
  query: Query
}

"The foo directive description"
directive @foo(url: String) on FIELD

directive @join__enumValue(graph: join__Graph!) repeatable on ENUM_VALUE

directive @join__field(graph: join__Graph, requires: join__FieldSet, provides: join__FieldSet, type: String, external: Boolean, override: String, usedOverridden: Boolean) repeatable on FIELD_DEFINITION | INPUT_FIELD_DEFINITION

directive @join__graph(name: String!, url: String!) on ENUM_VALUE

directive @join__implements(graph: join__Graph!, interface: String!) repeatable on INTERFACE | OBJECT

directive @join__type(graph: join__Graph!, key: join__FieldSet, extension: Boolean! = false, resolvable: Boolean! = true, isInterfaceObject: Boolean! = false) repeatable on ENUM | INPUT_OBJECT | INTERFACE | OBJECT | SCALAR | UNION

directive @join__unionMember(graph: join__Graph!, member: String!) repeatable on UNION

directive @link(url: String, as: String, for: link__Purpose, import: [link__Import]) repeatable on SCHEMA

"An enum"
enum E @join__type(graph: SUBGRAPH2) {
  "The A value"
  A @join__enumValue(graph: SUBGRAPH2)
  "The B value"
  B @join__enumValue(graph: SUBGRAPH2)
}

enum join__Graph {
  SUBGRAPH1 @join__graph(name: "Subgraph1", url: "https://subgraph1")
  SUBGRAPH2 @join__graph(name: "Subgraph2", url: "https://subgraph2")
}

enum link__Purpose {
  "SECURITY features provide metadata necessary to securely resolve fields."
  SECURITY
  "EXECUTION features provide metadata necessary for operation execution."
  EXECUTION
}

"Available queries\nNot much yet"
type Query @join__type(graph: SUBGRAPH1) @join__type(graph: SUBGRAPH2) {
  "Returns tea"
  t(
    "An argument that is very important"
    x: String!,
  ): String @join__field(graph: SUBGRAPH1)
}

scalar join__FieldSet

scalar link__Import
"#;
        let supergraph = Supergraph::compose(vec![&s1, &s2]).unwrap();
        // TODO currently printer does not respect multi line comments
        // TODO printer also adds extra comma after arguments
        assert_eq!(supergraph.print_sdl(), expected_supergraph_sdl);
    }

    #[test]
    fn can_compose_types_from_different_subgraphs() {
        let s1 = Subgraph::parse_and_expand(
            "SubgraphA",
            "https://subgraphA",
            r#"
                type Query {
                    products: [Product!]
                }

                type Product {
                    sku: String!
                    name: String!
                }
            "#,
        )
        .unwrap();

        let s2 = Subgraph::parse_and_expand(
            "SubgraphB",
            "https://subgraphB",
            r#"
                type User {
                    name: String
                    email: String!
                }
            "#,
        )
        .unwrap();

        let expected_supergraph_sdl = r#"schema @link(url: "https://specs.apollo.dev/link/v1.0") @link(url: "https://specs.apollo.dev/join/v0.3", for: EXECUTION) {
  query: Query
}

directive @join__enumValue(graph: join__Graph!) repeatable on ENUM_VALUE

directive @join__field(graph: join__Graph, requires: join__FieldSet, provides: join__FieldSet, type: String, external: Boolean, override: String, usedOverridden: Boolean) repeatable on FIELD_DEFINITION | INPUT_FIELD_DEFINITION

directive @join__graph(name: String!, url: String!) on ENUM_VALUE

directive @join__implements(graph: join__Graph!, interface: String!) repeatable on INTERFACE | OBJECT

directive @join__type(graph: join__Graph!, key: join__FieldSet, extension: Boolean! = false, resolvable: Boolean! = true, isInterfaceObject: Boolean! = false) repeatable on ENUM | INPUT_OBJECT | INTERFACE | OBJECT | SCALAR | UNION

directive @join__unionMember(graph: join__Graph!, member: String!) repeatable on UNION

directive @link(url: String, as: String, for: link__Purpose, import: [link__Import]) repeatable on SCHEMA

enum join__Graph {
  SUBGRAPHA @join__graph(name: "SubgraphA", url: "https://subgraphA")
  SUBGRAPHB @join__graph(name: "SubgraphB", url: "https://subgraphB")
}

enum link__Purpose {
  "SECURITY features provide metadata necessary to securely resolve fields."
  SECURITY
  "EXECUTION features provide metadata necessary for operation execution."
  EXECUTION
}

type Product @join__type(graph: SUBGRAPHA) {
  sku: String!
  name: String!
}

type Query @join__type(graph: SUBGRAPHA) @join__type(graph: SUBGRAPHB) {
  products: [Product!] @join__field(graph: SUBGRAPHA)
}

type User @join__type(graph: SUBGRAPHB) {
  name: String
  email: String!
}

scalar join__FieldSet

scalar link__Import
"#;
        let supergraph = Supergraph::compose(vec![&s1, &s2]).unwrap();
        assert_eq!(supergraph.print_sdl(), expected_supergraph_sdl);
    }

    #[test]
    fn compose_removes_federation_directives() {
        let s1 = Subgraph::parse_and_expand(
            "SubgraphA",
            "https://subgraphA",
            r#"
                extend schema @link(url: "https://specs.apollo.dev/federation/v2.5", import: [ "@key", "@provides", "@external" ])
                
                type Query {
                  products: [Product!] @provides(fields: "name")
                }
        
                type Product @key(fields: "sku") {
                  sku: String!
                  name: String! @external
                }
            "#,
        )
        .unwrap();

        let s2 = Subgraph::parse_and_expand(
            "SubgraphB",
            "https://subgraphB",
            r#"
                extend schema @link(url: "https://specs.apollo.dev/federation/v2.5", import: [ "@key", "@shareable" ])
            
                type Product @key(fields: "sku") {
                  sku: String!
                  name: String! @shareable
                }
            "#,
        )
        .unwrap();

        let expected_supergraph_sdl = r#"schema @link(url: "https://specs.apollo.dev/link/v1.0") @link(url: "https://specs.apollo.dev/join/v0.3", for: EXECUTION) {
  query: Query
}

directive @join__enumValue(graph: join__Graph!) repeatable on ENUM_VALUE

directive @join__field(graph: join__Graph, requires: join__FieldSet, provides: join__FieldSet, type: String, external: Boolean, override: String, usedOverridden: Boolean) repeatable on FIELD_DEFINITION | INPUT_FIELD_DEFINITION

directive @join__graph(name: String!, url: String!) on ENUM_VALUE

directive @join__implements(graph: join__Graph!, interface: String!) repeatable on INTERFACE | OBJECT

directive @join__type(graph: join__Graph!, key: join__FieldSet, extension: Boolean! = false, resolvable: Boolean! = true, isInterfaceObject: Boolean! = false) repeatable on ENUM | INPUT_OBJECT | INTERFACE | OBJECT | SCALAR | UNION

directive @join__unionMember(graph: join__Graph!, member: String!) repeatable on UNION

directive @link(url: String, as: String, for: link__Purpose, import: [link__Import]) repeatable on SCHEMA

enum join__Graph {
  SUBGRAPHA @join__graph(name: "SubgraphA", url: "https://subgraphA")
  SUBGRAPHB @join__graph(name: "SubgraphB", url: "https://subgraphB")
}

enum link__Purpose {
  "SECURITY features provide metadata necessary to securely resolve fields."
  SECURITY
  "EXECUTION features provide metadata necessary for operation execution."
  EXECUTION
}

type Product @join__type(graph: SUBGRAPHA, key: "sku") @join__type(graph: SUBGRAPHB, key: "sku") {
  sku: String!
  name: String! @join__field(graph: SUBGRAPHA, external: true) @join__field(graph: SUBGRAPHB)
}

type Query @join__type(graph: SUBGRAPHA) @join__type(graph: SUBGRAPHB) {
  products: [Product!] @join__field(graph: SUBGRAPHA, provides: "name")
}

scalar join__FieldSet

scalar link__Import
"#;

        let supergraph = Supergraph::compose(vec![&s1, &s2]).unwrap();
        assert_eq!(supergraph.print_sdl(), expected_supergraph_sdl);
    }
}
