use crate::federation_schema::{OptionLinksMetadata, ReferencerFederationSchemaMut};
use crate::location::{
    DirectiveArgumentDefinitionLocation, DirectiveDefinitionLocation, EnumTypeDefinitionLocation,
    EnumValueDefinitionLocation, InputObjectFieldDefinitionLocation,
    InputObjectTypeDefinitionLocation, InterfaceFieldArgumentDefinitionLocation,
    InterfaceFieldDefinitionLocation, InterfaceTypeDefinitionLocation,
    ObjectFieldArgumentDefinitionLocation, ObjectFieldDefinitionLocation,
    ObjectTypeDefinitionLocation, ScalarTypeDefinitionLocation, SchemaDefinitionLocation,
    SchemaRootDefinitionKind, SchemaRootDefinitionLocation, UnionTypeDefinitionLocation,
};
use crate::referencer::{
    DirectiveReferencers, EnumTypeReferencers, InputObjectTypeReferencers,
    InterfaceTypeReferencers, ObjectTypeReferencers, Referencers, ScalarTypeReferencers,
    UnionTypeReferencers,
};
use apollo_compiler::schema::{
    Component, ComponentStr, Directive, EnumType, EnumValueDefinition, ExtendedType,
    FieldDefinition, InputObjectType, InputValueDefinition, InterfaceType, Name, ObjectType,
    ScalarType, UnionType,
};
use apollo_compiler::{Node, Schema};
use indexmap::Equivalent;
use std::hash::Hash;
use std::ops::Deref;

impl SchemaDefinitionLocation {
    pub fn remove_directive_name<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
        Q: Hash + Equivalent<Name>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
        name: &Q,
    ) {
        self.remove_directive_name_references(schema.referencers.as_mut(), name);
        let Some(schema_definition) = self.try_make_mut(schema.schema) else {
            return;
        };
        schema_definition
            .make_mut()
            .directives
            .retain(|other_directive| !name.equivalent(&other_directive.name));
    }

    pub fn remove_directive<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
        directive: &Component<Directive>,
    ) {
        let Some(schema_definition) = self.try_make_mut(schema.schema) else {
            return;
        };
        if !schema_definition.directives.iter().any(|other_directive| {
            (other_directive.name == directive.name) && !other_directive.ptr_eq(directive)
        }) {
            self.remove_directive_name_references(schema.referencers.as_mut(), &directive.name);
        }
        schema_definition
            .make_mut()
            .directives
            .retain(|other_directive| !other_directive.ptr_eq(directive));
    }

    fn remove_directive_name_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(directive_referencers) = referencers.as_mut().directives.get_mut(name) else {
            return;
        };
        directive_referencers.schema = None;
    }
}

impl SchemaRootDefinitionLocation {
    pub fn remove<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
    ) {
        let Some(root_type) = self.try_get(schema.schema) else {
            return;
        };
        self.remove_references(root_type, schema.referencers.as_mut());
        let parent = self.parent().make_mut(schema.schema).make_mut();
        match self.root_kind {
            SchemaRootDefinitionKind::Query => {
                parent.query = None;
            }
            SchemaRootDefinitionKind::Mutation => {
                parent.mutation = None;
            }
            SchemaRootDefinitionKind::Subscription => {
                parent.subscription = None;
            }
        }
    }

    pub fn remove_recursive<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
    ) {
        self.remove(schema);
    }

    fn remove_references(&self, root_type: &ComponentStr, referencers: &mut Referencers) {
        let Some(object_type_referencers) =
            referencers.as_mut().object_types.get_mut(root_type.deref())
        else {
            return;
        };
        object_type_referencers.schema_roots.remove(self);
    }
}

impl ScalarTypeDefinitionLocation {
    pub fn remove<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
    ) -> Option<ScalarTypeReferencers> {
        let Some(referencers) = self.remove_internal(schema) else {
            return None;
        };
        for field in &referencers.object_fields {
            field.remove(schema);
        }
        for argument in &referencers.object_field_arguments {
            argument.remove(schema);
        }
        for field in &referencers.interface_fields {
            field.remove(schema);
        }
        for argument in &referencers.interface_field_arguments {
            argument.remove(schema);
        }
        for field in &referencers.input_object_fields {
            field.remove(schema);
        }
        for argument in &referencers.directive_arguments {
            argument.remove(schema);
        }
        Some(referencers)
    }

    pub fn remove_recursive<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
    ) {
        let Some(referencers) = self.remove_internal(schema) else {
            return;
        };
        for field in referencers.object_fields {
            field.remove_recursive(schema);
        }
        for argument in referencers.object_field_arguments {
            argument.remove(schema);
        }
        for field in referencers.interface_fields {
            field.remove_recursive(schema);
        }
        for argument in referencers.interface_field_arguments {
            argument.remove(schema);
        }
        for field in referencers.input_object_fields {
            field.remove_recursive(schema);
        }
        for argument in referencers.directive_arguments {
            argument.remove(schema);
        }
    }

    fn remove_internal<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
    ) -> Option<ScalarTypeReferencers> {
        let Some(type_) = self.try_get(schema.schema) else {
            return None;
        };
        self.remove_references(type_, schema.referencers.as_mut());
        schema.schema.types.remove(&self.type_name).unwrap();
        Some(
            schema
                .referencers
                .as_mut()
                .scalar_types
                .remove(&self.type_name)
                .unwrap(),
        )
    }

    pub fn remove_directive_name<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
        Q: Hash + Equivalent<Name>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
        name: &Q,
    ) {
        self.remove_directive_name_references(schema.referencers.as_mut(), name);
        let Some(type_) = self.try_make_mut(schema.schema) else {
            return;
        };
        type_
            .make_mut()
            .directives
            .retain(|other_directive| !name.equivalent(&other_directive.name));
    }

    pub fn remove_directive<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
        directive: &Component<Directive>,
    ) {
        let Some(type_) = self.try_make_mut(schema.schema) else {
            return;
        };
        if !type_.directives.iter().any(|other_directive| {
            (other_directive.name == directive.name) && !other_directive.ptr_eq(directive)
        }) {
            self.remove_directive_name_references(schema.referencers.as_mut(), &directive.name);
        }
        type_
            .make_mut()
            .directives
            .retain(|other_directive| !other_directive.ptr_eq(directive));
    }

    fn remove_references(&self, type_: &Node<ScalarType>, referencers: &mut Referencers) {
        for directive_reference in type_.directives.iter() {
            self.remove_directive_name_references(referencers, &directive_reference.name)
        }
    }

    fn remove_directive_name_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(directive_referencers) = referencers.as_mut().directives.get_mut(name) else {
            return;
        };
        directive_referencers.scalar_types.remove(self);
    }
}

