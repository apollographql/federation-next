use crate::schema::location::{
    DirectiveArgumentDefinitionLocation, EnumTypeDefinitionLocation, EnumValueDefinitionLocation,
    InputObjectFieldDefinitionLocation, InputObjectTypeDefinitionLocation,
    InterfaceFieldArgumentDefinitionLocation, InterfaceFieldDefinitionLocation,
    InterfaceTypeDefinitionLocation, ObjectFieldArgumentDefinitionLocation,
    ObjectFieldDefinitionLocation, ObjectTypeDefinitionLocation, ScalarTypeDefinitionLocation,
    SchemaDefinitionLocation, SchemaRootDefinitionLocation, UnionTypeDefinitionLocation,
};
use apollo_compiler::schema::Name;
use indexmap::{Equivalent, IndexMap, IndexSet};
use std::hash::Hash;

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
    pub(crate) fn contains_type_name<Q: Hash + Equivalent<Name>>(&self, name: &Q) -> bool {
        self.scalar_types.contains_key(name)
            || self.object_types.contains_key(name)
            || self.interface_types.contains_key(name)
            || self.union_types.contains_key(name)
            || self.enum_types.contains_key(name)
            || self.input_object_types.contains_key(name)
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ScalarTypeReferencers {
    pub(crate) object_fields: IndexSet<ObjectFieldDefinitionLocation>,
    pub(crate) object_field_arguments: IndexSet<ObjectFieldArgumentDefinitionLocation>,
    pub(crate) interface_fields: IndexSet<InterfaceFieldDefinitionLocation>,
    pub(crate) interface_field_arguments: IndexSet<InterfaceFieldArgumentDefinitionLocation>,
    pub(crate) input_object_fields: IndexSet<InputObjectFieldDefinitionLocation>,
    pub(crate) directive_arguments: IndexSet<DirectiveArgumentDefinitionLocation>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ObjectTypeReferencers {
    pub(crate) schema_roots: IndexSet<SchemaRootDefinitionLocation>,
    pub(crate) object_fields: IndexSet<ObjectFieldDefinitionLocation>,
    pub(crate) interface_fields: IndexSet<InterfaceFieldDefinitionLocation>,
    pub(crate) union_types: IndexSet<UnionTypeDefinitionLocation>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct InterfaceTypeReferencers {
    pub(crate) object_types: IndexSet<ObjectTypeDefinitionLocation>,
    pub(crate) object_fields: IndexSet<ObjectFieldDefinitionLocation>,
    pub(crate) interface_types: IndexSet<InterfaceTypeDefinitionLocation>,
    pub(crate) interface_fields: IndexSet<InterfaceFieldDefinitionLocation>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct UnionTypeReferencers {
    pub(crate) object_fields: IndexSet<ObjectFieldDefinitionLocation>,
    pub(crate) interface_fields: IndexSet<InterfaceFieldDefinitionLocation>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct EnumTypeReferencers {
    pub(crate) object_fields: IndexSet<ObjectFieldDefinitionLocation>,
    pub(crate) object_field_arguments: IndexSet<ObjectFieldArgumentDefinitionLocation>,
    pub(crate) interface_fields: IndexSet<InterfaceFieldDefinitionLocation>,
    pub(crate) interface_field_arguments: IndexSet<InterfaceFieldArgumentDefinitionLocation>,
    pub(crate) input_object_fields: IndexSet<InputObjectFieldDefinitionLocation>,
    pub(crate) directive_arguments: IndexSet<DirectiveArgumentDefinitionLocation>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct InputObjectTypeReferencers {
    pub(crate) object_field_arguments: IndexSet<ObjectFieldArgumentDefinitionLocation>,
    pub(crate) interface_field_arguments: IndexSet<InterfaceFieldArgumentDefinitionLocation>,
    pub(crate) input_object_fields: IndexSet<InputObjectFieldDefinitionLocation>,
    pub(crate) directive_arguments: IndexSet<DirectiveArgumentDefinitionLocation>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct DirectiveReferencers {
    pub(crate) schema: Option<SchemaDefinitionLocation>,
    pub(crate) scalar_types: IndexSet<ScalarTypeDefinitionLocation>,
    pub(crate) object_types: IndexSet<ObjectTypeDefinitionLocation>,
    pub(crate) object_fields: IndexSet<ObjectFieldDefinitionLocation>,
    pub(crate) object_field_arguments: IndexSet<ObjectFieldArgumentDefinitionLocation>,
    pub(crate) interface_types: IndexSet<InterfaceTypeDefinitionLocation>,
    pub(crate) interface_fields: IndexSet<InterfaceFieldDefinitionLocation>,
    pub(crate) interface_field_arguments: IndexSet<InterfaceFieldArgumentDefinitionLocation>,
    pub(crate) union_types: IndexSet<UnionTypeDefinitionLocation>,
    pub(crate) enum_types: IndexSet<EnumTypeDefinitionLocation>,
    pub(crate) enum_values: IndexSet<EnumValueDefinitionLocation>,
    pub(crate) input_object_types: IndexSet<InputObjectTypeDefinitionLocation>,
    pub(crate) input_object_fields: IndexSet<InputObjectFieldDefinitionLocation>,
    pub(crate) directive_arguments: IndexSet<DirectiveArgumentDefinitionLocation>,
}
