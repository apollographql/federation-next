use apollo_federation::query_plan::operation::normalize_operation;

//
// fields
//

#[test]
fn merge_same_fields_without_directives() {
    let operation_string = r#"
query Test {
  t {
    v1
  }
  t {
    v2
 }
}

type Query {
  t: T
}

type T {
  v1: Int
  v2: String
}
"#;
    let (schema, mut executable_document) =
        apollo_compiler::parse_mixed(operation_string, "document.graphql");
    if let Some((_, operation)) = executable_document.named_operations.first_mut() {
        let operation = operation.make_mut();
        normalize_operation(operation, &schema, &executable_document.fragments);
        let expected = r#"query Test {
  t {
    v1
    v2
  }
}"#;
        let actual = format!("{}", operation);
        assert_eq!(expected, actual);
    } else {
        panic!("unable to parse document")
    }
}

#[test]
fn merge_same_fields_with_same_directive() {
    let operation_with_directives = r#"
query Test($skipIf: Boolean!) {
  t @skip(if: $skipIf) {
    v1
  }
  t @skip(if: $skipIf) {
    v2
  }
}

type Query {
  t: T
}

type T {
  v1: Int
  v2: String
}
"#;
    let (schema, mut executable_document) =
        apollo_compiler::parse_mixed(operation_with_directives, "document.graphql");
    if let Some((_, operation)) = executable_document.named_operations.first_mut() {
        let operation = operation.make_mut();
        normalize_operation(operation, &schema, &executable_document.fragments);
        let expected = r#"query Test($skipIf: Boolean!) {
  t @skip(if: $skipIf) {
    v1
    v2
  }
}"#;
        let actual = format!("{}", operation);
        assert_eq!(expected, actual);
    } else {
        panic!("unable to parse document")
    }
}

#[test]
fn merge_same_fields_with_same_directive_but_different_arg_order() {
    let operation_with_directives_different_arg_order = r#"
query Test($skipIf: Boolean!) {
  t @customSkip(if: $skipIf, label: "foo") {
    v1
  }
  t @customSkip(label: "foo", if: $skipIf) {
    v2
  }
}

directive @customSkip(if: Boolean!, label: String!) on FIELD | INLINE_FRAGMENT

type Query {
  t: T
}

type T {
  v1: Int
  v2: String
}
"#;
    let (schema, mut executable_document) = apollo_compiler::parse_mixed(
        operation_with_directives_different_arg_order,
        "document.graphql",
    );
    if let Some((_, operation)) = executable_document.named_operations.first_mut() {
        let operation = operation.make_mut();
        normalize_operation(operation, &schema, &executable_document.fragments);
        let expected = r#"query Test($skipIf: Boolean!) {
  t @customSkip(if: $skipIf, label: "foo") {
    v1
    v2
  }
}"#;
        let actual = format!("{}", operation);
        assert_eq!(expected, actual);
    } else {
        panic!("unable to parse document")
    }
}

#[test]
fn do_not_merge_when_only_one_field_specifies_directive() {
    let operation_one_field_with_directives = r#"
query Test($skipIf: Boolean!) {
  t {
    v1
  }
  t @skip(if: $skipIf) {
    v2
  }
}

type Query {
  t: T
}

type T {
  v1: Int
  v2: String
}
"#;
    let (schema, mut executable_document) =
        apollo_compiler::parse_mixed(operation_one_field_with_directives, "document.graphql");
    if let Some((_, operation)) = executable_document.named_operations.first_mut() {
        let operation = operation.make_mut();
        normalize_operation(operation, &schema, &executable_document.fragments);
        let expected = r#"query Test($skipIf: Boolean!) {
  t {
    v1
  }
  t @skip(if: $skipIf) {
    v2
  }
}"#;
        let actual = format!("{}", operation);
        assert_eq!(expected, actual);
    } else {
        panic!("unable to parse document")
    }
}

#[test]
fn do_not_merge_when_fields_have_different_directives() {
    let operation_different_directives = r#"
query Test($skip1: Boolean!, $skip2: Boolean!) {
  t @skip(if: $skip1) {
    v1
  }
  t @skip(if: $skip2) {
    v2
  }
}

type Query {
  t: T
}

type T {
  v1: Int
  v2: String
}
"#;
    let (schema, mut executable_document) =
        apollo_compiler::parse_mixed(operation_different_directives, "document.graphql");
    if let Some((_, operation)) = executable_document.named_operations.first_mut() {
        let operation = operation.make_mut();
        normalize_operation(operation, &schema, &executable_document.fragments);
        let expected = r#"query Test($skip1: Boolean!, $skip2: Boolean!) {
  t @skip(if: $skip1) {
    v1
  }
  t @skip(if: $skip2) {
    v2
  }
}"#;
        let actual = format!("{}", operation);
        assert_eq!(expected, actual);
    } else {
        panic!("unable to parse document")
    }
}

