use std::fmt::{Display, Formatter};
use apollo_parser::ast::AstNode;

#[derive(Debug, Clone)]
pub enum AstNodeEnum {
    Name(apollo_parser::ast::Name),
    Document(apollo_parser::ast::Document),
    OperationDefinition(apollo_parser::ast::OperationDefinition),
    FragmentDefinition(apollo_parser::ast::FragmentDefinition),
    DirectiveDefinition(apollo_parser::ast::DirectiveDefinition),
    SchemaDefinition(apollo_parser::ast::SchemaDefinition),
    ScalarTypeDefinition(apollo_parser::ast::ScalarTypeDefinition),
    ObjectTypeDefinition(apollo_parser::ast::ObjectTypeDefinition),
    InterfaceTypeDefinition(apollo_parser::ast::InterfaceTypeDefinition),
    UnionTypeDefinition(apollo_parser::ast::UnionTypeDefinition),
    EnumTypeDefinition(apollo_parser::ast::EnumTypeDefinition),
    InputObjectTypeDefinition(apollo_parser::ast::InputObjectTypeDefinition),
    SchemaExtension(apollo_parser::ast::SchemaExtension),
    ScalarTypeExtension(apollo_parser::ast::ScalarTypeExtension),
    ObjectTypeExtension(apollo_parser::ast::ObjectTypeExtension),
    InterfaceTypeExtension(apollo_parser::ast::InterfaceTypeExtension),
    UnionTypeExtension(apollo_parser::ast::UnionTypeExtension),
    EnumTypeExtension(apollo_parser::ast::EnumTypeExtension),
    InputObjectTypeExtension(apollo_parser::ast::InputObjectTypeExtension),
    OperationType(apollo_parser::ast::OperationType),
    VariableDefinitions(apollo_parser::ast::VariableDefinitions),
    Directives(apollo_parser::ast::Directives),
    SelectionSet(apollo_parser::ast::SelectionSet),
    Field(apollo_parser::ast::Field),
    FragmentSpread(apollo_parser::ast::FragmentSpread),
    InlineFragment(apollo_parser::ast::InlineFragment),
    Alias(apollo_parser::ast::Alias),
    Arguments(apollo_parser::ast::Arguments),
    Argument(apollo_parser::ast::Argument),
    FragmentName(apollo_parser::ast::FragmentName),
    TypeCondition(apollo_parser::ast::TypeCondition),
    NamedType(apollo_parser::ast::NamedType),
    Variable(apollo_parser::ast::Variable),
    StringValue(apollo_parser::ast::StringValue),
    FloatValue(apollo_parser::ast::FloatValue),
    IntValue(apollo_parser::ast::IntValue),
    BooleanValue(apollo_parser::ast::BooleanValue),
    NullValue(apollo_parser::ast::NullValue),
    EnumValue(apollo_parser::ast::EnumValue),
    ListValue(apollo_parser::ast::ListValue),
    ObjectValue(apollo_parser::ast::ObjectValue),
    ObjectField(apollo_parser::ast::ObjectField),
    VariableDefinition(apollo_parser::ast::VariableDefinition),
    DefaultValue(apollo_parser::ast::DefaultValue),
    ListType(apollo_parser::ast::ListType),
    NonNullType(apollo_parser::ast::NonNullType),
    Directive(apollo_parser::ast::Directive),
    Description(apollo_parser::ast::Description),
    RootOperationTypeDefinition(apollo_parser::ast::RootOperationTypeDefinition),
    ImplementsInterfaces(apollo_parser::ast::ImplementsInterfaces),
    FieldsDefinition(apollo_parser::ast::FieldsDefinition),
    FieldDefinition(apollo_parser::ast::FieldDefinition),
    ArgumentsDefinition(apollo_parser::ast::ArgumentsDefinition),
    InputValueDefinition(apollo_parser::ast::InputValueDefinition),
    UnionMemberTypes(apollo_parser::ast::UnionMemberTypes),
    EnumValuesDefinition(apollo_parser::ast::EnumValuesDefinition),
    EnumValueDefinition(apollo_parser::ast::EnumValueDefinition),
    InputFieldsDefinition(apollo_parser::ast::InputFieldsDefinition),
    DirectiveLocations(apollo_parser::ast::DirectiveLocations),
    Definition(apollo_parser::ast::Definition),
    Selection(apollo_parser::ast::Selection),
    Value(apollo_parser::ast::Value),
    Type(apollo_parser::ast::Type),
}

