use crate::error::{FederationError, MultipleFederationErrors, SingleFederationError};
use crate::link::spec::{Identity, Url, Version};
use crate::link::spec_definition::{SpecDefinition, SpecDefinitions};
use crate::schema::position::DirectiveDefinitionPosition;
use crate::schema::position::EnumTypeDefinitionPosition;
use crate::schema::position::EnumValueDefinitionPosition;
use crate::schema::position::InputObjectFieldDefinitionPosition;
use crate::schema::position::InterfaceFieldDefinitionPosition;
use crate::schema::position::ObjectFieldDefinitionPosition;
use crate::schema::position::SchemaRootDefinitionKind;
use crate::schema::position::TypeDefinitionPosition;
use crate::schema::referencer::DirectiveReferencers;
use crate::schema::referencer2::TypeDefinitionReferencer;
use crate::schema::FederationSchema;
use apollo_compiler::name;
use apollo_compiler::schema::Component;
use apollo_compiler::schema::ComponentName;
use apollo_compiler::schema::Directive;
use apollo_compiler::schema::EnumValueDefinition;
use apollo_compiler::schema::ExtendedType;
use apollo_compiler::schema::FieldDefinition;
use apollo_compiler::schema::InputValueDefinition;
use apollo_compiler::schema::Name;
use apollo_compiler::schema::Value;
use apollo_compiler::Node;
use indexmap::IndexMap;
use indexmap::IndexSet;
use lazy_static::lazy_static;
use std::collections::HashSet;
use std::fmt;

pub(crate) const INACCESSIBLE_DIRECTIVE_NAME_IN_SPEC: Name = name!("inaccessible");

pub(crate) struct InaccessibleSpecDefinition {
    url: Url,
    minimum_federation_version: Option<Version>,
}

impl InaccessibleSpecDefinition {
    pub(crate) fn new(version: Version, minimum_federation_version: Option<Version>) -> Self {
        Self {
            url: Url {
                identity: Identity::inaccessible_identity(),
                version,
            },
            minimum_federation_version,
        }
    }

    pub(crate) fn inaccessible_directive(
        &self,
        schema: &FederationSchema,
    ) -> Result<Directive, FederationError> {
        let name_in_schema = self
            .directive_name_in_schema(schema, &INACCESSIBLE_DIRECTIVE_NAME_IN_SPEC)?
            .ok_or_else(|| SingleFederationError::Internal {
                message: "Unexpectedly could not find inaccessible spec in schema".to_owned(),
            })?;
        Ok(Directive {
            name: name_in_schema,
            arguments: Vec::new(),
        })
    }
}

impl SpecDefinition for InaccessibleSpecDefinition {
    fn url(&self) -> &Url {
        &self.url
    }

    fn minimum_federation_version(&self) -> Option<&Version> {
        self.minimum_federation_version.as_ref()
    }
}

lazy_static! {
    pub(crate) static ref INACCESSIBLE_VERSIONS: SpecDefinitions<InaccessibleSpecDefinition> = {
        let mut definitions = SpecDefinitions::new(Identity::inaccessible_identity());
        definitions.add(InaccessibleSpecDefinition::new(
            Version { major: 0, minor: 1 },
            None,
        ));
        definitions.add(InaccessibleSpecDefinition::new(
            Version { major: 0, minor: 2 },
            Some(Version { major: 2, minor: 0 }),
        ));
        definitions
    };
}

pub(crate) fn get_inaccessible_spec_definition_from_subgraph(
    schema: &FederationSchema,
) -> Result<&'static InaccessibleSpecDefinition, FederationError> {
    let inaccessible_link = schema
        .metadata()
        .as_ref()
        .and_then(|metadata| metadata.for_identity(&Identity::inaccessible_identity()))
        .ok_or_else(|| SingleFederationError::Internal {
            message: "Subgraph unexpectedly does not use inaccessible spec".to_owned(),
        })?;
    Ok(INACCESSIBLE_VERSIONS
        .find(&inaccessible_link.url.version)
        .ok_or_else(|| SingleFederationError::Internal {
            message: "Subgraph unexpectedly does not use a supported inaccessible spec version"
                .to_owned(),
        })?)
}

