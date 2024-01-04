use crate::error::{FederationError, SingleFederationError};
use crate::schema::position::{
    DirectiveArgumentDefinitionPosition, DirectiveDefinitionPosition, EnumTypeDefinitionPosition,
    EnumValueDefinitionPosition, InputObjectFieldDefinitionPosition,
    InputObjectTypeDefinitionPosition, InterfaceFieldArgumentDefinitionPosition,
    InterfaceFieldDefinitionPosition, InterfaceTypeDefinitionPosition,
    ObjectFieldArgumentDefinitionPosition, ObjectFieldDefinitionPosition,
    ObjectTypeDefinitionPosition, ScalarTypeDefinitionPosition, SchemaDefinitionPosition,
    SchemaRootDefinitionPosition, UnionTypeDefinitionPosition,
    UnionTypenameFieldDefinitionPosition,
};
use apollo_compiler::schema::Name;
use indexmap::{Equivalent, IndexMap, IndexSet};
use std::hash::Hash;

use super::position::TypeDefinitionPosition;

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

#[derive(Debug, Clone, Default)]
pub(crate) struct Referencers {
    pub(crate) scalar_types: IndexMap<Name, ScalarTypeReferencers>,
    pub(crate) object_types: IndexMap<Name, ObjectTypeReferencers>,
    pub(crate) interface_types: IndexMap<Name, InterfaceTypeReferencers>,
    pub(crate) union_types: IndexMap<Name, UnionTypeReferencers>,
    pub(crate) enum_types: IndexMap<Name, EnumTypeReferencers>,
    pub(crate) input_object_types: IndexMap<Name, InputObjectTypeReferencers>,
    pub(crate) directives: IndexMap<Name, DirectiveReferencers>,
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

    pub(crate) fn contains_type_name<Q: Hash + Equivalent<Name>>(&self, name: &Q) -> bool {
        self.scalar_types.contains_key(name)
            || self.object_types.contains_key(name)
            || self.interface_types.contains_key(name)
            || self.union_types.contains_key(name)
            || self.enum_types.contains_key(name)
            || self.input_object_types.contains_key(name)
    }

    pub(crate) fn get_scalar_type<Q: Hash + Equivalent<Name>>(
        &self,
        name: &Q,
    ) -> Result<&ScalarTypeReferencers, FederationError> {
        self.scalar_types.get(name).ok_or_else(|| {
            SingleFederationError::Internal {
                message: "Scalar type referencers unexpectedly missing type".to_owned(),
            }
            .into()
        })
    }

    pub(crate) fn get_object_type<Q: Hash + Equivalent<Name>>(
        &self,
        name: &Q,
    ) -> Result<&ObjectTypeReferencers, FederationError> {
        self.object_types.get(name).ok_or_else(|| {
            SingleFederationError::Internal {
                message: "Object type referencers unexpectedly missing type".to_owned(),
            }
            .into()
        })
    }

    pub(crate) fn get_interface_type<Q: Hash + Equivalent<Name>>(
        &self,
        name: &Q,
    ) -> Result<&InterfaceTypeReferencers, FederationError> {
        self.interface_types.get(name).ok_or_else(|| {
            SingleFederationError::Internal {
                message: "Interface type referencers unexpectedly missing type".to_owned(),
            }
            .into()
        })
    }

    pub(crate) fn get_union_type<Q: Hash + Equivalent<Name>>(
        &self,
        name: &Q,
    ) -> Result<&UnionTypeReferencers, FederationError> {
        self.union_types.get(name).ok_or_else(|| {
            SingleFederationError::Internal {
                message: "Union type referencers unexpectedly missing type".to_owned(),
            }
            .into()
        })
    }

    pub(crate) fn get_enum_type<Q: Hash + Equivalent<Name>>(
        &self,
        name: &Q,
    ) -> Result<&EnumTypeReferencers, FederationError> {
        self.enum_types.get(name).ok_or_else(|| {
            SingleFederationError::Internal {
                message: "Enum type referencers unexpectedly missing type".to_owned(),
            }
            .into()
        })
    }

    pub(crate) fn get_input_object_type<Q: Hash + Equivalent<Name>>(
        &self,
        name: &Q,
    ) -> Result<&InputObjectTypeReferencers, FederationError> {
        self.input_object_types.get(name).ok_or_else(|| {
            SingleFederationError::Internal {
                message: "Input object type referencers unexpectedly missing type".to_owned(),
            }
            .into()
        })
    }