// Would be really nice if we could macro the below, but enum_dispatch and similar crates only seem
// to work when both the enum and trait are in this crate. (The docs describe it as a limitation of
// procedural macros.)
impl Display for AstNodeEnum {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let source_string = match self {
            AstNodeEnum::Name(inner) => inner.source_string(),
            AstNodeEnum::Document(inner) => inner.source_string(),
            AstNodeEnum::OperationDefinition(inner) => inner.source_string(),
            AstNodeEnum::FragmentDefinition(inner) => inner.source_string(),
            AstNodeEnum::DirectiveDefinition(inner) => inner.source_string(),
            AstNodeEnum::SchemaDefinition(inner) => inner.source_string(),
            AstNodeEnum::ScalarTypeDefinition(inner) => inner.source_string(),
            AstNodeEnum::ObjectTypeDefinition(inner) => inner.source_string(),
            AstNodeEnum::InterfaceTypeDefinition(inner) => inner.source_string(),
            AstNodeEnum::UnionTypeDefinition(inner) => inner.source_string(),
            AstNodeEnum::EnumTypeDefinition(inner) => inner.source_string(),
            AstNodeEnum::InputObjectTypeDefinition(inner) => inner.source_string(),
            AstNodeEnum::SchemaExtension(inner) => inner.source_string(),
            AstNodeEnum::ScalarTypeExtension(inner) => inner.source_string(),
            AstNodeEnum::ObjectTypeExtension(inner) => inner.source_string(),
            AstNodeEnum::InterfaceTypeExtension(inner) => inner.source_string(),
            AstNodeEnum::UnionTypeExtension(inner) => inner.source_string(),
            AstNodeEnum::EnumTypeExtension(inner) => inner.source_string(),
            AstNodeEnum::InputObjectTypeExtension(inner) => inner.source_string(),
            AstNodeEnum::OperationType(inner) => inner.source_string(),
            AstNodeEnum::VariableDefinitions(inner) => inner.source_string(),
            AstNodeEnum::Directives(inner) => inner.source_string(),
            AstNodeEnum::SelectionSet(inner) => inner.source_string(),
            AstNodeEnum::Field(inner) => inner.source_string(),
            AstNodeEnum::FragmentSpread(inner) => inner.source_string(),
            AstNodeEnum::InlineFragment(inner) => inner.source_string(),
            AstNodeEnum::Alias(inner) => inner.source_string(),
            AstNodeEnum::Arguments(inner) => inner.source_string(),
            AstNodeEnum::Argument(inner) => inner.source_string(),
            AstNodeEnum::FragmentName(inner) => inner.source_string(),
            AstNodeEnum::TypeCondition(inner) => inner.source_string(),
            AstNodeEnum::NamedType(inner) => inner.source_string(),
            AstNodeEnum::Variable(inner) => inner.source_string(),
            AstNodeEnum::StringValue(inner) => inner.source_string(),
            AstNodeEnum::FloatValue(inner) => inner.source_string(),
            AstNodeEnum::IntValue(inner) => inner.source_string(),
            AstNodeEnum::BooleanValue(inner) => inner.source_string(),
            AstNodeEnum::NullValue(inner) => inner.source_string(),
            AstNodeEnum::EnumValue(inner) => inner.source_string(),
            AstNodeEnum::ListValue(inner) => inner.source_string(),
            AstNodeEnum::ObjectValue(inner) => inner.source_string(),
            AstNodeEnum::ObjectField(inner) => inner.source_string(),
            AstNodeEnum::VariableDefinition(inner) => inner.source_string(),
            AstNodeEnum::DefaultValue(inner) => inner.source_string(),
            AstNodeEnum::ListType(inner) => inner.source_string(),
            AstNodeEnum::NonNullType(inner) => inner.source_string(),
            AstNodeEnum::Directive(inner) => inner.source_string(),
            AstNodeEnum::Description(inner) => inner.source_string(),
            AstNodeEnum::RootOperationTypeDefinition(inner) => inner.source_string(),
            AstNodeEnum::ImplementsInterfaces(inner) => inner.source_string(),
            AstNodeEnum::FieldsDefinition(inner) => inner.source_string(),
            AstNodeEnum::FieldDefinition(inner) => inner.source_string(),
            AstNodeEnum::ArgumentsDefinition(inner) => inner.source_string(),
            AstNodeEnum::InputValueDefinition(inner) => inner.source_string(),
            AstNodeEnum::UnionMemberTypes(inner) => inner.source_string(),
            AstNodeEnum::EnumValuesDefinition(inner) => inner.source_string(),
            AstNodeEnum::EnumValueDefinition(inner) => inner.source_string(),
            AstNodeEnum::InputFieldsDefinition(inner) => inner.source_string(),
            AstNodeEnum::DirectiveLocations(inner) => inner.source_string(),
            AstNodeEnum::Definition(inner) => inner.source_string(),
            AstNodeEnum::Selection(inner) => inner.source_string(),
            AstNodeEnum::Value(inner) => inner.source_string(),
            AstNodeEnum::Type(inner) => inner.source_string(),
        };
        write!(f, "{}", source_string)
    }
}

