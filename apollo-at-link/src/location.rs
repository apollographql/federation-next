use apollo_compiler::schema::{
    Component, ComponentStr, DirectiveDefinition, EnumType, EnumValueDefinition, ExtendedType,
    FieldDefinition, InputObjectType, InputValueDefinition, InterfaceType, Name, ObjectType,
    ScalarType, SchemaDefinition, UnionType,
};
use apollo_compiler::{Node, Schema};
use std::fmt::{Display, Formatter};


pub enum TypeDefinitionLocation {
    ScalarTypeDefinitionLocation(ScalarTypeDefinitionLocation),
    ObjectTypeDefinitionLocation(ObjectTypeDefinitionLocation),
    InterfaceTypeDefinitionLocation(InterfaceTypeDefinitionLocation),
    UnionTypeDefinitionLocation(UnionTypeDefinitionLocation),
    EnumTypeDefinitionLocation(EnumTypeDefinitionLocation),
    InputObjectTypeDefinitionLocation(InputObjectTypeDefinitionLocation),
}

impl From<ScalarTypeDefinitionLocation> for TypeDefinitionLocation {
    fn from(value: ScalarTypeDefinitionLocation) -> Self {
        TypeDefinitionLocation::ScalarTypeDefinitionLocation(value)
    }
}

impl From<ObjectTypeDefinitionLocation> for TypeDefinitionLocation {
    fn from(value: ObjectTypeDefinitionLocation) -> Self {
        TypeDefinitionLocation::ObjectTypeDefinitionLocation(value)
    }
}

impl From<InterfaceTypeDefinitionLocation> for TypeDefinitionLocation {
    fn from(value: InterfaceTypeDefinitionLocation) -> Self {
        TypeDefinitionLocation::InterfaceTypeDefinitionLocation(value)
    }
}

impl From<UnionTypeDefinitionLocation> for TypeDefinitionLocation {
    fn from(value: UnionTypeDefinitionLocation) -> Self {
        TypeDefinitionLocation::UnionTypeDefinitionLocation(value)
    }
}

impl From<EnumTypeDefinitionLocation> for TypeDefinitionLocation {
    fn from(value: EnumTypeDefinitionLocation) -> Self {
        TypeDefinitionLocation::EnumTypeDefinitionLocation(value)
    }
}

impl From<InputObjectTypeDefinitionLocation> for TypeDefinitionLocation {
    fn from(value: InputObjectTypeDefinitionLocation) -> Self {
        TypeDefinitionLocation::InputObjectTypeDefinitionLocation(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SchemaDefinitionLocation;

impl SchemaDefinitionLocation {
    pub fn get<'a, 'schema>(&'a self, schema: &'schema Schema) -> &'schema Node<SchemaDefinition> {
        match schema.schema_definition {
            Some(ref schema_definition) => schema_definition,
            None => panic!("Schema has no schema definition"),
        }
    }

    pub fn try_get<'a, 'schema>(
        &'a self,
        schema: &'schema Schema,
    ) -> Option<&'schema Node<SchemaDefinition>> {
        match schema.schema_definition {
            Some(ref schema_definition) => Some(schema_definition),
            None => None,
        }
    }

    pub fn make_mut<'a, 'schema>(
        &'a self,
        schema: &'schema mut Schema,
    ) -> &'schema mut Node<SchemaDefinition> {
        match schema.schema_definition {
            Some(ref mut schema_definition) => schema_definition,
            None => panic!("Schema has no schema definition"),
        }
    }

