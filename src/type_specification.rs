use indexmap::{IndexMap, IndexSet};

use apollo_compiler::ast::{FieldDefinition, Value};
use apollo_compiler::schema::{
    Component, ComponentName, EnumType, EnumValueDefinition, ExtendedType, InputValueDefinition,
    Name, ObjectType, ScalarType, Type, UnionType,
};
use apollo_compiler::Node;

use crate::error::{FederationError, MultipleFederationErrors, SingleFederationError};
use crate::schema::position::{
    EnumTypeDefinitionPosition, ObjectTypeDefinitionPosition, ScalarTypeDefinitionPosition,
    TypeDefinitionPosition, UnionTypeDefinitionPosition,
};
use crate::schema::FederationSchema;

pub(crate) trait TypeSpecification {
    // PORT_NOTE: The JS version takes additional optional arguments `feature` and `asBuiltIn`.
    fn check_or_add(&self, schema: &mut FederationSchema) -> Result<(), FederationError>;
}

pub(crate) struct ScalarTypeSpecification {
    pub name: Name, // Type's name
}

impl TypeSpecification for ScalarTypeSpecification {
    fn check_or_add(&self, schema: &mut FederationSchema) -> Result<(), FederationError> {
        let existing = schema.try_get_type(self.name.clone());
        if let Some(existing) = existing {
            // Ignore redundant type specifications if they are are both scalar types.
            return ensure_expected_type_kind(TypeKind::Scalar, &existing);
        }

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
        let mut field_map = IndexMap::new();
        for ref field_spec in (self.fields)(schema) {
            let field_def: FieldDefinition = field_spec.into();
            field_map.insert(field_spec.name.clone(), Component::new(field_def));
        }

        let existing = schema.try_get_type(self.name.clone());
        if let Some(existing) = existing {
            // ensure existing definition is an object type
            ensure_expected_type_kind(TypeKind::Object, &existing)?;
            let existing_type = existing.get(schema.schema())?;
            let ExtendedType::Object(existing_obj_type) = existing_type else {
                return Err(FederationError::internal(format!(
                    "Expected ExtendedType::Object but got {}",
                    TypeKind::from(existing_type)
                )));
            };

            // ensure all expected fields are present in the existing object type
            let errors = ensure_same_fields(existing_obj_type, &field_map, schema);
            return MultipleFederationErrors::from_iter(errors).into_result();
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
                fields: field_map,
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
        let members = (self.members)(schema);
        let existing = schema.try_get_type(self.name.clone());

        // ensure new union has at least one member
        if members.is_empty() {
            if existing.is_some() {
                let union_type_name = &self.name;
                return Err(SingleFederationError::TypeDefinitionInvalid {
                    message: format!("Invalid definition of type {union_type_name}: expected the union type to not exist/have no members but it is defined.")
                }.into());
            }
            return Ok(()); // silently ignore empty unions
        }

        // ensure new union has the same members as the existing union
        if let Some(existing) = existing {
            ensure_expected_type_kind(TypeKind::Union, &existing)?;
            let existing_type = existing.get(schema.schema())?;
            let ExtendedType::Union(existing_union_type) = existing_type else {
                return Err(FederationError::internal(format!(
                    "Expected ExtendedType::Object but got {}",
                    TypeKind::from(existing_type)
                )));
            };
            if existing_union_type.members != members {
                let union_type_name = &self.name;
                let expected_member_names: Vec<String> = existing_union_type
                    .members
                    .iter()
                    .map(|name| name.to_string())
                    .collect();
                let actual_member_names: Vec<String> =
                    members.iter().map(|name| name.to_string()).collect();
                return Err(SingleFederationError::TypeDefinitionInvalid {
                    message: format!("Invalid definition of type {union_type_name}: expected members [{}] but found [{}]",
                    expected_member_names.join(", "), actual_member_names.join(", "))
                }.into());
            }
            return Ok(());
        }

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
                members,
            }),
        )
    }
}

