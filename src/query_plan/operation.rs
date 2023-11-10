use apollo_compiler::ast::{Argument, DirectiveList, FieldDefinition, Name, NamedType};
use apollo_compiler::executable::{
    Field, Fragment, InlineFragment, Operation, Selection, SelectionSet,
};
use apollo_compiler::{Node, Schema};
use indexmap::IndexMap;

// copy of apollo compiler types that store selections in a map so we can normalize it efficiently
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedSelectionSet {
    pub ty: NamedType,
    pub selections: IndexMap<String, NormalizedSelection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NormalizedSelection {
    NormalizedField(Node<NormalizedField>),
    NormalizedInlineFragment(Node<NormalizedInlineFragment>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedField {
    pub definition: Node<FieldDefinition>,
    pub alias: Option<Name>,
    pub name: Name,
    pub arguments: Vec<Node<Argument>>,
    pub directives: DirectiveList,
    pub selection_set: NormalizedSelectionSet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedInlineFragment {
    pub type_condition: Option<NamedType>,
    pub directives: DirectiveList,
    pub selection_set: NormalizedSelectionSet,
}

impl NormalizedSelectionSet {
    // cannot use From trait as we need to pass the fragments
    // otherwise we need two passes - one to change to map and another one to expand fragments
    fn from_selection_set(
        selection_set: &SelectionSet,
        fragments: &IndexMap<Name, Node<Fragment>>,
    ) -> Self {
        let normalized_selections = normalize_selections(&selection_set.selections, fragments);
        NormalizedSelectionSet {
            ty: selection_set.ty.clone(),
            selections: normalized_selections,
        }
    }
}

// impl From<SelectionSet> for NormalizedSelectionSet {
//     fn from(value: SelectionSet) -> Self {
//         let mut normalized = NormalizedSelectionSet {
//             ty: value.ty.clone(),
//             selections: IndexMap::new()
//         };
//         normalized
//     }
// }

impl From<NormalizedSelectionSet> for SelectionSet {
    fn from(val: NormalizedSelectionSet) -> Self {
        SelectionSet {
            ty: val.ty.clone(),
            selections: flatten_selections(&val.selections),
        }
    }
}

/// Converts vec of Selections to a map of NormalizedSelections
///
/// Expands all named fragments and merge duplicate selections.
fn normalize_selections(
    selections: &Vec<Selection>,
    fragments: &IndexMap<Name, Node<Fragment>>,
) -> IndexMap<String, NormalizedSelection> {
    let mut normalized: IndexMap<String, NormalizedSelection> = IndexMap::new();
    for selection in selections {
        match selection {
            Selection::Field(field) => {
                let expanded_selection_set =
                    normalize_selections(&field.selection_set.selections.to_owned(), fragments);
                let key = selection_field_key(field);

                if let NormalizedSelection::NormalizedField(field_entry) =
                    normalized.entry(key.to_owned()).or_insert(
                        NormalizedSelection::NormalizedField(Node::new(NormalizedField {
                            definition: field.definition.clone(),
                            alias: field.alias.clone(),
                            name: field.name.clone(),
                            arguments: field.arguments.clone(),
                            directives: field.directives.clone(),
                            selection_set: NormalizedSelectionSet {
                                ty: field.selection_set.ty.clone(),
                                selections: IndexMap::new(),
                            },
                        })),
                    )
                {
                    let merged_selections = merge_selections(
                        &field_entry.selection_set.selections,
                        &expanded_selection_set,
                    );
                    let mut selection_set = field_entry.selection_set.clone();
                    selection_set.selections = merged_selections;
                    field_entry.make_mut().selection_set = selection_set;
                    // field_entry.selection_set.selections = merged_selections;
                }
            }
            Selection::FragmentSpread(named_fragment) => {
                if let Some(fragment) = fragments.get(&named_fragment.fragment_name) {
                    let expanded_selection_set = normalize_selections(
                        &fragment.selection_set.selections.to_owned(),
                        fragments,
                    );
                    normalized = merge_selections(&normalized, &expanded_selection_set);
                } else {
                    // no fragment found
                }
            }
            Selection::InlineFragment(inline_fragment) => {
                let expanded_selection_set = normalize_selections(
                    &inline_fragment.selection_set.selections.to_owned(),
                    fragments,
                );
                let key = selection_inline_fragment_key(inline_fragment);

                if let NormalizedSelection::NormalizedInlineFragment(fragment_entry) = normalized
                    .entry(key.to_owned())
                    .or_insert(NormalizedSelection::NormalizedInlineFragment(Node::new(
                        NormalizedInlineFragment {
                            type_condition: inline_fragment.type_condition.clone(),
                            directives: inline_fragment.directives.clone(),
                            selection_set: NormalizedSelectionSet {
                                ty: inline_fragment.selection_set.ty.clone(),
                                selections: IndexMap::new(),
                            },
                        },
                    )))
                {
                    let merged_selections = merge_selections(
                        &fragment_entry.selection_set.selections,
                        &expanded_selection_set,
                    );
                    let mut selection_set = fragment_entry.selection_set.clone();
                    selection_set.selections = merged_selections;
                    fragment_entry.make_mut().selection_set = selection_set;
                }
            }
        }
    }
    normalized
}

fn merge_selections(
    source: &IndexMap<String, NormalizedSelection>,
    to_merge: &IndexMap<String, NormalizedSelection>,
) -> IndexMap<String, NormalizedSelection> {
    let mut merged_selections = source.clone();
    for (key, selection) in to_merge {
        if source.contains_key(key) {
            match selection {
                NormalizedSelection::NormalizedField(field_to_merge) => {
                    if let Some(NormalizedSelection::NormalizedField(source_field)) =
                        merged_selections.get_mut(key)
                    {
                        // todo skip deferred
                        // check if the same
                        let merged_field_selections = merge_selections(
                            &source_field.selection_set.selections,
                            &field_to_merge.selection_set.selections,
                        );
                        let merged_selection_set = NormalizedSelectionSet {
                            ty: source_field.selection_set.ty.clone(),
                            selections: merged_field_selections,
                        };
                        source_field.make_mut().selection_set = merged_selection_set;
                    } else {
                        // should never happen, mismatch on keys
                    }
                }
                NormalizedSelection::NormalizedInlineFragment(fragment_to_merge) => {
                    if let Some(NormalizedSelection::NormalizedInlineFragment(source_fragment)) =
                        merged_selections.get_mut(key)
                    {
                        // todo skip deferred
                        // check if the same
                        let merged_fragment_selections = merge_selections(
                            &source_fragment.selection_set.selections,
                            &fragment_to_merge.selection_set.selections,
                        );
                        let merged_selection_set = NormalizedSelectionSet {
                            ty: source_fragment.selection_set.ty.clone(),
                            selections: merged_fragment_selections,
                        };
                        source_fragment.make_mut().selection_set = merged_selection_set;
                    } else {
                        // should never happen, mismatch on keys
                    }
                }
            }
        } else {
            merged_selections.insert(key.to_owned(), selection.clone());
        }
    }
    merged_selections
}
fn selection_field_key(field: &Field) -> String {
    // TODO args
    let mut result = format!("{}", field.name);
    if !field.directives.is_empty() {
        result.push_str(format!(" {}", field.directives).as_str())
    }
    result
}

fn selection_inline_fragment_key(fragment: &InlineFragment) -> String {
    let mut result = format!(
        "...{}",
        fragment
            .type_condition
            .clone()
            .map_or("".to_owned(), |t| format!(" on {}", t))
    );
    if !fragment.directives.is_empty() {
        result.push_str(format!(" {}", fragment.directives).as_str())
    }
    result
}

fn flatten_selections(selections: &IndexMap<String, NormalizedSelection>) -> Vec<Selection> {
    let mut flattened = vec![];
    for selection in selections.values() {
        match selection {
            NormalizedSelection::NormalizedField(normalized_field) => {
                let selections = flatten_selections(&normalized_field.selection_set.selections);
                let field = Field {
                    definition: normalized_field.definition.to_owned(),
                    alias: normalized_field.alias.to_owned(),
                    name: normalized_field.name.to_owned(),
                    arguments: normalized_field.arguments.to_owned(),
                    directives: normalized_field.directives.to_owned(),
                    selection_set: SelectionSet {
                        ty: normalized_field.selection_set.ty.clone(),
                        selections,
                    },
                };
                flattened.push(Selection::Field(Node::new(field)));
            }
            NormalizedSelection::NormalizedInlineFragment(normalized_fragment) => {
                let selections = flatten_selections(&normalized_fragment.selection_set.selections);
                let fragment = InlineFragment {
                    type_condition: normalized_fragment.type_condition.to_owned(),
                    directives: normalized_fragment.directives.to_owned(),
                    selection_set: SelectionSet {
                        ty: normalized_fragment.selection_set.ty.clone(),
                        selections,
                    },
                };
                flattened.push(Selection::InlineFragment(Node::new(fragment)));
            }
        }
    }
    flattened
}

/// Normalizes selection set within specified operation.
///
/// This method applies following normalizations
/// - expands all fragments within an operation
/// - merge same selections
/// - removes all introspection fields from top-level selection
/// - attempts to remove all unnecessary/redundant inline fragments
pub fn normalize_operation(
    operation: &mut Operation,
    _schema: &Schema,
    fragments: &IndexMap<Name, Node<Fragment>>,
) {
    let mut normalized_selection_set =
        NormalizedSelectionSet::from_selection_set(&operation.selection_set, fragments);
    // removes top level introspection
    normalized_selection_set
        .selections
        .retain(|key, _| !key.starts_with("__"));

    // flatten back to vec
    operation.selection_set = SelectionSet::from(normalized_selection_set);
}

#[cfg(test)]
mod tests {
    use crate::query_plan::operation::normalize_operation;
    use apollo_compiler::executable::Name;
    use apollo_compiler::NodeStr;

    #[test]
    fn expands_named_fragments() {
        let operation_with_named_fragment = r#"
query NamedFragmentQuery {
  foo {
    id
    ...Bar
  }
}

fragment Bar on Foo {
  bar
  baz
}

type Query {
  foo: Foo
}

type Foo {
  id: ID!
  bar: String!
  baz: Int
}
"#;
        let (schema, mut executable_document) =
            apollo_compiler::parse_mixed(operation_with_named_fragment, "document.graphql");

        if let Some(operation) = executable_document
            .named_operations
            .get_mut(&Name::new_unchecked(NodeStr::new("NamedFragmentQuery")))
        {
            let operation = operation.make_mut();
            normalize_operation(operation, &schema, &executable_document.fragments);

            let expected = r#"query NamedFragmentQuery {
  foo {
    id
    bar
    baz
  }
}"#;
            let actual = format!("{}", operation);
            assert_eq!(expected, actual);
        }
    }

    #[test]
    fn expands_and_deduplicates_fragments() {
        let operation_with_named_fragment = r#"
query NestedFragmentQuery {
  foo {
    ...FirstFragment
    ...SecondFragment
  }
}

fragment FirstFragment on Foo {
  id
  bar
  baz
}

fragment SecondFragment on Foo {
  id
  bar
}

type Query {
  foo: Foo
}

type Foo {
  id: ID!
  bar: String!
  baz: String
}
"#;
        let (schema, mut executable_document) =
            apollo_compiler::parse_mixed(operation_with_named_fragment, "document.graphql");

        if let Some((_, operation)) = executable_document.named_operations.first_mut() {
            let operation = operation.make_mut();
            normalize_operation(operation, &schema, &executable_document.fragments);

            let expected = r#"query NestedFragmentQuery {
  foo {
    id
    bar
    baz
  }
}"#;
            let actual = format!("{}", operation);
            assert_eq!(expected, actual);
        }
    }

    #[test]
    fn can_remove_introspection_selections() {
        let operation_with_introspection = r#"
query TestIntrospectionQuery {
  __schema {
    types {
      name
    }
  }
}

type Query {
  foo: String
}
"#;
        let (schema, mut executable_document) =
            apollo_compiler::parse_mixed(operation_with_introspection, "document.graphql");
        if let Some(operation) = executable_document
            .named_operations
            .get_mut(&Name::new_unchecked(NodeStr::new("TestIntrospectionQuery")))
        {
            let operation = operation.make_mut();
            normalize_operation(operation, &schema, &executable_document.fragments);

            assert!(operation.selection_set.selections.is_empty());
        }
    }
}
