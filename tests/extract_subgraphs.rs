use apollo_compiler::coord;
use apollo_compiler::schema::Value;
use apollo_compiler::Node;
use apollo_federation::Supergraph;

#[test]
fn preserve_default_values_of_input_fields() {
    let supergraph = Supergraph::new(r#"
    schema
      @link(url: "https://specs.apollo.dev/link/v1.0")
      @link(url: "https://specs.apollo.dev/join/v0.2", for: EXECUTION)
    {
      query: Query
    }

    directive @join__field(graph: join__Graph!, requires: join__FieldSet, provides: join__FieldSet, type: String, external: Boolean, override: String, usedOverridden: Boolean) repeatable on FIELD_DEFINITION | INPUT_FIELD_DEFINITION

    directive @join__graph(name: String!, url: String!) on ENUM_VALUE

    directive @join__implements(graph: join__Graph!, interface: String!) repeatable on OBJECT | INTERFACE

    directive @join__type(graph: join__Graph!, key: join__FieldSet, extension: Boolean! = false, resolvable: Boolean! = true) repeatable on OBJECT | INTERFACE | UNION | ENUM | INPUT_OBJECT | SCALAR

    directive @link(url: String, as: String, for: link__Purpose, import: [link__Import]) repeatable on SCHEMA

    input Input
      @join__type(graph: SERVICE)
    {
      a: Int! = 1234
    }

    scalar join__FieldSet

    enum join__Graph {
      SERVICE @join__graph(name: "service", url: "")
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
      @join__type(graph: SERVICE)
    {
      field(input: Input!): String
    }
    "#).expect("should parse");

    let subgraphs = supergraph
        .extract_subgraphs()
        .expect("should extract subgraphs");

    let service = subgraphs
        .get("service")
        .expect("missing subgraph")
        .schema
        .schema();
    let field_a = coord!(Input.a).lookup_input_field(service).unwrap();
    assert_eq!(
        field_a.default_value,
        Some(Node::new(Value::Int(1234.into())))
    );
}

#[test]
fn erase_empty_types_due_to_overridden_fields() {
    let supergraph = Supergraph::new(r#"
    schema
      @link(url: "https://specs.apollo.dev/link/v1.0")
      @link(url: "https://specs.apollo.dev/join/v0.3", for: EXECUTION)
      @link(url: "https://specs.apollo.dev/tag/v0.3")
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

    directive @tag(name: String!) repeatable on FIELD_DEFINITION | OBJECT | INTERFACE | UNION | ARGUMENT_DEFINITION | SCALAR | ENUM | ENUM_VALUE | INPUT_OBJECT | INPUT_FIELD_DEFINITION | SCHEMA
    input Input
      @join__type(graph: B)
    {
      a: Int! = 1234
    }

    scalar join__FieldSet

    enum join__Graph {
      A @join__graph(name: "a", url: "")
      B @join__graph(name: "b", url: "")
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
      @join__type(graph: A)
    {
      field: String
    }

    type User
    @join__type(graph: A)
    @join__type(graph: B)
    {
      foo: String @join__field(graph: A, override: "b")

      bar: String @join__field(graph: A)

      baz: String @join__field(graph: A)
    }
    "#).expect("should parse");

    let subgraphs = supergraph
        .extract_subgraphs()
        .expect("should extract subgraphs");

    let b = subgraphs
        .get("b")
        .expect("missing subgraph")
        .schema
        .schema();
    assert!(!b.types.contains_key("User"));
}