impl ObjectTypeDefinitionLocation {
    pub fn remove<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
    ) -> Option<ObjectTypeReferencers> {
        let Some(referencers) = self.remove_internal(schema) else {
            return None;
        };
        for root in &referencers.schema_roots {
            root.remove(schema);
        }
        for field in &referencers.object_fields {
            field.remove(schema);
        }
        for field in &referencers.interface_fields {
            field.remove(schema);
        }
        for type_ in &referencers.union_types {
            type_.remove_member(schema, &self.type_name);
        }
        Some(referencers)
    }

    pub fn remove_recursive<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
    ) {
        let Some(referencers) = self.remove_internal(schema) else {
            return;
        };
        for root in referencers.schema_roots {
            root.remove_recursive(schema);
        }
        for field in referencers.object_fields {
            field.remove_recursive(schema);
        }
        for field in referencers.interface_fields {
            field.remove_recursive(schema);
        }
        for type_ in referencers.union_types {
            type_.remove_member_recursive(schema, &self.type_name);
        }
    }

    fn remove_internal<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
    ) -> Option<ObjectTypeReferencers> {
        let Some(type_) = self.try_get(schema.schema) else {
            return None;
        };
        self.remove_references(type_, schema.referencers.as_mut());
        for (field_name, field) in type_.fields.iter() {
            ObjectFieldDefinitionLocation {
                type_name: self.type_name.clone(),
                field_name: field_name.clone(),
            }
            .remove_references(field, schema.schema, schema.referencers.as_mut());
        }
        schema.schema.types.remove(&self.type_name).unwrap();
        Some(
            schema
                .referencers
                .as_mut()
                .object_types
                .remove(&self.type_name)
                .unwrap(),
        )
    }

    pub fn remove_directive_name<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
        Q: Hash + Equivalent<Name>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
        name: &Q,
    ) {
        self.remove_directive_name_references(schema.referencers.as_mut(), name);
        let Some(type_) = self.try_make_mut(schema.schema) else {
            return;
        };
        type_
            .make_mut()
            .directives
            .retain(|other_directive| !name.equivalent(&other_directive.name));
    }

    pub fn remove_directive<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
        directive: &Component<Directive>,
    ) {
        let Some(type_) = self.try_make_mut(schema.schema) else {
            return;
        };
        if !type_.directives.iter().any(|other_directive| {
            (other_directive.name == directive.name) && !other_directive.ptr_eq(directive)
        }) {
            self.remove_directive_name_references(schema.referencers.as_mut(), &directive.name);
        }
        type_
            .make_mut()
            .directives
            .retain(|other_directive| !other_directive.ptr_eq(directive));
    }

    pub fn remove_implements_interface<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
        Q: Hash + Equivalent<Name>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
        name: &Q,
    ) {
        self.remove_implements_interface_references(schema.referencers.as_mut(), name);
        let Some(type_) = self.try_make_mut(schema.schema) else {
            return;
        };
        type_
            .make_mut()
            .implements_interfaces
            .retain(|other_type| !name.equivalent(other_type.deref()));
    }

    fn remove_references(&self, type_: &Node<ObjectType>, referencers: &mut Referencers) {
        for directive_reference in type_.directives.iter() {
            self.remove_directive_name_references(referencers, &directive_reference.name);
        }
        for interface_type_reference in type_.implements_interfaces.iter() {
            self.remove_implements_interface_references(
                referencers,
                interface_type_reference.deref(),
            );
        }
    }

    fn remove_directive_name_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(directive_referencers) = referencers.as_mut().directives.get_mut(name) else {
            return;
        };
        directive_referencers.object_types.remove(self);
    }

    fn remove_implements_interface_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(interface_type_referencers) = referencers.as_mut().interface_types.get_mut(name)
        else {
            return;
        };
        interface_type_referencers.object_types.remove(self);
    }
}