pub(crate) struct EnumValueSpecification {
    pub name: Name,
    pub description: Option<String>,
}

pub(crate) struct EnumTypeSpecification {
    pub name: Name,
    pub values: Vec<EnumValueSpecification>,
}

impl TypeSpecification for EnumTypeSpecification {
    fn check_or_add(&self, schema: &mut FederationSchema) -> Result<(), FederationError> {
        let existing = schema.try_get_type(self.name.clone());
        if let Some(existing) = existing {
            ensure_expected_type_kind(TypeKind::Enum, &existing)?;
            let existing_type = existing.get(schema.schema())?;
            let ExtendedType::Enum(existing_type) = existing_type else {
                return Err(FederationError::internal(format!(
                    "Expected ExtendedType::Union but got {}",
                    TypeKind::from(existing_type)
                )));
            };

            let existing_value_set: IndexSet<Name> = existing_type
                .values
                .iter()
                .map(|val| val.0.clone())
                .collect();
            let actual_value_set: IndexSet<Name> =
                self.values.iter().map(|val| val.name.clone()).collect();
            if existing_value_set != actual_value_set {
                let enum_type_name = &self.name;
                let expected_value_names: Vec<String> = existing_value_set
                    .iter()
                    .map(|name| name.to_string())
                    .collect();
                let actual_value_names: Vec<String> = actual_value_set
                    .iter()
                    .map(|name| name.to_string())
                    .collect();
                return Err(SingleFederationError::TypeDefinitionInvalid {
                    message: format!("Invalid definition of type {enum_type_name}: expected values [{}] but found [{}].",
                    expected_value_names.join(", "), actual_value_names.join(", "))
                }.into());
            }
            return Ok(());
        }

        let type_pos = EnumTypeDefinitionPosition {
            type_name: self.name.clone(),
        };
        type_pos.pre_insert(schema)?;
        type_pos.insert(
            schema,
            Node::new(EnumType {
                description: None,
                name: type_pos.type_name.clone(),
                directives: Default::default(),
                values: self
                    .values
                    .iter()
                    .map(|val| {
                        (
                            val.name.clone(),
                            Component::new(EnumValueDefinition {
                                description: val.description.as_ref().map(|s| s.into()),
                                value: val.name.clone(),
                                directives: Default::default(),
                            }),
                        )
                    })
                    .collect(),
            }),
        )
    }
}

//////////////////////////////////////////////////////////////////////////////
// Helper functions for TypeSpecification implementations

// TODO: Consider moving this to elsewhere.
fn value_to_string(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Boolean(true) => "true".to_string(),
        Value::Boolean(false) => "false".to_string(),
        Value::Int(num) => format!("{num}"),
        Value::Float(num) => format!("{num}"),
        Value::String(str) => format!("{str}"),
        Value::Enum(name) => format!("{name}"),
        Value::Variable(name) => format!("${name}"),
        Value::List(items) => {
            let item_strings: Vec<_> = items.iter().map(|val| value_to_string(val)).collect();
            format!("[{}]", item_strings.join(", "))
        }
        Value::Object(fields) => {
            let field_strings: Vec<_> = fields
                .iter()
                .map(|(name, val)| format!("{name}: {}", value_to_string(val)))
                .collect();
            format!("{{{}}}", field_strings.join(", "))
        }
    }
}

// TODO: Consider moving this to the schema module.
#[derive(Clone, PartialEq, Eq, Hash, derive_more::Display)]
pub(crate) enum TypeKind {
    Scalar,
    Object,
    Interface,
    Union,
    Enum,
    InputObject,
}

impl From<&ExtendedType> for TypeKind {
    fn from(value: &ExtendedType) -> Self {
        match value {
            ExtendedType::Scalar(_) => TypeKind::Scalar,
            ExtendedType::Object(_) => TypeKind::Object,
            ExtendedType::Interface(_) => TypeKind::Interface,
            ExtendedType::Union(_) => TypeKind::Union,
            ExtendedType::Enum(_) => TypeKind::Enum,
            ExtendedType::InputObject(_) => TypeKind::InputObject,
        }
    }
}

