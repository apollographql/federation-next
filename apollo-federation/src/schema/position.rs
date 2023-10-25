use crate::error::{FederationError, SingleFederationError};
use crate::link::database::links_metadata;
use crate::link::spec_definition::SpecDefinition;
use crate::schema::referencer::{
    DirectiveReferencers, EnumTypeReferencers, InputObjectTypeReferencers,
    InterfaceTypeReferencers, ObjectTypeReferencers, Referencers, ScalarTypeReferencers,
    UnionTypeReferencers,
};
use crate::schema::FederationSchema;
use apollo_compiler::schema::{
    Component, ComponentStr, Directive, DirectiveDefinition, EnumType, EnumValueDefinition,
    ExtendedType, FieldDefinition, InputObjectType, InputValueDefinition, InterfaceType, Name,
    ObjectType, ScalarType, SchemaDefinition, UnionType,
};
use apollo_compiler::{Node, Schema};
use indexmap::{Equivalent, IndexSet};
use lazy_static::lazy_static;
use std::fmt::{Display, Formatter};
use std::hash::Hash;
use std::ops::Deref;
use strum::IntoEnumIterator;

pub(crate) enum TypeDefinitionPosition {
    Scalar(ScalarTypeDefinitionPosition),
    Object(ObjectTypeDefinitionPosition),
    Interface(InterfaceTypeDefinitionPosition),
    Union(UnionTypeDefinitionPosition),
    Enum(EnumTypeDefinitionPosition),
    InputObject(InputObjectTypeDefinitionPosition),
}

impl From<ScalarTypeDefinitionPosition> for TypeDefinitionPosition {
    fn from(value: ScalarTypeDefinitionPosition) -> Self {
        TypeDefinitionPosition::Scalar(value)
    }
}

impl From<ObjectTypeDefinitionPosition> for TypeDefinitionPosition {
    fn from(value: ObjectTypeDefinitionPosition) -> Self {
        TypeDefinitionPosition::Object(value)
    }
}

impl From<InterfaceTypeDefinitionPosition> for TypeDefinitionPosition {
    fn from(value: InterfaceTypeDefinitionPosition) -> Self {
        TypeDefinitionPosition::Interface(value)
    }
}

impl From<UnionTypeDefinitionPosition> for TypeDefinitionPosition {
    fn from(value: UnionTypeDefinitionPosition) -> Self {
        TypeDefinitionPosition::Union(value)
    }
}

impl From<EnumTypeDefinitionPosition> for TypeDefinitionPosition {
    fn from(value: EnumTypeDefinitionPosition) -> Self {
        TypeDefinitionPosition::Enum(value)
    }
}

impl From<InputObjectTypeDefinitionPosition> for TypeDefinitionPosition {
    fn from(value: InputObjectTypeDefinitionPosition) -> Self {
        TypeDefinitionPosition::InputObject(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct SchemaDefinitionPosition;

impl SchemaDefinitionPosition {
    pub(crate) fn get<'schema>(&self, schema: &'schema Schema) -> &'schema Node<SchemaDefinition> {
        &schema.schema_definition
    }

    fn make_mut<'schema>(
        &self,
        schema: &'schema mut Schema,
    ) -> &'schema mut Node<SchemaDefinition> {
        &mut schema.schema_definition
    }

    pub(crate) fn insert_directive(
        &self,
        schema: &mut FederationSchema,
        directive: Component<Directive>,
    ) -> Result<(), FederationError> {
        let schema_definition = self.make_mut(&mut schema.schema);
        if schema_definition
            .directives
            .iter()
            .any(|other_directive| other_directive.ptr_eq(&directive))
        {
            return Err(SingleFederationError::Internal {
                message: format!(
                    "Directive application \"@{}\" already exists on schema definition",
                    directive.name,
                ),
            }
            .into());
        }
        self.insert_directive_name_references(&mut schema.referencers, &directive.name)?;
        schema_definition.make_mut().directives.push(directive);
        schema.metadata = links_metadata(&schema.schema)?;
        Ok(())
    }

    pub(crate) fn remove_directive_name<Q: Hash + Equivalent<Name>>(
        &self,
        schema: &mut FederationSchema,
        name: &Q,
    ) -> Result<(), FederationError> {
        let is_link = Self::is_link(schema, name)?;
        self.remove_directive_name_references(&mut schema.referencers, name);
        self.make_mut(&mut schema.schema)
            .make_mut()
            .directives
            .retain(|other_directive| !name.equivalent(&other_directive.name));
        if is_link {
            schema.metadata = links_metadata(&schema.schema)?;
        }
        Ok(())
    }

    pub(crate) fn remove_directive(
        &self,
        schema: &mut FederationSchema,
        directive: &Component<Directive>,
    ) -> Result<(), FederationError> {
        let is_link = Self::is_link(schema, &directive.name)?;
        let schema_definition = self.make_mut(&mut schema.schema);
        if !schema_definition.directives.iter().any(|other_directive| {
            (other_directive.name == directive.name) && !other_directive.ptr_eq(directive)
        }) {
            self.remove_directive_name_references(&mut schema.referencers, &directive.name);
        }
        schema_definition
            .make_mut()
            .directives
            .retain(|other_directive| !other_directive.ptr_eq(directive));
        if is_link {
            schema.metadata = links_metadata(&schema.schema)?;
        }
        Ok(())
    }

    fn insert_references(
        &self,
        schema_definition: &Node<SchemaDefinition>,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        validate_component_directives(schema_definition.directives.deref())?;
        for directive_reference in schema_definition.directives.iter() {
            self.insert_directive_name_references(referencers, &directive_reference.name)?;
        }
        for root_kind in SchemaRootDefinitionKind::iter() {
            let child = SchemaRootDefinitionPosition {
                root_kind: root_kind.clone(),
            };
            match root_kind {
                SchemaRootDefinitionKind::Query => {
                    if let Some(root_type) = &schema_definition.query {
                        child.insert_references(root_type, referencers)?;
                    }
                }
                SchemaRootDefinitionKind::Mutation => {
                    if let Some(root_type) = &schema_definition.mutation {
                        child.insert_references(root_type, referencers)?;
                    }
                }
                SchemaRootDefinitionKind::Subscription => {
                    if let Some(root_type) = &schema_definition.subscription {
                        child.insert_references(root_type, referencers)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn insert_directive_name_references(
        &self,
        referencers: &mut Referencers,
        name: &Name,
    ) -> Result<(), FederationError> {
        let directive_referencers = referencers.directives.get_mut(name).ok_or_else(|| {
            SingleFederationError::Internal {
                message: format!(
                    "Schema definition's directive application \"@{}\" does not refer to an existing directive.",
                    name,
                ),
            }
        })?;
        directive_referencers.schema = Some(SchemaDefinitionPosition);
        Ok(())
    }

    fn remove_directive_name_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(directive_referencers) = referencers.directives.get_mut(name) else {
            return;
        };
        directive_referencers.schema = None;
    }

    fn is_link<Q: Hash + Equivalent<Name>>(
        schema: &FederationSchema,
        name: &Q,
    ) -> Result<bool, FederationError> {
        Ok(match &schema.metadata {
            Some(metadata) => {
                let link_spec_definition = metadata.link_spec_definition()?;
                let link_name_in_schema = link_spec_definition
                    .directive_name_in_schema(schema, &link_spec_definition.identity().name)?
                    .ok_or_else(|| SingleFederationError::Internal {
                        message: "Unexpectedly could not find core/link spec usage".to_owned(),
                    })?;
                name.equivalent(&Name::new(&link_name_in_schema))
            }
            None => false,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, strum_macros::Display, strum_macros::EnumIter)]
pub(crate) enum SchemaRootDefinitionKind {
    #[strum(to_string = "query")]
    Query,
    #[strum(to_string = "mutation")]
    Mutation,
    #[strum(to_string = "subscription")]
    Subscription,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct SchemaRootDefinitionPosition {
    pub(crate) root_kind: SchemaRootDefinitionKind,
}

impl SchemaRootDefinitionPosition {
    pub(crate) fn parent(&self) -> SchemaDefinitionPosition {
        SchemaDefinitionPosition
    }

    pub(crate) fn get<'schema>(
        &self,
        schema: &'schema Schema,
    ) -> Result<&'schema ComponentStr, FederationError> {
        let schema_definition = self.parent().get(schema);

        match self.root_kind {
            SchemaRootDefinitionKind::Query => schema_definition.query.as_ref().ok_or_else(|| {
                SingleFederationError::Internal {
                    message: format!("Schema definition has no root {} type", self),
                }
                .into()
            }),
            SchemaRootDefinitionKind::Mutation => {
                schema_definition.mutation.as_ref().ok_or_else(|| {
                    SingleFederationError::Internal {
                        message: format!("Schema definition has no root {} type", self),
                    }
                    .into()
                })
            }
            SchemaRootDefinitionKind::Subscription => {
                schema_definition.subscription.as_ref().ok_or_else(|| {
                    SingleFederationError::Internal {
                        message: format!("Schema definition has no root {} type", self),
                    }
                    .into()
                })
            }
        }
    }

    pub(crate) fn try_get<'schema>(
        &self,
        schema: &'schema Schema,
    ) -> Option<&'schema ComponentStr> {
        self.get(schema).ok()
    }

    fn make_mut<'schema>(
        &self,
        schema: &'schema mut Schema,
    ) -> Result<&'schema mut ComponentStr, FederationError> {
        let schema_definition = self.parent().make_mut(schema).make_mut();

        match self.root_kind {
            SchemaRootDefinitionKind::Query => schema_definition.query.as_mut().ok_or_else(|| {
                SingleFederationError::Internal {
                    message: format!("Schema definition has no root {} type", self),
                }
                .into()
            }),
            SchemaRootDefinitionKind::Mutation => {
                schema_definition.mutation.as_mut().ok_or_else(|| {
                    SingleFederationError::Internal {
                        message: format!("Schema definition has no root {} type", self),
                    }
                    .into()
                })
            }
            SchemaRootDefinitionKind::Subscription => {
                schema_definition.subscription.as_mut().ok_or_else(|| {
                    SingleFederationError::Internal {
                        message: format!("Schema definition has no root {} type", self),
                    }
                    .into()
                })
            }
        }
    }

    fn try_make_mut<'schema>(
        &self,
        schema: &'schema mut Schema,
    ) -> Option<&'schema mut ComponentStr> {
        if self.try_get(schema).is_some() {
            self.make_mut(schema).ok()
        } else {
            None
        }
    }

    pub(crate) fn insert(
        &self,
        schema: &mut FederationSchema,
        root_type: ComponentStr,
    ) -> Result<(), FederationError> {
        if self.try_get(&schema.schema).is_some() {
            return Err(SingleFederationError::Internal {
                message: format!("Root {} already exists on schema definition", self),
            }
            .into());
        }
        self.insert_references(&root_type, &mut schema.referencers)?;
        let parent = self.parent().make_mut(&mut schema.schema).make_mut();
        match self.root_kind {
            SchemaRootDefinitionKind::Query => {
                parent.query = Some(root_type);
            }
            SchemaRootDefinitionKind::Mutation => {
                parent.mutation = Some(root_type);
            }
            SchemaRootDefinitionKind::Subscription => {
                parent.subscription = Some(root_type);
            }
        }
        Ok(())
    }

    pub(crate) fn remove(&self, schema: &mut FederationSchema) {
        let Some(root_type) = self.try_get(&schema.schema) else {
            return;
        };
        self.remove_references(root_type, &mut schema.referencers);
        let parent = self.parent().make_mut(&mut schema.schema).make_mut();
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

    fn insert_references(
        &self,
        root_type: &ComponentStr,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        let object_type_referencers = referencers
            .object_types
            .get_mut(root_type.deref())
            .ok_or_else(|| SingleFederationError::Internal {
                message: format!(
                    "Root {} type \"{}\" does not refer to an existing object type.",
                    self,
                    root_type.deref()
                ),
            })?;
        object_type_referencers.schema_roots.insert(self.clone());
        Ok(())
    }

    fn remove_references(&self, root_type: &ComponentStr, referencers: &mut Referencers) {
        let Some(object_type_referencers) = referencers.object_types.get_mut(root_type.deref())
        else {
            return;
        };
        object_type_referencers.schema_roots.remove(self);
    }
}

impl Display for SchemaRootDefinitionPosition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.root_kind)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ScalarTypeDefinitionPosition {
    pub(crate) type_name: Name,
}

impl ScalarTypeDefinitionPosition {
    pub(crate) fn get<'schema>(
        &self,
        schema: &'schema Schema,
    ) -> Result<&'schema Node<ScalarType>, FederationError> {
        schema
            .types
            .get(&self.type_name)
            .ok_or_else(|| {
                SingleFederationError::Internal {
                    message: format!("Schema has no type \"{}\"", self),
                }
                .into()
            })
            .and_then(|type_| {
                if let ExtendedType::Scalar(type_) = type_ {
                    Ok(type_)
                } else {
                    Err(SingleFederationError::Internal {
                        message: format!("Schema type \"{}\" was not a scalar", self),
                    }
                    .into())
                }
            })
    }

    pub(crate) fn try_get<'schema>(
        &self,
        schema: &'schema Schema,
    ) -> Option<&'schema Node<ScalarType>> {
        self.get(schema).ok()
    }

    fn make_mut<'schema>(
        &self,
        schema: &'schema mut Schema,
    ) -> Result<&'schema mut Node<ScalarType>, FederationError> {
        schema
            .types
            .get_mut(&self.type_name)
            .ok_or_else(|| {
                SingleFederationError::Internal {
                    message: format!("Schema has no type \"{}\"", self),
                }
                .into()
            })
            .and_then(|type_| {
                if let ExtendedType::Scalar(type_) = type_ {
                    Ok(type_)
                } else {
                    Err(SingleFederationError::Internal {
                        message: format!("Schema type \"{}\" was not a scalar", self),
                    }
                    .into())
                }
            })
    }

    fn try_make_mut<'schema>(
        &self,
        schema: &'schema mut Schema,
    ) -> Option<&'schema mut Node<ScalarType>> {
        if self.try_get(schema).is_some() {
            self.make_mut(schema).ok()
        } else {
            None
        }
    }

    pub(crate) fn pre_insert(&self, schema: &mut FederationSchema) -> Result<(), FederationError> {
        if schema.referencers.contains_type_name(&self.type_name) {
            // TODO: Allow built-in shadowing instead of ignoring them
            if is_graphql_reserved_name(&self.type_name)
                || GRAPHQL_BUILTIN_SCALAR_NAMES.contains(self.type_name.deref())
            {
                return Ok(());
            }
            return Err(SingleFederationError::Internal {
                message: format!("Type \"{}\" has already been pre-inserted", self),
            }
            .into());
        }
        schema
            .referencers
            .scalar_types
            .insert(self.type_name.clone(), Default::default());
        Ok(())
    }

    pub(crate) fn insert(
        &self,
        schema: &mut FederationSchema,
        type_: Node<ScalarType>,
    ) -> Result<(), FederationError> {
        if !schema
            .referencers
            .scalar_types
            .contains_key(&self.type_name)
        {
            return Err(SingleFederationError::Internal {
                message: format!("Type \"{}\" has not been pre-inserted", self),
            }
            .into());
        }
        if schema.schema.types.contains_key(&self.type_name) {
            // TODO: Allow built-in shadowing instead of ignoring them
            if is_graphql_reserved_name(&self.type_name)
                || GRAPHQL_BUILTIN_SCALAR_NAMES.contains(self.type_name.deref())
            {
                return Ok(());
            }
            return Err(SingleFederationError::Internal {
                message: format!("Type \"{}\" already exists in schema", self),
            }
            .into());
        }
        self.insert_references(&type_, &mut schema.referencers)?;
        schema
            .schema
            .types
            .insert(self.type_name.clone(), ExtendedType::Scalar(type_));
        Ok(())
    }

    pub(crate) fn remove(
        &self,
        schema: &mut FederationSchema,
    ) -> Result<Option<ScalarTypeReferencers>, FederationError> {
        let Some(referencers) = self.remove_internal(schema)? else {
            return Ok(None);
        };
        for field in &referencers.object_fields {
            field.remove(schema)?;
        }
        for argument in &referencers.object_field_arguments {
            argument.remove(schema)?;
        }
        for field in &referencers.interface_fields {
            field.remove(schema)?;
        }
        for argument in &referencers.interface_field_arguments {
            argument.remove(schema)?;
        }
        for field in &referencers.input_object_fields {
            field.remove(schema)?;
        }
        for argument in &referencers.directive_arguments {
            argument.remove(schema)?;
        }
        Ok(Some(referencers))
    }

    pub(crate) fn remove_recursive(
        &self,
        schema: &mut FederationSchema,
    ) -> Result<(), FederationError> {
        let Some(referencers) = self.remove_internal(schema)? else {
            return Ok(());
        };
        for field in referencers.object_fields {
            field.remove_recursive(schema)?;
        }
        for argument in referencers.object_field_arguments {
            argument.remove(schema)?;
        }
        for field in referencers.interface_fields {
            field.remove_recursive(schema)?;
        }
        for argument in referencers.interface_field_arguments {
            argument.remove(schema)?;
        }
        for field in referencers.input_object_fields {
            field.remove_recursive(schema)?;
        }
        for argument in referencers.directive_arguments {
            argument.remove(schema)?;
        }
        Ok(())
    }

    fn remove_internal(
        &self,
        schema: &mut FederationSchema,
    ) -> Result<Option<ScalarTypeReferencers>, FederationError> {
        let Some(type_) = self.try_get(&schema.schema) else {
            return Ok(None);
        };
        self.remove_references(type_, &mut schema.referencers);
        schema.schema.types.remove(&self.type_name);
        Ok(Some(
            schema
                .referencers
                .scalar_types
                .remove(&self.type_name)
                .ok_or_else(|| SingleFederationError::Internal {
                    message: format!("Schema missing referencers for type \"{}\"", self),
                })?,
        ))
    }

    pub(crate) fn insert_directive(
        &self,
        schema: &mut FederationSchema,
        directive: Component<Directive>,
    ) -> Result<(), FederationError> {
        let type_ = self.make_mut(&mut schema.schema)?;
        if type_
            .directives
            .iter()
            .any(|other_directive| other_directive.ptr_eq(&directive))
        {
            return Err(SingleFederationError::Internal {
                message: format!(
                    "Directive application \"@{}\" already exists on scalar type \"{}\"",
                    directive.name, self,
                ),
            }
            .into());
        }
        self.insert_directive_name_references(&mut schema.referencers, &directive.name)?;
        type_.make_mut().directives.push(directive);
        Ok(())
    }

    pub(crate) fn remove_directive_name<Q: Hash + Equivalent<Name>>(
        &self,
        schema: &mut FederationSchema,
        name: &Q,
    ) {
        let Some(type_) = self.try_make_mut(&mut schema.schema) else {
            return;
        };
        self.remove_directive_name_references(&mut schema.referencers, name);
        type_
            .make_mut()
            .directives
            .retain(|other_directive| !name.equivalent(&other_directive.name));
    }

    pub(crate) fn remove_directive(
        &self,
        schema: &mut FederationSchema,
        directive: &Component<Directive>,
    ) {
        let Some(type_) = self.try_make_mut(&mut schema.schema) else {
            return;
        };
        if !type_.directives.iter().any(|other_directive| {
            (other_directive.name == directive.name) && !other_directive.ptr_eq(directive)
        }) {
            self.remove_directive_name_references(&mut schema.referencers, &directive.name);
        }
        type_
            .make_mut()
            .directives
            .retain(|other_directive| !other_directive.ptr_eq(directive));
    }

    fn insert_references(
        &self,
        type_: &Node<ScalarType>,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        validate_component_directives(type_.directives.deref())?;
        for directive_reference in type_.directives.iter() {
            self.insert_directive_name_references(referencers, &directive_reference.name)?;
        }
        Ok(())
    }

    fn remove_references(&self, type_: &Node<ScalarType>, referencers: &mut Referencers) {
        for directive_reference in type_.directives.iter() {
            self.remove_directive_name_references(referencers, &directive_reference.name);
        }
    }

    fn insert_directive_name_references(
        &self,
        referencers: &mut Referencers,
        name: &Name,
    ) -> Result<(), FederationError> {
        let directive_referencers = referencers.directives.get_mut(name).ok_or_else(|| {
            SingleFederationError::Internal {
                message: format!(
                    "Scalar type \"{}\"'s directive application \"@{}\" does not refer to an existing directive.",
                    self,
                    name,
                ),
            }
        })?;
        directive_referencers.scalar_types.insert(self.clone());
        Ok(())
    }

    fn remove_directive_name_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(directive_referencers) = referencers.directives.get_mut(name) else {
            return;
        };
        directive_referencers.scalar_types.remove(self);
    }
}