    pub fn try_make_mut<'a, 'schema>(
        &'a self,
        schema: &'schema mut Schema,
    ) -> Option<&'schema mut Node<SchemaDefinition>> {
        if self.try_get(schema).is_some() {
            Some(self.make_mut(schema))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SchemaRootDefinitionKind {
    Query,
    Mutation,
    Subscription,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SchemaRootDefinitionLocation {
    pub root_kind: SchemaRootDefinitionKind,
}

impl SchemaRootDefinitionLocation {
    pub fn parent(&self) -> SchemaDefinitionLocation {
        SchemaDefinitionLocation
    }

    pub fn get<'a, 'schema>(&'a self, schema: &'schema Schema) -> &'schema ComponentStr {
        let schema_definition = self.parent().get(schema);

        match self.root_kind {
            SchemaRootDefinitionKind::Query => match schema_definition.query {
                Some(ref root) => root,
                None => panic!("Schema definition has no query root type"),
            },
            SchemaRootDefinitionKind::Mutation => match schema_definition.mutation {
                Some(ref root) => root,
                None => panic!("Schema definition has no mutation root type"),
            },
            SchemaRootDefinitionKind::Subscription => match schema_definition.subscription {
                Some(ref root) => root,
                None => panic!("Schema definition has no subscription root type"),
            },
        }
    }

    pub fn try_get<'a, 'schema>(
        &'a self,
        schema: &'schema Schema,
    ) -> Option<&'schema ComponentStr> {
        let schema_definition = self.parent().try_get(schema)?;

        match self.root_kind {
            SchemaRootDefinitionKind::Query => match schema_definition.query {
                Some(ref root) => Some(root),
                None => None,
            },
            SchemaRootDefinitionKind::Mutation => match schema_definition.mutation {
                Some(ref root) => Some(root),
                None => None,
            },
            SchemaRootDefinitionKind::Subscription => match schema_definition.subscription {
                Some(ref root) => Some(root),
                None => None,
            },
        }
    }

    pub fn make_mut<'a, 'schema>(
        &'a self,
        schema: &'schema mut Schema,
    ) -> &'schema mut ComponentStr {
        let schema_definition = self.parent().make_mut(schema).make_mut();

        match self.root_kind {
            SchemaRootDefinitionKind::Query => match schema_definition.query {
                Some(ref mut root) => root,
                None => panic!("Schema definition has no query root type"),
            },
            SchemaRootDefinitionKind::Mutation => match schema_definition.mutation {
                Some(ref mut root) => root,
                None => panic!("Schema definition has no mutation root type"),
            },
            SchemaRootDefinitionKind::Subscription => match schema_definition.subscription {
                Some(ref mut root) => root,
                None => panic!("Schema definition has no subscription root type"),
            },
        }
    }

    pub fn try_make_mut<'a, 'schema>(
        &'a self,
        schema: &'schema mut Schema,
    ) -> Option<&'schema mut ComponentStr> {
        if self.try_get(schema).is_some() {
            Some(self.make_mut(schema))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScalarTypeDefinitionLocation {
    pub type_name: Name,
}

impl ScalarTypeDefinitionLocation {
    pub fn get<'a, 'schema>(&'a self, schema: &'schema Schema) -> &'schema Node<ScalarType> {
        match schema.types.get(&self.type_name) {
            Some(ExtendedType::Scalar(ref type_)) => type_,
            Some(_) => panic!("Schema type \"{}\" was not a scalar", self.type_name),
            None => panic!("Schema has no type \"{}\"", self.type_name),
        }
    }

    pub fn try_get<'a, 'schema>(
        &'a self,
        schema: &'schema Schema,
    ) -> Option<&'schema Node<ScalarType>> {
        match schema.types.get(&self.type_name) {
            Some(ExtendedType::Scalar(ref type_)) => Some(type_),
            _ => None,
        }
    }

    pub fn make_mut<'a, 'schema>(
        &'a self,
        schema: &'schema mut Schema,
    ) -> &'schema mut Node<ScalarType> {
        match schema.types.get_mut(&self.type_name) {
            Some(ExtendedType::Scalar(ref mut type_)) => type_,
            Some(_) => panic!("Schema type \"{}\" was not a scalar", self.type_name),
            None => panic!("Schema has no type \"{}\"", self.type_name),
        }
    }

    pub fn try_make_mut<'a, 'schema>(
        &'a self,
        schema: &'schema mut Schema,
    ) -> Option<&'schema mut Node<ScalarType>> {
        if self.try_get(schema).is_some() {
            Some(self.make_mut(schema))
        } else {
            None
        }
    }
}