impl From<&TypeDefinitionPosition> for TypeKind {
    fn from(value: &TypeDefinitionPosition) -> Self {
        match value {
            TypeDefinitionPosition::Scalar(_) => TypeKind::Scalar,
            TypeDefinitionPosition::Object(_) => TypeKind::Object,
            TypeDefinitionPosition::Interface(_) => TypeKind::Interface,
            TypeDefinitionPosition::Union(_) => TypeKind::Union,
            TypeDefinitionPosition::Enum(_) => TypeKind::Enum,
            TypeDefinitionPosition::InputObject(_) => TypeKind::InputObject,
        }
    }
}

fn ensure_expected_type_kind(
    expected: TypeKind,
    actual: &TypeDefinitionPosition,
) -> Result<(), FederationError> {
    let actual_kind: TypeKind = TypeKind::from(actual);
    if expected != actual_kind {
        Ok(())
    } else {
        let actual_type_name = actual.type_name();
        Err(SingleFederationError::TypeDefinitionInvalid {
            message: format!("Invalid definition for type {actual_type_name}: {actual_type_name} should be a {expected} but is defined as a {actual_kind}")
        }.into())
    }
}

/// Note: Non-null/list wrappers are ignored.
fn is_custom_scalar(ty: &Type, schema: &FederationSchema) -> bool {
    let type_name = ty.inner_named_type().as_str();
    schema
        .schema()
        .get_scalar(type_name)
        .is_some_and(|scalar| !scalar.is_built_in())
}

fn is_valid_input_type_redefinition(
    expected_type: &Type,
    actual_type: &Type,
    schema: &FederationSchema,
) -> bool {
    // If the expected type is a custom scalar, then we allow the redefinition to be another type (unless it's a custom scalar, in which
    // case it has to be the same scalar). The rational being that since graphQL does no validation of values passed to a custom scalar,
    // any code that gets some value as input for a custom scalar has to do validation manually, and so there is little harm in allowing
    // a redefinition with another type since any truly invalid value would failed that "manual validation". In practice, this leeway
    // make sense because many scalar will tend to accept only one kind of values (say, strings) and exists only to inform that said string
    // needs to follow a specific format, and in such case, letting user redefine the type as String adds flexibility while doing little harm.
    if expected_type.is_list() {
        return actual_type.is_list()
            && is_valid_input_type_redefinition(
                expected_type.item_type(),
                actual_type.item_type(),
                schema,
            );
    }
    if expected_type.is_non_null() {
        return actual_type.is_non_null()
            && is_valid_input_type_redefinition(
                &expected_type.clone().nullable(),
                &actual_type.clone().nullable(),
                schema,
            );
    }
    // invariant: expected_type/actual_type is not a list or a non-null type (thus a named type).
    is_custom_scalar(expected_type, schema) && !is_custom_scalar(actual_type, schema)
}

fn default_value_message(value: Option<&Value>) -> String {
    match value {
        None => "no default value".to_string(),
        Some(value) => format!("default value {}", value_to_string(value)),
    }
}