impl From<apollo_parser::ast::Name> for AstNodeEnum {
    fn from(value: apollo_parser::ast::Name) -> Self {
        AstNodeEnum::Name(value)
    }
}

impl From<apollo_parser::ast::Document> for AstNodeEnum {
    fn from(value: apollo_parser::ast::Document) -> Self {
        AstNodeEnum::Document(value)
    }
}

impl From<apollo_parser::ast::OperationDefinition> for AstNodeEnum {
    fn from(value: apollo_parser::ast::OperationDefinition) -> Self {
        AstNodeEnum::OperationDefinition(value)
    }
}

impl From<apollo_parser::ast::FragmentDefinition> for AstNodeEnum {
    fn from(value: apollo_parser::ast::FragmentDefinition) -> Self {
        AstNodeEnum::FragmentDefinition(value)
    }
}

impl From<apollo_parser::ast::DirectiveDefinition> for AstNodeEnum {
    fn from(value: apollo_parser::ast::DirectiveDefinition) -> Self {
        AstNodeEnum::DirectiveDefinition(value)
    }
}

impl From<apollo_parser::ast::SchemaDefinition> for AstNodeEnum {
    fn from(value: apollo_parser::ast::SchemaDefinition) -> Self {
        AstNodeEnum::SchemaDefinition(value)
    }
}

impl From<apollo_parser::ast::ScalarTypeDefinition> for AstNodeEnum {
    fn from(value: apollo_parser::ast::ScalarTypeDefinition) -> Self {
        AstNodeEnum::ScalarTypeDefinition(value)
    }
}

impl From<apollo_parser::ast::ObjectTypeDefinition> for AstNodeEnum {
    fn from(value: apollo_parser::ast::ObjectTypeDefinition) -> Self {
        AstNodeEnum::ObjectTypeDefinition(value)
    }
}

impl From<apollo_parser::ast::InterfaceTypeDefinition> for AstNodeEnum {
    fn from(value: apollo_parser::ast::InterfaceTypeDefinition) -> Self {
        AstNodeEnum::InterfaceTypeDefinition(value)
    }
}

impl From<apollo_parser::ast::UnionTypeDefinition> for AstNodeEnum {
    fn from(value: apollo_parser::ast::UnionTypeDefinition) -> Self {
        AstNodeEnum::UnionTypeDefinition(value)
    }
}

