use apollo_compiler::{
    ast::{NamedType, Type},
    schema::{InterfaceType, UnionType},
};

use super::position::{InterfaceTypeDefinitionPosition, TypeDefinitionPosition, UnionTypeDefinitionPosition};

pub(crate) enum AbstractType {
    Interface(InterfaceTypeDefinitionPosition),
    Union(UnionTypeDefinitionPosition),
}

pub(crate) enum WrapperType {
    List(Type),
    NonNull(Type),
}

pub(crate) fn base_type(ty: &Type) -> &NamedType {
    match ty {
        Type::Named(named) => named,
        Type::NonNullNamed(named) => named,
        Type::List(ty) => base_type(ty),
        Type::NonNullList(ty) => base_type(ty),
    }
}

pub(crate) fn is_abstract_type(ty: TypeDefinitionPosition) -> bool {
    match ty {
        crate::schema::position::TypeDefinitionPosition::Interface(_)
        | crate::schema::position::TypeDefinitionPosition::Union(_) => true,
        _ => false,
    }
}
/*
self
               .schema
               .get_type(definition.ty.inner_named_type().clone())?; */