impl Display for ScalarTypeDefinitionPosition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.type_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ObjectTypeDefinitionPosition {
    pub(crate) type_name: Name,
}

impl ObjectTypeDefinitionPosition {
    pub(crate) fn get<'schema>(
        &self,
        schema: &'schema Schema,
    ) -> Result<&'schema Node<ObjectType>, FederationError> {
        schema
            .types
            .get(&self.type_name)
            .ok_or_else(|| {
                SingleFederationError::Internal {
                    message: format!("Schema has no type \"{}\"", self),
                }
                .into()
            })
            .and_then(|type_| {
                if let ExtendedType::Object(type_) = type_ {
                    Ok(type_)
                } else {
                    Err(SingleFederationError::Internal {
                        message: format!("Schema type \"{}\" was not an object", self),
                    }
                    .into())
                }
            })
    }

    pub(crate) fn try_get<'schema>(
        &self,
        schema: &'schema Schema,
    ) -> Option<&'schema Node<ObjectType>> {
        self.get(schema).ok()
    }

    fn make_mut<'schema>(
        &self,
        schema: &'schema mut Schema,
    ) -> Result<&'schema mut Node<ObjectType>, FederationError> {
        schema
            .types
            .get_mut(&self.type_name)
            .ok_or_else(|| {
                SingleFederationError::Internal {
                    message: format!("Schema has no type \"{}\"", self),
                }
                .into()
            })
            .and_then(|type_| {
                if let ExtendedType::Object(type_) = type_ {
                    Ok(type_)
                } else {
                    Err(SingleFederationError::Internal {
                        message: format!("Schema type \"{}\" was not an object", self),
                    }
                    .into())
                }
            })
    }

    fn try_make_mut<'schema>(
        &self,
        schema: &'schema mut Schema,
    ) -> Option<&'schema mut Node<ObjectType>> {
        if self.try_get(schema).is_some() {
            self.make_mut(schema).ok()
        } else {
            None
        }
    }

    pub(crate) fn pre_insert(&self, schema: &mut FederationSchema) -> Result<(), FederationError> {
        if schema.referencers.contains_type_name(&self.type_name) {
            // TODO: Allow built-in shadowing instead of ignoring them
            if is_graphql_reserved_name(&self.type_name) {
                return Ok(());
            }
            return Err(SingleFederationError::Internal {
                message: format!("Type \"{}\" has already been pre-inserted", self),
            }
            .into());
        }
        schema
            .referencers
            .object_types
            .insert(self.type_name.clone(), Default::default());
        Ok(())
    }

    pub(crate) fn insert(
        &self,
        schema: &mut FederationSchema,
        type_: Node<ObjectType>,
    ) -> Result<(), FederationError> {
        if !schema
            .referencers
            .object_types
            .contains_key(&self.type_name)
        {
            return Err(SingleFederationError::Internal {
                message: format!("Type \"{}\" has not been pre-inserted", self),
            }
            .into());
        }
        if schema.schema.types.contains_key(&self.type_name) {
            // TODO: Allow built-in shadowing instead of ignoring them
            if is_graphql_reserved_name(&self.type_name) {
                return Ok(());
            }
            return Err(SingleFederationError::Internal {
                message: format!("Type \"{}\" already exists in schema", self),
            }
            .into());
        }
        self.insert_references(&type_, &schema.schema, &mut schema.referencers)?;
        schema
            .schema
            .types
            .insert(self.type_name.clone(), ExtendedType::Object(type_));
        Ok(())
    }

    pub(crate) fn remove(
        &self,
        schema: &mut FederationSchema,
    ) -> Result<Option<ObjectTypeReferencers>, FederationError> {
        let Some(referencers) = self.remove_internal(schema)? else {
            return Ok(None);
        };
        for root in &referencers.schema_roots {
            root.remove(schema);
        }
        for field in &referencers.object_fields {
            field.remove(schema)?;
        }
        for field in &referencers.interface_fields {
            field.remove(schema)?;
        }
        for type_ in &referencers.union_types {
            type_.remove_member(schema, &self.type_name);
        }
        Ok(Some(referencers))
    }

    pub(crate) fn remove_recursive(
        &self,
        schema: &mut FederationSchema,
    ) -> Result<(), FederationError> {
        let Some(referencers) = self.remove_internal(schema)? else {
            return Ok(());
        };
        for root in referencers.schema_roots {
            root.remove(schema);
        }
        for field in referencers.object_fields {
            field.remove_recursive(schema)?;
        }
        for field in referencers.interface_fields {
            field.remove_recursive(schema)?;
        }
        for type_ in referencers.union_types {
            type_.remove_member_recursive(schema, &self.type_name)?;
        }
        Ok(())
    }

    fn remove_internal(
        &self,
        schema: &mut FederationSchema,
    ) -> Result<Option<ObjectTypeReferencers>, FederationError> {
        let Some(type_) = self.try_get(&schema.schema) else {
            return Ok(None);
        };
        self.remove_references(type_, &schema.schema, &mut schema.referencers)?;
        schema.schema.types.remove(&self.type_name);
        Ok(Some(
            schema
                .referencers
                .object_types
                .remove(&self.type_name)
                .ok_or_else(|| SingleFederationError::Internal {
                    message: format!("Schema missing referencers for type \"{}\"", self),
                })?,
        ))
    }

    pub(crate) fn insert_directive(
        &self,
        schema: &mut FederationSchema,
        directive: Component<Directive>,
    ) -> Result<(), FederationError> {
        let type_ = self.make_mut(&mut schema.schema)?;
        if type_
            .directives
            .iter()
            .any(|other_directive| other_directive.ptr_eq(&directive))
        {
            return Err(SingleFederationError::Internal {
                message: format!(
                    "Directive application \"@{}\" already exists on object type \"{}\"",
                    directive.name, self,
                ),
            }
            .into());
        }
        self.insert_directive_name_references(&mut schema.referencers, &directive.name)?;
        type_.make_mut().directives.push(directive);
        Ok(())
    }

    pub(crate) fn remove_directive_name<Q: Hash + Equivalent<Name>>(
        &self,
        schema: &mut FederationSchema,
        name: &Q,
    ) {
        let Some(type_) = self.try_make_mut(&mut schema.schema) else {
            return;
        };
        self.remove_directive_name_references(&mut schema.referencers, name);
        type_
            .make_mut()
            .directives
            .retain(|other_directive| !name.equivalent(&other_directive.name));
    }

    pub(crate) fn remove_directive(
        &self,
        schema: &mut FederationSchema,
        directive: &Component<Directive>,
    ) {
        let Some(type_) = self.try_make_mut(&mut schema.schema) else {
            return;
        };
        if !type_.directives.iter().any(|other_directive| {
            (other_directive.name == directive.name) && !other_directive.ptr_eq(directive)
        }) {
            self.remove_directive_name_references(&mut schema.referencers, &directive.name);
        }
        type_
            .make_mut()
            .directives
            .retain(|other_directive| !other_directive.ptr_eq(directive));
    }

    pub(crate) fn insert_implements_interface(
        &self,
        schema: &mut FederationSchema,
        name: Name,
    ) -> Result<(), FederationError> {
        let type_ = self.make_mut(&mut schema.schema)?;
        self.insert_implements_interface_references(&mut schema.referencers, &name)?;
        type_
            .make_mut()
            .implements_interfaces
            .insert(ComponentStr::new(&name));
        Ok(())
    }

    pub(crate) fn remove_implements_interface<Q: Hash + Equivalent<Name>>(
        &self,
        schema: &mut FederationSchema,
        name: &Q,
    ) {
        let Some(type_) = self.try_make_mut(&mut schema.schema) else {
            return;
        };
        self.remove_implements_interface_references(&mut schema.referencers, name);
        type_
            .make_mut()
            .implements_interfaces
            .retain(|other_type| !name.equivalent(other_type.deref()));
    }

    fn insert_references(
        &self,
        type_: &Node<ObjectType>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        validate_component_directives(type_.directives.deref())?;
        for directive_reference in type_.directives.iter() {
            self.insert_directive_name_references(referencers, &directive_reference.name)?;
        }
        for interface_type_reference in type_.implements_interfaces.iter() {
            self.insert_implements_interface_references(
                referencers,
                interface_type_reference.deref(),
            )?;
        }
        for (field_name, field) in type_.fields.iter() {
            ObjectFieldDefinitionPosition {
                type_name: self.type_name.clone(),
                field_name: field_name.clone(),
            }
            .insert_references(field, schema, referencers)?;
        }
        Ok(())
    }

    fn remove_references(
        &self,
        type_: &Node<ObjectType>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        for directive_reference in type_.directives.iter() {
            self.remove_directive_name_references(referencers, &directive_reference.name);
        }
        for interface_type_reference in type_.implements_interfaces.iter() {
            self.remove_implements_interface_references(
                referencers,
                interface_type_reference.deref(),
            );
        }
        for (field_name, field) in type_.fields.iter() {
            ObjectFieldDefinitionPosition {
                type_name: self.type_name.clone(),
                field_name: field_name.clone(),
            }
            .remove_references(field, schema, referencers)?;
        }
        Ok(())
    }

    fn insert_directive_name_references(
        &self,
        referencers: &mut Referencers,
        name: &Name,
    ) -> Result<(), FederationError> {
        let directive_referencers = referencers.directives.get_mut(name).ok_or_else(|| {
            SingleFederationError::Internal {
                message: format!(
                    "Object type \"{}\"'s directive application \"@{}\" does not refer to an existing directive.",
                    self,
                    name,
                ),
            }
        })?;
        directive_referencers.object_types.insert(self.clone());
        Ok(())
    }

    fn remove_directive_name_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(directive_referencers) = referencers.directives.get_mut(name) else {
            return;
        };
        directive_referencers.object_types.remove(self);
    }

    fn insert_implements_interface_references(
        &self,
        referencers: &mut Referencers,
        name: &Name,
    ) -> Result<(), FederationError> {
        let interface_type_referencers = referencers.interface_types.get_mut(name).ok_or_else(|| {
            SingleFederationError::Internal {
                message: format!(
                    "Object type \"{}\"'s implements \"{}\" does not refer to an existing interface.",
                    self,
                    name,
                ),
            }
        })?;
        interface_type_referencers.object_types.insert(self.clone());
        Ok(())
    }

    fn remove_implements_interface_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(interface_type_referencers) = referencers.interface_types.get_mut(name) else {
            return;
        };
        interface_type_referencers.object_types.remove(self);
    }
}

impl Display for ObjectTypeDefinitionPosition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.type_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ObjectFieldDefinitionPosition {
    pub(crate) type_name: Name,
    pub(crate) field_name: Name,
}

impl ObjectFieldDefinitionPosition {
    pub(crate) fn parent(&self) -> ObjectTypeDefinitionPosition {
        ObjectTypeDefinitionPosition {
            type_name: self.type_name.clone(),
        }
    }

