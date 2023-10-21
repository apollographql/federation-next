use apollo_compiler::ast::{
    Argument, Field, FragmentDefinition, FragmentSpread, InlineFragment, OperationDefinition,
    VariableDefinition,
};
use apollo_compiler::schema::{
    Directive, DirectiveDefinition, EnumType, EnumValueDefinition, FieldDefinition,
    InputObjectType, InputValueDefinition, InterfaceType, ObjectType, ScalarType, SchemaDefinition,
    Type, UnionType, Value,
};
use apollo_compiler::Node;

#[derive(Clone, Debug)]
pub enum AstNode {
    OperationDefinition(Node<OperationDefinition>),
    FragmentDefinition(Node<FragmentDefinition>),
    VariableDefinition(Node<VariableDefinition>),
    Field(Node<Field>),
    FragmentSpread(Node<FragmentSpread>),
    InlineFragment(Node<InlineFragment>),
    Directive(Node<Directive>),
    Argument(Node<Argument>),
    Type(Node<Type>),
    Value(Node<Value>),
    DirectiveDefinition(Node<DirectiveDefinition>),
    SchemaDefinition(Node<SchemaDefinition>),
    ScalarType(Node<ScalarType>),
    ObjectType(Node<ObjectType>),
    InterfaceType(Node<InterfaceType>),
    UnionType(Node<UnionType>),
    EnumType(Node<EnumType>),
    InputObjectType(Node<InputObjectType>),
    FieldDefinition(Node<FieldDefinition>),
    InputValueDefinition(Node<InputValueDefinition>),
    EnumValueDefinition(Node<EnumValueDefinition>),
}

impl From<Node<OperationDefinition>> for AstNode {
    fn from(value: Node<OperationDefinition>) -> Self {
        AstNode::OperationDefinition(value)
    }
}

impl From<Node<FragmentDefinition>> for AstNode {
    fn from(value: Node<FragmentDefinition>) -> Self {
        AstNode::FragmentDefinition(value)
    }
}

impl From<Node<VariableDefinition>> for AstNode {
    fn from(value: Node<VariableDefinition>) -> Self {
        AstNode::VariableDefinition(value)
    }
}

impl From<Node<Field>> for AstNode {
    fn from(value: Node<Field>) -> Self {
        AstNode::Field(value)
    }
}

impl From<Node<FragmentSpread>> for AstNode {
    fn from(value: Node<FragmentSpread>) -> Self {
        AstNode::FragmentSpread(value)
    }
}

impl From<Node<InlineFragment>> for AstNode {
    fn from(value: Node<InlineFragment>) -> Self {
        AstNode::InlineFragment(value)
    }
}

impl From<Node<Directive>> for AstNode {
    fn from(value: Node<Directive>) -> Self {
        AstNode::Directive(value)
    }
}

impl From<Node<Argument>> for AstNode {
    fn from(value: Node<Argument>) -> Self {
        AstNode::Argument(value)
    }
}

impl From<Node<Type>> for AstNode {
    fn from(value: Node<Type>) -> Self {
        AstNode::Type(value)
    }
}

impl From<Node<Value>> for AstNode {
    fn from(value: Node<Value>) -> Self {
        AstNode::Value(value)
    }
}

impl From<Node<DirectiveDefinition>> for AstNode {
    fn from(value: Node<DirectiveDefinition>) -> Self {
        AstNode::DirectiveDefinition(value)
    }
}

impl From<Node<SchemaDefinition>> for AstNode {
    fn from(value: Node<SchemaDefinition>) -> Self {
        AstNode::SchemaDefinition(value)
    }
}

impl From<Node<ScalarType>> for AstNode {
    fn from(value: Node<ScalarType>) -> Self {
        AstNode::ScalarType(value)
    }
}

impl From<Node<ObjectType>> for AstNode {
    fn from(value: Node<ObjectType>) -> Self {
        AstNode::ObjectType(value)
    }
}

impl From<Node<InterfaceType>> for AstNode {
    fn from(value: Node<InterfaceType>) -> Self {
        AstNode::InterfaceType(value)
    }
}

impl From<Node<UnionType>> for AstNode {
    fn from(value: Node<UnionType>) -> Self {
        AstNode::UnionType(value)
    }
}

impl From<Node<EnumType>> for AstNode {
    fn from(value: Node<EnumType>) -> Self {
        AstNode::EnumType(value)
    }
}

impl From<Node<InputObjectType>> for AstNode {
    fn from(value: Node<InputObjectType>) -> Self {
        AstNode::InputObjectType(value)
    }
}

impl From<Node<FieldDefinition>> for AstNode {
    fn from(value: Node<FieldDefinition>) -> Self {
        AstNode::FieldDefinition(value)
    }
}

impl From<Node<InputValueDefinition>> for AstNode {
    fn from(value: Node<InputValueDefinition>) -> Self {
        AstNode::InputValueDefinition(value)
    }
}

impl From<Node<EnumValueDefinition>> for AstNode {
    fn from(value: Node<EnumValueDefinition>) -> Self {
        AstNode::EnumValueDefinition(value)
    }
}