impl Display for ScalarTypeDefinitionLocation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.type_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ObjectTypeDefinitionLocation {
    pub type_name: Name,
}

impl ObjectTypeDefinitionLocation {
    pub fn get<'a, 'schema>(&'a self, schema: &'schema Schema) -> &'schema Node<ObjectType> {
        match schema.types.get(&self.type_name) {
            Some(ExtendedType::Object(ref type_)) => type_,
            Some(_) => panic!("Schema type \"{}\" was not an object", self.type_name),
            None => panic!("Schema has no type \"{}\"", self.type_name),
        }
    }

    pub fn try_get<'a, 'schema>(
        &'a self,
        schema: &'schema Schema,
    ) -> Option<&'schema Node<ObjectType>> {
        match schema.types.get(&self.type_name) {
            Some(ExtendedType::Object(ref type_)) => Some(type_),
            _ => None,
        }
    }

    pub fn make_mut<'a, 'schema>(
        &'a self,
        schema: &'schema mut Schema,
    ) -> &'schema mut Node<ObjectType> {
        match schema.types.get_mut(&self.type_name) {
            Some(ExtendedType::Object(ref mut type_)) => type_,
            Some(_) => panic!("Schema type \"{}\" was not an object", self.type_name),
            None => panic!("Schema has no type \"{}\"", self.type_name),
        }
    }

    pub fn try_make_mut<'a, 'schema>(
        &'a self,
        schema: &'schema mut Schema,
    ) -> Option<&'schema mut Node<ObjectType>> {
        if self.try_get(schema).is_some() {
            Some(self.make_mut(schema))
        } else {
            None
        }
    }
}

impl Display for ObjectTypeDefinitionLocation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.type_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ObjectFieldDefinitionLocation {
    pub type_name: Name,
    pub field_name: Name,
}

impl ObjectFieldDefinitionLocation {
    pub fn parent(&self) -> ObjectTypeDefinitionLocation {
        ObjectTypeDefinitionLocation {
            type_name: self.type_name.clone(),
        }
    }

    pub fn get<'a, 'schema>(
        &'a self,
        schema: &'schema Schema,
    ) -> &'schema Component<FieldDefinition> {
        let parent = self.parent();
        let type_ = parent.get(schema);

        match type_.fields.get(&self.field_name) {
            Some(field) => field,
            None => panic!(
                "Object type \"{}\" has no field \"{}\"",
                parent, self.field_name
            ),
        }
    }

    pub fn try_get<'a, 'schema>(
        &'a self,
        schema: &'schema Schema,
    ) -> Option<&'schema Component<FieldDefinition>> {
        let type_ = self.parent().try_get(schema)?;

        match type_.fields.get(&self.field_name) {
            Some(field) => Some(field),
            None => None,
        }
    }

    pub fn make_mut<'a, 'schema>(
        &'a self,
        schema: &'schema mut Schema,
    ) -> &'schema mut Component<FieldDefinition> {
        let parent = self.parent();
        let type_ = parent.make_mut(schema).make_mut();

        match type_.fields.get_mut(&self.field_name) {
            Some(field) => field,
            None => panic!(
                "Object type \"{}\" has no field \"{}\"",
                parent, self.field_name
            ),
        }
    }

    pub fn try_make_mut<'a, 'schema>(
        &'a self,
        schema: &'schema mut Schema,
    ) -> Option<&'schema mut Component<FieldDefinition>> {
        if self.try_get(schema).is_some() {
            Some(self.make_mut(schema))
        } else {
            None
        }
    }
}

impl Display for ObjectFieldDefinitionLocation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.type_name, self.field_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ObjectFieldArgumentDefinitionLocation {
    pub type_name: Name,
    pub field_name: Name,
    pub argument_name: Name,
}

impl ObjectFieldArgumentDefinitionLocation {
    pub fn parent(&self) -> ObjectFieldDefinitionLocation {
        ObjectFieldDefinitionLocation {
            type_name: self.type_name.clone(),
            field_name: self.field_name.clone(),
        }
    }