    pub(crate) fn get<'schema>(
        &self,
        schema: &'schema Schema,
    ) -> Result<&'schema Component<FieldDefinition>, FederationError> {
        let parent = self.parent();
        let type_ = parent.get(schema)?;

        type_.fields.get(&self.field_name).ok_or_else(|| {
            SingleFederationError::Internal {
                message: format!(
                    "Object type \"{}\" has no field \"{}\"",
                    parent, self.field_name
                ),
            }
            .into()
        })
    }

    pub(crate) fn try_get<'schema>(
        &self,
        schema: &'schema Schema,
    ) -> Option<&'schema Component<FieldDefinition>> {
        self.get(schema).ok()
    }

    fn make_mut<'schema>(
        &self,
        schema: &'schema mut Schema,
    ) -> Result<&'schema mut Component<FieldDefinition>, FederationError> {
        let parent = self.parent();
        let type_ = parent.make_mut(schema)?.make_mut();

        type_.fields.get_mut(&self.field_name).ok_or_else(|| {
            SingleFederationError::Internal {
                message: format!(
                    "Object type \"{}\" has no field \"{}\"",
                    parent, self.field_name
                ),
            }
            .into()
        })
    }

    fn try_make_mut<'schema>(
        &self,
        schema: &'schema mut Schema,
    ) -> Option<&'schema mut Component<FieldDefinition>> {
        if self.try_get(schema).is_some() {
            self.make_mut(schema).ok()
        } else {
            None
        }
    }

    pub(crate) fn insert(
        &self,
        schema: &mut FederationSchema,
        field: Component<FieldDefinition>,
    ) -> Result<(), FederationError> {
        if self.field_name != field.name {
            return Err(SingleFederationError::Internal {
                message: format!(
                    "Object field \"{}\" given field named \"{}\"",
                    self, field.name,
                ),
            }
            .into());
        }
        // TODO: Handle __typename if it appears
        if self.try_get(&schema.schema).is_some() {
            return Err(SingleFederationError::Internal {
                message: format!("Object field \"{}\" already exists in schema", self),
            }
            .into());
        }
        self.insert_references(&field, &schema.schema, &mut schema.referencers)?;
        self.parent()
            .make_mut(&mut schema.schema)?
            .make_mut()
            .fields
            .insert(self.field_name.clone(), field);
        Ok(())
    }

    pub(crate) fn remove(&self, schema: &mut FederationSchema) -> Result<(), FederationError> {
        let Some(field) = self.try_get(&schema.schema) else {
            return Ok(());
        };
        self.remove_references(field, &schema.schema, &mut schema.referencers)?;
        self.parent()
            .make_mut(&mut schema.schema)?
            .make_mut()
            .fields
            .remove(&self.field_name);
        Ok(())
    }

    pub(crate) fn remove_recursive(
        &self,
        schema: &mut FederationSchema,
    ) -> Result<(), FederationError> {
        self.remove(schema)?;
        let parent = self.parent();
        let Some(type_) = parent.try_get(&schema.schema) else {
            return Ok(());
        };
        if type_.fields.is_empty() {
            parent.remove_recursive(schema)?;
        }
        Ok(())
    }

    pub(crate) fn insert_directive(
        &self,
        schema: &mut FederationSchema,
        directive: Node<Directive>,
    ) -> Result<(), FederationError> {
        let field = self.make_mut(&mut schema.schema)?;
        if field
            .directives
            .iter()
            .any(|other_directive| other_directive.ptr_eq(&directive))
        {
            return Err(SingleFederationError::Internal {
                message: format!(
                    "Directive application \"@{}\" already exists on object field \"{}\"",
                    directive.name, self,
                ),
            }
            .into());
        }
        self.insert_directive_name_references(&mut schema.referencers, &directive.name)?;
        field.make_mut().directives.push(directive);
        Ok(())
    }

    pub(crate) fn remove_directive_name<Q: Hash + Equivalent<Name>>(
        &self,
        schema: &mut FederationSchema,
        name: &Q,
    ) {
        let Some(field) = self.try_make_mut(&mut schema.schema) else {
            return;
        };
        self.remove_directive_name_references(&mut schema.referencers, name);
        field
            .make_mut()
            .directives
            .retain(|other_directive| !name.equivalent(&other_directive.name));
    }

    pub(crate) fn remove_directive(
        &self,
        schema: &mut FederationSchema,
        directive: &Node<Directive>,
    ) {
        let Some(field) = self.try_make_mut(&mut schema.schema) else {
            return;
        };
        if !field.directives.iter().any(|other_directive| {
            (other_directive.name == directive.name) && !other_directive.ptr_eq(directive)
        }) {
            self.remove_directive_name_references(&mut schema.referencers, &directive.name);
        }
        field
            .make_mut()
            .directives
            .retain(|other_directive| !other_directive.ptr_eq(directive));
    }

    fn insert_references(
        &self,
        field: &Component<FieldDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        validate_node_directives(field.directives.deref())?;
        for directive_reference in field.directives.iter() {
            self.insert_directive_name_references(referencers, &directive_reference.name)?;
        }
        self.insert_type_references(field, schema, referencers)?;
        validate_arguments(&field.arguments)?;
        for argument in field.arguments.iter() {
            ObjectFieldArgumentDefinitionPosition {
                type_name: self.type_name.clone(),
                field_name: self.field_name.clone(),
                argument_name: argument.name.clone(),
            }
            .insert_references(argument, schema, referencers)?;
        }
        Ok(())
    }

    fn remove_references(
        &self,
        field: &Component<FieldDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        for directive_reference in field.directives.iter() {
            self.remove_directive_name_references(referencers, &directive_reference.name);
        }
        self.remove_type_references(field, schema, referencers)?;
        for argument in field.arguments.iter() {
            ObjectFieldArgumentDefinitionPosition {
                type_name: self.type_name.clone(),
                field_name: self.field_name.clone(),
                argument_name: argument.name.clone(),
            }
            .remove_references(argument, schema, referencers)?;
        }
        Ok(())
    }

    fn insert_directive_name_references(
        &self,
        referencers: &mut Referencers,
        name: &Name,
    ) -> Result<(), FederationError> {
        let directive_referencers = referencers.directives.get_mut(name).ok_or_else(|| {
            SingleFederationError::Internal {
                message: format!(
                    "Object field \"{}\"'s directive application \"@{}\" does not refer to an existing directive.",
                    self,
                    name,
                ),
            }
        })?;
        directive_referencers.object_fields.insert(self.clone());
        Ok(())
    }

    fn remove_directive_name_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(directive_referencers) = referencers.directives.get_mut(name) else {
            return;
        };
        directive_referencers.object_fields.remove(self);
    }

    fn insert_type_references(
        &self,
        field: &Component<FieldDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        let output_type_reference = field.ty.inner_named_type();
        match schema.types.get(output_type_reference) {
            Some(ExtendedType::Scalar(_)) => {
                let scalar_type_referencers = referencers
                    .scalar_types
                    .get_mut(output_type_reference)
                    .ok_or_else(|| SingleFederationError::Internal {
                        message: format!(
                            "Schema missing referencers for type \"{}\"",
                            output_type_reference
                        ),
                    })?;
                scalar_type_referencers.object_fields.insert(self.clone());
            }
            Some(ExtendedType::Object(_)) => {
                let object_type_referencers = referencers
                    .object_types
                    .get_mut(output_type_reference)
                    .ok_or_else(|| SingleFederationError::Internal {
                        message: format!(
                            "Schema missing referencers for type \"{}\"",
                            output_type_reference
                        ),
                    })?;
                object_type_referencers.object_fields.insert(self.clone());
            }
            Some(ExtendedType::Interface(_)) => {
                let interface_type_referencers = referencers
                    .interface_types
                    .get_mut(output_type_reference)
                    .ok_or_else(|| SingleFederationError::Internal {
                        message: format!(
                            "Schema missing referencers for type \"{}\"",
                            output_type_reference
                        ),
                    })?;
                interface_type_referencers
                    .object_fields
                    .insert(self.clone());
            }
            Some(ExtendedType::Union(_)) => {
                let union_type_referencers = referencers
                    .union_types
                    .get_mut(output_type_reference)
                    .ok_or_else(|| SingleFederationError::Internal {
                        message: format!(
                            "Schema missing referencers for type \"{}\"",
                            output_type_reference
                        ),
                    })?;
                union_type_referencers.object_fields.insert(self.clone());
            }
            Some(ExtendedType::Enum(_)) => {
                let enum_type_referencers = referencers
                    .enum_types
                    .get_mut(output_type_reference)
                    .ok_or_else(|| SingleFederationError::Internal {
                    message: format!(
                        "Schema missing referencers for type \"{}\"",
                        output_type_reference
                    ),
                })?;
                enum_type_referencers.object_fields.insert(self.clone());
            }
            _ => {
                return Err(
                    SingleFederationError::Internal {
                        message: format!(
                            "Object field \"{}\"'s inner type \"{}\" does not refer to an existing output type.",
                            self,
                            output_type_reference.deref(),
                        )
                    }.into()
                );
            }
        }
        Ok(())
    }

    fn remove_type_references(
        &self,
        field: &Component<FieldDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        let output_type_reference = field.ty.inner_named_type();
        match schema.types.get(output_type_reference) {
            Some(ExtendedType::Scalar(_)) => {
                let Some(scalar_type_referencers) =
                    referencers.scalar_types.get_mut(output_type_reference)
                else {
                    return Ok(());
                };
                scalar_type_referencers.object_fields.remove(self);
            }
            Some(ExtendedType::Object(_)) => {
                let Some(object_type_referencers) =
                    referencers.object_types.get_mut(output_type_reference)
                else {
                    return Ok(());
                };
                object_type_referencers.object_fields.remove(self);
            }
            Some(ExtendedType::Interface(_)) => {
                let Some(interface_type_referencers) =
                    referencers.interface_types.get_mut(output_type_reference)
                else {
                    return Ok(());
                };
                interface_type_referencers.object_fields.remove(self);
            }
            Some(ExtendedType::Union(_)) => {
                let Some(union_type_referencers) =
                    referencers.union_types.get_mut(output_type_reference)
                else {
                    return Ok(());
                };
                union_type_referencers.object_fields.remove(self);
            }
            Some(ExtendedType::Enum(_)) => {
                let Some(enum_type_referencers) =
                    referencers.enum_types.get_mut(output_type_reference)
                else {
                    return Ok(());
                };
                enum_type_referencers.object_fields.remove(self);
            }
            _ => {
                return Err(
                    SingleFederationError::Internal {
                        message: format!(
                            "Object field \"{}\"'s inner type \"{}\" does not refer to an existing output type.",
                            self,
                            output_type_reference.deref(),
                        )
                    }.into()
                );
            }
        }
        Ok(())
    }
}

impl Display for ObjectFieldDefinitionPosition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.type_name, self.field_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ObjectFieldArgumentDefinitionPosition {
    pub(crate) type_name: Name,
    pub(crate) field_name: Name,
    pub(crate) argument_name: Name,
}

impl ObjectFieldArgumentDefinitionPosition {
    pub(crate) fn parent(&self) -> ObjectFieldDefinitionPosition {
        ObjectFieldDefinitionPosition {
            type_name: self.type_name.clone(),
            field_name: self.field_name.clone(),
        }
    }

    pub(crate) fn get<'schema>(
        &self,
        schema: &'schema Schema,
    ) -> Result<&'schema Node<InputValueDefinition>, FederationError> {
        let parent = self.parent();
        let type_ = parent.get(schema)?;

        type_
            .arguments
            .iter()
            .find(|a| a.name == self.argument_name)
            .ok_or_else(|| {
                SingleFederationError::Internal {
                    message: format!(
                        "Object field \"{}\" has no argument \"{}\"",
                        parent, self.argument_name
                    ),
                }
                .into()
            })
    }

    pub(crate) fn try_get<'schema>(
        &self,
        schema: &'schema Schema,
    ) -> Option<&'schema Node<InputValueDefinition>> {
        self.get(schema).ok()
    }

    fn make_mut<'schema>(
        &self,
        schema: &'schema mut Schema,
    ) -> Result<&'schema mut Node<InputValueDefinition>, FederationError> {
        let parent = self.parent();
        let type_ = parent.make_mut(schema)?.make_mut();

        type_
            .arguments
            .iter_mut()
            .find(|a| a.name == self.argument_name)
            .ok_or_else(|| {
                SingleFederationError::Internal {
                    message: format!(
                        "Object field \"{}\" has no argument \"{}\"",
                        parent, self.argument_name
                    ),
                }
                .into()
            })
    }

    fn try_make_mut<'schema>(
        &self,
        schema: &'schema mut Schema,
    ) -> Option<&'schema mut Node<InputValueDefinition>> {
        if self.try_get(schema).is_some() {
            self.make_mut(schema).ok()
        } else {
            None
        }
    }

    pub(crate) fn insert(
        &self,
        schema: &mut FederationSchema,
        argument: Node<InputValueDefinition>,
    ) -> Result<(), FederationError> {
        if self.argument_name != argument.name {
            return Err(SingleFederationError::Internal {
                message: format!(
                    "Object field argument \"{}\" given argument named \"{}\"",
                    self, argument.name,
                ),
            }
            .into());
        }
        if self.try_get(&schema.schema).is_some() {
            // TODO: Handle old spec edge case of arguments with non-unique names
            return Err(SingleFederationError::Internal {
                message: format!(
                    "Object field argument \"{}\" already exists in schema",
                    self,
                ),
            }
            .into());
        }
        self.insert_references(&argument, &schema.schema, &mut schema.referencers)?;
        self.parent()
            .make_mut(&mut schema.schema)?
            .make_mut()
            .arguments
            .push(argument);
        Ok(())
    }

    pub(crate) fn remove(&self, schema: &mut FederationSchema) -> Result<(), FederationError> {
        let Some(argument) = self.try_get(&schema.schema) else {
            return Ok(());
        };
        self.remove_references(argument, &schema.schema, &mut schema.referencers)?;
        self.parent()
            .make_mut(&mut schema.schema)?
            .make_mut()
            .arguments
            .retain(|other_argument| other_argument.name != self.argument_name);
        Ok(())
    }

    pub(crate) fn insert_directive(
        &self,
        schema: &mut FederationSchema,
        directive: Node<Directive>,
    ) -> Result<(), FederationError> {
        let argument = self.make_mut(&mut schema.schema)?;
        if argument
            .directives
            .iter()
            .any(|other_directive| other_directive.ptr_eq(&directive))
        {
            return Err(SingleFederationError::Internal {
                message: format!(
                    "Directive application \"@{}\" already exists on object field argument \"{}\"",
                    directive.name, self,
                ),
            }
            .into());
        }
        self.insert_directive_name_references(&mut schema.referencers, &directive.name)?;
        argument.make_mut().directives.push(directive);
        Ok(())
    }

    pub(crate) fn remove_directive_name<Q: Hash + Equivalent<Name>>(
        &self,
        schema: &mut FederationSchema,
        name: &Q,
    ) {
        let Some(argument) = self.try_make_mut(&mut schema.schema) else {
            return;
        };
        self.remove_directive_name_references(&mut schema.referencers, name);
        argument
            .make_mut()
            .directives
            .retain(|other_directive| !name.equivalent(&other_directive.name));
    }

    pub(crate) fn remove_directive(
        &self,
        schema: &mut FederationSchema,
        directive: &Node<Directive>,
    ) {
        let Some(argument) = self.try_make_mut(&mut schema.schema) else {
            return;
        };
        if !argument.directives.iter().any(|other_directive| {
            (other_directive.name == directive.name) && !other_directive.ptr_eq(directive)
        }) {
            self.remove_directive_name_references(&mut schema.referencers, &directive.name);
        }
        argument
            .make_mut()
            .directives
            .retain(|other_directive| !other_directive.ptr_eq(directive));
    }

    fn insert_references(
        &self,
        argument: &Node<InputValueDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        validate_node_directives(argument.directives.deref())?;
        for directive_reference in argument.directives.iter() {
            self.insert_directive_name_references(referencers, &directive_reference.name)?;
        }
        self.insert_type_references(argument, schema, referencers)
    }

    fn remove_references(
        &self,
        argument: &Node<InputValueDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        for directive_reference in argument.directives.iter() {
            self.remove_directive_name_references(referencers, &directive_reference.name);
        }
        self.remove_type_references(argument, schema, referencers)
    }

    fn insert_directive_name_references(
        &self,
        referencers: &mut Referencers,
        name: &Name,
    ) -> Result<(), FederationError> {
        let directive_referencers = referencers.directives.get_mut(name).ok_or_else(|| {
            SingleFederationError::Internal {
                message: format!(
                    "Object field argument \"{}\"'s directive application \"@{}\" does not refer to an existing directive.",
                    self,
                    name,
                ),
            }
        })?;
        directive_referencers
            .object_field_arguments
            .insert(self.clone());
        Ok(())
    }

    fn remove_directive_name_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(directive_referencers) = referencers.directives.get_mut(name) else {
            return;
        };
        directive_referencers.object_field_arguments.remove(self);
    }

    fn insert_type_references(
        &self,
        argument: &Node<InputValueDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        let input_type_reference = argument.ty.inner_named_type();
        match schema.types.get(input_type_reference) {
            Some(ExtendedType::Scalar(_)) => {
                let scalar_type_referencers = referencers
                    .scalar_types
                    .get_mut(input_type_reference)
                    .ok_or_else(|| SingleFederationError::Internal {
                        message: format!(
                            "Schema missing referencers for type \"{}\"",
                            input_type_reference
                        ),
                    })?;
                scalar_type_referencers
                    .object_field_arguments
                    .insert(self.clone());
            }
            Some(ExtendedType::Enum(_)) => {
                let enum_type_referencers = referencers
                    .enum_types
                    .get_mut(input_type_reference)
                    .ok_or_else(|| SingleFederationError::Internal {
                        message: format!(
                            "Schema missing referencers for type \"{}\"",
                            input_type_reference
                        ),
                    })?;
                enum_type_referencers
                    .object_field_arguments
                    .insert(self.clone());
            }
            Some(ExtendedType::InputObject(_)) => {
                let input_object_type_referencers = referencers
                    .input_object_types
                    .get_mut(input_type_reference)
                    .ok_or_else(|| SingleFederationError::Internal {
                        message: format!(
                            "Schema missing referencers for type \"{}\"",
                            input_type_reference
                        ),
                    })?;
                input_object_type_referencers
                    .object_field_arguments
                    .insert(self.clone());
            }
            _ => {
                return Err(
                    SingleFederationError::Internal {
                        message: format!(
                            "Object field argument \"{}\"'s inner type \"{}\" does not refer to an existing input type.",
                            self,
                            input_type_reference.deref(),
                        )
                    }.into()
                );
            }
        }
        Ok(())
    }

    fn remove_type_references(
        &self,
        argument: &Node<InputValueDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        let input_type_reference = argument.ty.inner_named_type();
        match schema.types.get(input_type_reference) {
            Some(ExtendedType::Scalar(_)) => {
                let Some(scalar_type_referencers) =
                    referencers.scalar_types.get_mut(input_type_reference)
                else {
                    return Ok(());
                };
                scalar_type_referencers.object_field_arguments.remove(self);
            }
            Some(ExtendedType::Enum(_)) => {
                let Some(enum_type_referencers) =
                    referencers.enum_types.get_mut(input_type_reference)
                else {
                    return Ok(());
                };
                enum_type_referencers.object_field_arguments.remove(self);
            }
            Some(ExtendedType::InputObject(_)) => {
                let Some(input_object_type_referencers) =
                    referencers.input_object_types.get_mut(input_type_reference)
                else {
                    return Ok(());
                };
                input_object_type_referencers
                    .object_field_arguments
                    .remove(self);
            }
            _ => {
                return Err(
                    SingleFederationError::Internal {
                        message: format!(
                            "Object field argument \"{}\"'s inner type \"{}\" does not refer to an existing input type.",
                            self,
                            input_type_reference.deref(),
                        )
                    }.into()
                );
            }
        }
        Ok(())
    }
}

impl Display for ObjectFieldArgumentDefinitionPosition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}.{}({}:)",
            self.type_name, self.field_name, self.argument_name
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct InterfaceTypeDefinitionPosition {
    pub(crate) type_name: Name,
}

