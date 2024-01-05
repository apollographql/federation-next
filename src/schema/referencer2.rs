// Experimental alternative referencer data structure used by @inaccessible.
// May be backed out?

use super::position::*;
use super::referencer::EnumTypeReferencers;
use super::referencer::InputObjectTypeReferencers;
use super::referencer::InterfaceTypeReferencers;
use super::referencer::ObjectTypeReferencers;
use super::referencer::Referencers;
use super::referencer::ScalarTypeReferencers;
use super::referencer::UnionTypeReferencers;
use indexmap::IndexMap;
use indexmap::IndexSet;

pub(crate) type TypeDefinitionReferencers =
    IndexMap<TypeDefinitionPosition, IndexSet<TypeDefinitionReferencer>>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, derive_more::Display)]
pub(crate) enum TypeDefinitionReferencer {
    SchemaRoot(SchemaRootDefinitionPosition),
    Object(ObjectTypeDefinitionPosition),
    ObjectField(ObjectFieldDefinitionPosition),
    ObjectFieldArgument(ObjectFieldArgumentDefinitionPosition),
    Interface(InterfaceTypeDefinitionPosition),
    InterfaceField(InterfaceFieldDefinitionPosition),
    InterfaceFieldArgument(InterfaceFieldArgumentDefinitionPosition),
    Union(UnionTypeDefinitionPosition),
    UnionField(UnionTypenameFieldDefinitionPosition),
    InputObjectField(InputObjectFieldDefinitionPosition),
    DirectiveArgument(DirectiveArgumentDefinitionPosition),
}

pub(crate) type DirectiveDefinitionReferencers =
    IndexMap<DirectiveDefinitionPosition, IndexSet<DirectiveDefinitionReferencer>>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum DirectiveDefinitionReferencer {
    Schema(SchemaDefinitionPosition),
    Scalar(ScalarTypeDefinitionPosition),
    Object(ObjectTypeDefinitionPosition),
    ObjectField(ObjectFieldDefinitionPosition),
    ObjectFieldArgument(ObjectFieldArgumentDefinitionPosition),
    Interface(InterfaceTypeDefinitionPosition),
    InterfaceField(InterfaceFieldDefinitionPosition),
    InterfaceFieldArgument(InterfaceFieldArgumentDefinitionPosition),
    Union(UnionTypeDefinitionPosition),
    Enum(EnumTypeDefinitionPosition),
    EnumValue(EnumValueDefinitionPosition),
    InputObject(InputObjectTypeDefinitionPosition),
    InputObjectField(InputObjectFieldDefinitionPosition),
    DirectiveArgument(DirectiveArgumentDefinitionPosition),
}

impl From<ObjectTypeReferencers> for IndexSet<TypeDefinitionReferencer> {
    fn from(referencers: ObjectTypeReferencers) -> Self {
        referencers
            .object_fields
            .into_iter()
            .map(TypeDefinitionReferencer::ObjectField)
            .chain(
                referencers
                    .interface_fields
                    .into_iter()
                    .map(TypeDefinitionReferencer::InterfaceField),
            )
            .chain(
                referencers
                    .union_types
                    .into_iter()
                    .map(TypeDefinitionReferencer::Union),
            )
            .chain(
                referencers
                    .schema_roots
                    .into_iter()
                    .map(TypeDefinitionReferencer::SchemaRoot),
            )
            .collect()
    }
}

impl From<InterfaceTypeReferencers> for IndexSet<TypeDefinitionReferencer> {
    fn from(referencers: InterfaceTypeReferencers) -> Self {
        referencers
            .object_fields
            .into_iter()
            .map(TypeDefinitionReferencer::ObjectField)
            .chain(
                referencers
                    .interface_fields
                    .into_iter()
                    .map(TypeDefinitionReferencer::InterfaceField),
            )
            .chain(
                referencers
                    .interface_types
                    .into_iter()
                    .map(TypeDefinitionReferencer::Interface),
            )
            .chain(
                referencers
                    .object_types
                    .into_iter()
                    .map(TypeDefinitionReferencer::Object),
            )
            .collect()
    }
}