// TODO: Generalize this function to support both field and directive definitions.
fn ensure_same_arguments(
    expected: &FieldDefinition,
    actual: &FieldDefinition,
    what: &str,
    schema: &FederationSchema,
) -> Vec<SingleFederationError> {
    let mut errors = vec![];

    // ensure expected arguments are a subset of actual arguments.
    for expected_arg in &expected.arguments {
        let actual_arg = actual.argument_by_name(&expected_arg.name);
        if actual_arg.is_none() {
            // Not declaring an optional argument is ok: that means you won't be able to pass a non-default value in your schema, but we allow you that.
            // But missing a required argument it not ok.
            if expected_arg.ty.is_non_null() && expected_arg.default_value.is_none() {
                let expected_arg_name = &expected_arg.name;
                errors.push(SingleFederationError::TypeDefinitionInvalid {
                    message: format!(
                        r#"Invalid definition for {what}: Missing required argument "{expected_arg_name}""#
                    )
                });
            }
            continue;
        }

        // ensure expected argument and actual argument have the same type.
        let actual_arg = actual_arg.unwrap();
        // TODO: Make it easy to get a cloned (inner) type from a Node<Type>.
        let mut actual_type = (*(actual_arg.ty)).clone();
        if actual_type.is_non_null() && !expected_arg.ty.is_non_null() {
            // It's ok to redefine an optional argument as mandatory. For instance, if you want to force people on your team to provide a "deprecation reason", you can
            // redefine @deprecated as `directive @deprecated(reason: String!)...` to get validation. In other words, you are allowed to always pass an argument that
            // is optional if you so wish.
            actual_type = actual_type.nullable();
        }
        // ensure argument type is compatible with the expected one and
        // argument's default value (if any) is compatible with the expected one
        if *expected_arg.ty != actual_type
            && is_valid_input_type_redefinition(&expected_arg.ty, &actual_type, schema)
        {
            let arg_name = &expected_arg.name;
            let expected_type = &expected_arg.ty;
            errors.push(SingleFederationError::TypeDefinitionInvalid {
                message: format!(
                    r#"Invalid definition for {what}: Argument "{arg_name}" should have type {expected_type} but found type {actual_type}"#
                )
            });
        } else if !actual_type.is_non_null()
            && expected_arg.default_value != actual_arg.default_value
        {
            let arg_name = &expected_arg.name;
            let expected_value = default_value_message(expected_arg.default_value.as_deref());
            let actual_value = default_value_message(actual_arg.default_value.as_deref());
            errors.push(SingleFederationError::TypeDefinitionInvalid {
                message: format!(
                    r#"Invalid definition for {what}: Argument "{arg_name}" should have {expected_value} but found {actual_value}"#
                )
            });
        }
    }

    // ensure actual arguments are a subset of expected arguments.
    for actual_arg in &actual.arguments {
        let expected_arg = expected.argument_by_name(&actual_arg.name);
        if expected_arg.is_none() {
            let arg_name = &actual_arg.name;
            errors.push(SingleFederationError::TypeDefinitionInvalid {
                message: format!(
                    r#"Invalid definition for {what}: unknown/unsupported argument "{arg_name}""#
                ),
            });
            // fall through to the next iteration
        }
    }

    errors
}

fn ensure_same_fields(
    existing_obj_type: &ObjectType,
    actual_fields: &IndexMap<Name, Component<FieldDefinition>>,
    schema: &FederationSchema,
) -> Vec<SingleFederationError> {
    let obj_type_name = existing_obj_type.name.clone();
    let mut errors = vec![];

    // ensure all actual fields are a subset of the existing object type's fields.
    for (actual_field_name, actual_field_def) in actual_fields {
        let expected_field = existing_obj_type.fields.get(actual_field_name);
        if expected_field.is_none() {
            errors.push(SingleFederationError::TypeDefinitionInvalid {
                message: format!(
                    "Invalid definition of type {}: missing field {}",
                    obj_type_name, actual_field_name
                ),
            });
            continue;
        }

        // ensure field types are as expected
        let expected_field = expected_field.unwrap();
        if actual_field_def.ty != expected_field.ty {
            let expected_field_type = &expected_field.ty;
            let actual_field_type = &actual_field_def.ty;
            errors.push(SingleFederationError::TypeDefinitionInvalid {
                message: format!("Invalid definition for field {actual_field_name} of type {obj_type_name}: should have type {expected_field_type} but found type {actual_field_type}")
            });
        }

        // ensure field arguments are as expected
        let mut arg_errors = ensure_same_arguments(
            expected_field,
            actual_field_def,
            &format!(r#"field "{}.{}""#, obj_type_name, expected_field.name),
            schema,
        );
        errors.append(&mut arg_errors);
    }

    errors
}