impl InterfaceTypeDefinitionPosition {
    pub(crate) fn get<'schema>(
        &self,
        schema: &'schema Schema,
    ) -> Result<&'schema Node<InterfaceType>, FederationError> {
        schema
            .types
            .get(&self.type_name)
            .ok_or_else(|| {
                SingleFederationError::Internal {
                    message: format!("Schema has no type \"{}\"", self),
                }
                .into()
            })
            .and_then(|type_| {
                if let ExtendedType::Interface(type_) = type_ {
                    Ok(type_)
                } else {
                    Err(SingleFederationError::Internal {
                        message: format!("Schema type \"{}\" was not an interface", self),
                    }
                    .into())
                }
            })
    }

    pub(crate) fn try_get<'schema>(
        &self,
        schema: &'schema Schema,
    ) -> Option<&'schema Node<InterfaceType>> {
        self.get(schema).ok()
    }

    fn make_mut<'schema>(
        &self,
        schema: &'schema mut Schema,
    ) -> Result<&'schema mut Node<InterfaceType>, FederationError> {
        schema
            .types
            .get_mut(&self.type_name)
            .ok_or_else(|| {
                SingleFederationError::Internal {
                    message: format!("Schema has no type \"{}\"", self),
                }
                .into()
            })
            .and_then(|type_| {
                if let ExtendedType::Interface(type_) = type_ {
                    Ok(type_)
                } else {
                    Err(SingleFederationError::Internal {
                        message: format!("Schema type \"{}\" was not an interface", self),
                    }
                    .into())
                }
            })
    }

    fn try_make_mut<'schema>(
        &self,
        schema: &'schema mut Schema,
    ) -> Option<&'schema mut Node<InterfaceType>> {
        if self.try_get(schema).is_some() {
            self.make_mut(schema).ok()
        } else {
            None
        }
    }

    pub(crate) fn pre_insert(&self, schema: &mut FederationSchema) -> Result<(), FederationError> {
        if schema.referencers.contains_type_name(&self.type_name) {
            // TODO: Allow built-in shadowing instead of ignoring them
            if is_graphql_reserved_name(&self.type_name) {
                return Ok(());
            }
            return Err(SingleFederationError::Internal {
                message: format!("Type \"{}\" has already been pre-inserted", self),
            }
            .into());
        }
        schema
            .referencers
            .interface_types
            .insert(self.type_name.clone(), Default::default());
        Ok(())
    }

    pub(crate) fn insert(
        &self,
        schema: &mut FederationSchema,
        type_: Node<InterfaceType>,
    ) -> Result<(), FederationError> {
        if !schema
            .referencers
            .interface_types
            .contains_key(&self.type_name)
        {
            return Err(SingleFederationError::Internal {
                message: format!("Type \"{}\" has not been pre-inserted", self),
            }
            .into());
        }
        if schema.schema.types.contains_key(&self.type_name) {
            // TODO: Allow built-in shadowing instead of ignoring them
            if is_graphql_reserved_name(&self.type_name) {
                return Ok(());
            }
            return Err(SingleFederationError::Internal {
                message: format!("Type \"{}\" already exists in schema", self),
            }
            .into());
        }
        self.insert_references(&type_, &schema.schema, &mut schema.referencers)?;
        schema
            .schema
            .types
            .insert(self.type_name.clone(), ExtendedType::Interface(type_));
        Ok(())
    }

    pub(crate) fn remove(
        &self,
        schema: &mut FederationSchema,
    ) -> Result<Option<InterfaceTypeReferencers>, FederationError> {
        let Some(referencers) = self.remove_internal(schema)? else {
            return Ok(None);
        };
        for type_ in &referencers.object_types {
            type_.remove_implements_interface(schema, &self.type_name);
        }
        for field in &referencers.object_fields {
            field.remove(schema)?;
        }
        for type_ in &referencers.interface_types {
            type_.remove_implements_interface(schema, &self.type_name);
        }
        for field in &referencers.interface_fields {
            field.remove(schema)?;
        }
        Ok(Some(referencers))
    }

    pub(crate) fn remove_recursive(
        &self,
        schema: &mut FederationSchema,
    ) -> Result<(), FederationError> {
        let Some(referencers) = self.remove_internal(schema)? else {
            return Ok(());
        };
        for type_ in referencers.object_types {
            type_.remove_implements_interface(schema, &self.type_name);
        }
        for field in referencers.object_fields {
            field.remove_recursive(schema)?;
        }
        for type_ in referencers.interface_types {
            type_.remove_implements_interface(schema, &self.type_name);
        }
        for field in referencers.interface_fields {
            field.remove_recursive(schema)?;
        }
        Ok(())
    }

    fn remove_internal(
        &self,
        schema: &mut FederationSchema,
    ) -> Result<Option<InterfaceTypeReferencers>, FederationError> {
        let Some(type_) = self.try_get(&schema.schema) else {
            return Ok(None);
        };
        self.remove_references(type_, &schema.schema, &mut schema.referencers)?;
        schema.schema.types.remove(&self.type_name);
        Ok(Some(
            schema
                .referencers
                .interface_types
                .remove(&self.type_name)
                .ok_or_else(|| SingleFederationError::Internal {
                    message: format!("Schema missing referencers for type \"{}\"", self),
                })?,
        ))
    }

    pub(crate) fn insert_directive(
        &self,
        schema: &mut FederationSchema,
        directive: Component<Directive>,
    ) -> Result<(), FederationError> {
        let type_ = self.make_mut(&mut schema.schema)?;
        if type_
            .directives
            .iter()
            .any(|other_directive| other_directive.ptr_eq(&directive))
        {
            return Err(SingleFederationError::Internal {
                message: format!(
                    "Directive application \"@{}\" already exists on interface type \"{}\"",
                    directive.name, self,
                ),
            }
            .into());
        }
        self.insert_directive_name_references(&mut schema.referencers, &directive.name)?;
        type_.make_mut().directives.push(directive);
        Ok(())
    }

    pub(crate) fn remove_directive_name<Q: Hash + Equivalent<Name>>(
        &self,
        schema: &mut FederationSchema,
        name: &Q,
    ) {
        let Some(type_) = self.try_make_mut(&mut schema.schema) else {
            return;
        };
        self.remove_directive_name_references(&mut schema.referencers, name);
        type_
            .make_mut()
            .directives
            .retain(|other_directive| !name.equivalent(&other_directive.name));
    }

    pub(crate) fn remove_directive(
        &self,
        schema: &mut FederationSchema,
        directive: &Component<Directive>,
    ) {
        let Some(type_) = self.try_make_mut(&mut schema.schema) else {
            return;
        };
        if !type_.directives.iter().any(|other_directive| {
            (other_directive.name == directive.name) && !other_directive.ptr_eq(directive)
        }) {
            self.remove_directive_name_references(&mut schema.referencers, &directive.name);
        }
        type_
            .make_mut()
            .directives
            .retain(|other_directive| !other_directive.ptr_eq(directive));
    }

    pub(crate) fn insert_implements_interface(
        &self,
        schema: &mut FederationSchema,
        name: Name,
    ) -> Result<(), FederationError> {
        let type_ = self.make_mut(&mut schema.schema)?;
        self.insert_implements_interface_references(&mut schema.referencers, &name)?;
        type_
            .make_mut()
            .implements_interfaces
            .insert(ComponentStr::new(&name));
        Ok(())
    }

    pub(crate) fn remove_implements_interface<Q: Hash + Equivalent<Name>>(
        &self,
        schema: &mut FederationSchema,
        name: &Q,
    ) {
        let Some(type_) = self.try_make_mut(&mut schema.schema) else {
            return;
        };
        self.remove_implements_interface_references(&mut schema.referencers, name);
        type_
            .make_mut()
            .implements_interfaces
            .retain(|other_type| !name.equivalent(other_type.deref()));
    }

    fn insert_references(
        &self,
        type_: &Node<InterfaceType>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        validate_component_directives(type_.directives.deref())?;
        for directive_reference in type_.directives.iter() {
            self.insert_directive_name_references(referencers, &directive_reference.name)?;
        }
        for interface_type_reference in type_.implements_interfaces.iter() {
            self.insert_implements_interface_references(
                referencers,
                interface_type_reference.deref(),
            )?;
        }
        for (field_name, field) in type_.fields.iter() {
            InterfaceFieldDefinitionPosition {
                type_name: self.type_name.clone(),
                field_name: field_name.clone(),
            }
            .insert_references(field, schema, referencers)?;
        }
        Ok(())
    }

    fn remove_references(
        &self,
        type_: &Node<InterfaceType>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        for directive_reference in type_.directives.iter() {
            self.remove_directive_name_references(referencers, &directive_reference.name);
        }
        for interface_type_reference in type_.implements_interfaces.iter() {
            self.remove_implements_interface_references(
                referencers,
                interface_type_reference.deref(),
            );
        }
        for (field_name, field) in type_.fields.iter() {
            InterfaceFieldDefinitionPosition {
                type_name: self.type_name.clone(),
                field_name: field_name.clone(),
            }
            .remove_references(field, schema, referencers)?;
        }
        Ok(())
    }

    fn insert_directive_name_references(
        &self,
        referencers: &mut Referencers,
        name: &Name,
    ) -> Result<(), FederationError> {
        let directive_referencers = referencers.directives.get_mut(name).ok_or_else(|| {
            SingleFederationError::Internal {
                message: format!(
                    "Interface type \"{}\"'s directive application \"@{}\" does not refer to an existing directive.",
                    self,
                    name,
                ),
            }
        })?;
        directive_referencers.interface_types.insert(self.clone());
        Ok(())
    }

    fn remove_directive_name_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(directive_referencers) = referencers.directives.get_mut(name) else {
            return;
        };
        directive_referencers.interface_types.remove(self);
    }

    fn insert_implements_interface_references(
        &self,
        referencers: &mut Referencers,
        name: &Name,
    ) -> Result<(), FederationError> {
        let interface_type_referencers = referencers.interface_types.get_mut(name).ok_or_else(|| {
            SingleFederationError::Internal {
                message: format!(
                    "Interface type \"{}\"'s implements \"{}\" does not refer to an existing interface.",
                    self,
                    name,
                ),
            }
        })?;
        interface_type_referencers
            .interface_types
            .insert(self.clone());
        Ok(())
    }

    fn remove_implements_interface_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(interface_type_referencers) = referencers.interface_types.get_mut(name) else {
            return;
        };
        interface_type_referencers.interface_types.remove(self);
    }
}

impl Display for InterfaceTypeDefinitionPosition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.type_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct InterfaceFieldDefinitionPosition {
    pub(crate) type_name: Name,
    pub(crate) field_name: Name,
}

impl InterfaceFieldDefinitionPosition {
    pub(crate) fn parent(&self) -> InterfaceTypeDefinitionPosition {
        InterfaceTypeDefinitionPosition {
            type_name: self.type_name.clone(),
        }
    }

    pub(crate) fn get<'schema>(
        &self,
        schema: &'schema Schema,
    ) -> Result<&'schema Component<FieldDefinition>, FederationError> {
        let parent = self.parent();
        let type_ = parent.get(schema)?;

        type_.fields.get(&self.field_name).ok_or_else(|| {
            SingleFederationError::Internal {
                message: format!(
                    "Interface type \"{}\" has no field \"{}\"",
                    parent, self.field_name
                ),
            }
            .into()
        })
    }

    pub(crate) fn try_get<'schema>(
        &self,
        schema: &'schema Schema,
    ) -> Option<&'schema Component<FieldDefinition>> {
        self.get(schema).ok()
    }

    fn make_mut<'schema>(
        &self,
        schema: &'schema mut Schema,
    ) -> Result<&'schema mut Component<FieldDefinition>, FederationError> {
        let parent = self.parent();
        let type_ = parent.make_mut(schema)?.make_mut();

        type_.fields.get_mut(&self.field_name).ok_or_else(|| {
            SingleFederationError::Internal {
                message: format!(
                    "Interface type \"{}\" has no field \"{}\"",
                    parent, self.field_name
                ),
            }
            .into()
        })
    }

    fn try_make_mut<'schema>(
        &self,
        schema: &'schema mut Schema,
    ) -> Option<&'schema mut Component<FieldDefinition>> {
        if self.try_get(schema).is_some() {
            self.make_mut(schema).ok()
        } else {
            None
        }
    }

    pub(crate) fn insert(
        &self,
        schema: &mut FederationSchema,
        field: Component<FieldDefinition>,
    ) -> Result<(), FederationError> {
        if self.field_name != field.name {
            return Err(SingleFederationError::Internal {
                message: format!(
                    "Interface field \"{}\" given field named \"{}\"",
                    self, field.name,
                ),
            }
            .into());
        }
        // TODO: Handle __typename if it appears
        if self.try_get(&schema.schema).is_some() {
            return Err(SingleFederationError::Internal {
                message: format!("Interface field \"{}\" already exists in schema", self),
            }
            .into());
        }
        self.insert_references(&field, &schema.schema, &mut schema.referencers)?;
        self.parent()
            .make_mut(&mut schema.schema)?
            .make_mut()
            .fields
            .insert(self.field_name.clone(), field);
        Ok(())
    }

    pub(crate) fn remove(&self, schema: &mut FederationSchema) -> Result<(), FederationError> {
        let Some(field) = self.try_get(&schema.schema) else {
            return Ok(());
        };
        self.remove_references(field, &schema.schema, &mut schema.referencers)?;
        self.parent()
            .make_mut(&mut schema.schema)?
            .make_mut()
            .fields
            .remove(&self.field_name);
        Ok(())
    }

    pub(crate) fn remove_recursive(
        &self,
        schema: &mut FederationSchema,
    ) -> Result<(), FederationError> {
        self.remove(schema)?;
        let parent = self.parent();
        let Some(type_) = parent.try_get(&schema.schema) else {
            return Ok(());
        };
        if type_.fields.is_empty() {
            parent.remove_recursive(schema)?;
        }
        Ok(())
    }

    pub(crate) fn insert_directive(
        &self,
        schema: &mut FederationSchema,
        directive: Node<Directive>,
    ) -> Result<(), FederationError> {
        let field = self.make_mut(&mut schema.schema)?;
        if field
            .directives
            .iter()
            .any(|other_directive| other_directive.ptr_eq(&directive))
        {
            return Err(SingleFederationError::Internal {
                message: format!(
                    "Directive application \"@{}\" already exists on interface field \"{}\"",
                    directive.name, self,
                ),
            }
            .into());
        }
        self.insert_directive_name_references(&mut schema.referencers, &directive.name)?;
        field.make_mut().directives.push(directive);
        Ok(())
    }

    pub(crate) fn remove_directive_name<Q: Hash + Equivalent<Name>>(
        &self,
        schema: &mut FederationSchema,
        name: &Q,
    ) {
        let Some(field) = self.try_make_mut(&mut schema.schema) else {
            return;
        };
        self.remove_directive_name_references(&mut schema.referencers, name);
        field
            .make_mut()
            .directives
            .retain(|other_directive| !name.equivalent(&other_directive.name));
    }

    pub(crate) fn remove_directive(
        &self,
        schema: &mut FederationSchema,
        directive: &Node<Directive>,
    ) {
        let Some(field) = self.try_make_mut(&mut schema.schema) else {
            return;
        };
        if !field.directives.iter().any(|other_directive| {
            (other_directive.name == directive.name) && !other_directive.ptr_eq(directive)
        }) {
            self.remove_directive_name_references(&mut schema.referencers, &directive.name);
        }
        field
            .make_mut()
            .directives
            .retain(|other_directive| !other_directive.ptr_eq(directive));
    }

    fn insert_references(
        &self,
        field: &Component<FieldDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        validate_node_directives(field.directives.deref())?;
        for directive_reference in field.directives.iter() {
            self.insert_directive_name_references(referencers, &directive_reference.name)?;
        }
        self.insert_type_references(field, schema, referencers)?;
        validate_arguments(&field.arguments)?;
        for argument in field.arguments.iter() {
            InterfaceFieldArgumentDefinitionPosition {
                type_name: self.type_name.clone(),
                field_name: self.field_name.clone(),
                argument_name: argument.name.clone(),
            }
            .insert_references(argument, schema, referencers)?;
        }
        Ok(())
    }

    fn remove_references(
        &self,
        field: &Component<FieldDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        for directive_reference in field.directives.iter() {
            self.remove_directive_name_references(referencers, &directive_reference.name);
        }
        self.remove_type_references(field, schema, referencers)?;
        for argument in field.arguments.iter() {
            InterfaceFieldArgumentDefinitionPosition {
                type_name: self.type_name.clone(),
                field_name: self.field_name.clone(),
                argument_name: argument.name.clone(),
            }
            .remove_references(argument, schema, referencers)?;
        }
        Ok(())
    }

    fn insert_directive_name_references(
        &self,
        referencers: &mut Referencers,
        name: &Name,
    ) -> Result<(), FederationError> {
        let directive_referencers = referencers.directives.get_mut(name).ok_or_else(|| {
            SingleFederationError::Internal {
                message: format!(
                    "Interface field \"{}\"'s directive application \"@{}\" does not refer to an existing directive.",
                    self,
                    name,
                ),
            }
        })?;
        directive_referencers.interface_fields.insert(self.clone());
        Ok(())
    }

    fn remove_directive_name_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(directive_referencers) = referencers.directives.get_mut(name) else {
            return;
        };
        directive_referencers.interface_fields.remove(self);
    }

    fn insert_type_references(
        &self,
        field: &Component<FieldDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        let output_type_reference = field.ty.inner_named_type();
        match schema.types.get(output_type_reference) {
            Some(ExtendedType::Scalar(_)) => {
                let scalar_type_referencers = referencers
                    .scalar_types
                    .get_mut(output_type_reference)
                    .ok_or_else(|| SingleFederationError::Internal {
                        message: format!(
                            "Schema missing referencers for type \"{}\"",
                            output_type_reference
                        ),
                    })?;
                scalar_type_referencers
                    .interface_fields
                    .insert(self.clone());
            }
            Some(ExtendedType::Object(_)) => {
                let object_type_referencers = referencers
                    .object_types
                    .get_mut(output_type_reference)
                    .ok_or_else(|| SingleFederationError::Internal {
                        message: format!(
                            "Schema missing referencers for type \"{}\"",
                            output_type_reference
                        ),
                    })?;
                object_type_referencers
                    .interface_fields
                    .insert(self.clone());
            }
            Some(ExtendedType::Interface(_)) => {
                let interface_type_referencers = referencers
                    .interface_types
                    .get_mut(output_type_reference)
                    .ok_or_else(|| SingleFederationError::Internal {
                        message: format!(
                            "Schema missing referencers for type \"{}\"",
                            output_type_reference
                        ),
                    })?;
                interface_type_referencers
                    .interface_fields
                    .insert(self.clone());
            }
            Some(ExtendedType::Union(_)) => {
                let union_type_referencers = referencers
                    .union_types
                    .get_mut(output_type_reference)
                    .ok_or_else(|| SingleFederationError::Internal {
                        message: format!(
                            "Schema missing referencers for type \"{}\"",
                            output_type_reference
                        ),
                    })?;
                union_type_referencers.interface_fields.insert(self.clone());
            }
            Some(ExtendedType::Enum(_)) => {
                let enum_type_referencers = referencers
                    .enum_types
                    .get_mut(output_type_reference)
                    .ok_or_else(|| SingleFederationError::Internal {
                    message: format!(
                        "Schema missing referencers for type \"{}\"",
                        output_type_reference
                    ),
                })?;
                enum_type_referencers.interface_fields.insert(self.clone());
            }
            _ => {
                return Err(
                    SingleFederationError::Internal {
                        message: format!(
                            "Interface field \"{}\"'s inner type \"{}\" does not refer to an existing output type.",
                            self,
                            output_type_reference.deref(),
                        )
                    }.into()
                );
            }
        }
        Ok(())
    }

    fn remove_type_references(
        &self,
        field: &Component<FieldDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        let output_type_reference = field.ty.inner_named_type();
        match schema.types.get(output_type_reference) {
            Some(ExtendedType::Scalar(_)) => {
                let Some(scalar_type_referencers) =
                    referencers.scalar_types.get_mut(output_type_reference)
                else {
                    return Ok(());
                };
                scalar_type_referencers.interface_fields.remove(self);
            }
            Some(ExtendedType::Object(_)) => {
                let Some(object_type_referencers) =
                    referencers.object_types.get_mut(output_type_reference)
                else {
                    return Ok(());
                };
                object_type_referencers.interface_fields.remove(self);
            }
            Some(ExtendedType::Interface(_)) => {
                let Some(interface_type_referencers) =
                    referencers.interface_types.get_mut(output_type_reference)
                else {
                    return Ok(());
                };
                interface_type_referencers.interface_fields.remove(self);
            }
            Some(ExtendedType::Union(_)) => {
                let Some(union_type_referencers) =
                    referencers.union_types.get_mut(output_type_reference)
                else {
                    return Ok(());
                };
                union_type_referencers.interface_fields.remove(self);
            }
            Some(ExtendedType::Enum(_)) => {
                let Some(enum_type_referencers) =
                    referencers.enum_types.get_mut(output_type_reference)
                else {
                    return Ok(());
                };
                enum_type_referencers.interface_fields.remove(self);
            }
            _ => {
                return Err(
                    SingleFederationError::Internal {
                        message: format!(
                            "Interface field \"{}\"'s inner type \"{}\" does not refer to an existing output type.",
                            self,
                            output_type_reference.deref(),
                        )
                    }.into()
                );
            }
        }
        Ok(())
    }
}