    pub fn get<'a, 'schema>(
        &'a self,
        schema: &'schema Schema,
    ) -> &'schema Node<InputValueDefinition> {
        let parent = self.parent();
        let type_ = parent.get(schema);

        match type_
            .arguments
            .iter()
            .find(|a| a.name == self.argument_name)
        {
            Some(argument) => argument,
            None => panic!(
                "Object field \"{}\" has no argument \"{}\"",
                parent, self.argument_name
            ),
        }
    }

    pub fn try_get<'a, 'schema>(
        &'a self,
        schema: &'schema Schema,
    ) -> Option<&'schema Node<InputValueDefinition>> {
        let type_ = self.parent().try_get(schema)?;

        match type_
            .arguments
            .iter()
            .find(|a| a.name == self.argument_name)
        {
            Some(argument) => Some(argument),
            None => None,
        }
    }

    pub fn make_mut<'a, 'schema>(
        &'a self,
        schema: &'schema mut Schema,
    ) -> &'schema mut Node<InputValueDefinition> {
        let parent = self.parent();
        let type_ = parent.make_mut(schema).make_mut();

        match type_
            .arguments
            .iter_mut()
            .find(|a| a.name == self.argument_name)
        {
            Some(argument) => argument,
            None => panic!(
                "Object field \"{}\" has no argument \"{}\"",
                parent, self.argument_name
            ),
        }
    }

    pub fn try_make_mut<'a, 'schema>(
        &'a self,
        schema: &'schema mut Schema,
    ) -> Option<&'schema mut Node<InputValueDefinition>> {
        if self.try_get(schema).is_some() {
            Some(self.make_mut(schema))
        } else {
            None
        }
    }
}

impl Display for ObjectFieldArgumentDefinitionLocation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}.{}({}:)",
            self.type_name, self.field_name, self.argument_name
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InterfaceTypeDefinitionLocation {
    pub type_name: Name,
}

impl InterfaceTypeDefinitionLocation {
    pub fn get<'a, 'schema>(&'a self, schema: &'schema Schema) -> &'schema Node<InterfaceType> {
        match schema.types.get(&self.type_name) {
            Some(ExtendedType::Interface(ref type_)) => type_,
            Some(_) => panic!("Schema type \"{}\" was not an interface", self.type_name),
            None => panic!("Schema has no type \"{}\"", self.type_name),
        }
    }

    pub fn try_get<'a, 'schema>(
        &'a self,
        schema: &'schema Schema,
    ) -> Option<&'schema Node<InterfaceType>> {
        match schema.types.get(&self.type_name) {
            Some(ExtendedType::Interface(ref type_)) => Some(type_),
            _ => None,
        }
    }

    pub fn make_mut<'a, 'schema>(
        &'a self,
        schema: &'schema mut Schema,
    ) -> &'schema mut Node<InterfaceType> {
        match schema.types.get_mut(&self.type_name) {
            Some(ExtendedType::Interface(ref mut type_)) => type_,
            Some(_) => panic!("Schema type \"{}\" was not an interface", self.type_name),
            None => panic!("Schema has no type \"{}\"", self.type_name),
        }
    }

    pub fn try_make_mut<'a, 'schema>(
        &'a self,
        schema: &'schema mut Schema,
    ) -> Option<&'schema mut Node<InterfaceType>> {
        if self.try_get(schema).is_some() {
            Some(self.make_mut(schema))
        } else {
            None
        }
    }
}

impl Display for InterfaceTypeDefinitionLocation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.type_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InterfaceFieldDefinitionLocation {
    pub type_name: Name,
    pub field_name: Name,
}

impl InterfaceFieldDefinitionLocation {
    pub fn parent(&self) -> InterfaceTypeDefinitionLocation {
        InterfaceTypeDefinitionLocation {
            type_name: self.type_name.clone(),
        }
    }

    pub fn get<'a, 'schema>(
        &'a self,
        schema: &'schema Schema,
    ) -> &'schema Component<FieldDefinition> {
        let parent = self.parent();
        let type_ = parent.get(schema);