fn validate_inaccessible_on_imported_types(
    schema: &FederationSchema,
    referencers: &DirectiveReferencers,
    errors: &mut MultipleFederationErrors,
) {
    let metadata = schema.metadata().unwrap();
    macro_rules! iter_type_names {
        ( $iterable:expr ) => {
            $iterable.iter().map(|position| &position.type_name)
        };
    }

    let feature_referencer_type_names = referencers
        .scalar_types
        .iter()
        .map(|scalar| &scalar.type_name)
        .chain(iter_type_names!(referencers.object_types))
        .chain(iter_type_names!(referencers.object_fields))
        .chain(iter_type_names!(referencers.object_field_arguments))
        .chain(iter_type_names!(referencers.interface_types))
        .chain(iter_type_names!(referencers.interface_fields))
        .chain(iter_type_names!(referencers.interface_field_arguments))
        .chain(iter_type_names!(referencers.union_types))
        .chain(iter_type_names!(referencers.enum_types))
        .chain(iter_type_names!(referencers.enum_values))
        .chain(iter_type_names!(referencers.input_object_types))
        .chain(iter_type_names!(referencers.input_object_fields))
        .filter(|type_name| metadata.source_link_of_type(type_name).is_some());

    let mut raised_errors = HashSet::new();
    for type_name in feature_referencer_type_names {
        let first_occurrence = raised_errors.insert(type_name);
        if first_occurrence {
            errors.push(
                SingleFederationError::DisallowedInaccessible {
                    message: format!("Core feature type `{type_name}` cannot use @inaccessible."),
                }
                .into(),
            )
        }
    }

    for argument in &referencers.directive_arguments {
        if metadata
            .source_link_of_directive(&argument.directive_name)
            .is_some()
        {
            errors.push(
                SingleFederationError::DisallowedInaccessible {
                    message: format!(
                        "Core feature directive `@{}` cannot use @inaccessible.",
                        argument.directive_name,
                    ),
                }
                .into(),
            )
        }
    }
}

enum HasArgumentDefinitionsPosition {
    ObjectField(ObjectFieldDefinitionPosition),
    InterfaceField(InterfaceFieldDefinitionPosition),
    DirectiveDefinition(DirectiveDefinitionPosition),
}
impl fmt::Display for HasArgumentDefinitionsPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ObjectField(x) => x.fmt(f),
            Self::InterfaceField(x) => x.fmt(f),
            Self::DirectiveDefinition(x) => x.fmt(f),
        }
    }
}

fn validate_inaccessible_in_default_value(
    schema: &FederationSchema,
    inaccessible_directive: &Name,
    value_type: &ExtendedType,
    default_value: &Value,
    // TODO avoid eagerly stringifying this
    value_position: String,
    errors: &mut MultipleFederationErrors,
) {
    match (default_value, value_type) {
        // Input fields can be referenced by schema default values. When an
        // input field is hidden (but its parent isn't), we check that the
        // arguments/input fields with such default values aren't in the API
        // schema.
        (Value::Object(value), ExtendedType::InputObject(type_)) => {
            for (field_name, child_value) in value {
                let Some(field) = type_.fields.get(field_name) else {
                    return;
                };
                if field.directives.has(inaccessible_directive) {
                    let input_field_position = InputObjectFieldDefinitionPosition {
                        type_name: type_.name.clone(),
                        field_name: field_name.clone(),
                    };
                    errors.push(SingleFederationError::DefaultValueUsesInaccessible {
                        message: format!("Input field `{input_field_position}` is @inaccessible but is used in the default value of `{value_position}`, which is in the API schema."),
                    }.into());
                }

                if let Some(field_type) = schema.schema().types.get(field.ty.inner_named_type()) {
                    validate_inaccessible_in_default_value(
                        schema,
                        inaccessible_directive,
                        field_type,
                        child_value,
                        value_position.clone(),
                        errors,
                    );
                }
            }
        }
        (Value::List(list), _) => {
            for child_value in list {
                validate_inaccessible_in_default_value(
                    schema,
                    inaccessible_directive,
                    value_type,
                    child_value,
                    value_position.clone(),
                    errors,
                );
            }
        }
        // Enum values can be referenced by schema default values. When an
        // enum value is hidden (but its parent isn't), we check that the
        // arguments/input fields with such default values aren't in the API
        // schema.
        //
        // For back-compat, this also supports using string literals where an enum value is
        // expected.
        (Value::Enum(_) | Value::String(_), ExtendedType::Enum(type_)) => {
            let value = match default_value {
                Value::Enum(name) => name.clone(),
                // It's no problem if this name is invalid.
                Value::String(node_str) => Name::new_unchecked(node_str.clone()),
                // Guaranteed to be enum or string by parent match branch.
                _ => unreachable!(),
            };
            let Some(enum_value) = type_.values.get(&value) else {
                return;
            };
            if enum_value.directives.has(inaccessible_directive) {
                let enum_value_position = EnumValueDefinitionPosition {
                    type_name: type_.name.clone(),
                    value_name: enum_value.value.clone(),
                };
                errors.push(SingleFederationError::DefaultValueUsesInaccessible {
                    message: format!("Enum value `{enum_value_position}` is @inaccessible but is used in the default value of `{value_position}`, which is in the API schema."),
                }.into());
            }
        }
        _ => {}
    }
}