impl ObjectFieldDefinitionLocation {
    pub fn remove<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
    ) {
        let Some(field) = self.try_get(schema.schema) else {
            return;
        };
        self.remove_references(field, schema.schema, schema.referencers.as_mut());
        for argument in field.arguments.iter() {
            ObjectFieldArgumentDefinitionLocation {
                type_name: self.type_name.clone(),
                field_name: self.field_name.clone(),
                argument_name: argument.name.clone(),
            }
            .remove_references(argument, schema.schema, schema.referencers.as_mut());
        }
        self.parent()
            .make_mut(schema.schema)
            .make_mut()
            .fields
            .remove(&self.field_name);
    }

    pub fn remove_recursive<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
    ) {
        self.remove(schema);
        let parent = self.parent();
        let Some(type_) = parent.try_get(schema.schema) else {
            return;
        };
        if type_.fields.is_empty() {
            parent.remove_recursive(schema);
        }
    }

    pub fn remove_directive_name<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
        Q: Hash + Equivalent<Name>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
        name: &Q,
    ) {
        self.remove_directive_name_references(schema.referencers.as_mut(), name);
        let Some(field) = self.try_make_mut(schema.schema) else {
            return;
        };
        field
            .make_mut()
            .directives
            .retain(|other_directive| !name.equivalent(&other_directive.name));
    }

    pub fn remove_directive<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
        directive: &Component<Directive>,
    ) {
        let Some(field) = self.try_make_mut(schema.schema) else {
            return;
        };
        if !field.directives.iter().any(|other_directive| {
            (other_directive.name == directive.name) && !other_directive.ptr_eq(directive)
        }) {
            self.remove_directive_name_references(schema.referencers.as_mut(), &directive.name);
        }
        field
            .make_mut()
            .directives
            .retain(|other_directive| !other_directive.ptr_eq(directive));
    }

    fn remove_references(
        &self,
        field: &Component<FieldDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) {
        for directive_reference in field.directives.iter() {
            self.remove_directive_name_references(referencers, &directive_reference.name)
        }
        self.remove_type_references(field, schema, referencers);
    }

    fn remove_directive_name_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(directive_referencers) = referencers.as_mut().directives.get_mut(name) else {
            return;
        };
        directive_referencers.object_fields.remove(self);
    }

    fn remove_type_references(
        &self,
        field: &Component<FieldDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) {
        let output_type_reference = field.ty.inner_named_type();
        match schema.types.get(output_type_reference) {
            Some(ExtendedType::Scalar(_)) => {
                let Some(scalar_type_referencers) = referencers
                    .as_mut()
                    .scalar_types
                    .get_mut(output_type_reference)
                else {
                    return;
                };
                scalar_type_referencers.object_fields.remove(self);
            }
            Some(ExtendedType::Object(_)) => {
                let Some(object_type_referencers) = referencers
                    .as_mut()
                    .object_types
                    .get_mut(output_type_reference)
                else {
                    return;
                };
                object_type_referencers.object_fields.remove(self);
            }
            Some(ExtendedType::Interface(_)) => {
                let Some(interface_type_referencers) = referencers
                    .as_mut()
                    .interface_types
                    .get_mut(output_type_reference)
                else {
                    return;
                };
                interface_type_referencers.object_fields.remove(self);
            }
            Some(ExtendedType::Union(_)) => {
                let Some(union_type_referencers) = referencers
                    .as_mut()
                    .union_types
                    .get_mut(output_type_reference)
                else {
                    return;
                };
                union_type_referencers.object_fields.remove(self);
            }
            Some(ExtendedType::Enum(_)) => {
                let Some(enum_type_referencers) = referencers
                    .as_mut()
                    .enum_types
                    .get_mut(output_type_reference)
                else {
                    return;
                };
                enum_type_referencers.object_fields.remove(self);
            }
            _ => {
                panic!(
                    "Object field \"{}\"'s inner type \"{}\" does not refer to an existing output type.",
                    self,
                    output_type_reference.deref(),
                )
            }
        }
    }
}

impl ObjectFieldArgumentDefinitionLocation {
    pub fn remove<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
    ) {
        let Some(argument) = self.try_get(schema.schema) else {
            return;
        };
        self.remove_references(argument, schema.schema, schema.referencers.as_mut());
        self.parent()
            .make_mut(schema.schema)
            .make_mut()
            .arguments
            .retain(|other_argument| other_argument.name != self.argument_name);
    }

    pub fn remove_directive_name<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
        Q: Hash + Equivalent<Name>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
        name: &Q,
    ) {
        self.remove_directive_name_references(schema.referencers.as_mut(), name);
        let Some(argument) = self.try_make_mut(schema.schema) else {
            return;
        };
        argument
            .make_mut()
            .directives
            .retain(|other_directive| !name.equivalent(&other_directive.name));
    }

    pub fn remove_directive<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
        directive: &Component<Directive>,
    ) {
        let Some(argument) = self.try_make_mut(schema.schema) else {
            return;
        };
        if !argument.directives.iter().any(|other_directive| {
            (other_directive.name == directive.name) && !other_directive.ptr_eq(directive)
        }) {
            self.remove_directive_name_references(schema.referencers.as_mut(), &directive.name);
        }
        argument
            .make_mut()
            .directives
            .retain(|other_directive| !other_directive.ptr_eq(directive));
    }

    fn remove_references(
        &self,
        argument: &Node<InputValueDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) {
        for directive_reference in argument.directives.iter() {
            self.remove_directive_name_references(referencers, &directive_reference.name)
        }
        self.remove_type_references(argument, schema, referencers);
    }

    fn remove_directive_name_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(directive_referencers) = referencers.as_mut().directives.get_mut(name) else {
            return;
        };
        directive_referencers.object_field_arguments.remove(self);
    }

    fn remove_type_references(
        &self,
        argument: &Node<InputValueDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) {
        let input_type_reference = argument.ty.inner_named_type();
        match schema.types.get(input_type_reference) {
            Some(ExtendedType::Scalar(_)) => {
                let Some(scalar_type_referencers) = referencers
                    .as_mut()
                    .scalar_types
                    .get_mut(input_type_reference)
                else {
                    return;
                };
                scalar_type_referencers.object_field_arguments.remove(self);
            }
            Some(ExtendedType::Enum(_)) => {
                let Some(enum_type_referencers) = referencers
                    .as_mut()
                    .enum_types
                    .get_mut(input_type_reference)
                else {
                    return;
                };
                enum_type_referencers.object_field_arguments.remove(self);
            }
            Some(ExtendedType::InputObject(_)) => {
                let Some(input_object_type_referencers) = referencers
                    .as_mut()
                    .input_object_types
                    .get_mut(input_type_reference)
                else {
                    return;
                };
                input_object_type_referencers
                    .object_field_arguments
                    .remove(self);
            }
            _ => {
                panic!(
                    "Object field argument \"{}\"'s inner type \"{}\" does not refer to an existing input type.",
                    self,
                    input_type_reference.deref(),
                )
            }
        }
    }
}