impl From<apollo_parser::ast::EnumTypeDefinition> for AstNodeEnum {
    fn from(value: apollo_parser::ast::EnumTypeDefinition) -> Self {
        AstNodeEnum::EnumTypeDefinition(value)
    }
}

impl From<apollo_parser::ast::InputObjectTypeDefinition> for AstNodeEnum {
    fn from(value: apollo_parser::ast::InputObjectTypeDefinition) -> Self {
        AstNodeEnum::InputObjectTypeDefinition(value)
    }
}

impl From<apollo_parser::ast::SchemaExtension> for AstNodeEnum {
    fn from(value: apollo_parser::ast::SchemaExtension) -> Self {
        AstNodeEnum::SchemaExtension(value)
    }
}

impl From<apollo_parser::ast::ScalarTypeExtension> for AstNodeEnum {
    fn from(value: apollo_parser::ast::ScalarTypeExtension) -> Self {
        AstNodeEnum::ScalarTypeExtension(value)
    }
}

impl From<apollo_parser::ast::ObjectTypeExtension> for AstNodeEnum {
    fn from(value: apollo_parser::ast::ObjectTypeExtension) -> Self {
        AstNodeEnum::ObjectTypeExtension(value)
    }
}

impl From<apollo_parser::ast::InterfaceTypeExtension> for AstNodeEnum {
    fn from(value: apollo_parser::ast::InterfaceTypeExtension) -> Self {
        AstNodeEnum::InterfaceTypeExtension(value)
    }
}

impl From<apollo_parser::ast::UnionTypeExtension> for AstNodeEnum {
    fn from(value: apollo_parser::ast::UnionTypeExtension) -> Self {
        AstNodeEnum::UnionTypeExtension(value)
    }
}

impl From<apollo_parser::ast::EnumTypeExtension> for AstNodeEnum {
    fn from(value: apollo_parser::ast::EnumTypeExtension) -> Self {
        AstNodeEnum::EnumTypeExtension(value)
    }
}

impl From<apollo_parser::ast::InputObjectTypeExtension> for AstNodeEnum {
    fn from(value: apollo_parser::ast::InputObjectTypeExtension) -> Self {
        AstNodeEnum::InputObjectTypeExtension(value)
    }
}

impl From<apollo_parser::ast::OperationType> for AstNodeEnum {
    fn from(value: apollo_parser::ast::OperationType) -> Self {
        AstNodeEnum::OperationType(value)
    }
}

impl From<apollo_parser::ast::VariableDefinitions> for AstNodeEnum {
    fn from(value: apollo_parser::ast::VariableDefinitions) -> Self {
        AstNodeEnum::VariableDefinitions(value)
    }
}

impl From<apollo_parser::ast::Directives> for AstNodeEnum {
    fn from(value: apollo_parser::ast::Directives) -> Self {
        AstNodeEnum::Directives(value)
    }
}

impl From<apollo_parser::ast::SelectionSet> for AstNodeEnum {
    fn from(value: apollo_parser::ast::SelectionSet) -> Self {
        AstNodeEnum::SelectionSet(value)
    }
}

impl From<apollo_parser::ast::Field> for AstNodeEnum {
    fn from(value: apollo_parser::ast::Field) -> Self {
        AstNodeEnum::Field(value)
    }
}

impl From<apollo_parser::ast::FragmentSpread> for AstNodeEnum {
    fn from(value: apollo_parser::ast::FragmentSpread) -> Self {
        AstNodeEnum::FragmentSpread(value)
    }
}

impl From<apollo_parser::ast::InlineFragment> for AstNodeEnum {
    fn from(value: apollo_parser::ast::InlineFragment) -> Self {
        AstNodeEnum::InlineFragment(value)
    }
}

impl From<apollo_parser::ast::Alias> for AstNodeEnum {
    fn from(value: apollo_parser::ast::Alias) -> Self {
        AstNodeEnum::Alias(value)
    }
}