#[test]
fn do_not_merge_fields_with_defer_directive() {
    let operation_defer_fields = r#"
query Test {
  t @defer {
    v1
  }
  t @defer {
    v2
  }
}

type Query {
  t: T
}

type T {
  v1: Int
  v2: String
}
"#;
    let (schema, mut executable_document) =
        apollo_compiler::parse_mixed(operation_defer_fields, "document.graphql");
    if let Some((_, operation)) = executable_document.named_operations.first_mut() {
        let operation = operation.make_mut();
        normalize_operation(operation, &schema, &executable_document.fragments);
        let expected = r#"query Test {
  t @defer {
    v1
  }
  t @defer {
    v2
  }
}"#;
        let actual = format!("{}", operation);
        assert_eq!(expected, actual);
    } else {
        panic!("unable to parse document")
    }
}

#[test]
fn merge_nested_field_selections() {
    let nested_operation = r#"
query Test {
  t {
    t1
    v @defer {
      v1
    }
  }
  t {
    t1
    t2
    v @defer {
      v2
    }
  }
}

type Query {
  t: T
}

type T {
  t1: Int
  t2: String
  v: V
}

type V {
  v1: Int
  v2: String
}
"#;
    let (schema, mut executable_document) =
        apollo_compiler::parse_mixed(nested_operation, "document.graphql");
    if let Some((_, operation)) = executable_document.named_operations.first_mut() {
        let operation = operation.make_mut();
        normalize_operation(operation, &schema, &executable_document.fragments);
        let expected = r#"query Test {
  t {
    t1
    v @defer {
      v1
    }
    t2
    v @defer {
      v2
    }
  }
}"#;
        let actual = format!("{}", operation);
        assert_eq!(expected, actual);
    } else {
        panic!("unable to parse document")
    }
}

//
// inline fragments
//

#[test]
fn merge_same_fragment_without_directives() {
    let operation_with_fragments = r#"
query Test {
  t {
    ... on T {
      v1
    }
    ... on T {
      v2
    }
  }
}

type Query {
  t: T
}

type T {
  v1: Int
  v2: String
}
"#;
    let (schema, mut executable_document) =
        apollo_compiler::parse_mixed(operation_with_fragments, "document.graphql");
    if let Some((_, operation)) = executable_document.named_operations.first_mut() {
        let operation = operation.make_mut();
        normalize_operation(operation, &schema, &executable_document.fragments);
        let expected = r#"query Test {
  t {
    v1
    v2
  }
}"#;
        let actual = format!("{}", operation);
        assert_eq!(expected, actual);
    } else {
        panic!("unable to parse document")
    }
}

#[test]
fn merge_same_fragments_with_same_directives() {
    let operation_fragments_with_directives = r#"
query Test($skipIf: Boolean!) {
  t {
    ... on T @skip(if: $skipIf) {
      v1
    }
    ... on T @skip(if: $skipIf) {
      v2
    }
  }
}

type Query {
  t: T
}

type T {
  v1: Int
  v2: String
}
"#;
    let (schema, mut executable_document) =
        apollo_compiler::parse_mixed(operation_fragments_with_directives, "document.graphql");
    if let Some((_, operation)) = executable_document.named_operations.first_mut() {
        let operation = operation.make_mut();
        normalize_operation(operation, &schema, &executable_document.fragments);
        let expected = r#"query Test($skipIf: Boolean!) {
  t {
    ... on T @skip(if: $skipIf) {
      v1
      v2
    }
  }
}"#;
        let actual = format!("{}", operation);
        assert_eq!(expected, actual);
    } else {
        panic!("unable to parse document")
    }
}

#[test]
fn merge_same_fragments_with_same_directive_but_different_arg_order() {
    let operation_fragments_with_directives_args_order = r#"
query Test($skipIf: Boolean!) {
  t {
    ... on T @customSkip(if: $skipIf, label: "foo") {
      v1
    }
    ... on T @customSkip(label: "foo", if: $skipIf) {
      v2
    }
  }
}

directive @customSkip(if: Boolean!, label: String!) on FIELD | INLINE_FRAGMENT

type Query {
  t: T
}

type T {
  v1: Int
  v2: String
}
"#;
    let (schema, mut executable_document) = apollo_compiler::parse_mixed(
        operation_fragments_with_directives_args_order,
        "document.graphql",
    );
    if let Some((_, operation)) = executable_document.named_operations.first_mut() {
        let operation = operation.make_mut();
        normalize_operation(operation, &schema, &executable_document.fragments);
        let expected = r#"query Test($skipIf: Boolean!) {
  t {
    ... on T @customSkip(if: $skipIf, label: "foo") {
      v1
      v2
    }
  }
}"#;
        let actual = format!("{}", operation);
        assert_eq!(expected, actual);
    } else {
        panic!("unable to parse document")
    }
}

