use crate::error::FederationError;
use crate::query_plan::operation::{FragmentSpreadNormalizationOption, NormalizedSelectionSet};
use crate::schema::position::{
    CompositeTypeDefinitionPosition, FieldDefinitionPosition, InterfaceTypeDefinitionPosition,
    ObjectTypeDefinitionPosition, UnionTypeDefinitionPosition,
};
use crate::schema::{FederationSchema, ValidFederationSchema};
use apollo_compiler::executable::{FieldSet, Selection, SelectionSet};
use apollo_compiler::schema::{ExtendedType, NamedType};
use apollo_compiler::validation::Valid;
use apollo_compiler::{NodeStr, Schema};
use indexmap::IndexMap;

// TODO: In the JS codebase, this optionally runs an additional validation to forbid aliases, and
// has some error-rewriting to help give the user better hints around non-existent fields.
pub(crate) fn parse_field_set(
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
        FragmentSpreadNormalizationOption::InlineFragmentSpread,
    )
}

/// This exists because there's a single callsite in extract_subgraphs_from_supergraph() that needs
/// to parse field sets before the schema has finished building. Outside that case, you should
/// always use `parse_field_set()` instead.
// TODO: As noted in the single callsite, ideally we could move the parsing to after extraction, but
// it takes time to determine whether that impacts correctness, so we're leaving it for later.
pub(crate) fn parse_field_set_without_normalization(
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

// PORT_NOTE: The JS codebase called this `collectTargetFields()`, but this naming didn't make it
// apparent that this was collecting from a field set, so we've renamed it accordingly. Note that
// the JS function also optionally collected interface field implementations, but we've split that
// off into a separate function.
pub(crate) fn collect_target_fields_from_field_set(
    schema: &Valid<Schema>,
    parent_type_name: NamedType,
    value: NodeStr,
) -> Result<Vec<FieldDefinitionPosition>, FederationError> {
    // Note this parsing takes care of adding curly braces ("{" and "}") if they aren't in the
    // string.
    let field_set = FieldSet::parse_and_validate(
        schema,
        parent_type_name,
        value.as_str(),
        "field_set.graphql",
    )?;
    let mut stack = vec![&field_set.selection_set];
    let mut fields = vec![];
    while let Some(selection_set) = stack.pop() {
        let Some(parent_type) = schema.types.get(&selection_set.ty) else {
            return Err(FederationError::internal(
                "Unexpectedly missing selection set type from schema.",
            ));
        };
        let parent_type_position: CompositeTypeDefinitionPosition = match parent_type {
            ExtendedType::Object(_) => ObjectTypeDefinitionPosition {
                type_name: selection_set.ty.clone(),
            }
            .into(),
            ExtendedType::Interface(_) => InterfaceTypeDefinitionPosition {
                type_name: selection_set.ty.clone(),
            }
            .into(),
            ExtendedType::Union(_) => UnionTypeDefinitionPosition {
                type_name: selection_set.ty.clone(),
            }
            .into(),
            _ => {
                return Err(FederationError::internal(
                    "Unexpectedly encountered non-composite type for selection set.",
                ));
            }
        };
        // The stack iterates through what we push in reverse order, so we iterate through
        // selections in reverse order to fix it.
        for selection in selection_set.selections.iter().rev() {
            match selection {
                Selection::Field(field) => {
                    fields.push(parent_type_position.field(field.name.clone())?);
                    if !field.selection_set.selections.is_empty() {
                        stack.push(&field.selection_set);
                    }
                }
                Selection::FragmentSpread(_) => {
                    return Err(FederationError::internal(
                        "Unexpectedly encountered fragment spread in FieldSet.",
                    ));
                }
                Selection::InlineFragment(inline_fragment) => {
                    stack.push(&inline_fragment.selection_set);
                }
            }
        }
    }
    Ok(fields)
}

// PORT_NOTE: This is meant as a companion function for collect_target_fields_from_field_set(), as
// some callers will also want to include interface field implementations.
pub(crate) fn add_interface_field_implementations(
    fields: Vec<FieldDefinitionPosition>,
    schema: &FederationSchema,
) -> Result<Vec<FieldDefinitionPosition>, FederationError> {
    let mut new_fields = vec![];
    for field in fields {
        let interface_field = if let FieldDefinitionPosition::Interface(field) = &field {
            Some(field.clone())
        } else {
            None
        };
        new_fields.push(field);
        if let Some(interface_field) = interface_field {
            for implementing_type in &schema
                .referencers
                .get_interface_type(&interface_field.type_name)?
                .object_types
            {
                new_fields.push(
                    implementing_type
                        .field(interface_field.field_name.clone())
                        .into(),
                );
            }
        }
    }
    Ok(new_fields)
}