        match type_.fields.get(&self.field_name) {
            Some(field) => field,
            None => panic!(
                "Interface type \"{}\" has no field \"{}\"",
                parent, self.field_name
            ),
        }
    }

    pub fn try_get<'a, 'schema>(
        &'a self,
        schema: &'schema Schema,
    ) -> Option<&'schema Component<FieldDefinition>> {
        let type_ = self.parent().try_get(schema)?;

        match type_.fields.get(&self.field_name) {
            Some(field) => Some(field),
            None => None,
        }
    }

    pub fn make_mut<'a, 'schema>(
        &'a self,
        schema: &'schema mut Schema,
    ) -> &'schema mut Component<FieldDefinition> {
        let parent = self.parent();
        let type_ = parent.make_mut(schema).make_mut();

        match type_.fields.get_mut(&self.field_name) {
            Some(field) => field,
            None => panic!(
                "Interface type \"{}\" has no field \"{}\"",
                parent, self.field_name
            ),
        }
    }

    pub fn try_make_mut<'a, 'schema>(
        &'a self,
        schema: &'schema mut Schema,
    ) -> Option<&'schema mut Component<FieldDefinition>> {
        if self.try_get(schema).is_some() {
            Some(self.make_mut(schema))
        } else {
            None
        }
    }
}

impl Display for InterfaceFieldDefinitionLocation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.type_name, self.field_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InterfaceFieldArgumentDefinitionLocation {
    pub type_name: Name,
    pub field_name: Name,
    pub argument_name: Name,
}

impl InterfaceFieldArgumentDefinitionLocation {
    pub fn parent(&self) -> InterfaceFieldDefinitionLocation {
        InterfaceFieldDefinitionLocation {
            type_name: self.type_name.clone(),
            field_name: self.field_name.clone(),
        }
    }

    pub fn get<'a, 'schema>(
        &'a self,
        schema: &'schema Schema,
    ) -> &'schema Node<InputValueDefinition> {
        let parent = self.parent();
        let type_ = parent.get(schema);

        match type_
            .arguments
            .iter()
            .find(|a| a.name == self.argument_name)
        {
            Some(argument) => argument,
            None => panic!(
                "Interface field \"{}\" has no argument \"{}\"",
                parent, self.argument_name
            ),
        }
    }

    pub fn try_get<'a, 'schema>(
        &'a self,
        schema: &'schema Schema,
    ) -> Option<&'schema Node<InputValueDefinition>> {
        let type_ = self.parent().try_get(schema)?;

        match type_
            .arguments
            .iter()
            .find(|a| a.name == self.argument_name)
        {
            Some(argument) => Some(argument),
            None => None,
        }
    }

    pub fn make_mut<'a, 'schema>(
        &'a self,
        schema: &'schema mut Schema,
    ) -> &'schema mut Node<InputValueDefinition> {
        let parent = self.parent();
        let type_ = parent.make_mut(schema).make_mut();

        match type_
            .arguments
            .iter_mut()
            .find(|a| a.name == self.argument_name)
        {
            Some(argument) => argument,
            None => panic!(
                "Interface field \"{}\" has no argument \"{}\"",
                parent, self.argument_name
            ),
        }
    }

    pub fn try_make_mut<'a, 'schema>(
        &'a self,
        schema: &'schema mut Schema,
    ) -> Option<&'schema mut Node<InputValueDefinition>> {
        if self.try_get(schema).is_some() {
            Some(self.make_mut(schema))
        } else {
            None
        }
    }
}

impl Display for InterfaceFieldArgumentDefinitionLocation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}.{}({}:)",
            self.type_name, self.field_name, self.argument_name
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnionTypeDefinitionLocation {
    pub type_name: Name,
}

