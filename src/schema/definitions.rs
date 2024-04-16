use apollo_compiler::{
    ast::{NamedType, Type},
    Schema,
};

use crate::error::{FederationError, SingleFederationError};

use super::position::{
    InterfaceTypeDefinitionPosition, TypeDefinitionPosition, UnionTypeDefinitionPosition,
};

pub(crate) enum AbstractType {
    Interface(InterfaceTypeDefinitionPosition),
    Union(UnionTypeDefinitionPosition),
}

pub(crate) enum WrapperType {
    List(Type),
    NonNull(Type),
}

pub(crate) fn is_abstract_type(ty: TypeDefinitionPosition) -> bool {
    matches!(
        ty,
        crate::schema::position::TypeDefinitionPosition::Interface(_)
            | crate::schema::position::TypeDefinitionPosition::Union(_)
    )
}

pub(crate) fn is_composite_type(ty: &NamedType, schema: &Schema) -> Result<bool, FederationError> {
    Ok(matches!(
        schema
            .types
            .get(ty)
            .ok_or_else(|| SingleFederationError::Internal {
                message: format!("Cannot find type `'{}\'", ty),
            })?,
        apollo_compiler::schema::ExtendedType::Object(_)
            | apollo_compiler::schema::ExtendedType::Interface(_)
            | apollo_compiler::schema::ExtendedType::Union(_)
    ))
}

/**
 * This essentially follows the beginning of https://spec.graphql.org/draft/#SameResponseShape().
 * That is, the types cannot be merged unless:
 * - they have the same nullability and "list-ability", potentially recursively.
 * - their base type is either both composite, or are the same type.
 */
pub(crate) fn types_can_be_merged(
    t1: &Type,
    t2: &Type,
    schema: &Schema,
) -> Result<bool, FederationError> {
    if t1.is_non_null() {
        return Ok(if t2.is_non_null() {
            types_can_be_merged(&(t1.clone().nullable()), &(t2.clone().nullable()), schema)?
        } else {
            false
        });
    }
    if t1.is_list() {
        return Ok(if t2.is_list() {
            types_can_be_merged(t1.item_type(), t2.item_type(), schema)?
        } else {
            false
        });
    }

    if is_composite_type(t1.inner_named_type(), schema)? {
        return is_composite_type(t2.inner_named_type(), schema);
    }

    Ok(same_type(t1, t2))
}

/**
 * Tests whether 2 types are the "same" type.
 *
 * To be the same type, for this method, is defined as having the samee name for named types
 * or, for wrapper types, the same wrapper type and recursively same wrapped one.
 *
 * This method does not check that both types are from the same schema and does not validate
 * that the structure of named types is the same. Also note that it does not check the "kind"
 * of the type, which is actually relied on due to @interfaceObject (where the "same" type
 * can be an interface in one subgraph but an object type in another, while fundamentally being
 * the same type).
 */
pub(crate) fn same_type(t1: &Type, t2: &Type) -> bool {
    if t1.is_list() {
        return if t2.is_list() {
            same_type(t1.item_type(), t2.item_type())
        } else {
            false
        };
    }

    if t1.is_non_null() {
        return if t2.is_non_null() {
            same_type(&(t1.clone().nullable()), &(t2.clone().nullable()))
        } else {
            false
        };
    }

    t1.inner_named_type() == t2.inner_named_type()
}