impl InterfaceTypeDefinitionLocation {
    pub fn remove<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
    ) -> Option<InterfaceTypeReferencers> {
        let Some(referencers) = self.remove_internal(schema) else {
            return None;
        };
        for type_ in &referencers.object_types {
            type_.remove_implements_interface(schema, &self.type_name);
        }
        for field in &referencers.object_fields {
            field.remove(schema);
        }
        for type_ in &referencers.interface_types {
            type_.remove_implements_interface(schema, &self.type_name);
        }
        for field in &referencers.interface_fields {
            field.remove(schema);
        }
        Some(referencers)
    }

    pub fn remove_recursive<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
    ) {
        let Some(referencers) = self.remove_internal(schema) else {
            return;
        };
        for type_ in referencers.object_types {
            type_.remove_implements_interface(schema, &self.type_name);
        }
        for field in referencers.object_fields {
            field.remove_recursive(schema);
        }
        for type_ in referencers.interface_types {
            type_.remove_implements_interface(schema, &self.type_name);
        }
        for field in referencers.interface_fields {
            field.remove_recursive(schema);
        }
    }

    fn remove_internal<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
    ) -> Option<InterfaceTypeReferencers> {
        let Some(type_) = self.try_get(schema.schema) else {
            return None;
        };
        self.remove_references(type_, schema.referencers.as_mut());
        for (field_name, field) in type_.fields.iter() {
            InterfaceFieldDefinitionLocation {
                type_name: self.type_name.clone(),
                field_name: field_name.clone(),
            }
            .remove_references(field, schema.schema, schema.referencers.as_mut());
        }
        schema.schema.types.remove(&self.type_name).unwrap();
        Some(
            schema
                .referencers
                .as_mut()
                .interface_types
                .remove(&self.type_name)
                .unwrap(),
        )
    }

    pub fn remove_directive_name<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
        Q: Hash + Equivalent<Name>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
        name: &Q,
    ) {
        self.remove_directive_name_references(schema.referencers.as_mut(), name);
        let Some(type_) = self.try_make_mut(schema.schema) else {
            return;
        };
        type_
            .make_mut()
            .directives
            .retain(|other_directive| !name.equivalent(&other_directive.name));
    }

    pub fn remove_directive<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
        directive: &Component<Directive>,
    ) {
        let Some(type_) = self.try_make_mut(schema.schema) else {
            return;
        };
        if !type_.directives.iter().any(|other_directive| {
            (other_directive.name == directive.name) && !other_directive.ptr_eq(directive)
        }) {
            self.remove_directive_name_references(schema.referencers.as_mut(), &directive.name);
        }
        type_
            .make_mut()
            .directives
            .retain(|other_directive| !other_directive.ptr_eq(directive));
    }

    pub fn remove_implements_interface<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
        Q: Hash + Equivalent<Name>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
        name: &Q,
    ) {
        self.remove_implements_interface_references(schema.referencers.as_mut(), name);
        let Some(type_) = self.try_make_mut(schema.schema) else {
            return;
        };
        type_
            .make_mut()
            .implements_interfaces
            .retain(|other_type| !name.equivalent(other_type.deref()));
    }

    fn remove_references(&self, type_: &Node<InterfaceType>, referencers: &mut Referencers) {
        for directive_reference in type_.directives.iter() {
            self.remove_directive_name_references(referencers, &directive_reference.name);
        }
        for interface_type_reference in type_.implements_interfaces.iter() {
            self.remove_implements_interface_references(
                referencers,
                interface_type_reference.deref(),
            );
        }
    }

    fn remove_directive_name_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(directive_referencers) = referencers.as_mut().directives.get_mut(name) else {
            return;
        };
        directive_referencers.interface_types.remove(self);
    }

    fn remove_implements_interface_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(interface_type_referencers) = referencers.as_mut().interface_types.get_mut(name)
        else {
            return;
        };
        interface_type_referencers.interface_types.remove(self);
    }
}

impl InterfaceFieldDefinitionLocation {
    pub fn remove<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
    ) {
        let Some(field) = self.try_get(schema.schema) else {
            return;
        };
        self.remove_references(field, schema.schema, schema.referencers.as_mut());
        for argument in field.arguments.iter() {
            InterfaceFieldArgumentDefinitionLocation {
                type_name: self.type_name.clone(),
                field_name: self.field_name.clone(),
                argument_name: argument.name.clone(),
            }
            .remove_references(argument, schema.schema, schema.referencers.as_mut());
        }
        self.parent()
            .make_mut(schema.schema)
            .make_mut()
            .fields
            .remove(&self.field_name);
    }

    pub fn remove_recursive<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
    ) {
        self.remove(schema);
        let parent = self.parent();
        let Some(type_) = parent.try_get(schema.schema) else {
            return;
        };
        if type_.fields.is_empty() {
            parent.remove_recursive(schema);
        }
    }

    pub fn remove_directive_name<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
        Q: Hash + Equivalent<Name>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
        name: &Q,
    ) {
        self.remove_directive_name_references(schema.referencers.as_mut(), name);
        let Some(field) = self.try_make_mut(schema.schema) else {
            return;
        };
        field
            .make_mut()
            .directives
            .retain(|other_directive| !name.equivalent(&other_directive.name));
    }

    pub fn remove_directive<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
        directive: &Component<Directive>,
    ) {
        let Some(field) = self.try_make_mut(schema.schema) else {
            return;
        };
        if !field.directives.iter().any(|other_directive| {
            (other_directive.name == directive.name) && !other_directive.ptr_eq(directive)
        }) {
            self.remove_directive_name_references(schema.referencers.as_mut(), &directive.name);
        }
        field
            .make_mut()
            .directives
            .retain(|other_directive| !other_directive.ptr_eq(directive));
    }

    fn remove_references(
        &self,
        field: &Component<FieldDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) {
        for directive_reference in field.directives.iter() {
            self.remove_directive_name_references(referencers, &directive_reference.name)
        }
        self.remove_type_references(field, schema, referencers);
    }

    fn remove_directive_name_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(directive_referencers) = referencers.as_mut().directives.get_mut(name) else {
            return;
        };
        directive_referencers.interface_fields.remove(self);
    }

    fn remove_type_references(
        &self,
        field: &Component<FieldDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) {
        let output_type_reference = field.ty.inner_named_type();
        match schema.types.get(output_type_reference) {
            Some(ExtendedType::Scalar(_)) => {
                let Some(scalar_type_referencers) = referencers
                    .as_mut()
                    .scalar_types
                    .get_mut(output_type_reference)
                else {
                    return;
                };
                scalar_type_referencers.interface_fields.remove(self);
            }
            Some(ExtendedType::Object(_)) => {
                let Some(object_type_referencers) = referencers
                    .as_mut()
                    .object_types
                    .get_mut(output_type_reference)
                else {
                    return;
                };
                object_type_referencers.interface_fields.remove(self);
            }
            Some(ExtendedType::Interface(_)) => {
                let Some(interface_type_referencers) = referencers
                    .as_mut()
                    .interface_types
                    .get_mut(output_type_reference)
                else {
                    return;
                };
                interface_type_referencers.interface_fields.remove(self);
            }
            Some(ExtendedType::Union(_)) => {
                let Some(union_type_referencers) = referencers
                    .as_mut()
                    .union_types
                    .get_mut(output_type_reference)
                else {
                    return;
                };
                union_type_referencers.interface_fields.remove(self);
            }
            Some(ExtendedType::Enum(_)) => {
                let Some(enum_type_referencers) = referencers
                    .as_mut()
                    .enum_types
                    .get_mut(output_type_reference)
                else {
                    return;
                };
                enum_type_referencers.interface_fields.remove(self);
            }
            _ => {
                panic!(
                    "Interface field \"{}\"'s inner type \"{}\" does not refer to an existing output type.",
                    self,
                    output_type_reference.deref(),
                )
            }
        }
    }
}