fn validate_inaccessible_in_arguments(
    schema: &FederationSchema,
    inaccessible_directive: &Name,
    usage_position: HasArgumentDefinitionsPosition,
    arguments: &Vec<Node<InputValueDefinition>>,
    errors: &mut MultipleFederationErrors,
) {
    let types = &schema.schema().types;
    for arg in arguments {
        let arg_name = &arg.name;
        let arg_inaccessible = arg.directives.has(inaccessible_directive);
        if arg_inaccessible
            && arg.is_required()
            // TODO remove after update to apollo-compiler 1.0.0-beta.12
            && arg.default_value.is_none()
        {
            let kind = match usage_position {
                HasArgumentDefinitionsPosition::DirectiveDefinition(_) => "directive",
                _ => "field",
            };
            errors.push(SingleFederationError::RequiredInaccessible {
                message: format!("Argument `{usage_position}({arg_name}:)` is @inaccessible but is a required argument of its {kind}."),
            }.into());
        }

        if let (Some(default_value), Some(arg_type)) =
            (&arg.default_value, types.get(arg.ty.inner_named_type()))
        {
            validate_inaccessible_in_default_value(
                schema,
                inaccessible_directive,
                arg_type,
                default_value,
                format!("{usage_position}({arg_name}:)"),
                errors,
            );
        }
    }
}

fn validate_inaccessible_in_fields(
    schema: &FederationSchema,
    inaccessible_directive: &Name,
    type_position: &TypeDefinitionPosition,
    fields: &IndexMap<Name, Component<FieldDefinition>>,
    implements: &IndexSet<ComponentName>,
    errors: &mut MultipleFederationErrors,
) {
    let mut has_inaccessible_field = false;
    let mut has_accessible_field = false;
    for (field_name, field) in fields {
        let super_fields = implements
            .iter()
            .filter_map(|interface_name| {
                schema
                    .schema()
                    .type_field(interface_name, field_name)
                    .ok()
                    .map(|field| (interface_name, field))
            })
            .collect::<Vec<_>>();

        let field_inaccessible = field.directives.has(inaccessible_directive);
        if field_inaccessible {
            has_inaccessible_field = true;

            if let Some((interface_name, super_field)) = super_fields
                .iter()
                .find(|super_field| !super_field.1.directives.has(inaccessible_directive))
            {
                let interface_name = interface_name.as_str();
                let super_field_name = &super_field.name;
                errors.push(
                    SingleFederationError::ImplementedByInaccessible {
                        message: format!("Field `{type_position}.{field_name}` is @inaccessible but implements the interface field `{interface_name}.{super_field_name}`, which is in the API schema."),
                    }
                    .into(),
                );
            }
        } else {
            has_accessible_field = true;
        }

        for arg in &field.arguments {
            let arg_name = &arg.name;
            let arg_inaccessible = arg.directives.has(inaccessible_directive);

            for (interface_name, super_field) in super_fields.iter() {
                let Some(super_arg) = super_field
                    .arguments
                    .iter()
                    .find(|super_arg| super_arg.name == *arg_name)
                else {
                    continue;
                };

                let interface_name = interface_name.as_str();
                let super_field_name = &super_field.name;
                if arg_inaccessible != super_arg.directives.has(inaccessible_directive) {
                    if arg_inaccessible {
                        errors.push(SingleFederationError::ImplementedByInaccessible {
                            message: format!("Argument `{type_position}.{field_name}({arg_name}:)` is @inaccessible but implements the interface argument `{interface_name}.{super_field_name}({arg_name}:)` which is in the API schema."),
                        }.into());
                    } else {
                        errors.push(SingleFederationError::ImplementedByInaccessible {
                            message: format!("Argument `{interface_name}.{super_field_name}({arg_name}:)` is @inaccessible but is implemented by the argument `{type_position}.{field_name}({arg_name}:)` which is in the API schema."),
                        }.into());
                    }
                }
            }
        }

        validate_inaccessible_in_arguments(
            schema,
            inaccessible_directive,
            match type_position {
                TypeDefinitionPosition::Object(object) => {
                    HasArgumentDefinitionsPosition::ObjectField(object.field(field.name.clone()))
                }
                TypeDefinitionPosition::Interface(interface) => {
                    HasArgumentDefinitionsPosition::InterfaceField(
                        interface.field(field.name.clone()),
                    )
                }
                _ => unreachable!(),
            },
            &field.arguments,
            errors,
        );
    }

    if has_inaccessible_field && !has_accessible_field {
        errors.push(SingleFederationError::OnlyInaccessibleChildren {
            message: format!("Type `{type_position}` is in the API schema but all of its members are @inaccessible."),
        }.into());
    }
}