impl Display for InterfaceFieldDefinitionPosition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.type_name, self.field_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct InterfaceFieldArgumentDefinitionPosition {
    pub(crate) type_name: Name,
    pub(crate) field_name: Name,
    pub(crate) argument_name: Name,
}

impl InterfaceFieldArgumentDefinitionPosition {
    pub(crate) fn parent(&self) -> InterfaceFieldDefinitionPosition {
        InterfaceFieldDefinitionPosition {
            type_name: self.type_name.clone(),
            field_name: self.field_name.clone(),
        }
    }

    pub(crate) fn get<'schema>(
        &self,
        schema: &'schema Schema,
    ) -> Result<&'schema Node<InputValueDefinition>, FederationError> {
        let parent = self.parent();
        let type_ = parent.get(schema)?;

        type_
            .arguments
            .iter()
            .find(|a| a.name == self.argument_name)
            .ok_or_else(|| {
                SingleFederationError::Internal {
                    message: format!(
                        "Interface field \"{}\" has no argument \"{}\"",
                        parent, self.argument_name
                    ),
                }
                .into()
            })
    }

    pub(crate) fn try_get<'schema>(
        &self,
        schema: &'schema Schema,
    ) -> Option<&'schema Node<InputValueDefinition>> {
        self.get(schema).ok()
    }

    fn make_mut<'schema>(
        &self,
        schema: &'schema mut Schema,
    ) -> Result<&'schema mut Node<InputValueDefinition>, FederationError> {
        let parent = self.parent();
        let type_ = parent.make_mut(schema)?.make_mut();

        type_
            .arguments
            .iter_mut()
            .find(|a| a.name == self.argument_name)
            .ok_or_else(|| {
                SingleFederationError::Internal {
                    message: format!(
                        "Interface field \"{}\" has no argument \"{}\"",
                        parent, self.argument_name
                    ),
                }
                .into()
            })
    }

    fn try_make_mut<'schema>(
        &self,
        schema: &'schema mut Schema,
    ) -> Option<&'schema mut Node<InputValueDefinition>> {
        if self.try_get(schema).is_some() {
            self.make_mut(schema).ok()
        } else {
            None
        }
    }

    pub(crate) fn insert(
        &self,
        schema: &mut FederationSchema,
        argument: Node<InputValueDefinition>,
    ) -> Result<(), FederationError> {
        if self.argument_name != argument.name {
            return Err(SingleFederationError::Internal {
                message: format!(
                    "Interface field argument \"{}\" given argument named \"{}\"",
                    self, argument.name,
                ),
            }
            .into());
        }
        if self.try_get(&schema.schema).is_some() {
            // TODO: Handle old spec edge case of arguments with non-unique names
            return Err(SingleFederationError::Internal {
                message: format!(
                    "Interface field argument \"{}\" already exists in schema",
                    self,
                ),
            }
            .into());
        }
        self.insert_references(&argument, &schema.schema, &mut schema.referencers)?;
        self.parent()
            .make_mut(&mut schema.schema)?
            .make_mut()
            .arguments
            .push(argument);
        Ok(())
    }
    pub(crate) fn remove(&self, schema: &mut FederationSchema) -> Result<(), FederationError> {
        let Some(argument) = self.try_get(&schema.schema) else {
            return Ok(());
        };
        self.remove_references(argument, &schema.schema, &mut schema.referencers)?;
        self.parent()
            .make_mut(&mut schema.schema)?
            .make_mut()
            .arguments
            .retain(|other_argument| other_argument.name != self.argument_name);
        Ok(())
    }

    pub(crate) fn insert_directive(
        &self,
        schema: &mut FederationSchema,
        directive: Node<Directive>,
    ) -> Result<(), FederationError> {
        let argument = self.make_mut(&mut schema.schema)?;
        if argument
            .directives
            .iter()
            .any(|other_directive| other_directive.ptr_eq(&directive))
        {
            return Err(
                SingleFederationError::Internal {
                    message: format!(
                        "Directive application \"@{}\" already exists on interface field argument \"{}\"",
                        directive.name,
                        self,
                    )
                }.into()
            );
        }
        self.insert_directive_name_references(&mut schema.referencers, &directive.name)?;
        argument.make_mut().directives.push(directive);
        Ok(())
    }

    pub(crate) fn remove_directive_name<Q: Hash + Equivalent<Name>>(
        &self,
        schema: &mut FederationSchema,
        name: &Q,
    ) {
        let Some(argument) = self.try_make_mut(&mut schema.schema) else {
            return;
        };
        self.remove_directive_name_references(&mut schema.referencers, name);
        argument
            .make_mut()
            .directives
            .retain(|other_directive| !name.equivalent(&other_directive.name));
    }

    pub(crate) fn remove_directive(
        &self,
        schema: &mut FederationSchema,
        directive: &Node<Directive>,
    ) {
        let Some(argument) = self.try_make_mut(&mut schema.schema) else {
            return;
        };
        if !argument.directives.iter().any(|other_directive| {
            (other_directive.name == directive.name) && !other_directive.ptr_eq(directive)
        }) {
            self.remove_directive_name_references(&mut schema.referencers, &directive.name);
        }
        argument
            .make_mut()
            .directives
            .retain(|other_directive| !other_directive.ptr_eq(directive));
    }

    fn insert_references(
        &self,
        argument: &Node<InputValueDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        validate_node_directives(argument.directives.deref())?;
        for directive_reference in argument.directives.iter() {
            self.insert_directive_name_references(referencers, &directive_reference.name)?;
        }
        self.insert_type_references(argument, schema, referencers)
    }

    fn remove_references(
        &self,
        argument: &Node<InputValueDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        for directive_reference in argument.directives.iter() {
            self.remove_directive_name_references(referencers, &directive_reference.name);
        }
        self.remove_type_references(argument, schema, referencers)
    }

    fn insert_directive_name_references(
        &self,
        referencers: &mut Referencers,
        name: &Name,
    ) -> Result<(), FederationError> {
        let directive_referencers = referencers.directives.get_mut(name).ok_or_else(|| {
            SingleFederationError::Internal {
                message: format!(
                    "Interface field argument \"{}\"'s directive application \"@{}\" does not refer to an existing directive.",
                    self,
                    name,
                ),
            }
        })?;
        directive_referencers
            .interface_field_arguments
            .insert(self.clone());
        Ok(())
    }

    fn remove_directive_name_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(directive_referencers) = referencers.directives.get_mut(name) else {
            return;
        };
        directive_referencers.interface_field_arguments.remove(self);
    }

    fn insert_type_references(
        &self,
        argument: &Node<InputValueDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        let input_type_reference = argument.ty.inner_named_type();
        match schema.types.get(input_type_reference) {
            Some(ExtendedType::Scalar(_)) => {
                let scalar_type_referencers = referencers
                    .scalar_types
                    .get_mut(input_type_reference)
                    .ok_or_else(|| SingleFederationError::Internal {
                        message: format!(
                            "Schema missing referencers for type \"{}\"",
                            input_type_reference
                        ),
                    })?;
                scalar_type_referencers
                    .interface_field_arguments
                    .insert(self.clone());
            }
            Some(ExtendedType::Enum(_)) => {
                let enum_type_referencers = referencers
                    .enum_types
                    .get_mut(input_type_reference)
                    .ok_or_else(|| SingleFederationError::Internal {
                        message: format!(
                            "Schema missing referencers for type \"{}\"",
                            input_type_reference
                        ),
                    })?;
                enum_type_referencers
                    .interface_field_arguments
                    .insert(self.clone());
            }
            Some(ExtendedType::InputObject(_)) => {
                let input_object_type_referencers = referencers
                    .input_object_types
                    .get_mut(input_type_reference)
                    .ok_or_else(|| SingleFederationError::Internal {
                        message: format!(
                            "Schema missing referencers for type \"{}\"",
                            input_type_reference
                        ),
                    })?;
                input_object_type_referencers
                    .interface_field_arguments
                    .insert(self.clone());
            }
            _ => {
                return Err(
                    SingleFederationError::Internal {
                        message: format!(
                            "Interface field argument \"{}\"'s inner type \"{}\" does not refer to an existing input type.",
                            self,
                            input_type_reference.deref(),
                        )
                    }.into()
                );
            }
        }
        Ok(())
    }

    fn remove_type_references(
        &self,
        argument: &Node<InputValueDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        let input_type_reference = argument.ty.inner_named_type();
        match schema.types.get(input_type_reference) {
            Some(ExtendedType::Scalar(_)) => {
                let Some(scalar_type_referencers) =
                    referencers.scalar_types.get_mut(input_type_reference)
                else {
                    return Ok(());
                };
                scalar_type_referencers
                    .interface_field_arguments
                    .remove(self);
            }
            Some(ExtendedType::Enum(_)) => {
                let Some(enum_type_referencers) =
                    referencers.enum_types.get_mut(input_type_reference)
                else {
                    return Ok(());
                };
                enum_type_referencers.interface_field_arguments.remove(self);
            }
            Some(ExtendedType::InputObject(_)) => {
                let Some(input_object_type_referencers) =
                    referencers.input_object_types.get_mut(input_type_reference)
                else {
                    return Ok(());
                };
                input_object_type_referencers
                    .interface_field_arguments
                    .remove(self);
            }
            _ => {
                return Err(
                    SingleFederationError::Internal {
                        message: format!(
                            "Interface field argument \"{}\"'s inner type \"{}\" does not refer to an existing input type.",
                            self,
                            input_type_reference.deref(),
                        )
                    }.into()
                );
            }
        }
        Ok(())
    }
}

impl Display for InterfaceFieldArgumentDefinitionPosition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}.{}({}:)",
            self.type_name, self.field_name, self.argument_name
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct UnionTypeDefinitionPosition {
    pub(crate) type_name: Name,
}

impl UnionTypeDefinitionPosition {
    pub(crate) fn get<'schema>(
        &self,
        schema: &'schema Schema,
    ) -> Result<&'schema Node<UnionType>, FederationError> {
        schema
            .types
            .get(&self.type_name)
            .ok_or_else(|| {
                SingleFederationError::Internal {
                    message: format!("Schema has no type \"{}\"", self),
                }
                .into()
            })
            .and_then(|type_| {
                if let ExtendedType::Union(type_) = type_ {
                    Ok(type_)
                } else {
                    Err(SingleFederationError::Internal {
                        message: format!("Schema type \"{}\" was not an union", self),
                    }
                    .into())
                }
            })
    }

    pub(crate) fn try_get<'schema>(
        &self,
        schema: &'schema Schema,
    ) -> Option<&'schema Node<UnionType>> {
        self.get(schema).ok()
    }

    fn make_mut<'schema>(
        &self,
        schema: &'schema mut Schema,
    ) -> Result<&'schema mut Node<UnionType>, FederationError> {
        schema
            .types
            .get_mut(&self.type_name)
            .ok_or_else(|| {
                SingleFederationError::Internal {
                    message: format!("Schema has no type \"{}\"", self),
                }
                .into()
            })
            .and_then(|type_| {
                if let ExtendedType::Union(type_) = type_ {
                    Ok(type_)
                } else {
                    Err(SingleFederationError::Internal {
                        message: format!("Schema type \"{}\" was not an union", self),
                    }
                    .into())
                }
            })
    }

    fn try_make_mut<'schema>(
        &self,
        schema: &'schema mut Schema,
    ) -> Option<&'schema mut Node<UnionType>> {
        if self.try_get(schema).is_some() {
            self.make_mut(schema).ok()
        } else {
            None
        }
    }

    pub(crate) fn pre_insert(&self, schema: &mut FederationSchema) -> Result<(), FederationError> {
        if schema.referencers.contains_type_name(&self.type_name) {
            // TODO: Allow built-in shadowing instead of ignoring them
            if is_graphql_reserved_name(&self.type_name) {
                return Ok(());
            }
            return Err(SingleFederationError::Internal {
                message: format!("Type \"{}\" has already been pre-inserted", self),
            }
            .into());
        }
        schema
            .referencers
            .union_types
            .insert(self.type_name.clone(), Default::default());
        Ok(())
    }

    pub(crate) fn insert(
        &self,
        schema: &mut FederationSchema,
        type_: Node<UnionType>,
    ) -> Result<(), FederationError> {
        if !schema.referencers.union_types.contains_key(&self.type_name) {
            return Err(SingleFederationError::Internal {
                message: format!("Type \"{}\" has not been pre-inserted", self),
            }
            .into());
        }
        if schema.schema.types.contains_key(&self.type_name) {
            // TODO: Allow built-in shadowing instead of ignoring them
            if is_graphql_reserved_name(&self.type_name) {
                return Ok(());
            }
            return Err(SingleFederationError::Internal {
                message: format!("Type \"{}\" already exists in schema", self),
            }
            .into());
        }
        self.insert_references(&type_, &mut schema.referencers)?;
        schema
            .schema
            .types
            .insert(self.type_name.clone(), ExtendedType::Union(type_));
        Ok(())
    }

    pub(crate) fn remove(
        &self,
        schema: &mut FederationSchema,
    ) -> Result<Option<UnionTypeReferencers>, FederationError> {
        let Some(referencers) = self.remove_internal(schema)? else {
            return Ok(None);
        };
        for field in &referencers.object_fields {
            field.remove(schema)?;
        }
        for field in &referencers.interface_fields {
            field.remove(schema)?;
        }
        Ok(Some(referencers))
    }

    pub(crate) fn remove_recursive(
        &self,
        schema: &mut FederationSchema,
    ) -> Result<(), FederationError> {
        let Some(referencers) = self.remove_internal(schema)? else {
            return Ok(());
        };
        for field in referencers.object_fields {
            field.remove_recursive(schema)?;
        }
        for field in referencers.interface_fields {
            field.remove_recursive(schema)?;
        }
        Ok(())
    }

    fn remove_internal(
        &self,
        schema: &mut FederationSchema,
    ) -> Result<Option<UnionTypeReferencers>, FederationError> {
        let Some(type_) = self.try_get(&schema.schema) else {
            return Ok(None);
        };
        self.remove_references(type_, &mut schema.referencers);
        schema.schema.types.remove(&self.type_name);
        Ok(Some(
            schema
                .referencers
                .union_types
                .remove(&self.type_name)
                .ok_or_else(|| SingleFederationError::Internal {
                    message: format!("Schema missing referencers for type \"{}\"", self),
                })?,
        ))
    }

    pub(crate) fn insert_directive(
        &self,
        schema: &mut FederationSchema,
        directive: Component<Directive>,
    ) -> Result<(), FederationError> {
        let type_ = self.make_mut(&mut schema.schema)?;
        if type_
            .directives
            .iter()
            .any(|other_directive| other_directive.ptr_eq(&directive))
        {
            return Err(SingleFederationError::Internal {
                message: format!(
                    "Directive application \"@{}\" already exists on union type \"{}\"",
                    directive.name, self,
                ),
            }
            .into());
        }
        self.insert_directive_name_references(&mut schema.referencers, &directive.name)?;
        type_.make_mut().directives.push(directive);
        Ok(())
    }

    pub(crate) fn remove_directive_name<Q: Hash + Equivalent<Name>>(
        &self,
        schema: &mut FederationSchema,
        name: &Q,
    ) {
        let Some(type_) = self.try_make_mut(&mut schema.schema) else {
            return;
        };
        self.remove_directive_name_references(&mut schema.referencers, name);
        type_
            .make_mut()
            .directives
            .retain(|other_directive| !name.equivalent(&other_directive.name));
    }

    pub(crate) fn remove_directive(
        &self,
        schema: &mut FederationSchema,
        directive: &Component<Directive>,
    ) {
        let Some(type_) = self.try_make_mut(&mut schema.schema) else {
            return;
        };
        if !type_.directives.iter().any(|other_directive| {
            (other_directive.name == directive.name) && !other_directive.ptr_eq(directive)
        }) {
            self.remove_directive_name_references(&mut schema.referencers, &directive.name);
        }
        type_
            .make_mut()
            .directives
            .retain(|other_directive| !other_directive.ptr_eq(directive));
    }

    pub(crate) fn insert_member(
        &self,
        schema: &mut FederationSchema,
        name: Name,
    ) -> Result<(), FederationError> {
        let type_ = self.make_mut(&mut schema.schema)?;
        self.insert_member_references(&mut schema.referencers, &name)?;
        type_.make_mut().members.insert(ComponentStr::new(&name));
        Ok(())
    }

    pub(crate) fn remove_member<Q: Hash + Equivalent<Name>>(
        &self,
        schema: &mut FederationSchema,
        name: &Q,
    ) {
        let Some(type_) = self.try_make_mut(&mut schema.schema) else {
            return;
        };
        self.remove_member_references(&mut schema.referencers, name);
        type_
            .make_mut()
            .members
            .retain(|other_type| !name.equivalent(other_type.deref()));
    }

    pub(crate) fn remove_member_recursive<Q: Hash + Equivalent<Name>>(
        &self,
        schema: &mut FederationSchema,
        name: &Q,
    ) -> Result<(), FederationError> {
        self.remove_member(schema, name);
        let Some(type_) = self.try_get(&schema.schema) else {
            return Ok(());
        };
        if type_.members.is_empty() {
            self.remove_recursive(schema)?;
        }
        Ok(())
    }

    fn insert_references(
        &self,
        type_: &Node<UnionType>,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        validate_component_directives(type_.directives.deref())?;
        for directive_reference in type_.directives.iter() {
            self.insert_directive_name_references(referencers, &directive_reference.name)?;
        }
        for object_type_reference in type_.members.iter() {
            self.insert_member_references(referencers, object_type_reference.deref())?;
        }
        Ok(())
    }

    fn remove_references(&self, type_: &Node<UnionType>, referencers: &mut Referencers) {
        for directive_reference in type_.directives.iter() {
            self.remove_directive_name_references(referencers, &directive_reference.name);
        }
        for object_type_reference in type_.members.iter() {
            self.remove_member_references(referencers, object_type_reference.deref());
        }
    }

    fn insert_directive_name_references(
        &self,
        referencers: &mut Referencers,
        name: &Name,
    ) -> Result<(), FederationError> {
        let directive_referencers = referencers.directives.get_mut(name).ok_or_else(|| {
            SingleFederationError::Internal {
                message: format!(
                    "Union type \"{}\"'s directive application \"@{}\" does not refer to an existing directive.",
                    self,
                    name,
                ),
            }
        })?;
        directive_referencers.union_types.insert(self.clone());
        Ok(())
    }

    fn remove_directive_name_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(directive_referencers) = referencers.directives.get_mut(name) else {
            return;
        };
        directive_referencers.union_types.remove(self);
    }

    fn insert_member_references(
        &self,
        referencers: &mut Referencers,
        name: &Name,
    ) -> Result<(), FederationError> {
        let object_type_referencers = referencers.object_types.get_mut(name).ok_or_else(|| {
            SingleFederationError::Internal {
                message: format!(
                    "Union type \"{}\"'s member \"{}\" does not refer to an existing object.",
                    self, name,
                ),
            }
        })?;
        object_type_referencers.union_types.insert(self.clone());
        Ok(())
    }

    fn remove_member_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(object_type_referencers) = referencers.object_types.get_mut(name) else {
            return;
        };
        object_type_referencers.union_types.remove(self);
    }
}