impl InterfaceFieldArgumentDefinitionLocation {
    pub fn remove<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
    ) {
        let Some(argument) = self.try_get(schema.schema) else {
            return;
        };
        self.remove_references(argument, schema.schema, schema.referencers.as_mut());
        self.parent()
            .make_mut(schema.schema)
            .make_mut()
            .arguments
            .retain(|other_argument| other_argument.name != self.argument_name);
    }

    pub fn remove_directive_name<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
        Q: Hash + Equivalent<Name>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
        name: &Q,
    ) {
        self.remove_directive_name_references(schema.referencers.as_mut(), name);
        let Some(argument) = self.try_make_mut(schema.schema) else {
            return;
        };
        argument
            .make_mut()
            .directives
            .retain(|other_directive| !name.equivalent(&other_directive.name));
    }

    pub fn remove_directive<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
        directive: &Component<Directive>,
    ) {
        let Some(argument) = self.try_make_mut(schema.schema) else {
            return;
        };
        if !argument.directives.iter().any(|other_directive| {
            (other_directive.name == directive.name) && !other_directive.ptr_eq(directive)
        }) {
            self.remove_directive_name_references(schema.referencers.as_mut(), &directive.name);
        }
        argument
            .make_mut()
            .directives
            .retain(|other_directive| !other_directive.ptr_eq(directive));
    }

    fn remove_references(
        &self,
        argument: &Node<InputValueDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) {
        for directive_reference in argument.directives.iter() {
            self.remove_directive_name_references(referencers, &directive_reference.name)
        }
        self.remove_type_references(argument, schema, referencers);
    }

    fn remove_directive_name_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(directive_referencers) = referencers.as_mut().directives.get_mut(name) else {
            return;
        };
        directive_referencers.interface_field_arguments.remove(self);
    }

    fn remove_type_references(
        &self,
        argument: &Node<InputValueDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) {
        let input_type_reference = argument.ty.inner_named_type();
        match schema.types.get(input_type_reference) {
            Some(ExtendedType::Scalar(_)) => {
                let Some(scalar_type_referencers) = referencers
                    .as_mut()
                    .scalar_types
                    .get_mut(input_type_reference)
                else {
                    return;
                };
                scalar_type_referencers
                    .interface_field_arguments
                    .remove(self);
            }
            Some(ExtendedType::Enum(_)) => {
                let Some(enum_type_referencers) = referencers
                    .as_mut()
                    .enum_types
                    .get_mut(input_type_reference)
                else {
                    return;
                };
                enum_type_referencers.interface_field_arguments.remove(self);
            }
            Some(ExtendedType::InputObject(_)) => {
                let Some(input_object_type_referencers) = referencers
                    .as_mut()
                    .input_object_types
                    .get_mut(input_type_reference)
                else {
                    return;
                };
                input_object_type_referencers
                    .interface_field_arguments
                    .remove(self);
            }
            _ => {
                panic!(
                    "Interface field argument \"{}\"'s inner type \"{}\" does not refer to an existing input type.",
                    self,
                    input_type_reference.deref(),
                )
            }
        }
    }
}

impl UnionTypeDefinitionLocation {
    pub fn remove<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
    ) -> Option<UnionTypeReferencers> {
        let Some(referencers) = self.remove_internal(schema) else {
            return None;
        };
        for field in &referencers.object_fields {
            field.remove(schema);
        }
        for field in &referencers.interface_fields {
            field.remove(schema);
        }
        Some(referencers)
    }

    pub fn remove_recursive<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
    ) {
        let Some(referencers) = self.remove_internal(schema) else {
            return;
        };
        for field in referencers.object_fields {
            field.remove_recursive(schema);
        }
        for field in referencers.interface_fields {
            field.remove_recursive(schema);
        }
    }

    fn remove_internal<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
    ) -> Option<UnionTypeReferencers> {
        let Some(type_) = self.try_get(schema.schema) else {
            return None;
        };
        self.remove_references(type_, schema.referencers.as_mut());
        schema.schema.types.remove(&self.type_name).unwrap();
        Some(
            schema
                .referencers
                .as_mut()
                .union_types
                .remove(&self.type_name)
                .unwrap(),
        )
    }

    pub fn remove_directive_name<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
        Q: Hash + Equivalent<Name>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
        name: &Q,
    ) {
        self.remove_directive_name_references(schema.referencers.as_mut(), name);
        let Some(type_) = self.try_make_mut(schema.schema) else {
            return;
        };
        type_
            .make_mut()
            .directives
            .retain(|other_directive| !name.equivalent(&other_directive.name));
    }

    pub fn remove_directive<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
        directive: &Component<Directive>,
    ) {
        let Some(type_) = self.try_make_mut(schema.schema) else {
            return;
        };
        if !type_.directives.iter().any(|other_directive| {
            (other_directive.name == directive.name) && !other_directive.ptr_eq(directive)
        }) {
            self.remove_directive_name_references(schema.referencers.as_mut(), &directive.name);
        }
        type_
            .make_mut()
            .directives
            .retain(|other_directive| !other_directive.ptr_eq(directive));
    }

    pub fn remove_member<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
        Q: Hash + Equivalent<Name>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
        name: &Q,
    ) {
        self.remove_member_references(schema.referencers.as_mut(), name);
        let Some(type_) = self.try_make_mut(schema.schema) else {
            return;
        };
        type_
            .make_mut()
            .members
            .retain(|other_type| !name.equivalent(other_type.deref()));
    }

    pub fn remove_member_recursive<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
        Q: Hash + Equivalent<Name>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
        name: &Q,
    ) {
        self.remove_member(schema, name);
        let Some(type_) = self.try_get(schema.schema) else {
            return;
        };
        if type_.members.is_empty() {
            self.remove_recursive(schema);
        }
    }

    fn remove_references(&self, type_: &Node<UnionType>, referencers: &mut Referencers) {
        for directive_reference in type_.directives.iter() {
            self.remove_directive_name_references(referencers, &directive_reference.name);
        }
        for object_type_reference in type_.members.iter() {
            self.remove_member_references(referencers, object_type_reference.deref());
        }
    }

    fn remove_directive_name_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(directive_referencers) = referencers.as_mut().directives.get_mut(name) else {
            return;
        };
        directive_referencers.union_types.remove(self);
    }

    fn remove_member_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(object_type_referencers) = referencers.as_mut().object_types.get_mut(name) else {
            return;
        };
        object_type_referencers.union_types.remove(self);
    }
}

