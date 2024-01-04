use apollo_compiler::validation::Valid;
use apollo_compiler::Schema;
use apollo_federation::error::FederationError;
use apollo_federation::Supergraph;

const INACCESSIBLE_V02_HEADER: &str = r#"
    directive @link(url: String!, as: String, import: [link__Import], for: link__Purpose) repeatable on SCHEMA

    scalar link__Import

    enum link__Purpose {
      EXECUTION
      SECURITY
    }

    directive @inaccessible on FIELD_DEFINITION | OBJECT | INTERFACE | UNION | ARGUMENT_DEFINITION | SCALAR | ENUM | ENUM_VALUE | INPUT_OBJECT | INPUT_FIELD_DEFINITION

    schema
      @link(url: "https://specs.apollo.dev/link/v0.2")
      @link(url: "https://specs.apollo.dev/inaccessible/v0.2")
    {
      query: Query
    }
"#;

fn inaccessible_to_api_schema(input: &str) -> Result<Valid<Schema>, FederationError> {
    let sdl = format!("{INACCESSIBLE_V02_HEADER}{input}");
    let graph = Supergraph::new(&sdl).unwrap();
    graph.to_api_schema()
}

#[test]
fn inaccessible_types_with_accessible_references() {
    let errors = inaccessible_to_api_schema(
        r#"
      # Query types can't be inaccessible
      type Query @inaccessible {
        someField: String
      }

      # Inaccessible object type
      type Object @inaccessible {
        someField: String
      }

      # Inaccessible object type can't be referenced by object field in the API
      # schema
      type Referencer1 implements Referencer2 {
        someField: Object!
      }

      # Inaccessible object type can't be referenced by interface field in the
      # API schema
      interface Referencer2 {
        someField: Object
      }

      # Inaccessible object type can't be referenced by union member with a
      # non-inaccessible parent and no non-inaccessible siblings
      union Referencer3 = Object
    "#,
    )
    .expect_err("should return validation errors");

    insta::assert_display_snapshot!(errors);
}

#[test]
fn inaccessible_interface_with_accessible_references() {
    let errors = inaccessible_to_api_schema(
        r#"
      type Query {
        someField: String
      }

      # Inaccessible interface type
      interface Interface @inaccessible {
        someField: String
      }

      # Inaccessible interface type can't be referenced by object field in the
      # API schema
      type Referencer1 implements Referencer2 {
        someField: [Interface!]!
      }

      # Inaccessible interface type can't be referenced by interface field in
      # the API schema
      interface Referencer2 {
        someField: [Interface]
      }
    "#,
    )
    .expect_err("should return validation errors");

    insta::assert_display_snapshot!(errors);
}

#[test]
fn inaccessible_union_with_accessible_references() {
    let errors = inaccessible_to_api_schema(
        r#"
      type Query {
        someField: String
      }

      # Inaccessible union type
      union Union @inaccessible = Query

      # Inaccessible union type can't be referenced by object field in the API
      # schema
      type Referencer1 implements Referencer2 {
        someField: Union!
      }

      # Inaccessible union type can't be referenced by interface field in the
      # API schema
      interface Referencer2 {
        someField: Union
      }
    "#,
    )
    .expect_err("should return validation errors");

    insta::assert_display_snapshot!(errors);
}

#[test]
fn inaccessible_input_object_with_accessible_references() {
    let errors = inaccessible_to_api_schema(
        r#"
      type Query {
        someField: String
      }

      # Inaccessible input object type
      input InputObject @inaccessible {
        someField: String
      }

      # Inaccessible input object type can't be referenced by object field
      # argument in the API schema
      type Referencer1 implements Referencer2 {
        someField(someArg: InputObject): String
      }

      # Inaccessible input object type can't be referenced by interface field
      # argument in the API schema
      interface Referencer2 {
        someField(someArg: InputObject): String
      }

      # Inaccessible input object type can't be referenced by input object field
      # in the API schema
      input Referencer3 {
        someField: InputObject
      }

      # Inaccessible input object type can't be referenced by directive argument
      # in the API schema
      directive @referencer4(someArg: InputObject) on QUERY
    "#,
    )
    .expect_err("should return validation errors");

    insta::assert_display_snapshot!(errors);
}

#[test]
fn inaccessible_enum_with_accessible_references() {
    let errors = inaccessible_to_api_schema(
        r#"
      type Query {
        someField: String
      }

      # Inaccessible enum type
      enum Enum @inaccessible {
        SOME_VALUE
      }

      # Inaccessible enum type can't be referenced by object field in the API
      # schema
      type Referencer1 implements Referencer2 {
        somefield: [Enum!]!
      }

      # Inaccessible enum type can't be referenced by interface field in the API
      # schema
      interface Referencer2 {
        somefield: [Enum]
      }

      # Inaccessible enum type can't be referenced by object field argument in
      # the API schema
      type Referencer3 implements Referencer4 {
        someField(someArg: Enum): String
      }

      # Inaccessible enum type can't be referenced by interface field argument
      # in the API schema
      interface Referencer4 {
        someField(someArg: Enum): String
      }

      # Inaccessible enum type can't be referenced by input object field in the
      # API schema
      input Referencer5 {
        someField: Enum
      }

      # Inaccessible enum type can't be referenced by directive argument in the
      # API schema
      directive @referencer6(someArg: Enum) on FRAGMENT_SPREAD
    "#,
    )
    .expect_err("should return validation errors");

    insta::assert_display_snapshot!(errors);
}

#[test]
fn inaccessible_scalar_with_accessible_references() {
    let errors = inaccessible_to_api_schema(
        r#"
      type Query {
        someField: String
      }

      # Inaccessible scalar type
      scalar Scalar @inaccessible

      # Inaccessible scalar type can't be referenced by object field in the API
      # schema
      type Referencer1 implements Referencer2 {
        somefield: [[Scalar!]!]!
      }

      # Inaccessible scalar type can't be referenced by interface field in the
      # API schema
      interface Referencer2 {
        somefield: [[Scalar]]
      }

      # Inaccessible scalar type can't be referenced by object field argument in
      # the API schema
      type Referencer3 implements Referencer4 {
        someField(someArg: Scalar): String
      }

      # Inaccessible scalar type can't be referenced by interface field argument
      # in the API schema
      interface Referencer4 {
        someField(someArg: Scalar): String
      }

      # Inaccessible scalar type can't be referenced by input object field in
      # the API schema
      input Referencer5 {
        someField: Scalar
      }

      # Inaccessible scalar type can't be referenced by directive argument in
      # the API schema
      directive @referencer6(someArg: Scalar) on MUTATION
    "#,
    )
    .expect_err("should return validation errors");

    insta::assert_display_snapshot!(errors);
}

#[test]
fn remove_inaccessible() {
    // let s = api_schema(r#""#);
}