impl Display for UnionTypeDefinitionPosition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.type_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct EnumTypeDefinitionPosition {
    pub(crate) type_name: Name,
}

impl EnumTypeDefinitionPosition {
    pub(crate) fn get<'schema>(
        &self,
        schema: &'schema Schema,
    ) -> Result<&'schema Node<EnumType>, FederationError> {
        schema
            .types
            .get(&self.type_name)
            .ok_or_else(|| {
                SingleFederationError::Internal {
                    message: format!("Schema has no type \"{}\"", self),
                }
                .into()
            })
            .and_then(|type_| {
                if let ExtendedType::Enum(type_) = type_ {
                    Ok(type_)
                } else {
                    Err(SingleFederationError::Internal {
                        message: format!("Schema type \"{}\" was not an enum", self),
                    }
                    .into())
                }
            })
    }

    pub(crate) fn try_get<'schema>(
        &self,
        schema: &'schema Schema,
    ) -> Option<&'schema Node<EnumType>> {
        self.get(schema).ok()
    }

    fn make_mut<'schema>(
        &self,
        schema: &'schema mut Schema,
    ) -> Result<&'schema mut Node<EnumType>, FederationError> {
        schema
            .types
            .get_mut(&self.type_name)
            .ok_or_else(|| {
                SingleFederationError::Internal {
                    message: format!("Schema has no type \"{}\"", self),
                }
                .into()
            })
            .and_then(|type_| {
                if let ExtendedType::Enum(type_) = type_ {
                    Ok(type_)
                } else {
                    Err(SingleFederationError::Internal {
                        message: format!("Schema type \"{}\" was not an enum", self),
                    }
                    .into())
                }
            })
    }

    fn try_make_mut<'schema>(
        &self,
        schema: &'schema mut Schema,
    ) -> Option<&'schema mut Node<EnumType>> {
        if self.try_get(schema).is_some() {
            self.make_mut(schema).ok()
        } else {
            None
        }
    }

    pub(crate) fn pre_insert(&self, schema: &mut FederationSchema) -> Result<(), FederationError> {
        if schema.referencers.contains_type_name(&self.type_name) {
            // TODO: Allow built-in shadowing instead of ignoring them
            if is_graphql_reserved_name(&self.type_name) {
                return Ok(());
            }
            return Err(SingleFederationError::Internal {
                message: format!("Type \"{}\" has already been pre-inserted", self),
            }
            .into());
        }
        schema
            .referencers
            .enum_types
            .insert(self.type_name.clone(), Default::default());
        Ok(())
    }

    pub(crate) fn insert(
        &self,
        schema: &mut FederationSchema,
        type_: Node<EnumType>,
    ) -> Result<(), FederationError> {
        if !schema.referencers.enum_types.contains_key(&self.type_name) {
            return Err(SingleFederationError::Internal {
                message: format!("Type \"{}\" has not been pre-inserted", self),
            }
            .into());
        }
        if schema.schema.types.contains_key(&self.type_name) {
            // TODO: Allow built-in shadowing instead of ignoring them
            if is_graphql_reserved_name(&self.type_name) {
                return Ok(());
            }
            return Err(SingleFederationError::Internal {
                message: format!("Type \"{}\" already exists in schema", self),
            }
            .into());
        }
        self.insert_references(&type_, &mut schema.referencers)?;
        schema
            .schema
            .types
            .insert(self.type_name.clone(), ExtendedType::Enum(type_));
        Ok(())
    }

    pub(crate) fn remove(
        &self,
        schema: &mut FederationSchema,
    ) -> Result<Option<EnumTypeReferencers>, FederationError> {
        let Some(referencers) = self.remove_internal(schema)? else {
            return Ok(None);
        };
        for field in &referencers.object_fields {
            field.remove(schema)?;
        }
        for argument in &referencers.object_field_arguments {
            argument.remove(schema)?;
        }
        for field in &referencers.interface_fields {
            field.remove(schema)?;
        }
        for argument in &referencers.interface_field_arguments {
            argument.remove(schema)?;
        }
        for field in &referencers.input_object_fields {
            field.remove(schema)?;
        }
        for argument in &referencers.directive_arguments {
            argument.remove(schema)?;
        }
        Ok(Some(referencers))
    }

    pub(crate) fn remove_recursive(
        &self,
        schema: &mut FederationSchema,
    ) -> Result<(), FederationError> {
        let Some(referencers) = self.remove_internal(schema)? else {
            return Ok(());
        };
        for field in referencers.object_fields {
            field.remove_recursive(schema)?;
        }
        for argument in referencers.object_field_arguments {
            argument.remove(schema)?;
        }
        for field in referencers.interface_fields {
            field.remove_recursive(schema)?;
        }
        for argument in referencers.interface_field_arguments {
            argument.remove(schema)?;
        }
        for field in referencers.input_object_fields {
            field.remove_recursive(schema)?;
        }
        for argument in referencers.directive_arguments {
            argument.remove(schema)?;
        }
        Ok(())
    }

    fn remove_internal(
        &self,
        schema: &mut FederationSchema,
    ) -> Result<Option<EnumTypeReferencers>, FederationError> {
        let Some(type_) = self.try_get(&schema.schema) else {
            return Ok(None);
        };
        self.remove_references(type_, &mut schema.referencers);
        schema.schema.types.remove(&self.type_name);
        Ok(Some(
            schema
                .referencers
                .enum_types
                .remove(&self.type_name)
                .ok_or_else(|| SingleFederationError::Internal {
                    message: format!("Schema missing referencers for type \"{}\"", self),
                })?,
        ))
    }

    pub(crate) fn insert_directive(
        &self,
        schema: &mut FederationSchema,
        directive: Component<Directive>,
    ) -> Result<(), FederationError> {
        let type_ = self.make_mut(&mut schema.schema)?;
        if type_
            .directives
            .iter()
            .any(|other_directive| other_directive.ptr_eq(&directive))
        {
            return Err(SingleFederationError::Internal {
                message: format!(
                    "Directive application \"@{}\" already exists on enum type \"{}\"",
                    directive.name, self,
                ),
            }
            .into());
        }
        self.insert_directive_name_references(&mut schema.referencers, &directive.name)?;
        type_.make_mut().directives.push(directive);
        Ok(())
    }

    pub(crate) fn remove_directive_name<Q: Hash + Equivalent<Name>>(
        &self,
        schema: &mut FederationSchema,
        name: &Q,
    ) {
        let Some(type_) = self.try_make_mut(&mut schema.schema) else {
            return;
        };
        self.remove_directive_name_references(&mut schema.referencers, name);
        type_
            .make_mut()
            .directives
            .retain(|other_directive| !name.equivalent(&other_directive.name));
    }

    pub(crate) fn remove_directive(
        &self,
        schema: &mut FederationSchema,
        directive: &Component<Directive>,
    ) {
        let Some(type_) = self.try_make_mut(&mut schema.schema) else {
            return;
        };
        if !type_.directives.iter().any(|other_directive| {
            (other_directive.name == directive.name) && !other_directive.ptr_eq(directive)
        }) {
            self.remove_directive_name_references(&mut schema.referencers, &directive.name);
        }
        type_
            .make_mut()
            .directives
            .retain(|other_directive| !other_directive.ptr_eq(directive));
    }

    fn insert_references(
        &self,
        type_: &Node<EnumType>,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        validate_component_directives(type_.directives.deref())?;
        for directive_reference in type_.directives.iter() {
            self.insert_directive_name_references(referencers, &directive_reference.name)?;
        }
        for (value_name, value) in type_.values.iter() {
            EnumValueDefinitionPosition {
                type_name: self.type_name.clone(),
                value_name: value_name.clone(),
            }
            .insert_references(value, referencers)?;
        }
        Ok(())
    }

    fn remove_references(&self, type_: &Node<EnumType>, referencers: &mut Referencers) {
        for directive_reference in type_.directives.iter() {
            self.remove_directive_name_references(referencers, &directive_reference.name);
        }
        for (value_name, value) in type_.values.iter() {
            EnumValueDefinitionPosition {
                type_name: self.type_name.clone(),
                value_name: value_name.clone(),
            }
            .remove_references(value, referencers);
        }
    }

    fn insert_directive_name_references(
        &self,
        referencers: &mut Referencers,
        name: &Name,
    ) -> Result<(), FederationError> {
        let directive_referencers = referencers.directives.get_mut(name).ok_or_else(|| {
            SingleFederationError::Internal {
                message: format!(
                    "Enum type \"{}\"'s directive application \"@{}\" does not refer to an existing directive.",
                    self,
                    name,
                ),
            }
        })?;
        directive_referencers.enum_types.insert(self.clone());
        Ok(())
    }

    fn remove_directive_name_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(directive_referencers) = referencers.directives.get_mut(name) else {
            return;
        };
        directive_referencers.enum_types.remove(self);
    }
}

impl Display for EnumTypeDefinitionPosition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.type_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct EnumValueDefinitionPosition {
    pub(crate) type_name: Name,
    pub(crate) value_name: Name,
}

impl EnumValueDefinitionPosition {
    pub(crate) fn parent(&self) -> EnumTypeDefinitionPosition {
        EnumTypeDefinitionPosition {
            type_name: self.type_name.clone(),
        }
    }

    pub(crate) fn get<'schema>(
        &self,
        schema: &'schema Schema,
    ) -> Result<&'schema Component<EnumValueDefinition>, FederationError> {
        let parent = self.parent();
        let type_ = parent.get(schema)?;

        type_.values.get(&self.value_name).ok_or_else(|| {
            SingleFederationError::Internal {
                message: format!(
                    "Enum type \"{}\" has no value \"{}\"",
                    parent, self.value_name
                ),
            }
            .into()
        })
    }

    pub(crate) fn try_get<'schema>(
        &self,
        schema: &'schema Schema,
    ) -> Option<&'schema Component<EnumValueDefinition>> {
        self.get(schema).ok()
    }

    fn make_mut<'schema>(
        &self,
        schema: &'schema mut Schema,
    ) -> Result<&'schema mut Component<EnumValueDefinition>, FederationError> {
        let parent = self.parent();
        let type_ = parent.make_mut(schema)?.make_mut();

        type_.values.get_mut(&self.value_name).ok_or_else(|| {
            SingleFederationError::Internal {
                message: format!(
                    "Enum type \"{}\" has no value \"{}\"",
                    parent, self.value_name
                ),
            }
            .into()
        })
    }

    fn try_make_mut<'schema>(
        &self,
        schema: &'schema mut Schema,
    ) -> Option<&'schema mut Component<EnumValueDefinition>> {
        if self.try_get(schema).is_some() {
            self.make_mut(schema).ok()
        } else {
            None
        }
    }

    pub(crate) fn insert(
        &self,
        schema: &mut FederationSchema,
        value: Component<EnumValueDefinition>,
    ) -> Result<(), FederationError> {
        if self.value_name != value.value {
            return Err(SingleFederationError::Internal {
                message: format!(
                    "Enum value \"{}\" given argument named \"{}\"",
                    self, value.value,
                ),
            }
            .into());
        }
        if self.try_get(&schema.schema).is_some() {
            return Err(SingleFederationError::Internal {
                message: format!("Enum value \"{}\" already exists in schema", self,),
            }
            .into());
        }
        self.insert_references(&value, &mut schema.referencers)?;
        self.parent()
            .make_mut(&mut schema.schema)?
            .make_mut()
            .values
            .insert(self.value_name.clone(), value);
        Ok(())
    }

    pub(crate) fn remove(&self, schema: &mut FederationSchema) -> Result<(), FederationError> {
        let Some(value) = self.try_get(&schema.schema) else {
            return Ok(());
        };
        self.remove_references(value, &mut schema.referencers);
        self.parent()
            .make_mut(&mut schema.schema)?
            .make_mut()
            .values
            .remove(&self.value_name);
        Ok(())
    }

    pub(crate) fn remove_recursive(
        &self,
        schema: &mut FederationSchema,
    ) -> Result<(), FederationError> {
        self.remove(schema)?;
        let parent = self.parent();
        let Some(type_) = parent.try_get(&schema.schema) else {
            return Ok(());
        };
        if type_.values.is_empty() {
            parent.remove_recursive(schema)?;
        }
        Ok(())
    }

    pub(crate) fn insert_directive(
        &self,
        schema: &mut FederationSchema,
        directive: Node<Directive>,
    ) -> Result<(), FederationError> {
        let value = self.make_mut(&mut schema.schema)?;
        if value
            .directives
            .iter()
            .any(|other_directive| other_directive.ptr_eq(&directive))
        {
            return Err(SingleFederationError::Internal {
                message: format!(
                    "Directive application \"@{}\" already exists on enum value \"{}\"",
                    directive.name, self,
                ),
            }
            .into());
        }
        self.insert_directive_name_references(&mut schema.referencers, &directive.name)?;
        value.make_mut().directives.push(directive);
        Ok(())
    }

    pub(crate) fn remove_directive_name<Q: Hash + Equivalent<Name>>(
        &self,
        schema: &mut FederationSchema,
        name: &Q,
    ) {
        let Some(value) = self.try_make_mut(&mut schema.schema) else {
            return;
        };
        self.remove_directive_name_references(&mut schema.referencers, name);
        value
            .make_mut()
            .directives
            .retain(|other_directive| !name.equivalent(&other_directive.name));
    }

    pub(crate) fn remove_directive(
        &self,
        schema: &mut FederationSchema,
        directive: &Node<Directive>,
    ) {
        let Some(value) = self.try_make_mut(&mut schema.schema) else {
            return;
        };
        if !value.directives.iter().any(|other_directive| {
            (other_directive.name == directive.name) && !other_directive.ptr_eq(directive)
        }) {
            self.remove_directive_name_references(&mut schema.referencers, &directive.name);
        }
        value
            .make_mut()
            .directives
            .retain(|other_directive| !other_directive.ptr_eq(directive));
    }

    fn insert_references(
        &self,
        value: &Component<EnumValueDefinition>,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        validate_node_directives(value.directives.deref())?;
        for directive_reference in value.directives.iter() {
            self.insert_directive_name_references(referencers, &directive_reference.name)?;
        }
        Ok(())
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

    fn insert_directive_name_references(
        &self,
        referencers: &mut Referencers,
        name: &Name,
    ) -> Result<(), FederationError> {
        let directive_referencers = referencers.directives.get_mut(name).ok_or_else(|| {
            SingleFederationError::Internal {
                message: format!(
                    "Enum value \"{}\"'s directive application \"@{}\" does not refer to an existing directive.",
                    self,
                    name,
                ),
            }
        })?;
        directive_referencers.enum_values.insert(self.clone());
        Ok(())
    }

    fn remove_directive_name_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(directive_referencers) = referencers.directives.get_mut(name) else {
            return;
        };
        directive_referencers.enum_values.remove(self);
    }
}

impl Display for EnumValueDefinitionPosition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.type_name, self.value_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct InputObjectTypeDefinitionPosition {
    pub(crate) type_name: Name,
}