impl EnumTypeDefinitionLocation {
    pub fn remove<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
    ) -> Option<EnumTypeReferencers> {
        let Some(referencers) = self.remove_internal(schema) else {
            return None;
        };
        for field in &referencers.object_fields {
            field.remove(schema);
        }
        for argument in &referencers.object_field_arguments {
            argument.remove(schema);
        }
        for field in &referencers.interface_fields {
            field.remove(schema);
        }
        for argument in &referencers.interface_field_arguments {
            argument.remove(schema);
        }
        for field in &referencers.input_object_fields {
            field.remove(schema);
        }
        for argument in &referencers.directive_arguments {
            argument.remove(schema);
        }
        Some(referencers)
    }

    pub fn remove_recursive<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
    ) {
        let Some(referencers) = self.remove_internal(schema) else {
            return;
        };
        for field in referencers.object_fields {
            field.remove_recursive(schema);
        }
        for argument in referencers.object_field_arguments {
            argument.remove(schema);
        }
        for field in referencers.interface_fields {
            field.remove_recursive(schema);
        }
        for argument in referencers.interface_field_arguments {
            argument.remove(schema);
        }
        for field in referencers.input_object_fields {
            field.remove_recursive(schema);
        }
        for argument in referencers.directive_arguments {
            argument.remove(schema);
        }
    }

    fn remove_internal<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
    ) -> Option<EnumTypeReferencers> {
        let Some(type_) = self.try_get(schema.schema) else {
            return None;
        };
        self.remove_references(type_, schema.referencers.as_mut());
        for (value_name, value) in type_.values.iter() {
            EnumValueDefinitionLocation {
                type_name: self.type_name.clone(),
                value_name: value_name.clone(),
            }
            .remove_references(value, schema.referencers.as_mut());
        }
        schema.schema.types.remove(&self.type_name).unwrap();
        Some(
            schema
                .referencers
                .as_mut()
                .enum_types
                .remove(&self.type_name)
                .unwrap(),
        )
    }

    pub fn remove_directive_name<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
        Q: Hash + Equivalent<Name>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
        name: &Q,
    ) {
        self.remove_directive_name_references(schema.referencers.as_mut(), name);
        let Some(type_) = self.try_make_mut(schema.schema) else {
            return;
        };
        type_
            .make_mut()
            .directives
            .retain(|other_directive| !name.equivalent(&other_directive.name));
    }

    pub fn remove_directive<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
        directive: &Component<Directive>,
    ) {
        let Some(type_) = self.try_make_mut(schema.schema) else {
            return;
        };
        if !type_.directives.iter().any(|other_directive| {
            (other_directive.name == directive.name) && !other_directive.ptr_eq(directive)
        }) {
            self.remove_directive_name_references(schema.referencers.as_mut(), &directive.name);
        }
        type_
            .make_mut()
            .directives
            .retain(|other_directive| !other_directive.ptr_eq(directive));
    }

    fn remove_references(&self, type_: &Node<EnumType>, referencers: &mut Referencers) {
        for directive_reference in type_.directives.iter() {
            self.remove_directive_name_references(referencers, &directive_reference.name);
        }
    }

    fn remove_directive_name_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(directive_referencers) = referencers.as_mut().directives.get_mut(name) else {
            return;
        };
        directive_referencers.enum_types.remove(self);
    }
}

impl EnumValueDefinitionLocation {
    pub fn remove<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
    ) {
        let Some(value) = self.try_get(schema.schema) else {
            return;
        };
        self.remove_references(value, schema.referencers.as_mut());
        self.parent()
            .make_mut(schema.schema)
            .make_mut()
            .values
            .remove(&self.value_name);
    }

    pub fn remove_recursive<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
    ) {
        self.remove(schema);
        let parent = self.parent();
        let Some(type_) = parent.try_get(schema.schema) else {
            return;
        };
        if type_.values.is_empty() {
            parent.remove_recursive(schema);
        }
    }

    pub fn remove_directive_name<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
        Q: Hash + Equivalent<Name>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
        name: &Q,
    ) {
        self.remove_directive_name_references(schema.referencers.as_mut(), name);
        let Some(value) = self.try_make_mut(schema.schema) else {
            return;
        };
        value
            .make_mut()
            .directives
            .retain(|other_directive| !name.equivalent(&other_directive.name));
    }

    pub fn remove_directive<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
        directive: &Component<Directive>,
    ) {
        let Some(value) = self.try_make_mut(schema.schema) else {
            return;
        };
        if !value.directives.iter().any(|other_directive| {
            (other_directive.name == directive.name) && !other_directive.ptr_eq(directive)
        }) {
            self.remove_directive_name_references(schema.referencers.as_mut(), &directive.name);
        }
        value
            .make_mut()
            .directives
            .retain(|other_directive| !other_directive.ptr_eq(directive));
    }

    fn remove_references(
        &self,
        value: &Component<EnumValueDefinition>,
        referencers: &mut Referencers,
    ) {
        for directive_reference in value.directives.iter() {
            self.remove_directive_name_references(referencers, &directive_reference.name);
        }
    }

    fn remove_directive_name_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(directive_referencers) = referencers.as_mut().directives.get_mut(name) else {
            return;
        };
        directive_referencers.enum_values.remove(self);
    }
}