impl From<apollo_parser::ast::Arguments> for AstNodeEnum {
    fn from(value: apollo_parser::ast::Arguments) -> Self {
        AstNodeEnum::Arguments(value)
    }
}

impl From<apollo_parser::ast::Argument> for AstNodeEnum {
    fn from(value: apollo_parser::ast::Argument) -> Self {
        AstNodeEnum::Argument(value)
    }
}

impl From<apollo_parser::ast::FragmentName> for AstNodeEnum {
    fn from(value: apollo_parser::ast::FragmentName) -> Self {
        AstNodeEnum::FragmentName(value)
    }
}

impl From<apollo_parser::ast::TypeCondition> for AstNodeEnum {
    fn from(value: apollo_parser::ast::TypeCondition) -> Self {
        AstNodeEnum::TypeCondition(value)
    }
}

impl From<apollo_parser::ast::NamedType> for AstNodeEnum {
    fn from(value: apollo_parser::ast::NamedType) -> Self {
        AstNodeEnum::NamedType(value)
    }
}

impl From<apollo_parser::ast::Variable> for AstNodeEnum {
    fn from(value: apollo_parser::ast::Variable) -> Self {
        AstNodeEnum::Variable(value)
    }
}

impl From<apollo_parser::ast::StringValue> for AstNodeEnum {
    fn from(value: apollo_parser::ast::StringValue) -> Self {
        AstNodeEnum::StringValue(value)
    }
}

impl From<apollo_parser::ast::FloatValue> for AstNodeEnum {
    fn from(value: apollo_parser::ast::FloatValue) -> Self {
        AstNodeEnum::FloatValue(value)
    }
}

impl From<apollo_parser::ast::IntValue> for AstNodeEnum {
    fn from(value: apollo_parser::ast::IntValue) -> Self {
        AstNodeEnum::IntValue(value)
    }
}

impl From<apollo_parser::ast::BooleanValue> for AstNodeEnum {
    fn from(value: apollo_parser::ast::BooleanValue) -> Self {
        AstNodeEnum::BooleanValue(value)
    }
}

impl From<apollo_parser::ast::NullValue> for AstNodeEnum {
    fn from(value: apollo_parser::ast::NullValue) -> Self {
        AstNodeEnum::NullValue(value)
    }
}

impl From<apollo_parser::ast::EnumValue> for AstNodeEnum {
    fn from(value: apollo_parser::ast::EnumValue) -> Self {
        AstNodeEnum::EnumValue(value)
    }
}

impl From<apollo_parser::ast::ListValue> for AstNodeEnum {
    fn from(value: apollo_parser::ast::ListValue) -> Self {
        AstNodeEnum::ListValue(value)
    }
}

impl From<apollo_parser::ast::ObjectValue> for AstNodeEnum {
    fn from(value: apollo_parser::ast::ObjectValue) -> Self {
        AstNodeEnum::ObjectValue(value)
    }
}

impl From<apollo_parser::ast::ObjectField> for AstNodeEnum {
    fn from(value: apollo_parser::ast::ObjectField) -> Self {
        AstNodeEnum::ObjectField(value)
    }
}

impl From<apollo_parser::ast::VariableDefinition> for AstNodeEnum {
    fn from(value: apollo_parser::ast::VariableDefinition) -> Self {
        AstNodeEnum::VariableDefinition(value)
    }
}

impl From<apollo_parser::ast::DefaultValue> for AstNodeEnum {
    fn from(value: apollo_parser::ast::DefaultValue) -> Self {
        AstNodeEnum::DefaultValue(value)
    }
}

impl From<apollo_parser::ast::ListType> for AstNodeEnum {
    fn from(value: apollo_parser::ast::ListType) -> Self {
        AstNodeEnum::ListType(value)
    }
}