fn validate_inaccessible_in_values(
    inaccessible_directive: &Name,
    enum_position: &EnumTypeDefinitionPosition,
    values: &IndexMap<Name, Component<EnumValueDefinition>>,
    errors: &mut MultipleFederationErrors,
) {
    let mut has_inaccessible_value = false;
    let mut has_accessible_value = false;
    for value in values.values() {
        let value_inaccessible = value.directives.has(inaccessible_directive);
        has_inaccessible_value |= value_inaccessible;
        has_accessible_value |= !value_inaccessible;
    }

    // At this point, we know the type must be in the API schema. Check that at least one of the children is accessible.
    if has_inaccessible_value && !has_accessible_value {
        errors.push(SingleFederationError::OnlyInaccessibleChildren {
            message: format!("Type `{enum_position}` is in the API schema but all of its members are @inaccessible."),
        }.into());
    }
}

pub fn validate_inaccessible(schema: &FederationSchema) -> Result<(), FederationError> {
    let inaccessible_spec = get_inaccessible_spec_definition_from_subgraph(schema)?;
    let directive_name = inaccessible_spec
        .directive_name_in_schema(schema, &INACCESSIBLE_DIRECTIVE_NAME_IN_SPEC)?
        .ok_or_else(|| SingleFederationError::Internal {
            message: "Unexpectedly could not find inaccessible spec in schema".to_owned(),
        })?;

    let mut errors = MultipleFederationErrors { errors: vec![] };

    let referencers = schema.referencers();
    let type_definitions = referencers.to_type_definition_referencers();

    let inaccessible_referencers = referencers.get_directive(&directive_name)?;
    validate_inaccessible_on_imported_types(schema, inaccessible_referencers, &mut errors);

    for position in schema.get_types() {
        let Ok(ty) = position.get(schema.schema()) else {
            continue;
        };
        let metadata = schema.metadata().unwrap();
        if metadata.source_link_of_type(position.type_name()).is_some() {
            // Linked types cannot use @inaccessible: already checked above
            continue;
        }

        // The JavaScript implementation checks for @inaccessible on built-in types, as well.
        // We don't do that here because definitions of built-in types are already rejected
        // by apollo-rs validation.

        if !ty.directives().has(&directive_name) {
            // This type must be in the API schema. For types with children (all types except scalar),
            // we check that at least one of the children is accessible.
            match &position {
                TypeDefinitionPosition::Union(union_position) => {
                    let union_ = union_position.get(schema.schema())?;
                    let any_accessible_member = union_.members.iter().any(|member| {
                        !schema
                            .schema()
                            .types
                            .get(&member.name)
                            .is_some_and(|ty| ty.directives().has("inaccessible"))
                    });

                    if !any_accessible_member {
                        errors.push(SingleFederationError::OnlyInaccessibleChildren {
                            message: format!("Type `{position}` is in the API schema but all of its members are @inaccessible."),
                        }.into());
                    }
                }
                TypeDefinitionPosition::Object(object_position) => {
                    let object = object_position.get(schema.schema())?;
                    validate_inaccessible_in_fields(
                        schema,
                        &directive_name,
                        &position,
                        &object.fields,
                        &object.implements_interfaces,
                        &mut errors,
                    );
                }
                TypeDefinitionPosition::Interface(interface_position) => {
                    let interface = interface_position.get(schema.schema())?;
                    validate_inaccessible_in_fields(
                        schema,
                        &directive_name,
                        &position,
                        &interface.fields,
                        &interface.implements_interfaces,
                        &mut errors,
                    );
                }
                TypeDefinitionPosition::InputObject(input_object_position) => {
                    let input_object = input_object_position.get(schema.schema())?;
                    let mut has_inaccessible_field = false;
                    let mut has_accessible_field = false;
                    for field in input_object.fields.values() {
                        let field_inaccessible = field.directives.has(&directive_name);
                        has_inaccessible_field |= field_inaccessible;
                        has_accessible_field |= !field_inaccessible;

                        if field_inaccessible
                            && field.is_required()
                            // TODO remove after update to apollo-compiler 1.0.0-beta.12
                            && field.default_value.is_none()
                        {
                            errors.push(SingleFederationError::RequiredInaccessible{
                                message: format!("Input field `{position}` is @inaccessible but is a required input field of its type."),
                            }.into());
                        }
                        if let (Some(default_value), Some(field_type)) = (
                            &field.default_value,
                            schema.schema().types.get(field.ty.inner_named_type()),
                        ) {
                            validate_inaccessible_in_default_value(
                                schema,
                                &directive_name,
                                field_type,
                                default_value,
                                input_object_position.field(field.name.clone()).to_string(),
                                &mut errors,
                            );
                        }
                    }
                    if has_inaccessible_field && !has_accessible_field {
                        errors.push(SingleFederationError::OnlyInaccessibleChildren {
                            message: format!("Type `{position}` is in the API schema but all of its input fields are @inaccessible."),
                        }.into());
                    }
                }
                TypeDefinitionPosition::Enum(enum_position) => {
                    let enum_ = enum_position.get(schema.schema())?;
                    validate_inaccessible_in_values(
                        &directive_name,
                        enum_position,
                        &enum_.values,
                        &mut errors,
                    );
                }
                _ => {}
            }

            // This type is in the API schema, so we do not have to check if it is referenced
            // by other types.
            continue;
        }

        let Some(references) = type_definitions.get(&position) else {
            // This type is not in the API schema, but there are no references, so we already know
            // it's OK to remove.
            continue;
        };

        // Types can be referenced by other schema elements in a few ways:
        // 1. Fields, arguments, and input fields may have the type as their base
        //    type.
        // 2. Union types may have the type as a member (for object types).
        // 3. Object and interface types may implement the type (for interface
        //    types).
        // 4. Schemas may have the type as a root operation type (for object
        //    types).
        //
        // When a type is hidden, the referencer must follow certain rules for the
        // schema to be valid. Respectively, these rules are:
        // 1. The field/argument/input field must not be in the API schema.
        // 2. The union type, if empty, must not be in the API schema.
        // 3. No rules are imposed in this case.
        // 4. The root operation type must not be the query type.
        //
        // We validate the 2nd rule above. The other rules are validated here.
        for ref_position in references.iter() {
            // HACK: This manually checks if the referencer or any of its parents
            // has an @inaccessible directive. It's very different from the JS code,
            // it may be better to refactor to what JS does...
            let ref_inaccessible = match ref_position {
                TypeDefinitionReferencer::SchemaRoot(root) => {
                    if root.root_kind == SchemaRootDefinitionKind::Query {
                        errors.push(SingleFederationError::QueryRootTypeInaccessible {
                            message: format!("Type `{position}` is @inaccessible but is the query root type, which must be in the API schema."),
                        }.into());
                    }
                    continue;
                }
                TypeDefinitionReferencer::Union(_) => {
                    // This type will be removed from the union, or the whole union will be removed
                    // if all its members are inaccessible. This is checked above.
                    continue;
                }
                TypeDefinitionReferencer::Object(_) => {
                    // Direct references from an object come from its `implements` list,
                    // and this type will be removed from it.
                    continue;
                }
                TypeDefinitionReferencer::Interface(_) => {
                    // Direct references from an interface come from its `implements` list,
                    // and this type will be removed from it.
                    continue;
                }

                // General types
                TypeDefinitionReferencer::ObjectField(ref_position) => {
                    ref_position
                        .get(schema.schema())?
                        .directives
                        .has(&directive_name)
                        || ref_position
                            .parent()
                            .get(schema.schema())?
                            .directives
                            .has(&directive_name)
                }
                TypeDefinitionReferencer::ObjectFieldArgument(_) => false,
                TypeDefinitionReferencer::InterfaceField(ref_position) => {
                    ref_position
                        .get(schema.schema())?
                        .directives
                        .has(&directive_name)
                        || ref_position
                            .parent()
                            .get(schema.schema())?
                            .directives
                            .has(&directive_name)
                }
                TypeDefinitionReferencer::InterfaceFieldArgument(_) => false,
                TypeDefinitionReferencer::UnionField(ref_position) => ref_position
                    .get(schema.schema())?
                    .directives
                    .has(&directive_name),
                TypeDefinitionReferencer::InputObjectField(ref_position) => {
                    ref_position
                        .get(schema.schema())?
                        .directives
                        .has(&directive_name)
                        || ref_position
                            .parent()
                            .get(schema.schema())?
                            .directives
                            .has(&directive_name)
                }
                TypeDefinitionReferencer::DirectiveArgument(_) => false,
            };

            if !ref_inaccessible {
                errors.push(SingleFederationError::ReferencedInaccessible {
                    message: format!("Type `{position}` is @inaccessible but is referenced by `{ref_position}`, which is in the API schema."),
                }.into());
            }
        }
    }

    for position in schema.get_directive_definitions() {
        let Ok(directive) = position.get(schema.schema()) else {
            continue;
        };

        validate_inaccessible_in_arguments(
            schema,
            &directive_name,
            HasArgumentDefinitionsPosition::DirectiveDefinition(position),
            &directive.arguments,
            &mut errors,
        );
    }

    if !errors.errors.is_empty() {
        return Err(errors.into());
    }

    Ok(())
}