    pub(crate) fn get_directive<Q: Hash + Equivalent<Name>>(
        &self,
        name: &Q,
    ) -> Result<&DirectiveReferencers, FederationError> {
        self.directives.get(name).ok_or_else(|| {
            SingleFederationError::Internal {
                message: "Directive referencers unexpectedly missing directive".to_owned(),
            }
            .into()
        })
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ScalarTypeReferencers {
    pub(crate) object_fields: IndexSet<ObjectFieldDefinitionPosition>,
    pub(crate) object_field_arguments: IndexSet<ObjectFieldArgumentDefinitionPosition>,
    pub(crate) interface_fields: IndexSet<InterfaceFieldDefinitionPosition>,
    pub(crate) interface_field_arguments: IndexSet<InterfaceFieldArgumentDefinitionPosition>,
    pub(crate) union_fields: IndexSet<UnionTypenameFieldDefinitionPosition>,
    pub(crate) input_object_fields: IndexSet<InputObjectFieldDefinitionPosition>,
    pub(crate) directive_arguments: IndexSet<DirectiveArgumentDefinitionPosition>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ObjectTypeReferencers {
    pub(crate) schema_roots: IndexSet<SchemaRootDefinitionPosition>,
    pub(crate) object_fields: IndexSet<ObjectFieldDefinitionPosition>,
    pub(crate) interface_fields: IndexSet<InterfaceFieldDefinitionPosition>,
    pub(crate) union_types: IndexSet<UnionTypeDefinitionPosition>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct InterfaceTypeReferencers {
    pub(crate) object_types: IndexSet<ObjectTypeDefinitionPosition>,
    pub(crate) object_fields: IndexSet<ObjectFieldDefinitionPosition>,
    pub(crate) interface_types: IndexSet<InterfaceTypeDefinitionPosition>,
    pub(crate) interface_fields: IndexSet<InterfaceFieldDefinitionPosition>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct UnionTypeReferencers {
    pub(crate) object_fields: IndexSet<ObjectFieldDefinitionPosition>,
    pub(crate) interface_fields: IndexSet<InterfaceFieldDefinitionPosition>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct EnumTypeReferencers {
    pub(crate) object_fields: IndexSet<ObjectFieldDefinitionPosition>,
    pub(crate) object_field_arguments: IndexSet<ObjectFieldArgumentDefinitionPosition>,
    pub(crate) interface_fields: IndexSet<InterfaceFieldDefinitionPosition>,
    pub(crate) interface_field_arguments: IndexSet<InterfaceFieldArgumentDefinitionPosition>,
    pub(crate) input_object_fields: IndexSet<InputObjectFieldDefinitionPosition>,
    pub(crate) directive_arguments: IndexSet<DirectiveArgumentDefinitionPosition>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct InputObjectTypeReferencers {
    pub(crate) object_field_arguments: IndexSet<ObjectFieldArgumentDefinitionPosition>,
    pub(crate) interface_field_arguments: IndexSet<InterfaceFieldArgumentDefinitionPosition>,
    pub(crate) input_object_fields: IndexSet<InputObjectFieldDefinitionPosition>,
    pub(crate) directive_arguments: IndexSet<DirectiveArgumentDefinitionPosition>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct DirectiveReferencers {
    pub(crate) schema: Option<SchemaDefinitionPosition>,
    pub(crate) scalar_types: IndexSet<ScalarTypeDefinitionPosition>,
    pub(crate) object_types: IndexSet<ObjectTypeDefinitionPosition>,
    pub(crate) object_fields: IndexSet<ObjectFieldDefinitionPosition>,
    pub(crate) object_field_arguments: IndexSet<ObjectFieldArgumentDefinitionPosition>,
    pub(crate) interface_types: IndexSet<InterfaceTypeDefinitionPosition>,
    pub(crate) interface_fields: IndexSet<InterfaceFieldDefinitionPosition>,
    pub(crate) interface_field_arguments: IndexSet<InterfaceFieldArgumentDefinitionPosition>,
    pub(crate) union_types: IndexSet<UnionTypeDefinitionPosition>,
    pub(crate) enum_types: IndexSet<EnumTypeDefinitionPosition>,
    pub(crate) enum_values: IndexSet<EnumValueDefinitionPosition>,
    pub(crate) input_object_types: IndexSet<InputObjectTypeDefinitionPosition>,
    pub(crate) input_object_fields: IndexSet<InputObjectFieldDefinitionPosition>,
    pub(crate) directive_arguments: IndexSet<DirectiveArgumentDefinitionPosition>,
}