#[test]
fn do_not_merge_when_only_one_fragment_specifies_directive() {
    let operation_one_fragment_with_directive = r#"
query Test($skipIf: Boolean!) {
  t {
    ... on T {
      v1
    }
    ... on T @skip(if: $skipIf) {
      v2
    }
  }
}

type Query {
  t: T
}

type T {
  v1: Int
  v2: String
}
"#;
    let (schema, mut executable_document) =
        apollo_compiler::parse_mixed(operation_one_fragment_with_directive, "document.graphql");
    if let Some((_, operation)) = executable_document.named_operations.first_mut() {
        let operation = operation.make_mut();
        normalize_operation(operation, &schema, &executable_document.fragments);
        let expected = r#"query Test($skipIf: Boolean!) {
  t {
    v1
    ... on T @skip(if: $skipIf) {
      v2
    }
  }
}"#;
        let actual = format!("{}", operation);
        assert_eq!(expected, actual);
    } else {
        panic!("unable to parse document")
    }
}

#[test]
fn do_not_merge_when_fragments_have_different_directives() {
    let operation_fragments_with_different_directive = r#"
query Test($skip1: Boolean!, $skip2: Boolean!) {
  t {
    ... on T @skip(if: $skip1) {
      v1
    }
    ... on T @skip(if: $skip2) {
      v2
    }
  }
}

type Query {
  t: T
}

type T {
  v1: Int
  v2: String
}
"#;
    let (schema, mut executable_document) = apollo_compiler::parse_mixed(
        operation_fragments_with_different_directive,
        "document.graphql",
    );
    if let Some((_, operation)) = executable_document.named_operations.first_mut() {
        let operation = operation.make_mut();
        normalize_operation(operation, &schema, &executable_document.fragments);
        let expected = r#"query Test($skip1: Boolean!, $skip2: Boolean!) {
  t {
    ... on T @skip(if: $skip1) {
      v1
    }
    ... on T @skip(if: $skip2) {
      v2
    }
  }
}"#;
        let actual = format!("{}", operation);
        assert_eq!(expected, actual);
    } else {
        panic!("unable to parse document")
    }
}

#[test]
fn do_not_merge_fragments_with_defer_directive() {
    let operation_fragments_with_defer = r#"
query Test {
  t {
    ... on T @defer {
      v1
    }
    ... on T @defer {
      v2
    }
  }
}

type Query {
  t: T
}

type T {
  v1: Int
  v2: String
}
"#;
    let (schema, mut executable_document) =
        apollo_compiler::parse_mixed(operation_fragments_with_defer, "document.graphql");
    if let Some((_, operation)) = executable_document.named_operations.first_mut() {
        let operation = operation.make_mut();
        normalize_operation(operation, &schema, &executable_document.fragments);
        let expected = r#"query Test {
  t {
    ... on T @defer {
      v1
    }
    ... on T @defer {
      v2
    }
  }
}"#;
        let actual = format!("{}", operation);
        assert_eq!(expected, actual);
    } else {
        panic!("unable to parse document")
    }
}

#[test]
fn merge_nested_fragments() {
    let operation_nested_fragments = r#"
query Test {
  t {
    ... on T {
      t1
    }
    ... on T {
      v @defer {
        v1
      }
    }
  }
  t {
    ... on T {
      t1
      t2
    }
    ... on T {
      v @defer {
        v2
      }
    }
  }
}

type Query {
  t: T
}

type T {
  t1: Int
  t2: String
  v: V
}

type V {
  v1: Int
  v2: String
}
"#;
    let (schema, mut executable_document) =
        apollo_compiler::parse_mixed(operation_nested_fragments, "document.graphql");
    if let Some((_, operation)) = executable_document.named_operations.first_mut() {
        let operation = operation.make_mut();
        normalize_operation(operation, &schema, &executable_document.fragments);
        let expected = r#"query Test {
  t {
    t1
    v @defer {
      v1
    }
    t2
    v @defer {
      v2
    }
  }
}"#;
        let actual = format!("{}", operation);
        assert_eq!(expected, actual);
    } else {
        panic!("unable to parse document")
    }
}