impl InputObjectTypeDefinitionLocation {
    pub fn remove<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
    ) -> Option<InputObjectTypeReferencers> {
        let Some(referencers) = self.remove_internal(schema) else {
            return None;
        };
        for argument in &referencers.object_field_arguments {
            argument.remove(schema);
        }
        for argument in &referencers.interface_field_arguments {
            argument.remove(schema);
        }
        for field in &referencers.input_object_fields {
            field.remove(schema);
        }
        for argument in &referencers.directive_arguments {
            argument.remove(schema);
        }
        Some(referencers)
    }

    pub fn remove_recursive<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
    ) {
        let Some(referencers) = self.remove_internal(schema) else {
            return;
        };
        for argument in referencers.object_field_arguments {
            argument.remove(schema);
        }
        for argument in referencers.interface_field_arguments {
            argument.remove(schema);
        }
        for field in referencers.input_object_fields {
            field.remove_recursive(schema);
        }
        for argument in referencers.directive_arguments {
            argument.remove(schema);
        }
    }

    fn remove_internal<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
    ) -> Option<InputObjectTypeReferencers> {
        let Some(type_) = self.try_get(schema.schema) else {
            return None;
        };
        self.remove_references(type_, schema.referencers.as_mut());
        for (field_name, field) in type_.fields.iter() {
            InputObjectFieldDefinitionLocation {
                type_name: self.type_name.clone(),
                field_name: field_name.clone(),
            }
            .remove_references(field, schema.schema, schema.referencers.as_mut());
        }
        schema.schema.types.remove(&self.type_name).unwrap();
        Some(
            schema
                .referencers
                .as_mut()
                .input_object_types
                .remove(&self.type_name)
                .unwrap(),
        )
    }

    pub fn remove_directive_name<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
        Q: Hash + Equivalent<Name>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
        name: &Q,
    ) {
        self.remove_directive_name_references(schema.referencers.as_mut(), name);
        let Some(type_) = self.try_make_mut(schema.schema) else {
            return;
        };
        type_
            .make_mut()
            .directives
            .retain(|other_directive| !name.equivalent(&other_directive.name));
    }

    pub fn remove_directive<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
        directive: &Component<Directive>,
    ) {
        let Some(type_) = self.try_make_mut(schema.schema) else {
            return;
        };
        if !type_.directives.iter().any(|other_directive| {
            (other_directive.name == directive.name) && !other_directive.ptr_eq(directive)
        }) {
            self.remove_directive_name_references(schema.referencers.as_mut(), &directive.name);
        }
        type_
            .make_mut()
            .directives
            .retain(|other_directive| !other_directive.ptr_eq(directive));
    }

    fn remove_references(&self, type_: &Node<InputObjectType>, referencers: &mut Referencers) {
        for directive_reference in type_.directives.iter() {
            self.remove_directive_name_references(referencers, &directive_reference.name);
        }
    }

    fn remove_directive_name_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(directive_referencers) = referencers.as_mut().directives.get_mut(name) else {
            return;
        };
        directive_referencers.input_object_types.remove(self);
    }
}

impl InputObjectFieldDefinitionLocation {
    pub fn remove<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
    ) {
        let Some(field) = self.try_get(schema.schema) else {
            return;
        };
        self.remove_references(field, schema.schema, schema.referencers.as_mut());
        self.parent()
            .make_mut(schema.schema)
            .make_mut()
            .fields
            .remove(&self.field_name);
    }

    pub fn remove_recursive<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
    ) {
        self.remove(schema);
        let parent = self.parent();
        let Some(type_) = parent.try_get(schema.schema) else {
            return;
        };
        if type_.fields.is_empty() {
            parent.remove_recursive(schema);
        }
    }

    pub fn remove_directive_name<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
        Q: Hash + Equivalent<Name>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
        name: &Q,
    ) {
        self.remove_directive_name_references(schema.referencers.as_mut(), name);
        let Some(field) = self.try_make_mut(schema.schema) else {
            return;
        };
        field
            .make_mut()
            .directives
            .retain(|other_directive| !name.equivalent(&other_directive.name));
    }

    pub fn remove_directive<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
        directive: &Component<Directive>,
    ) {
        let Some(field) = self.try_make_mut(schema.schema) else {
            return;
        };
        if !field.directives.iter().any(|other_directive| {
            (other_directive.name == directive.name) && !other_directive.ptr_eq(directive)
        }) {
            self.remove_directive_name_references(schema.referencers.as_mut(), &directive.name);
        }
        field
            .make_mut()
            .directives
            .retain(|other_directive| !other_directive.ptr_eq(directive));
    }

    fn remove_references(
        &self,
        field: &Component<InputValueDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) {
        for directive_reference in field.directives.iter() {
            self.remove_directive_name_references(referencers, &directive_reference.name)
        }
        self.remove_type_references(field, schema, referencers);
    }

    fn remove_directive_name_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(directive_referencers) = referencers.as_mut().directives.get_mut(name) else {
            return;
        };
        directive_referencers.input_object_fields.remove(self);
    }

    fn remove_type_references(
        &self,
        field: &Component<InputValueDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) {
        let input_type_reference = field.ty.inner_named_type();
        match schema.types.get(input_type_reference) {
            Some(ExtendedType::Scalar(_)) => {
                let Some(scalar_type_referencers) = referencers
                    .as_mut()
                    .scalar_types
                    .get_mut(input_type_reference)
                else {
                    return;
                };
                scalar_type_referencers.input_object_fields.remove(self);
            }
            Some(ExtendedType::Enum(_)) => {
                let Some(enum_type_referencers) = referencers
                    .as_mut()
                    .enum_types
                    .get_mut(input_type_reference)
                else {
                    return;
                };
                enum_type_referencers.input_object_fields.remove(self);
            }
            Some(ExtendedType::InputObject(_)) => {
                let Some(input_object_type_referencers) = referencers
                    .as_mut()
                    .input_object_types
                    .get_mut(input_type_reference)
                else {
                    return;
                };
                input_object_type_referencers
                    .input_object_fields
                    .remove(self);
            }
            _ => {
                panic!(
                    "Input object field \"{}\"'s inner type \"{}\" does not refer to an existing input type.",
                    self,
                    input_type_reference.deref(),
                )
            }
        }
    }
}