impl InputObjectTypeDefinitionPosition {
    pub(crate) fn get<'schema>(
        &self,
        schema: &'schema Schema,
    ) -> Result<&'schema Node<InputObjectType>, FederationError> {
        schema
            .types
            .get(&self.type_name)
            .ok_or_else(|| {
                SingleFederationError::Internal {
                    message: format!("Schema has no type \"{}\"", self),
                }
                .into()
            })
            .and_then(|type_| {
                if let ExtendedType::InputObject(type_) = type_ {
                    Ok(type_)
                } else {
                    Err(SingleFederationError::Internal {
                        message: format!("Schema type \"{}\" was not an input object", self),
                    }
                    .into())
                }
            })
    }

    pub(crate) fn try_get<'schema>(
        &self,
        schema: &'schema Schema,
    ) -> Option<&'schema Node<InputObjectType>> {
        self.get(schema).ok()
    }

    fn make_mut<'schema>(
        &self,
        schema: &'schema mut Schema,
    ) -> Result<&'schema mut Node<InputObjectType>, FederationError> {
        schema
            .types
            .get_mut(&self.type_name)
            .ok_or_else(|| {
                SingleFederationError::Internal {
                    message: format!("Schema has no type \"{}\"", self),
                }
                .into()
            })
            .and_then(|type_| {
                if let ExtendedType::InputObject(type_) = type_ {
                    Ok(type_)
                } else {
                    Err(SingleFederationError::Internal {
                        message: format!("Schema type \"{}\" was not an input object", self),
                    }
                    .into())
                }
            })
    }

    fn try_make_mut<'schema>(
        &self,
        schema: &'schema mut Schema,
    ) -> Option<&'schema mut Node<InputObjectType>> {
        if self.try_get(schema).is_some() {
            self.make_mut(schema).ok()
        } else {
            None
        }
    }

    pub(crate) fn pre_insert(&self, schema: &mut FederationSchema) -> Result<(), FederationError> {
        if schema.referencers.contains_type_name(&self.type_name) {
            // TODO: Allow built-in shadowing instead of ignoring them
            if is_graphql_reserved_name(&self.type_name) {
                return Ok(());
            }
            return Err(SingleFederationError::Internal {
                message: format!("Type \"{}\" has already been pre-inserted", self),
            }
            .into());
        }
        schema
            .referencers
            .input_object_types
            .insert(self.type_name.clone(), Default::default());
        Ok(())
    }

    pub(crate) fn insert(
        &self,
        schema: &mut FederationSchema,
        type_: Node<InputObjectType>,
    ) -> Result<(), FederationError> {
        if !schema
            .referencers
            .input_object_types
            .contains_key(&self.type_name)
        {
            return Err(SingleFederationError::Internal {
                message: format!("Type \"{}\" has not been pre-inserted", self),
            }
            .into());
        }
        if schema.schema.types.contains_key(&self.type_name) {
            // TODO: Allow built-in shadowing instead of ignoring them
            if is_graphql_reserved_name(&self.type_name) {
                return Ok(());
            }
            return Err(SingleFederationError::Internal {
                message: format!("Type \"{}\" already exists in schema", self),
            }
            .into());
        }
        self.insert_references(&type_, &schema.schema, &mut schema.referencers)?;
        schema
            .schema
            .types
            .insert(self.type_name.clone(), ExtendedType::InputObject(type_));
        Ok(())
    }

    pub(crate) fn remove(
        &self,
        schema: &mut FederationSchema,
    ) -> Result<Option<InputObjectTypeReferencers>, FederationError> {
        let Some(referencers) = self.remove_internal(schema)? else {
            return Ok(None);
        };
        for argument in &referencers.object_field_arguments {
            argument.remove(schema)?;
        }
        for argument in &referencers.interface_field_arguments {
            argument.remove(schema)?;
        }
        for field in &referencers.input_object_fields {
            field.remove(schema)?;
        }
        for argument in &referencers.directive_arguments {
            argument.remove(schema)?;
        }
        Ok(Some(referencers))
    }

    pub(crate) fn remove_recursive(
        &self,
        schema: &mut FederationSchema,
    ) -> Result<(), FederationError> {
        let Some(referencers) = self.remove_internal(schema)? else {
            return Ok(());
        };
        for argument in referencers.object_field_arguments {
            argument.remove(schema)?;
        }
        for argument in referencers.interface_field_arguments {
            argument.remove(schema)?;
        }
        for field in referencers.input_object_fields {
            field.remove_recursive(schema)?;
        }
        for argument in referencers.directive_arguments {
            argument.remove(schema)?;
        }
        Ok(())
    }

    fn remove_internal(
        &self,
        schema: &mut FederationSchema,
    ) -> Result<Option<InputObjectTypeReferencers>, FederationError> {
        let Some(type_) = self.try_get(&schema.schema) else {
            return Ok(None);
        };
        self.remove_references(type_, &schema.schema, &mut schema.referencers)?;
        schema.schema.types.remove(&self.type_name);
        Ok(Some(
            schema
                .referencers
                .input_object_types
                .remove(&self.type_name)
                .ok_or_else(|| SingleFederationError::Internal {
                    message: format!("Schema missing referencers for type \"{}\"", self),
                })?,
        ))
    }

    pub(crate) fn insert_directive(
        &self,
        schema: &mut FederationSchema,
        directive: Component<Directive>,
    ) -> Result<(), FederationError> {
        let type_ = self.make_mut(&mut schema.schema)?;
        if type_
            .directives
            .iter()
            .any(|other_directive| other_directive.ptr_eq(&directive))
        {
            return Err(SingleFederationError::Internal {
                message: format!(
                    "Directive application \"@{}\" already exists on input object type \"{}\"",
                    directive.name, self,
                ),
            }
            .into());
        }
        self.insert_directive_name_references(&mut schema.referencers, &directive.name)?;
        type_.make_mut().directives.push(directive);
        Ok(())
    }

    pub(crate) fn remove_directive_name<Q: Hash + Equivalent<Name>>(
        &self,
        schema: &mut FederationSchema,
        name: &Q,
    ) {
        let Some(type_) = self.try_make_mut(&mut schema.schema) else {
            return;
        };
        self.remove_directive_name_references(&mut schema.referencers, name);
        type_
            .make_mut()
            .directives
            .retain(|other_directive| !name.equivalent(&other_directive.name));
    }

    pub(crate) fn remove_directive(
        &self,
        schema: &mut FederationSchema,
        directive: &Component<Directive>,
    ) {
        let Some(type_) = self.try_make_mut(&mut schema.schema) else {
            return;
        };
        if !type_.directives.iter().any(|other_directive| {
            (other_directive.name == directive.name) && !other_directive.ptr_eq(directive)
        }) {
            self.remove_directive_name_references(&mut schema.referencers, &directive.name);
        }
        type_
            .make_mut()
            .directives
            .retain(|other_directive| !other_directive.ptr_eq(directive));
    }

    fn insert_references(
        &self,
        type_: &Node<InputObjectType>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        validate_component_directives(type_.directives.deref())?;
        for directive_reference in type_.directives.iter() {
            self.insert_directive_name_references(referencers, &directive_reference.name)?;
        }
        for (field_name, field) in type_.fields.iter() {
            InputObjectFieldDefinitionPosition {
                type_name: self.type_name.clone(),
                field_name: field_name.clone(),
            }
            .insert_references(field, schema, referencers)?;
        }
        Ok(())
    }

    fn remove_references(
        &self,
        type_: &Node<InputObjectType>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        for directive_reference in type_.directives.iter() {
            self.remove_directive_name_references(referencers, &directive_reference.name);
        }
        for (field_name, field) in type_.fields.iter() {
            InputObjectFieldDefinitionPosition {
                type_name: self.type_name.clone(),
                field_name: field_name.clone(),
            }
            .remove_references(field, schema, referencers)?;
        }
        Ok(())
    }

    fn insert_directive_name_references(
        &self,
        referencers: &mut Referencers,
        name: &Name,
    ) -> Result<(), FederationError> {
        let directive_referencers = referencers.directives.get_mut(name).ok_or_else(|| {
            SingleFederationError::Internal {
                message: format!(
                    "Input object type \"{}\"'s directive application \"@{}\" does not refer to an existing directive.",
                    self,
                    name,
                ),
            }
        })?;
        directive_referencers
            .input_object_types
            .insert(self.clone());
        Ok(())
    }

    fn remove_directive_name_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(directive_referencers) = referencers.directives.get_mut(name) else {
            return;
        };
        directive_referencers.input_object_types.remove(self);
    }
}

impl Display for InputObjectTypeDefinitionPosition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.type_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct InputObjectFieldDefinitionPosition {
    pub(crate) type_name: Name,
    pub(crate) field_name: Name,
}

impl InputObjectFieldDefinitionPosition {
    pub(crate) fn parent(&self) -> InputObjectTypeDefinitionPosition {
        InputObjectTypeDefinitionPosition {
            type_name: self.type_name.clone(),
        }
    }

    pub(crate) fn get<'schema>(
        &self,
        schema: &'schema Schema,
    ) -> Result<&'schema Component<InputValueDefinition>, FederationError> {
        let parent = self.parent();
        let type_ = parent.get(schema)?;

        type_.fields.get(&self.field_name).ok_or_else(|| {
            SingleFederationError::Internal {
                message: format!(
                    "Input object type \"{}\" has no field \"{}\"",
                    parent, self.field_name
                ),
            }
            .into()
        })
    }

    pub(crate) fn try_get<'schema>(
        &self,
        schema: &'schema Schema,
    ) -> Option<&'schema Component<InputValueDefinition>> {
        self.get(schema).ok()
    }

    fn make_mut<'schema>(
        &self,
        schema: &'schema mut Schema,
    ) -> Result<&'schema mut Component<InputValueDefinition>, FederationError> {
        let parent = self.parent();
        let type_ = parent.make_mut(schema)?.make_mut();

        type_.fields.get_mut(&self.field_name).ok_or_else(|| {
            SingleFederationError::Internal {
                message: format!(
                    "Input object type \"{}\" has no field \"{}\"",
                    parent, self.field_name
                ),
            }
            .into()
        })
    }

    fn try_make_mut<'schema>(
        &self,
        schema: &'schema mut Schema,
    ) -> Option<&'schema mut Component<InputValueDefinition>> {
        if self.try_get(schema).is_some() {
            self.make_mut(schema).ok()
        } else {
            None
        }
    }

    pub(crate) fn insert(
        &self,
        schema: &mut FederationSchema,
        field: Component<InputValueDefinition>,
    ) -> Result<(), FederationError> {
        if self.field_name != field.name {
            return Err(SingleFederationError::Internal {
                message: format!(
                    "Input object field \"{}\" given field named \"{}\"",
                    self, field.name,
                ),
            }
            .into());
        }
        if self.try_get(&schema.schema).is_some() {
            return Err(SingleFederationError::Internal {
                message: format!("Input object field \"{}\" already exists in schema", self),
            }
            .into());
        }
        self.insert_references(&field, &schema.schema, &mut schema.referencers)?;
        self.parent()
            .make_mut(&mut schema.schema)?
            .make_mut()
            .fields
            .insert(self.field_name.clone(), field);
        Ok(())
    }

    pub(crate) fn remove(&self, schema: &mut FederationSchema) -> Result<(), FederationError> {
        let Some(field) = self.try_get(&schema.schema) else {
            return Ok(());
        };
        self.remove_references(field, &schema.schema, &mut schema.referencers)?;
        self.parent()
            .make_mut(&mut schema.schema)?
            .make_mut()
            .fields
            .remove(&self.field_name);
        Ok(())
    }

    pub(crate) fn remove_recursive(
        &self,
        schema: &mut FederationSchema,
    ) -> Result<(), FederationError> {
        self.remove(schema)?;
        let parent = self.parent();
        let Some(type_) = parent.try_get(&schema.schema) else {
            return Ok(());
        };
        if type_.fields.is_empty() {
            parent.remove_recursive(schema)?;
        }
        Ok(())
    }

    pub(crate) fn insert_directive(
        &self,
        schema: &mut FederationSchema,
        directive: Node<Directive>,
    ) -> Result<(), FederationError> {
        let field = self.make_mut(&mut schema.schema)?;
        if field
            .directives
            .iter()
            .any(|other_directive| other_directive.ptr_eq(&directive))
        {
            return Err(SingleFederationError::Internal {
                message: format!(
                    "Directive application \"@{}\" already exists on input object field \"{}\"",
                    directive.name, self,
                ),
            }
            .into());
        }
        self.insert_directive_name_references(&mut schema.referencers, &directive.name)?;
        field.make_mut().directives.push(directive);
        Ok(())
    }

    pub(crate) fn remove_directive_name<Q: Hash + Equivalent<Name>>(
        &self,
        schema: &mut FederationSchema,
        name: &Q,
    ) {
        let Some(field) = self.try_make_mut(&mut schema.schema) else {
            return;
        };
        self.remove_directive_name_references(&mut schema.referencers, name);
        field
            .make_mut()
            .directives
            .retain(|other_directive| !name.equivalent(&other_directive.name));
    }

    pub(crate) fn remove_directive(
        &self,
        schema: &mut FederationSchema,
        directive: &Node<Directive>,
    ) {
        let Some(field) = self.try_make_mut(&mut schema.schema) else {
            return;
        };
        if !field.directives.iter().any(|other_directive| {
            (other_directive.name == directive.name) && !other_directive.ptr_eq(directive)
        }) {
            self.remove_directive_name_references(&mut schema.referencers, &directive.name);
        }
        field
            .make_mut()
            .directives
            .retain(|other_directive| !other_directive.ptr_eq(directive));
    }

    fn insert_references(
        &self,
        field: &Component<InputValueDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        validate_node_directives(field.directives.deref())?;
        for directive_reference in field.directives.iter() {
            self.insert_directive_name_references(referencers, &directive_reference.name)?;
        }
        self.insert_type_references(field, schema, referencers)
    }

    fn remove_references(
        &self,
        field: &Component<InputValueDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        for directive_reference in field.directives.iter() {
            self.remove_directive_name_references(referencers, &directive_reference.name);
        }
        self.remove_type_references(field, schema, referencers)
    }

    fn insert_directive_name_references(
        &self,
        referencers: &mut Referencers,
        name: &Name,
    ) -> Result<(), FederationError> {
        let directive_referencers = referencers.directives.get_mut(name).ok_or_else(|| {
            SingleFederationError::Internal {
                message: format!(
                    "Input object field \"{}\"'s directive application \"@{}\" does not refer to an existing directive.",
                    self,
                    name,
                ),
            }
        })?;
        directive_referencers
            .input_object_fields
            .insert(self.clone());
        Ok(())
    }

    fn remove_directive_name_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(directive_referencers) = referencers.directives.get_mut(name) else {
            return;
        };
        directive_referencers.input_object_fields.remove(self);
    }

    fn insert_type_references(
        &self,
        argument: &Node<InputValueDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        let input_type_reference = argument.ty.inner_named_type();
        match schema.types.get(input_type_reference) {
            Some(ExtendedType::Scalar(_)) => {
                let scalar_type_referencers = referencers
                    .scalar_types
                    .get_mut(input_type_reference)
                    .ok_or_else(|| SingleFederationError::Internal {
                        message: format!(
                            "Schema missing referencers for type \"{}\"",
                            input_type_reference
                        ),
                    })?;
                scalar_type_referencers
                    .input_object_fields
                    .insert(self.clone());
            }
            Some(ExtendedType::Enum(_)) => {
                let enum_type_referencers = referencers
                    .enum_types
                    .get_mut(input_type_reference)
                    .ok_or_else(|| SingleFederationError::Internal {
                        message: format!(
                            "Schema missing referencers for type \"{}\"",
                            input_type_reference
                        ),
                    })?;
                enum_type_referencers
                    .input_object_fields
                    .insert(self.clone());
            }
            Some(ExtendedType::InputObject(_)) => {
                let input_object_type_referencers = referencers
                    .input_object_types
                    .get_mut(input_type_reference)
                    .ok_or_else(|| SingleFederationError::Internal {
                        message: format!(
                            "Schema missing referencers for type \"{}\"",
                            input_type_reference
                        ),
                    })?;
                input_object_type_referencers
                    .input_object_fields
                    .insert(self.clone());
            }
            _ => {
                return Err(
                    SingleFederationError::Internal {
                        message: format!(
                            "Input object field \"{}\"'s inner type \"{}\" does not refer to an existing input type.",
                            self,
                            input_type_reference.deref(),
                        )
                    }.into()
                );
            }
        }
        Ok(())
    }

    fn remove_type_references(
        &self,
        field: &Component<InputValueDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        let input_type_reference = field.ty.inner_named_type();
        match schema.types.get(input_type_reference) {
            Some(ExtendedType::Scalar(_)) => {
                let Some(scalar_type_referencers) =
                    referencers.scalar_types.get_mut(input_type_reference)
                else {
                    return Ok(());
                };
                scalar_type_referencers.input_object_fields.remove(self);
            }
            Some(ExtendedType::Enum(_)) => {
                let Some(enum_type_referencers) =
                    referencers.enum_types.get_mut(input_type_reference)
                else {
                    return Ok(());
                };
                enum_type_referencers.input_object_fields.remove(self);
            }
            Some(ExtendedType::InputObject(_)) => {
                let Some(input_object_type_referencers) =
                    referencers.input_object_types.get_mut(input_type_reference)
                else {
                    return Ok(());
                };
                input_object_type_referencers
                    .input_object_fields
                    .remove(self);
            }
            _ => {
                return Err(
                    SingleFederationError::Internal {
                        message: format!(
                            "Input object field \"{}\"'s inner type \"{}\" does not refer to an existing input type.",
                            self,
                            input_type_reference.deref(),
                        )
                    }.into()
                );
            }
        }
        Ok(())
    }
}

impl Display for InputObjectFieldDefinitionPosition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.type_name, self.field_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct DirectiveDefinitionPosition {
    pub(crate) directive_name: Name,
}

