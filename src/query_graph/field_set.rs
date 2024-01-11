use crate::error::{FederationError, SingleFederationError};
use crate::query_plan::operation::NormalizedSelectionSet;
use crate::schema::ValidFederationSchema;
use apollo_compiler::executable::{FieldSet, SelectionSet};
use apollo_compiler::schema::NamedType;
use apollo_compiler::validation::Valid;
use apollo_compiler::{NodeStr, Schema};
use indexmap::IndexMap;

// TODO: In the JS codebase, this optionally runs an additional validation to forbid aliases, and
// has some error-rewriting to help give the user better hints around non-existent fields.
pub(super) fn parse_field_set(
    schema: &ValidFederationSchema,
    parent_type_name: NamedType,
    value: NodeStr,
) -> Result<NormalizedSelectionSet, FederationError> {
    // Note this parsing takes care of adding curly braces ("{" and "}") if they aren't in the
    // string.
    let field_set = FieldSet::parse_and_validate(
        schema.schema(),
        parent_type_name,
        value.as_str(),
        "field_set.graphql",
    )?;
    NormalizedSelectionSet::normalize_and_expand_fragments(
        &field_set.selection_set,
        &IndexMap::new(),
        schema,
    )
}

/// This exists because there's a single callsite in extract_subgraphs_from_supergraph() that needs
/// to parse field sets before the schema has finished building. Outside that case, you should
/// always use `parse_field_set()` instead.
// TODO: As noted in the single callsite, ideally we could move the parsing to after extraction, but
// it takes time to determine whether that impacts correctness, so we're leaving it for later.
pub(super) fn parse_field_set_without_normalization(
    schema: &Valid<Schema>,
    parent_type_name: NamedType,
    value: NodeStr,
) -> Result<SelectionSet, FederationError> {
    // Note this parsing takes care of adding curly braces ("{" and "}") if they aren't in the
    // string.
    let field_set = FieldSet::parse_and_validate(
        schema,
        parent_type_name,
        value.as_str(),
        "field_set.graphql",
    )?;
    Ok(field_set.into_inner().selection_set)
}

pub(super) fn merge_selection_sets(
    mut selection_sets: impl Iterator<Item = NormalizedSelectionSet> + ExactSizeIterator,
) -> Result<NormalizedSelectionSet, FederationError> {
    let Some(mut first) = selection_sets.next() else {
        return Err(SingleFederationError::Internal {
            message: "".to_owned(),
        }
        .into());
    };
    first.merge_into(selection_sets)?;
    Ok(first)
}

pub(super) fn equal_selection_sets(
    _a: &NormalizedSelectionSet,
    _b: &NormalizedSelectionSet,
) -> Result<bool, FederationError> {
    // TODO: Once operation processing is done, we should be able to call into that logic here.
    // We're specifically wanting the equivalent of something like
    // ```
    // selectionSetOfNode(...).equals(selectionSetOfNode(...));
    // ```
    // from the JS codebase. It may be more performant for federation-next to use its own
    // representation instead of repeatedly inter-converting between its representation and the
    // apollo-rs one, but we'll cross that bridge if we come to it.
    todo!();
}