impl From<apollo_parser::ast::NonNullType> for AstNodeEnum {
    fn from(value: apollo_parser::ast::NonNullType) -> Self {
        AstNodeEnum::NonNullType(value)
    }
}

impl From<apollo_parser::ast::Directive> for AstNodeEnum {
    fn from(value: apollo_parser::ast::Directive) -> Self {
        AstNodeEnum::Directive(value)
    }
}

impl From<apollo_parser::ast::Description> for AstNodeEnum {
    fn from(value: apollo_parser::ast::Description) -> Self {
        AstNodeEnum::Description(value)
    }
}

impl From<apollo_parser::ast::RootOperationTypeDefinition> for AstNodeEnum {
    fn from(value: apollo_parser::ast::RootOperationTypeDefinition) -> Self {
        AstNodeEnum::RootOperationTypeDefinition(value)
    }
}

impl From<apollo_parser::ast::ImplementsInterfaces> for AstNodeEnum {
    fn from(value: apollo_parser::ast::ImplementsInterfaces) -> Self {
        AstNodeEnum::ImplementsInterfaces(value)
    }
}

impl From<apollo_parser::ast::FieldsDefinition> for AstNodeEnum {
    fn from(value: apollo_parser::ast::FieldsDefinition) -> Self {
        AstNodeEnum::FieldsDefinition(value)
    }
}

impl From<apollo_parser::ast::FieldDefinition> for AstNodeEnum {
    fn from(value: apollo_parser::ast::FieldDefinition) -> Self {
        AstNodeEnum::FieldDefinition(value)
    }
}

impl From<apollo_parser::ast::ArgumentsDefinition> for AstNodeEnum {
    fn from(value: apollo_parser::ast::ArgumentsDefinition) -> Self {
        AstNodeEnum::ArgumentsDefinition(value)
    }
}

impl From<apollo_parser::ast::InputValueDefinition> for AstNodeEnum {
    fn from(value: apollo_parser::ast::InputValueDefinition) -> Self {
        AstNodeEnum::InputValueDefinition(value)
    }
}

impl From<apollo_parser::ast::UnionMemberTypes> for AstNodeEnum {
    fn from(value: apollo_parser::ast::UnionMemberTypes) -> Self {
        AstNodeEnum::UnionMemberTypes(value)
    }
}

impl From<apollo_parser::ast::EnumValuesDefinition> for AstNodeEnum {
    fn from(value: apollo_parser::ast::EnumValuesDefinition) -> Self {
        AstNodeEnum::EnumValuesDefinition(value)
    }
}

impl From<apollo_parser::ast::EnumValueDefinition> for AstNodeEnum {
    fn from(value: apollo_parser::ast::EnumValueDefinition) -> Self {
        AstNodeEnum::EnumValueDefinition(value)
    }
}

impl From<apollo_parser::ast::InputFieldsDefinition> for AstNodeEnum {
    fn from(value: apollo_parser::ast::InputFieldsDefinition) -> Self {
        AstNodeEnum::InputFieldsDefinition(value)
    }
}

impl From<apollo_parser::ast::DirectiveLocations> for AstNodeEnum {
    fn from(value: apollo_parser::ast::DirectiveLocations) -> Self {
        AstNodeEnum::DirectiveLocations(value)
    }
}

impl From<apollo_parser::ast::Definition> for AstNodeEnum {
    fn from(value: apollo_parser::ast::Definition) -> Self {
        AstNodeEnum::Definition(value)
    }
}

impl From<apollo_parser::ast::Selection> for AstNodeEnum {
    fn from(value: apollo_parser::ast::Selection) -> Self {
        AstNodeEnum::Selection(value)
    }
}

impl From<apollo_parser::ast::Value> for AstNodeEnum {
    fn from(value: apollo_parser::ast::Value) -> Self {
        AstNodeEnum::Value(value)
    }
}

impl From<apollo_parser::ast::Type> for AstNodeEnum {
    fn from(value: apollo_parser::ast::Type) -> Self {
        AstNodeEnum::Type(value)
    }
}