impl From<ScalarTypeReferencers> for IndexSet<TypeDefinitionReferencer> {
    fn from(referencers: ScalarTypeReferencers) -> Self {
        referencers
            .object_fields
            .into_iter()
            .map(TypeDefinitionReferencer::ObjectField)
            .chain(
                referencers
                    .object_field_arguments
                    .into_iter()
                    .map(TypeDefinitionReferencer::ObjectFieldArgument),
            )
            .chain(
                referencers
                    .interface_fields
                    .into_iter()
                    .map(TypeDefinitionReferencer::InterfaceField),
            )
            .chain(
                referencers
                    .interface_field_arguments
                    .into_iter()
                    .map(TypeDefinitionReferencer::InterfaceFieldArgument),
            )
            .chain(
                referencers
                    .union_fields
                    .into_iter()
                    .map(TypeDefinitionReferencer::UnionField),
            )
            .chain(
                referencers
                    .input_object_fields
                    .into_iter()
                    .map(TypeDefinitionReferencer::InputObjectField),
            )
            .chain(
                referencers
                    .directive_arguments
                    .into_iter()
                    .map(TypeDefinitionReferencer::DirectiveArgument),
            )
            .collect()
    }
}

impl From<UnionTypeReferencers> for IndexSet<TypeDefinitionReferencer> {
    fn from(referencers: UnionTypeReferencers) -> Self {
        referencers
            .object_fields
            .into_iter()
            .map(TypeDefinitionReferencer::ObjectField)
            .chain(
                referencers
                    .interface_fields
                    .into_iter()
                    .map(TypeDefinitionReferencer::InterfaceField),
            )
            .collect()
    }
}

impl From<EnumTypeReferencers> for IndexSet<TypeDefinitionReferencer> {
    fn from(referencers: EnumTypeReferencers) -> Self {
        referencers
            .object_fields
            .into_iter()
            .map(TypeDefinitionReferencer::ObjectField)
            .chain(
                referencers
                    .object_field_arguments
                    .into_iter()
                    .map(TypeDefinitionReferencer::ObjectFieldArgument),
            )
            .chain(
                referencers
                    .interface_fields
                    .into_iter()
                    .map(TypeDefinitionReferencer::InterfaceField),
            )
            .chain(
                referencers
                    .interface_field_arguments
                    .into_iter()
                    .map(TypeDefinitionReferencer::InterfaceFieldArgument),
            )
            .chain(
                referencers
                    .input_object_fields
                    .into_iter()
                    .map(TypeDefinitionReferencer::InputObjectField),
            )
            .chain(
                referencers
                    .directive_arguments
                    .into_iter()
                    .map(TypeDefinitionReferencer::DirectiveArgument),
            )
            .collect()
    }
}

impl From<InputObjectTypeReferencers> for IndexSet<TypeDefinitionReferencer> {
    fn from(referencers: InputObjectTypeReferencers) -> Self {
        referencers
            .object_field_arguments
            .into_iter()
            .map(TypeDefinitionReferencer::ObjectFieldArgument)
            .chain(
                referencers
                    .interface_field_arguments
                    .into_iter()
                    .map(TypeDefinitionReferencer::InterfaceFieldArgument),
            )
            .chain(
                referencers
                    .input_object_fields
                    .into_iter()
                    .map(TypeDefinitionReferencer::InputObjectField),
            )
            .chain(
                referencers
                    .directive_arguments
                    .into_iter()
                    .map(TypeDefinitionReferencer::DirectiveArgument),
            )
            .collect()
    }
}

impl Referencers {
    pub(crate) fn to_type_definition_referencers(&self) -> TypeDefinitionReferencers {
        let c = self.clone();
        let mut map = IndexMap::new();
        for (scalar_name, referencers) in c.scalar_types {
            map.insert(
                TypeDefinitionPosition::Scalar(ScalarTypeDefinitionPosition {
                    type_name: scalar_name,
                }),
                referencers.into(),
            );
        }
        for (object_name, referencers) in c.object_types {
            map.insert(
                TypeDefinitionPosition::Object(ObjectTypeDefinitionPosition {
                    type_name: object_name,
                }),
                referencers.into(),
            );
        }
        for (interface_name, referencers) in c.interface_types {
            map.insert(
                TypeDefinitionPosition::Interface(InterfaceTypeDefinitionPosition {
                    type_name: interface_name,
                }),
                referencers.into(),
            );
        }
        for (union_name, referencers) in c.union_types {
            map.insert(
                TypeDefinitionPosition::Union(UnionTypeDefinitionPosition {
                    type_name: union_name,
                }),
                referencers.into(),
            );
        }
        for (enum_name, referencers) in c.enum_types {
            map.insert(
                TypeDefinitionPosition::Enum(EnumTypeDefinitionPosition {
                    type_name: enum_name,
                }),
                referencers.into(),
            );
        }
        for (input_object_name, referencers) in c.input_object_types {
            map.insert(
                TypeDefinitionPosition::InputObject(InputObjectTypeDefinitionPosition {
                    type_name: input_object_name,
                }),
                referencers.into(),
            );
        }
        map
    }
}