impl UnionTypeDefinitionLocation {
    pub fn get<'a, 'schema>(&'a self, schema: &'schema Schema) -> &'schema Node<UnionType> {
        match schema.types.get(&self.type_name) {
            Some(ExtendedType::Union(ref type_)) => type_,
            Some(_) => panic!("Schema type \"{}\" was not a union", self.type_name),
            None => panic!("Schema has no type \"{}\"", self.type_name),
        }
    }

    pub fn try_get<'a, 'schema>(
        &'a self,
        schema: &'schema Schema,
    ) -> Option<&'schema Node<UnionType>> {
        match schema.types.get(&self.type_name) {
            Some(ExtendedType::Union(ref type_)) => Some(type_),
            _ => None,
        }
    }

    pub fn make_mut<'a, 'schema>(
        &'a self,
        schema: &'schema mut Schema,
    ) -> &'schema mut Node<UnionType> {
        match schema.types.get_mut(&self.type_name) {
            Some(ExtendedType::Union(ref mut type_)) => type_,
            Some(_) => panic!("Schema type \"{}\" was not a union", self.type_name),
            None => panic!("Schema has no type \"{}\"", self.type_name),
        }
    }

    pub fn try_make_mut<'a, 'schema>(
        &'a self,
        schema: &'schema mut Schema,
    ) -> Option<&'schema mut Node<UnionType>> {
        if self.try_get(schema).is_some() {
            Some(self.make_mut(schema))
        } else {
            None
        }
    }
}

impl Display for UnionTypeDefinitionLocation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.type_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EnumTypeDefinitionLocation {
    pub type_name: Name,
}

impl EnumTypeDefinitionLocation {
    pub fn get<'a, 'schema>(&'a self, schema: &'schema Schema) -> &'schema Node<EnumType> {
        match schema.types.get(&self.type_name) {
            Some(ExtendedType::Enum(ref type_)) => type_,
            Some(_) => panic!("Schema type \"{}\" was not an enum", self.type_name),
            None => panic!("Schema has no type \"{}\"", self.type_name),
        }
    }

    pub fn try_get<'a, 'schema>(
        &'a self,
        schema: &'schema Schema,
    ) -> Option<&'schema Node<EnumType>> {
        match schema.types.get(&self.type_name) {
            Some(ExtendedType::Enum(ref type_)) => Some(type_),
            _ => None,
        }
    }

    pub fn make_mut<'a, 'schema>(
        &'a self,
        schema: &'schema mut Schema,
    ) -> &'schema mut Node<EnumType> {
        match schema.types.get_mut(&self.type_name) {
            Some(ExtendedType::Enum(ref mut type_)) => type_,
            Some(_) => panic!("Schema type \"{}\" was not an enum", self.type_name),
            None => panic!("Schema has no type \"{}\"", self.type_name),
        }
    }

    pub fn try_make_mut<'a, 'schema>(
        &'a self,
        schema: &'schema mut Schema,
    ) -> Option<&'schema mut Node<EnumType>> {
        if self.try_get(schema).is_some() {
            Some(self.make_mut(schema))
        } else {
            None
        }
    }
}

impl Display for EnumTypeDefinitionLocation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.type_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EnumValueDefinitionLocation {
    pub type_name: Name,
    pub value_name: Name,
}

impl EnumValueDefinitionLocation {
    pub fn parent(&self) -> EnumTypeDefinitionLocation {
        EnumTypeDefinitionLocation {
            type_name: self.type_name.clone(),
        }
    }

    pub fn get<'a, 'schema>(
        &'a self,
        schema: &'schema Schema,
    ) -> &'schema Component<EnumValueDefinition> {
        let parent = self.parent();
        let type_ = parent.get(schema);

