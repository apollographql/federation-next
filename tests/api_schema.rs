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

    insta::assert_display_snapshot!(errors, @r###"
    The following errors occurred:

      - Type `Query` is @inaccessible but is the query root type, which must be in the API schema.

      - Type `Object` is @inaccessible but is referenced by `Referencer1.someField`, which is in the API schema.

      - Type `Object` is @inaccessible but is referenced by `Referencer2.someField`, which is in the API schema.

      - Type `Referencer3` is in the API schema but all of its members are @inaccessible.
    "###);
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

    insta::assert_display_snapshot!(errors, @r###"
    The following errors occurred:

      - Type `Interface` is @inaccessible but is referenced by `Referencer1.someField`, which is in the API schema.

      - Type `Interface` is @inaccessible but is referenced by `Referencer2.someField`, which is in the API schema.
    "###);
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

    insta::assert_display_snapshot!(errors, @r###"
    The following errors occurred:

      - Type `Union` is @inaccessible but is referenced by `Referencer1.someField`, which is in the API schema.

      - Type `Union` is @inaccessible but is referenced by `Referencer2.someField`, which is in the API schema.
    "###);
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

    insta::assert_display_snapshot!(errors, @r###"
    The following errors occurred:

      - Type `InputObject` is @inaccessible but is referenced by `Referencer1.someField(someArg:)`, which is in the API schema.

      - Type `InputObject` is @inaccessible but is referenced by `Referencer2.someField(someArg:)`, which is in the API schema.

      - Type `InputObject` is @inaccessible but is referenced by `Referencer3.someField`, which is in the API schema.

      - Type `InputObject` is @inaccessible but is referenced by `@referencer4(someArg:)`, which is in the API schema.
    "###);
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

    insta::assert_display_snapshot!(errors, @r###"
    The following errors occurred:

      - Type `Enum` is @inaccessible but is referenced by `Referencer1.somefield`, which is in the API schema.

      - Type `Enum` is @inaccessible but is referenced by `Referencer3.someField(someArg:)`, which is in the API schema.

      - Type `Enum` is @inaccessible but is referenced by `Referencer2.somefield`, which is in the API schema.

      - Type `Enum` is @inaccessible but is referenced by `Referencer4.someField(someArg:)`, which is in the API schema.

      - Type `Enum` is @inaccessible but is referenced by `Referencer5.someField`, which is in the API schema.

      - Type `Enum` is @inaccessible but is referenced by `@referencer6(someArg:)`, which is in the API schema.
    "###);
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

    insta::assert_display_snapshot!(errors, @r###"
    The following errors occurred:

      - Type `Scalar` is @inaccessible but is referenced by `Referencer1.somefield`, which is in the API schema.

      - Type `Scalar` is @inaccessible but is referenced by `Referencer3.someField(someArg:)`, which is in the API schema.

      - Type `Scalar` is @inaccessible but is referenced by `Referencer2.somefield`, which is in the API schema.

      - Type `Scalar` is @inaccessible but is referenced by `Referencer4.someField(someArg:)`, which is in the API schema.

      - Type `Scalar` is @inaccessible but is referenced by `Referencer5.someField`, which is in the API schema.

      - Type `Scalar` is @inaccessible but is referenced by `@referencer6(someArg:)`, which is in the API schema.
    "###);
}

#[test]
fn inaccessible_object_field_with_accessible_references() {
    let errors = inaccessible_to_api_schema(
        r#"
      extend schema {
        mutation: Mutation
        subscription: Subscription
      }

      # Inaccessible object field can't have a non-inaccessible parent query
      # type and no non-inaccessible siblings
      type Query {
        privateField: String @inaccessible
        otherPrivateField: Float @inaccessible
      }

      # Inaccessible object field can't have a non-inaccessible parent mutation
      # type and no non-inaccessible siblings
      type Mutation {
        privateField: String @inaccessible
        otherPrivateField: Float @inaccessible
      }

      # Inaccessible object field can't have a non-inaccessible parent
      # subscription type and no non-inaccessible siblings
      type Subscription {
        privateField: String @inaccessible
        otherPrivateField: Float @inaccessible
      }

      # Inaccessible object field
      type Object implements Referencer1 {
        someField: String
        privateField: String @inaccessible
      }

      # Inaccessible object field can't be referenced by interface field in the
      # API schema
      interface Referencer1 {
        privateField: String
      }

      # Inaccessible object field can't have a non-inaccessible parent object
      # type and no non-inaccessible siblings
      type Referencer2 {
        privateField: String @inaccessible
        otherPrivateField: Float @inaccessible
      }
    "#,
    )
    .expect_err("should return validation errors");

    insta::assert_display_snapshot!(errors, @r###"
    The following errors occurred:

      - Type `Query` is in the API schema but all of its members are @inaccessible.

      - Type `Mutation` is in the API schema but all of its members are @inaccessible.

      - Type `Subscription` is in the API schema but all of its members are @inaccessible.

      - Field `Object.privateField` is @inaccessible but implements the interface field `Referencer1.privateField`, which is in the API schema.

      - Type `Referencer2` is in the API schema but all of its members are @inaccessible.
    "###);
}

#[test]
fn inaccessible_interface_field_with_accessible_references() {
    let errors = inaccessible_to_api_schema(
        r#"
      type Query {
        someField: String
      }

      # Inaccessible interface field
      interface Interface implements Referencer1 {
        someField: String
        privateField: String @inaccessible
      }

      # Inaccessible interface field can't be referenced by interface field in
      # the API schema
      interface Referencer1 {
        privateField: String
      }

      # Inaccessible interface field can't have a non-inaccessible parent object
      # type and no non-inaccessible siblings
      interface Referencer2 {
        privateField: String @inaccessible
        otherPrivateField: Float @inaccessible
      }
    "#,
    )
    .expect_err("should return validation errors");

    insta::assert_display_snapshot!(errors, @r###"
    The following errors occurred:

      - Field `Interface.privateField` is @inaccessible but implements the interface field `Referencer1.privateField`, which is in the API schema.

      - Type `Referencer2` is in the API schema but all of its members are @inaccessible.
    "###);
}

#[test]
fn inaccessible_object_field_arguments_with_accessible_references() {
    let errors = inaccessible_to_api_schema(
        r#"
      type Query {
        someField(someArg: String): String
      }

      # Inaccessible object field argument
      type Object implements Referencer1 {
        someField(privateArg: String @inaccessible): String
      }

      # Inaccessible object field argument can't be referenced by interface
      # field argument in the API schema
      interface Referencer1 {
        someField(privateArg: String): String
      }

      # Inaccessible object field argument can't be a required argument
      type ObjectRequired {
        someField(privateArg: String! @inaccessible): String
      }
    "#,
    )
    .expect_err("should return validation errors");

    insta::assert_display_snapshot!(errors, @r###""###);
}

#[test]
fn inaccessible_interface_field_arguments_with_accessible_references() {
    let errors = inaccessible_to_api_schema(
        r#"
      type Query {
        someField(someArg: String): String
      }

      # Inaccessible interface field argument
      interface Interface implements Referencer1 {
        someField(privateArg: String! = "default" @inaccessible): String
      }

      # Inaccessible interface field argument can't be referenced by interface
      # field argument in the API schema
      interface Referencer1 {
        someField(privateArg: String! = "default"): String
      }

      # Inaccessible object field argument can't be a required argument
      type InterfaceRequired {
        someField(privateArg: String! @inaccessible): String
      }

      # Inaccessible object field argument can't be implemented by a required
      # object field argument in the API schema
      type Referencer2 implements Interface & Referencer1 {
        someField(privateArg: String!): String
      }

      # Inaccessible object field argument can't be implemented by a required
      # interface field argument in the API schema
      interface Referencer3 implements Interface & Referencer1 {
        someField(privateArg: String!): String
      }
    "#,
    )
    .expect_err("should return validation errors");

    insta::assert_display_snapshot!(errors, @r###""###);
}

#[test]
fn inaccessible_input_object_fields_with_accessible_references() {
    let errors = inaccessible_to_api_schema(
        r#"
      type Query {
        someField: String
      }

      # Inaccessible input object field
      input InputObject {
        someField: String
        privateField: String @inaccessible
      }

      # Inaccessible input object field can't be referenced by default value of
      # object field argument in the API schema
      type Referencer1 implements Referencer2 {
        someField(someArg: InputObject = { privateField: "" }): String
      }

      # Inaccessible input object field can't be referenced by default value of
      # interface field argument in the API schema
      interface Referencer2 {
        someField(someArg: InputObject = { privateField: "" }): String
      }

      # Inaccessible input object field can't be referenced by default value of
      # input object field in the API schema
      input Referencer3 {
        someField: InputObject = { privateField: "" }
      }

      # Inaccessible input object field can't be referenced by default value of
      # directive argument in the API schema
      directive @referencer4(
        someArg: InputObject = { privateField: "" }
      ) on FIELD

      # Inaccessible input object field can't have a non-inaccessible parent
      # and no non-inaccessible siblings
      input Referencer5 {
        privateField: String @inaccessible
        otherPrivateField: Float @inaccessible
      }

      # Inaccessible input object field can't be a required field
      input InputObjectRequired {
        someField: String
        privateField: String! @inaccessible
      }
    "#,
    )
    .expect_err("should return validation errors");

    insta::assert_display_snapshot!(errors, @r###"
    The following errors occurred:

      - Input field `InputObject.privateField` is @inaccessible but is used in the default value of `Referencer1.someField(someArg:)`, which is in the API schema.

      - Input field `InputObject.privateField` is @inaccessible but is used in the default value of `Referencer2.someField(someArg:)`, which is in the API schema.

      - Input field `InputObject.privateField` is @inaccessible but is used in the default value of `Referencer3.someField`, which is in the API schema.

      - Type `Referencer5` is in the API schema but all of its input fields are @inaccessible.

      - Input field `InputObjectRequired` is @inaccessible but is a required input field of its type.

      - Input field `InputObject.privateField` is @inaccessible but is used in the default value of `@referencer4(someArg:)`, which is in the API schema.
    "###);
}

#[test]
fn inaccessible_enum_values_with_accessible_references() {
    let errors = inaccessible_to_api_schema(
        r#"
      type Query {
        someField: String
      }

      # Inaccessible enum value
      enum Enum {
        SOME_VALUE
        PRIVATE_VALUE @inaccessible
      }

      # Inaccessible enum value can't be referenced by default value of object
      # field argument in the API schema
      type Referencer1 implements Referencer2 {
        someField(someArg: Enum = PRIVATE_VALUE): String
      }

      # Inaccessible enum value can't be referenced by default value of
      # interface field argument in the API schema
      interface Referencer2 {
        someField(someArg: Enum = PRIVATE_VALUE): String
      }

      # Inaccessible enum value can't be referenced by default value of input
      # object field in the API schema
      input Referencer3 {
        someField: Enum = PRIVATE_VALUE
      }

      # Inaccessible input enum value can't be referenced by default value of
      # directive argument in the API schema
      directive @referencer4(someArg: Enum = PRIVATE_VALUE) on INLINE_FRAGMENT

      # Inaccessible enum value can't have a non-inaccessible parent and no
      # non-inaccessible siblings
      enum Referencer5 {
        PRIVATE_VALUE @inaccessible
        OTHER_PRIVATE_VALUE @inaccessible
      }
    "#,
    )
    .expect_err("should return validation errors");

    insta::assert_display_snapshot!(errors, @r###"
    The following errors occurred:

      - Enum value `Enum.PRIVATE_VALUE` is @inaccessible but is used in the default value of `Referencer1.someField(someArg:)`, which is in the API schema.

      - Enum value `Enum.PRIVATE_VALUE` is @inaccessible but is used in the default value of `Referencer2.someField(someArg:)`, which is in the API schema.

      - Enum value `Enum.PRIVATE_VALUE` is @inaccessible but is used in the default value of `Referencer3.someField`, which is in the API schema.

      - Type `Referencer5` is in the API schema but all of its members are @inaccessible.

      - Enum value `Enum.PRIVATE_VALUE` is @inaccessible but is used in the default value of `@referencer4(someArg:)`, which is in the API schema.
    "###);
}
