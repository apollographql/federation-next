use crate::error::{FederationError, MultipleFederationErrors, SingleFederationError};
use crate::link::spec::{Identity, Url, Version};
use crate::link::spec_definition::{SpecDefinition, SpecDefinitions};
use crate::schema::position::DirectiveDefinitionPosition;
use crate::schema::position::EnumTypeDefinitionPosition;
use crate::schema::position::InputObjectFieldDefinitionPosition;
use crate::schema::position::InterfaceFieldDefinitionPosition;
use crate::schema::position::ObjectFieldDefinitionPosition;
use crate::schema::position::TypeDefinitionPosition;
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

// Should nested inaccessible fields be accepted?
// ```graphql
// input WithInaccessible { accessible: Boolean inaccessible: Boolean @inaccessible }
// input DefaultNested { nested: WithInaccessible }
// type Object { field(arg: DefaultNested = { nested: { inaccessible: true } } }
// ```
fn validate_inaccessible_in_arguments(
    schema: &FederationSchema,
    inaccessible_directive: &Name,
    usage_position: HasArgumentDefinitionsPosition,
    arguments: &Vec<Node<InputValueDefinition>>,
    errors: &mut MultipleFederationErrors,
) {
    for arg in arguments {
        let Some(default_value) = &arg.default_value else {
            continue;
        };
        let arg_type = schema.schema().types.get(arg.ty.inner_named_type());

        match &**default_value {
            Value::Object(value) => {
                let Some(ExtendedType::InputObject(type_)) = arg_type else {
                    // Argument types must be input objects or scalars, only input objects are relevant
                    // here.
                    continue;
                };
                for (field_name, _) in value {
                    let Some(field) = type_.fields.get(field_name) else {
                        continue;
                    };
                    if field.directives.has(inaccessible_directive) {
                        let input_field_position = InputObjectFieldDefinitionPosition {
                            type_name: type_.name.clone(),
                            field_name: field_name.clone(),
                        };
                        errors.push(
                    SingleFederationError::DefaultValueUsesInaccessible {
                        message: format!("Input field `{input_field_position}` is @inaccessible but is used in the default value of `{usage_position}({}:)`, which is in the API schema.", arg.name),
                    }
                    .into(),
                );
                    }
                }
            }
            Value::Enum(value) => {
                let Some(ExtendedType::Enum(type_)) = arg_type else {
                    // Argument types must be input objects or scalars, only input objects are relevant
                    // here.
                    continue;
                };
                let Some(enum_value) = type_.values.get(value) else {
                    continue;
                };
                if enum_value.directives.has(inaccessible_directive) {
                    errors.push(
                    SingleFederationError::DefaultValueUsesInaccessible {
                        message: format!("Enum value `{}.{}` is @inaccessible but is used in the default value of `{usage_position}({}:)`, which is in the API schema.", type_.name, enum_value.value, arg.name),
                    }
                    .into(),
                );
                }
            }
            _ => {}
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
        let mut super_fields = implements.iter().filter_map(|interface_name| {
            schema
                .schema()
                .type_field(interface_name, field_name)
                .ok()
                .map(|field| (interface_name, field))
        });

        if field.directives.has(inaccessible_directive) {
            has_inaccessible_field = true;

            if let Some((interface_name, super_field)) = super_fields
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
    schema: &FederationSchema,
    inaccessible_directive: &Name,
    enum_position: &EnumTypeDefinitionPosition,
    values: &IndexMap<Name, Component<EnumValueDefinition>>,
    errors: &mut MultipleFederationErrors,
) {
    let mut has_inaccessible_value = false;
    let mut has_accessible_value = false;
    for (value_name, value) in values {
        let value_inaccessible = value.directives.has(inaccessible_directive);
        has_inaccessible_value |= value_inaccessible;
        has_accessible_value |= !value_inaccessible;
    }

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
    for position in schema.get_types() {
        let Ok(ty) = position.get(schema.schema()) else {
            continue;
        };
        let is_inaccessible = ty.directives().has(&directive_name);

        match &position {
            TypeDefinitionPosition::Union(union_position) if !is_inaccessible => {
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
                    if field_inaccessible && field.ty.is_non_null() {
                        errors.push(SingleFederationError::RequiredInaccessible{
                            message: format!("Input field `{position}` is @inaccessible but is a required input field of its type."),
                        }.into());
                    }
                    has_inaccessible_field |= field_inaccessible;
                    has_accessible_field |= !field_inaccessible;

                    let Some(default_value) = &field.default_value else {
                        continue;
                    };
                    let field_type = schema.schema().types.get(field.ty.inner_named_type());

                    let usage_position = input_object_position.field(field.name.clone());
                    match &**default_value {
                        Value::Object(value) => {
                            let Some(ExtendedType::InputObject(type_)) = field_type else {
                                // Argument types must be input objects or scalars, only input objects are relevant
                                // here.
                                continue;
                            };
                            for (field_name, _) in value {
                                let Some(field) = type_.fields.get(field_name) else {
                                    continue;
                                };
                                if field.directives.has(&directive_name) {
                                    let input_field_position = InputObjectFieldDefinitionPosition {
                                        type_name: type_.name.clone(),
                                        field_name: field_name.clone(),
                                    };
                                    errors.push(
                                SingleFederationError::DefaultValueUsesInaccessible {
                                    message: format!("Input field `{input_field_position}` is @inaccessible but is used in the default value of `{usage_position}`, which is in the API schema."),
                                }
                                .into(),
                            );
                                }
                            }
                        }
                        Value::Enum(value) => {
                            let Some(ExtendedType::Enum(type_)) = field_type else {
                                // Argument types must be input objects or scalars, only input objects are relevant
                                // here.
                                continue;
                            };
                            let Some(enum_value) = type_.values.get(value) else {
                                continue;
                            };
                            if enum_value.directives.has(&directive_name) {
                                errors.push(
                                SingleFederationError::DefaultValueUsesInaccessible {
                                    message: format!("Enum value `{}.{}` is @inaccessible but is used in the default value of `{usage_position}`, which is in the API schema.", type_.name, enum_value.value)
                                }
                                .into(),
                            );
                            }
                        }
                        _ => {}
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
                    schema,
                    &directive_name,
                    enum_position,
                    &enum_.values,
                    &mut errors,
                );
            }
            _ => {}
        }

        // Nothing more to do for accessible types.
        if !is_inaccessible {
            continue;
        }

        let Some(references) = type_definitions.get(&position) else {
            continue; // no references, OK to remove
        };

        for ref_position in references.iter() {
            let ref_inaccessible = match ref_position {
                TypeDefinitionReferencer::SchemaRoot(_) => {
                    errors.push(SingleFederationError::QueryRootTypeInaccessible {
                        message: format!("Type `{position}` is @inaccessible but is the query root type, which must be in the API schema."),
                    }.into());
                    continue;
                }
                TypeDefinitionReferencer::Union(_) => {
                    // This type will be removed from the union
                    continue;
                }
                // General types
                TypeDefinitionReferencer::Object(ref_position) => ref_position
                    .get(schema.schema())?
                    .directives
                    .has(&directive_name),
                TypeDefinitionReferencer::ObjectField(ref_position) => ref_position
                    .get(schema.schema())?
                    .directives
                    .has(&directive_name),
                TypeDefinitionReferencer::ObjectFieldArgument(_) => false,
                TypeDefinitionReferencer::Interface(ref_position) => ref_position
                    .get(schema.schema())?
                    .directives
                    .has(&directive_name),
                TypeDefinitionReferencer::InterfaceField(ref_position) => ref_position
                    .get(schema.schema())?
                    .directives
                    .has(&directive_name),
                TypeDefinitionReferencer::InterfaceFieldArgument(_) => false,
                TypeDefinitionReferencer::UnionField(ref_position) => ref_position
                    .get(schema.schema())?
                    .directives
                    .has(&directive_name),
                TypeDefinitionReferencer::InputObjectField(ref_position) => ref_position
                    .get(schema.schema())?
                    .directives
                    .has(&directive_name),
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

    todo!()
}