        match type_.values.get(&self.value_name) {
            Some(value) => value,
            None => panic!(
                "Enum type \"{}\" has no value \"{}\"",
                parent, self.value_name
            ),
        }
    }

    pub fn try_get<'a, 'schema>(
        &'a self,
        schema: &'schema Schema,
    ) -> Option<&'schema Component<EnumValueDefinition>> {
        let type_ = self.parent().try_get(schema)?;

        match type_.values.get(&self.value_name) {
            Some(value) => Some(value),
            None => None,
        }
    }

    pub fn make_mut<'a, 'schema>(
        &'a self,
        schema: &'schema mut Schema,
    ) -> &'schema mut Component<EnumValueDefinition> {
        let parent = self.parent();
        let type_ = parent.make_mut(schema).make_mut();

        match type_.values.get_mut(&self.value_name) {
            Some(value) => value,
            None => panic!(
                "Enum type \"{}\" has no value \"{}\"",
                parent, self.value_name
            ),
        }
    }

    pub fn try_make_mut<'a, 'schema>(
        &'a self,
        schema: &'schema mut Schema,
    ) -> Option<&'schema mut Component<EnumValueDefinition>> {
        if self.try_get(schema).is_some() {
            Some(self.make_mut(schema))
        } else {
            None
        }
    }
}

impl Display for EnumValueDefinitionLocation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.type_name, self.value_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InputObjectTypeDefinitionLocation {
    pub type_name: Name,
}

impl InputObjectTypeDefinitionLocation {
    pub fn get<'a, 'schema>(&'a self, schema: &'schema Schema) -> &'schema Node<InputObjectType> {
        match schema.types.get(&self.type_name) {
            Some(ExtendedType::InputObject(ref type_)) => type_,
            Some(_) => panic!("Schema type \"{}\" was not an input object", self.type_name),
            None => panic!("Schema has no type \"{}\"", self.type_name),
        }
    }

    pub fn try_get<'a, 'schema>(
        &'a self,
        schema: &'schema Schema,
    ) -> Option<&'schema Node<InputObjectType>> {
        match schema.types.get(&self.type_name) {
            Some(ExtendedType::InputObject(ref type_)) => Some(type_),
            _ => None,
        }
    }

    pub fn make_mut<'a, 'schema>(
        &'a self,
        schema: &'schema mut Schema,
    ) -> &'schema mut Node<InputObjectType> {
        match schema.types.get_mut(&self.type_name) {
            Some(ExtendedType::InputObject(ref mut type_)) => type_,
            Some(_) => panic!("Schema type \"{}\" was not an input object", self.type_name),
            None => panic!("Schema has no type \"{}\"", self.type_name),
        }
    }

    pub fn try_make_mut<'a, 'schema>(
        &'a self,
        schema: &'schema mut Schema,
    ) -> Option<&'schema mut Node<InputObjectType>> {
        if self.try_get(schema).is_some() {
            Some(self.make_mut(schema))
        } else {
            None
        }
    }
}

impl Display for InputObjectTypeDefinitionLocation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.type_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InputObjectFieldDefinitionLocation {
    pub type_name: Name,
    pub field_name: Name,
}

impl InputObjectFieldDefinitionLocation {
    pub fn parent(&self) -> InputObjectTypeDefinitionLocation {
        InputObjectTypeDefinitionLocation {
            type_name: self.type_name.clone(),
        }
    }

    pub fn get<'a, 'schema>(
        &'a self,
        schema: &'schema Schema,
    ) -> &'schema Component<InputValueDefinition> {
        let parent = self.parent();
        let type_ = parent.get(schema);

        match type_.fields.get(&self.field_name) {
            Some(field) => field,
            None => panic!(
                "Input object type \"{}\" has no field \"{}\"",
                parent, self.field_name
            ),
        }
    }

    pub fn try_get<'a, 'schema>(
        &'a self,
        schema: &'schema Schema,
    ) -> Option<&'schema Component<InputValueDefinition>> {
        let type_ = self.parent().try_get(schema)?;

        match type_.fields.get(&self.field_name) {
            Some(field) => Some(field),
            None => None,
        }
    }

    pub fn make_mut<'a, 'schema>(
        &'a self,
        schema: &'schema mut Schema,
    ) -> &'schema mut Component<InputValueDefinition> {
        let parent = self.parent();
        let type_ = parent.make_mut(schema).make_mut();

        match type_.fields.get_mut(&self.field_name) {
            Some(field) => field,
            None => panic!(
                "Input object type \"{}\" has no field \"{}\"",
                parent, self.field_name
            ),
        }
    }

    pub fn try_make_mut<'a, 'schema>(
        &'a self,
        schema: &'schema mut Schema,
    ) -> Option<&'schema mut Component<InputValueDefinition>> {
        if self.try_get(schema).is_some() {
            Some(self.make_mut(schema))
        } else {
            None
        }
    }
}