pub fn remove_inaccessible_elements(schema: &mut FederationSchema) -> Result<(), FederationError> {
    let inaccessible_spec = get_inaccessible_spec_definition_from_subgraph(schema)?;
    let directive_name = inaccessible_spec
        .directive_name_in_schema(schema, &INACCESSIBLE_DIRECTIVE_NAME_IN_SPEC)?
        .ok_or_else(|| SingleFederationError::Internal {
            message: "Unexpectedly could not find inaccessible spec in schema".to_owned(),
        })?;

    let referencers = schema.referencers();
    // Clone so there's no live borrow.
    let inaccessible_referencers = referencers.get_directive(&directive_name)?.clone();

    // Remove fields from inaccessible types first. If any inaccessible type has a field
    // that references another inaccessible type, it would prevent the other type from being
    // removed.
    // We need an intermediate allocation as `.remove()` requires mutable access to the schema and
    // looking up fields requires immutable access.
    let mut inaccessible_children: Vec<ObjectFieldDefinitionPosition> = vec![];
    for position in &inaccessible_referencers.object_types {
        let object = position.get(schema.schema())?;
        inaccessible_children.extend(
            object
                .fields
                .keys()
                .map(|field_name| position.field(field_name.clone())),
        );
    }
    for field in inaccessible_children {
        field.remove(schema)?;
    }

    let mut inaccessible_children: Vec<InterfaceFieldDefinitionPosition> = vec![];
    for position in &inaccessible_referencers.interface_types {
        let object = position.get(schema.schema())?;
        inaccessible_children.extend(
            object
                .fields
                .keys()
                .map(|field_name| position.field(field_name.clone())),
        );
    }
    for field in inaccessible_children {
        field.remove(schema)?;
    }

    for argument in inaccessible_referencers.interface_field_arguments {
        argument.remove(schema)?;
    }
    for argument in inaccessible_referencers.object_field_arguments {
        argument.remove(schema)?;
    }
    for field in inaccessible_referencers.interface_fields {
        field.remove(schema)?;
    }
    for field in inaccessible_referencers.object_fields {
        field.remove(schema)?;
    }
    for ty in inaccessible_referencers.union_types {
        ty.remove(schema)?;
    }
    for ty in inaccessible_referencers.object_types {
        ty.remove(schema)?;
    }
    for ty in inaccessible_referencers.interface_types {
        ty.remove(schema)?;
    }
    for ty in inaccessible_referencers.enum_types {
        ty.remove(schema)?;
    }
    for ty in inaccessible_referencers.scalar_types {
        ty.remove(schema)?;
    }

    Ok(())
}