impl DirectiveDefinitionLocation {
    pub fn remove<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
    ) -> Option<DirectiveReferencers> {
        let Some(referencers) = self.remove_internal(schema) else {
            return None;
        };
        if let Some(schema_definition) = &referencers.schema {
            if let Some(directive_reference_parent) = schema_definition.try_make_mut(schema.schema)
            {
                directive_reference_parent
                    .make_mut()
                    .directives
                    .retain(|other_directive| other_directive.name != self.directive_name);
            }
        }
        for type_ in &referencers.scalar_types {
            type_.remove_directive_name(schema, &self.directive_name);
        }
        for type_ in &referencers.object_types {
            type_.remove_directive_name(schema, &self.directive_name);
        }
        for field in &referencers.object_fields {
            field.remove_directive_name(schema, &self.directive_name);
        }
        for argument in &referencers.object_field_arguments {
            argument.remove_directive_name(schema, &self.directive_name);
        }
        for type_ in &referencers.interface_types {
            type_.remove_directive_name(schema, &self.directive_name);
        }
        for field in &referencers.interface_fields {
            field.remove_directive_name(schema, &self.directive_name);
        }
        for argument in &referencers.interface_field_arguments {
            argument.remove_directive_name(schema, &self.directive_name);
        }
        for type_ in &referencers.union_types {
            type_.remove_directive_name(schema, &self.directive_name);
        }
        for type_ in &referencers.enum_types {
            type_.remove_directive_name(schema, &self.directive_name);
        }
        for value in &referencers.enum_values {
            value.remove_directive_name(schema, &self.directive_name);
        }
        for type_ in &referencers.input_object_types {
            type_.remove_directive_name(schema, &self.directive_name);
        }
        for field in &referencers.input_object_fields {
            field.remove_directive_name(schema, &self.directive_name);
        }
        for argument in &referencers.directive_arguments {
            argument.remove_directive_name(schema, &self.directive_name);
        }
        Some(referencers)
    }

    fn remove_internal<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
    ) -> Option<DirectiveReferencers> {
        let Some(directive) = self.try_get(schema.schema) else {
            return None;
        };
        for argument in directive.arguments.iter() {
            DirectiveArgumentDefinitionLocation {
                directive_name: self.directive_name.clone(),
                argument_name: argument.name.clone(),
            }
            .remove_references(argument, schema.schema, schema.referencers.as_mut());
        }
        schema
            .schema
            .directive_definitions
            .remove(&self.directive_name)
            .unwrap();
        Some(
            schema
                .referencers
                .as_mut()
                .directives
                .remove(&self.directive_name)
                .unwrap(),
        )
    }
}

impl DirectiveArgumentDefinitionLocation {
    pub fn remove<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
    ) {
        let Some(argument) = self.try_get(schema.schema) else {
            return;
        };
        self.remove_references(argument, schema.schema, schema.referencers.as_mut());
        self.parent()
            .make_mut(schema.schema)
            .make_mut()
            .arguments
            .retain(|other_argument| other_argument.name != self.argument_name);
    }

    pub fn remove_directive_name<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
        Q: Hash + Equivalent<Name>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
        name: &Q,
    ) {
        self.remove_directive_name_references(schema.referencers.as_mut(), name);
        let Some(argument) = self.try_make_mut(schema.schema) else {
            return;
        };
        argument
            .make_mut()
            .directives
            .retain(|other_directive| !name.equivalent(&other_directive.name));
    }

    pub fn remove_directive<
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    >(
        &self,
        schema: &mut ReferencerFederationSchemaMut<T, U>,
        directive: &Component<Directive>,
    ) {
        let Some(argument) = self.try_make_mut(schema.schema) else {
            return;
        };
        if !argument.directives.iter().any(|other_directive| {
            (other_directive.name == directive.name) && !other_directive.ptr_eq(directive)
        }) {
            self.remove_directive_name_references(schema.referencers.as_mut(), &directive.name);
        }
        argument
            .make_mut()
            .directives
            .retain(|other_directive| !other_directive.ptr_eq(directive));
    }

    fn remove_references(
        &self,
        argument: &Node<InputValueDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) {
        for directive_reference in argument.directives.iter() {
            self.remove_directive_name_references(referencers, &directive_reference.name)
        }
        self.remove_type_references(argument, schema, referencers);
    }

    fn remove_directive_name_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(directive_referencers) = referencers.as_mut().directives.get_mut(name) else {
            return;
        };
        directive_referencers.directive_arguments.remove(self);
    }

    fn remove_type_references(
        &self,
        argument: &Node<InputValueDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) {
        let input_type_reference = argument.ty.inner_named_type();
        match schema.types.get(input_type_reference) {
            Some(ExtendedType::Scalar(_)) => {
                let Some(scalar_type_referencers) = referencers
                    .as_mut()
                    .scalar_types
                    .get_mut(input_type_reference)
                else {
                    return;
                };
                scalar_type_referencers.directive_arguments.remove(self);
            }
            Some(ExtendedType::Enum(_)) => {
                let Some(enum_type_referencers) = referencers
                    .as_mut()
                    .enum_types
                    .get_mut(input_type_reference)
                else {
                    return;
                };
                enum_type_referencers.directive_arguments.remove(self);
            }
            Some(ExtendedType::InputObject(_)) => {
                let Some(input_object_type_referencers) = referencers
                    .as_mut()
                    .input_object_types
                    .get_mut(input_type_reference)
                else {
                    return;
                };
                input_object_type_referencers
                    .directive_arguments
                    .remove(self);
            }
            _ => {
                panic!(
                    "Directive argument \"{}\"'s inner type \"{}\" does not refer to an existing input type.",
                    self,
                    input_type_reference.deref(),
                )
            }
        }
    }
}