impl DirectiveDefinitionPosition {
    pub(crate) fn get<'schema>(
        &self,
        schema: &'schema Schema,
    ) -> Result<&'schema Node<DirectiveDefinition>, FederationError> {
        schema
            .directive_definitions
            .get(&self.directive_name)
            .ok_or_else(|| {
                SingleFederationError::Internal {
                    message: format!("Schema has no directive \"{}\"", self),
                }
                .into()
            })
    }

    pub(crate) fn try_get<'schema>(
        &self,
        schema: &'schema Schema,
    ) -> Option<&'schema Node<DirectiveDefinition>> {
        self.get(schema).ok()
    }

    fn make_mut<'schema>(
        &self,
        schema: &'schema mut Schema,
    ) -> Result<&'schema mut Node<DirectiveDefinition>, FederationError> {
        schema
            .directive_definitions
            .get_mut(&self.directive_name)
            .ok_or_else(|| {
                SingleFederationError::Internal {
                    message: format!("Schema has no directive \"{}\"", self),
                }
                .into()
            })
    }

    fn try_make_mut<'schema>(
        &self,
        schema: &'schema mut Schema,
    ) -> Option<&'schema mut Node<DirectiveDefinition>> {
        if self.try_get(schema).is_some() {
            self.make_mut(schema).ok()
        } else {
            None
        }
    }

    pub(crate) fn pre_insert(&self, schema: &mut FederationSchema) -> Result<(), FederationError> {
        if schema
            .referencers
            .directives
            .contains_key(&self.directive_name)
        {
            // TODO: Allow built-in shadowing instead of ignoring them
            if is_graphql_reserved_name(&self.directive_name)
                || GRAPHQL_BUILTIN_DIRECTIVE_NAMES.contains(self.directive_name.deref())
            {
                return Ok(());
            }
            return Err(SingleFederationError::Internal {
                message: format!("Directive \"{}\" has already been pre-inserted", self),
            }
            .into());
        }
        schema
            .referencers
            .directives
            .insert(self.directive_name.clone(), Default::default());
        Ok(())
    }

    pub(crate) fn insert(
        &self,
        schema: &mut FederationSchema,
        directive: Node<DirectiveDefinition>,
    ) -> Result<(), FederationError> {
        if !schema
            .referencers
            .directives
            .contains_key(&self.directive_name)
        {
            return Err(SingleFederationError::Internal {
                message: format!("Directive \"{}\" has not been pre-inserted", self),
            }
            .into());
        }
        if schema
            .schema
            .directive_definitions
            .contains_key(&self.directive_name)
        {
            // TODO: Allow built-in shadowing instead of ignoring them
            if is_graphql_reserved_name(&self.directive_name)
                || GRAPHQL_BUILTIN_DIRECTIVE_NAMES.contains(self.directive_name.deref())
            {
                return Ok(());
            }
            return Err(SingleFederationError::Internal {
                message: format!("Directive \"{}\" already exists in schema", self),
            }
            .into());
        }
        self.insert_references(&directive, &schema.schema, &mut schema.referencers)?;
        schema
            .schema
            .directive_definitions
            .insert(self.directive_name.clone(), directive);
        Ok(())
    }

    pub(crate) fn remove(
        &self,
        schema: &mut FederationSchema,
    ) -> Result<Option<DirectiveReferencers>, FederationError> {
        let Some(referencers) = self.remove_internal(schema)? else {
            return Ok(None);
        };
        if let Some(schema_definition) = &referencers.schema {
            schema_definition.remove_directive_name(schema, &self.directive_name)?;
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
        Ok(Some(referencers))
    }

    fn remove_internal(
        &self,
        schema: &mut FederationSchema,
    ) -> Result<Option<DirectiveReferencers>, FederationError> {
        let Some(directive) = self.try_get(&schema.schema) else {
            return Ok(None);
        };
        self.remove_references(directive, &schema.schema, &mut schema.referencers)?;
        schema
            .schema
            .directive_definitions
            .remove(&self.directive_name);
        Ok(Some(
            schema
                .referencers
                .directives
                .remove(&self.directive_name)
                .ok_or_else(|| SingleFederationError::Internal {
                    message: format!("Schema missing referencers for directive \"{}\"", self),
                })?,
        ))
    }

    fn insert_references(
        &self,
        directive: &Node<DirectiveDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        for argument in directive.arguments.iter() {
            DirectiveArgumentDefinitionPosition {
                directive_name: self.directive_name.clone(),
                argument_name: argument.name.clone(),
            }
            .insert_references(argument, schema, referencers)?;
        }
        Ok(())
    }

    fn remove_references(
        &self,
        directive: &Node<DirectiveDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        for argument in directive.arguments.iter() {
            DirectiveArgumentDefinitionPosition {
                directive_name: self.directive_name.clone(),
                argument_name: argument.name.clone(),
            }
            .remove_references(argument, schema, referencers)?;
        }
        Ok(())
    }
}

impl Display for DirectiveDefinitionPosition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "@{}", self.directive_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct DirectiveArgumentDefinitionPosition {
    pub(crate) directive_name: Name,
    pub(crate) argument_name: Name,
}

impl DirectiveArgumentDefinitionPosition {
    pub(crate) fn parent(&self) -> DirectiveDefinitionPosition {
        DirectiveDefinitionPosition {
            directive_name: self.directive_name.clone(),
        }
    }

    pub(crate) fn get<'schema>(
        &self,
        schema: &'schema Schema,
    ) -> Result<&'schema Node<InputValueDefinition>, FederationError> {
        let parent = self.parent();
        let type_ = parent.get(schema)?;

        type_
            .arguments
            .iter()
            .find(|a| a.name == self.argument_name)
            .ok_or_else(|| {
                SingleFederationError::Internal {
                    message: format!(
                        "Directive \"{}\" has no argument \"{}\"",
                        parent, self.argument_name
                    ),
                }
                .into()
            })
    }

    pub(crate) fn try_get<'schema>(
        &self,
        schema: &'schema Schema,
    ) -> Option<&'schema Node<InputValueDefinition>> {
        self.get(schema).ok()
    }

    fn make_mut<'schema>(
        &self,
        schema: &'schema mut Schema,
    ) -> Result<&'schema mut Node<InputValueDefinition>, FederationError> {
        let parent = self.parent();
        let type_ = parent.make_mut(schema)?.make_mut();

        type_
            .arguments
            .iter_mut()
            .find(|a| a.name == self.argument_name)
            .ok_or_else(|| {
                SingleFederationError::Internal {
                    message: format!(
                        "Directive \"{}\" has no argument \"{}\"",
                        parent, self.argument_name
                    ),
                }
                .into()
            })
    }

    fn try_make_mut<'schema>(
        &self,
        schema: &'schema mut Schema,
    ) -> Option<&'schema mut Node<InputValueDefinition>> {
        if self.try_get(schema).is_some() {
            self.make_mut(schema).ok()
        } else {
            None
        }
    }

    pub(crate) fn insert(
        &self,
        schema: &mut FederationSchema,
        argument: Node<InputValueDefinition>,
    ) -> Result<(), FederationError> {
        if self.argument_name != argument.name {
            return Err(SingleFederationError::Internal {
                message: format!(
                    "Directive argument \"{}\" given argument named \"{}\"",
                    self, argument.name,
                ),
            }
            .into());
        }
        if self.try_get(&schema.schema).is_some() {
            // TODO: Handle old spec edge case of arguments with non-unique names
            return Err(SingleFederationError::Internal {
                message: format!("Directive argument \"{}\" already exists in schema", self,),
            }
            .into());
        }
        self.insert_references(&argument, &schema.schema, &mut schema.referencers)?;
        self.parent()
            .make_mut(&mut schema.schema)?
            .make_mut()
            .arguments
            .push(argument);
        Ok(())
    }

    pub(crate) fn remove(&self, schema: &mut FederationSchema) -> Result<(), FederationError> {
        let Some(argument) = self.try_get(&schema.schema) else {
            return Ok(());
        };
        self.remove_references(argument, &schema.schema, &mut schema.referencers)?;
        self.parent()
            .make_mut(&mut schema.schema)?
            .make_mut()
            .arguments
            .retain(|other_argument| other_argument.name != self.argument_name);
        Ok(())
    }

    pub(crate) fn insert_directive(
        &self,
        schema: &mut FederationSchema,
        directive: Node<Directive>,
    ) -> Result<(), FederationError> {
        let argument = self.make_mut(&mut schema.schema)?;
        if argument
            .directives
            .iter()
            .any(|other_directive| other_directive.ptr_eq(&directive))
        {
            return Err(SingleFederationError::Internal {
                message: format!(
                    "Directive application \"@{}\" already exists on directive argument \"{}\"",
                    directive.name, self,
                ),
            }
            .into());
        }
        self.insert_directive_name_references(&mut schema.referencers, &directive.name)?;
        argument.make_mut().directives.push(directive);
        Ok(())
    }

    pub(crate) fn remove_directive_name<Q: Hash + Equivalent<Name>>(
        &self,
        schema: &mut FederationSchema,
        name: &Q,
    ) {
        let Some(argument) = self.try_make_mut(&mut schema.schema) else {
            return;
        };
        self.remove_directive_name_references(&mut schema.referencers, name);
        argument
            .make_mut()
            .directives
            .retain(|other_directive| !name.equivalent(&other_directive.name));
    }

    pub(crate) fn remove_directive(
        &self,
        schema: &mut FederationSchema,
        directive: &Node<Directive>,
    ) {
        let Some(argument) = self.try_make_mut(&mut schema.schema) else {
            return;
        };
        if !argument.directives.iter().any(|other_directive| {
            (other_directive.name == directive.name) && !other_directive.ptr_eq(directive)
        }) {
            self.remove_directive_name_references(&mut schema.referencers, &directive.name);
        }
        argument
            .make_mut()
            .directives
            .retain(|other_directive| !other_directive.ptr_eq(directive));
    }

    fn insert_references(
        &self,
        argument: &Node<InputValueDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        validate_node_directives(argument.directives.deref())?;
        for directive_reference in argument.directives.iter() {
            self.insert_directive_name_references(referencers, &directive_reference.name)?;
        }
        self.insert_type_references(argument, schema, referencers)
    }

    fn remove_references(
        &self,
        argument: &Node<InputValueDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        for directive_reference in argument.directives.iter() {
            self.remove_directive_name_references(referencers, &directive_reference.name);
        }
        self.remove_type_references(argument, schema, referencers)
    }

    fn insert_directive_name_references(
        &self,
        referencers: &mut Referencers,
        name: &Name,
    ) -> Result<(), FederationError> {
        let directive_referencers = referencers.directives.get_mut(name).ok_or_else(|| {
            SingleFederationError::Internal {
                message: format!(
                    "Directive argument \"{}\"'s directive application \"@{}\" does not refer to an existing directive.",
                    self,
                    name,
                ),
            }
        })?;
        directive_referencers
            .directive_arguments
            .insert(self.clone());
        Ok(())
    }

    fn remove_directive_name_references<Q: Hash + Equivalent<Name>>(
        &self,
        referencers: &mut Referencers,
        name: &Q,
    ) {
        let Some(directive_referencers) = referencers.directives.get_mut(name) else {
            return;
        };
        directive_referencers.directive_arguments.remove(self);
    }

    fn insert_type_references(
        &self,
        argument: &Node<InputValueDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        let input_type_reference = argument.ty.inner_named_type();
        match schema.types.get(input_type_reference) {
            Some(ExtendedType::Scalar(_)) => {
                let scalar_type_referencers = referencers
                    .scalar_types
                    .get_mut(input_type_reference)
                    .ok_or_else(|| SingleFederationError::Internal {
                        message: format!(
                            "Schema missing referencers for type \"{}\"",
                            input_type_reference
                        ),
                    })?;
                scalar_type_referencers
                    .directive_arguments
                    .insert(self.clone());
            }
            Some(ExtendedType::Enum(_)) => {
                let enum_type_referencers = referencers
                    .enum_types
                    .get_mut(input_type_reference)
                    .ok_or_else(|| SingleFederationError::Internal {
                        message: format!(
                            "Schema missing referencers for type \"{}\"",
                            input_type_reference
                        ),
                    })?;
                enum_type_referencers
                    .directive_arguments
                    .insert(self.clone());
            }
            Some(ExtendedType::InputObject(_)) => {
                let input_object_type_referencers = referencers
                    .input_object_types
                    .get_mut(input_type_reference)
                    .ok_or_else(|| SingleFederationError::Internal {
                        message: format!(
                            "Schema missing referencers for type \"{}\"",
                            input_type_reference
                        ),
                    })?;
                input_object_type_referencers
                    .directive_arguments
                    .insert(self.clone());
            }
            _ => {
                return Err(
                    SingleFederationError::Internal {
                        message: format!(
                            "Directive argument \"{}\"'s inner type \"{}\" does not refer to an existing input type.",
                            self,
                            input_type_reference.deref(),
                        )
                    }.into()
                );
            }
        }
        Ok(())
    }

    fn remove_type_references(
        &self,
        argument: &Node<InputValueDefinition>,
        schema: &Schema,
        referencers: &mut Referencers,
    ) -> Result<(), FederationError> {
        let input_type_reference = argument.ty.inner_named_type();
        match schema.types.get(input_type_reference) {
            Some(ExtendedType::Scalar(_)) => {
                let Some(scalar_type_referencers) =
                    referencers.scalar_types.get_mut(input_type_reference)
                else {
                    return Ok(());
                };
                scalar_type_referencers.directive_arguments.remove(self);
            }
            Some(ExtendedType::Enum(_)) => {
                let Some(enum_type_referencers) =
                    referencers.enum_types.get_mut(input_type_reference)
                else {
                    return Ok(());
                };
                enum_type_referencers.directive_arguments.remove(self);
            }
            Some(ExtendedType::InputObject(_)) => {
                let Some(input_object_type_referencers) =
                    referencers.input_object_types.get_mut(input_type_reference)
                else {
                    return Ok(());
                };
                input_object_type_referencers
                    .directive_arguments
                    .remove(self);
            }
            _ => {
                return Err(
                    SingleFederationError::Internal {
                        message: format!(
                            "Directive argument \"{}\"'s inner type \"{}\" does not refer to an existing input type.",
                            self,
                            input_type_reference.deref(),
                        )
                    }.into()
                );
            }
        }
        Ok(())
    }
}

impl Display for DirectiveArgumentDefinitionPosition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "@{}({}:)", self.directive_name, self.argument_name)
    }
}

fn is_graphql_reserved_name(name: &str) -> bool {
    name.starts_with("__")
}

lazy_static! {
    static ref GRAPHQL_BUILTIN_SCALAR_NAMES: IndexSet<String> = {
        IndexSet::from([
            "Int".to_owned(),
            "Float".to_owned(),
            "String".to_owned(),
            "Boolean".to_owned(),
            "ID".to_owned(),
        ])
    };
    static ref GRAPHQL_BUILTIN_DIRECTIVE_NAMES: IndexSet<String> = {
        IndexSet::from([
            "include".to_owned(),
            "skip".to_owned(),
            "deprecated".to_owned(),
            "specifiedBy".to_owned(),
            "defer".to_owned(),
        ])
    };
}

fn validate_component_directives(
    directives: &[Component<Directive>],
) -> Result<(), FederationError> {
    for directive in directives.iter() {
        if directives
            .iter()
            .filter(|other_directive| other_directive.ptr_eq(directive))
            .count()
            > 1
        {
            return Err(SingleFederationError::Internal {
                message: format!(
                    "Directive application \"@{}\" is duplicated on schema element",
                    directive.name,
                ),
            }
            .into());
        }
    }
    Ok(())
}

fn validate_node_directives(directives: &[Node<Directive>]) -> Result<(), FederationError> {
    for directive in directives.iter() {
        if directives
            .iter()
            .filter(|other_directive| other_directive.ptr_eq(directive))
            .count()
            > 1
        {
            return Err(SingleFederationError::Internal {
                message: format!(
                    "Directive application \"@{}\" is duplicated on schema element",
                    directive.name,
                ),
            }
            .into());
        }
    }
    Ok(())
}

fn validate_arguments(arguments: &[Node<InputValueDefinition>]) -> Result<(), FederationError> {
    for argument in arguments.iter() {
        if arguments
            .iter()
            .filter(|other_argument| other_argument.name == argument.name)
            .count()
            > 1
        {
            return Err(SingleFederationError::Internal {
                message: format!(
                    "Argument \"{}\" is duplicated on schema element",
                    argument.name,
                ),
            }
            .into());
        }
    }
    Ok(())
}

impl FederationSchema {
    pub(crate) fn new(schema: Schema) -> Result<FederationSchema, FederationError> {
        let metadata = links_metadata(&schema)?;
        let mut referencers: Referencers = Default::default();

        // Shallow pass to populate referencers for types/directives.
        for (type_name, type_) in schema.types.iter() {
            match type_ {
                ExtendedType::Scalar(_) => {
                    referencers
                        .scalar_types
                        .insert(type_name.clone(), Default::default());
                }
                ExtendedType::Object(_) => {
                    referencers
                        .object_types
                        .insert(type_name.clone(), Default::default());
                }
                ExtendedType::Interface(_) => {
                    referencers
                        .interface_types
                        .insert(type_name.clone(), Default::default());
                }
                ExtendedType::Union(_) => {
                    referencers
                        .union_types
                        .insert(type_name.clone(), Default::default());
                }
                ExtendedType::Enum(_) => {
                    referencers
                        .enum_types
                        .insert(type_name.clone(), Default::default());
                }
                ExtendedType::InputObject(_) => {
                    referencers
                        .input_object_types
                        .insert(type_name.clone(), Default::default());
                }
            }
        }
        for directive_name in schema.directive_definitions.keys() {
            referencers
                .directives
                .insert(directive_name.clone(), Default::default());
        }

        // Deep pass to find references.
        SchemaDefinitionPosition.insert_references(&schema.schema_definition, &mut referencers)?;
        for (type_name, type_) in schema.types.iter() {
            match type_ {
                ExtendedType::Scalar(type_) => {
                    ScalarTypeDefinitionPosition {
                        type_name: type_name.clone(),
                    }
                    .insert_references(type_, &mut referencers)?;
                }
                ExtendedType::Object(type_) => {
                    ObjectTypeDefinitionPosition {
                        type_name: type_name.clone(),
                    }
                    .insert_references(type_, &schema, &mut referencers)?;
                }
                ExtendedType::Interface(type_) => {
                    InterfaceTypeDefinitionPosition {
                        type_name: type_name.clone(),
                    }
                    .insert_references(type_, &schema, &mut referencers)?;
                }
                ExtendedType::Union(type_) => {
                    UnionTypeDefinitionPosition {
                        type_name: type_name.clone(),
                    }
                    .insert_references(type_, &mut referencers)?;
                }
                ExtendedType::Enum(type_) => {
                    EnumTypeDefinitionPosition {
                        type_name: type_name.clone(),
                    }
                    .insert_references(type_, &mut referencers)?;
                }
                ExtendedType::InputObject(type_) => {
                    InputObjectTypeDefinitionPosition {
                        type_name: type_name.clone(),
                    }
                    .insert_references(type_, &schema, &mut referencers)?;
                }
            }
        }
        for (directive_name, directive) in schema.directive_definitions.iter() {
            DirectiveDefinitionPosition {
                directive_name: directive_name.clone(),
            }
            .insert_references(directive, &schema, &mut referencers)?;
        }

        Ok(FederationSchema {
            schema,
            metadata,
            referencers,
        })
    }
}
