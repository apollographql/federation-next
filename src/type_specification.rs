use indexmap::{IndexMap, IndexSet};

use apollo_compiler::ast::FieldDefinition;
use apollo_compiler::schema::{
    Component, ComponentName, InputValueDefinition, Name, ObjectType, ScalarType, Type, UnionType,
};
use apollo_compiler::Node;

use crate::error::FederationError;
use crate::schema::position::{
    ObjectTypeDefinitionPosition, ScalarTypeDefinitionPosition, UnionTypeDefinitionPosition,
};
use crate::schema::FederationSchema;

pub(crate) trait TypeSpecification {
    fn check_or_add(&self, schema: &mut FederationSchema) -> Result<(), FederationError>;
}

pub(crate) struct ScalarTypeSpecification {
    pub name: Name, // Type's name
}

impl TypeSpecification for ScalarTypeSpecification {
    fn check_or_add(&self, schema: &mut FederationSchema) -> Result<(), FederationError> {
        let type_pos = ScalarTypeDefinitionPosition {
            type_name: self.name.clone(),
        };
        type_pos.pre_insert(schema)?;
        type_pos.insert(
            schema,
            Node::new(ScalarType {
                description: None,
                name: type_pos.type_name.clone(),
                directives: Default::default(),
            }),
        )
    }
}

pub(crate) struct FieldSpecification {
    pub name: Name,
    pub ty: Type,
    pub arguments: Vec<Node<InputValueDefinition>>,
}

impl From<&FieldSpecification> for FieldDefinition {
    fn from(field_spec: &FieldSpecification) -> Self {
        FieldDefinition {
            description: None,
            name: field_spec.name.clone(),
            arguments: field_spec.arguments.clone(),
            ty: field_spec.ty.clone(),
            directives: Default::default(),
        }
    }
}

pub(crate) struct ObjectTypeSpecification {
    pub name: Name,
    pub fields: fn(&FederationSchema) -> Vec<FieldSpecification>,
}

impl TypeSpecification for ObjectTypeSpecification {
    fn check_or_add(&self, schema: &mut FederationSchema) -> Result<(), FederationError> {
        let mut fields = IndexMap::new();
        for ref field_spec in (self.fields)(schema) {
            fields.insert(field_spec.name.clone(), Component::new(field_spec.into()));
        }
        let type_pos = ObjectTypeDefinitionPosition {
            type_name: self.name.clone(),
        };
        type_pos.pre_insert(schema)?;
        type_pos.insert(
            schema,
            Node::new(ObjectType {
                description: None,
                name: type_pos.type_name.clone(),
                implements_interfaces: Default::default(),
                directives: Default::default(),
                fields,
            }),
        )
    }
}

pub(crate) struct UnionTypeSpecification<F>
where
    F: Fn(&FederationSchema) -> IndexSet<ComponentName>,
{
    pub name: Name,
    pub members: F,
}

impl<F> TypeSpecification for UnionTypeSpecification<F>
where
    F: Fn(&FederationSchema) -> IndexSet<ComponentName>,
{
    fn check_or_add(&self, schema: &mut FederationSchema) -> Result<(), FederationError> {
        let type_pos = UnionTypeDefinitionPosition {
            type_name: self.name.clone(),
        };
        type_pos.pre_insert(schema)?;
        type_pos.insert(
            schema,
            Node::new(UnionType {
                description: None,
                name: type_pos.type_name.clone(),
                directives: Default::default(),
                members: (self.members)(schema),
            }),
        )
    }
}