impl Display for InputObjectFieldDefinitionLocation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.type_name, self.field_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DirectiveDefinitionLocation {
    pub directive_name: Name,
}

impl DirectiveDefinitionLocation {
    pub fn get<'a, 'schema>(
        &'a self,
        schema: &'schema Schema,
    ) -> &'schema Node<DirectiveDefinition> {
        match schema.directive_definitions.get(&self.directive_name) {
            Some(directive) => directive,
            None => panic!("Schema has no directive \"{}\"", self.directive_name),
        }
    }

    pub fn try_get<'a, 'schema>(
        &'a self,
        schema: &'schema Schema,
    ) -> Option<&'schema Node<DirectiveDefinition>> {
        match schema.directive_definitions.get(&self.directive_name) {
            Some(directive) => Some(directive),
            _ => None,
        }
    }

    pub fn make_mut<'a, 'schema>(
        &'a self,
        schema: &'schema mut Schema,
    ) -> &'schema mut Node<DirectiveDefinition> {
        match schema.directive_definitions.get_mut(&self.directive_name) {
            Some(directive) => directive,
            None => panic!("Schema has no directive \"{}\"", self.directive_name),
        }
    }

    pub fn try_make_mut<'a, 'schema>(
        &'a self,
        schema: &'schema mut Schema,
    ) -> Option<&'schema mut Node<DirectiveDefinition>> {
        if self.try_get(schema).is_some() {
            Some(self.make_mut(schema))
        } else {
            None
        }
    }
}

impl Display for DirectiveDefinitionLocation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "@{}", self.directive_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DirectiveArgumentDefinitionLocation {
    pub directive_name: Name,
    pub argument_name: Name,
}

impl DirectiveArgumentDefinitionLocation {
    pub fn parent(&self) -> DirectiveDefinitionLocation {
        DirectiveDefinitionLocation {
            directive_name: self.directive_name.clone(),
        }
    }

    pub fn get<'a, 'schema>(
        &'a self,
        schema: &'schema Schema,
    ) -> &'schema Node<InputValueDefinition> {
        let parent = self.parent();
        let type_ = parent.get(schema);

        match type_
            .arguments
            .iter()
            .find(|a| a.name == self.argument_name)
        {
            Some(argument) => argument,
            None => panic!(
                "Directive \"{}\" has no argument \"{}\"",
                parent, self.argument_name
            ),
        }
    }

    pub fn try_get<'a, 'schema>(
        &'a self,
        schema: &'schema Schema,
    ) -> Option<&'schema Node<InputValueDefinition>> {
        let type_ = self.parent().try_get(schema)?;

        match type_
            .arguments
            .iter()
            .find(|a| a.name == self.argument_name)
        {
            Some(argument) => Some(argument),
            None => None,
        }
    }

    pub fn make_mut<'a, 'schema>(
        &'a self,
        schema: &'schema mut Schema,
    ) -> &'schema mut Node<InputValueDefinition> {
        let parent = self.parent();
        let type_ = parent.make_mut(schema).make_mut();

        match type_
            .arguments
            .iter_mut()
            .find(|a| a.name == self.argument_name)
        {
            Some(argument) => argument,
            None => panic!(
                "Directive \"{}\" has no argument \"{}\"",
                parent, self.argument_name
            ),
        }
    }

    pub fn try_make_mut<'a, 'schema>(
        &'a self,
        schema: &'schema mut Schema,
    ) -> Option<&'schema mut Node<InputValueDefinition>> {
        if self.try_get(schema).is_some() {
            Some(self.make_mut(schema))
        } else {
            None
        }
    }
}

impl Display for DirectiveArgumentDefinitionLocation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "@{}({}:)", self.directive_name, self.argument_name)
    }
}
