use crate::location::{
    DirectiveArgumentDefinitionLocation, EnumTypeDefinitionLocation, EnumValueDefinitionLocation,
    InputObjectFieldDefinitionLocation, InputObjectTypeDefinitionLocation,
    InterfaceFieldArgumentDefinitionLocation, InterfaceFieldDefinitionLocation,
    InterfaceTypeDefinitionLocation, ObjectFieldArgumentDefinitionLocation,
    ObjectFieldDefinitionLocation, ObjectTypeDefinitionLocation, ScalarTypeDefinitionLocation,
    SchemaDefinitionLocation, SchemaRootDefinitionKind, SchemaRootDefinitionLocation,
    UnionTypeDefinitionLocation,
};
use apollo_compiler::schema::{ExtendedType, Name};
use apollo_compiler::Schema;
use indexmap::{IndexMap, IndexSet};
use std::ops::Deref;

#[derive(Debug, Clone, Default)]
pub struct Referencers {
    pub scalar_types: IndexMap<Name, ScalarTypeReferencers>,
    pub object_types: IndexMap<Name, ObjectTypeReferencers>,
    pub interface_types: IndexMap<Name, InterfaceTypeReferencers>,
    pub union_types: IndexMap<Name, UnionTypeReferencers>,
    pub enum_types: IndexMap<Name, EnumTypeReferencers>,
    pub input_object_types: IndexMap<Name, InputObjectTypeReferencers>,
    pub directives: IndexMap<Name, DirectiveReferencers>,
}

impl AsMut<Referencers> for Referencers {
    fn as_mut(&mut self) -> &mut Referencers {
        self
    }
}

impl AsRef<Referencers> for Referencers {
    fn as_ref(&self) -> &Referencers {
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct ScalarTypeReferencers {
    pub object_fields: IndexSet<ObjectFieldDefinitionLocation>,
    pub object_field_arguments: IndexSet<ObjectFieldArgumentDefinitionLocation>,
    pub interface_fields: IndexSet<InterfaceFieldDefinitionLocation>,
    pub interface_field_arguments: IndexSet<InterfaceFieldArgumentDefinitionLocation>,
    pub input_object_fields: IndexSet<InputObjectFieldDefinitionLocation>,
    pub directive_arguments: IndexSet<DirectiveArgumentDefinitionLocation>,
}

#[derive(Debug, Clone, Default)]
pub struct ObjectTypeReferencers {
    pub schema_roots: IndexSet<SchemaRootDefinitionLocation>,
    pub object_fields: IndexSet<ObjectFieldDefinitionLocation>,
    pub interface_fields: IndexSet<InterfaceFieldDefinitionLocation>,
    pub union_types: IndexSet<UnionTypeDefinitionLocation>,
}

#[derive(Debug, Clone, Default)]
pub struct InterfaceTypeReferencers {
    pub object_types: IndexSet<ObjectTypeDefinitionLocation>,
    pub object_fields: IndexSet<ObjectFieldDefinitionLocation>,
    pub interface_types: IndexSet<InterfaceTypeDefinitionLocation>,
    pub interface_fields: IndexSet<InterfaceFieldDefinitionLocation>,
}

#[derive(Debug, Clone, Default)]
pub struct UnionTypeReferencers {
    pub object_fields: IndexSet<ObjectFieldDefinitionLocation>,
    pub interface_fields: IndexSet<InterfaceFieldDefinitionLocation>,
}

#[derive(Debug, Clone, Default)]
pub struct EnumTypeReferencers {
    pub object_fields: IndexSet<ObjectFieldDefinitionLocation>,
    pub object_field_arguments: IndexSet<ObjectFieldArgumentDefinitionLocation>,
    pub interface_fields: IndexSet<InterfaceFieldDefinitionLocation>,
    pub interface_field_arguments: IndexSet<InterfaceFieldArgumentDefinitionLocation>,
    pub input_object_fields: IndexSet<InputObjectFieldDefinitionLocation>,
    pub directive_arguments: IndexSet<DirectiveArgumentDefinitionLocation>,
}

#[derive(Debug, Clone, Default)]
pub struct InputObjectTypeReferencers {
    pub object_field_arguments: IndexSet<ObjectFieldArgumentDefinitionLocation>,
    pub interface_field_arguments: IndexSet<InterfaceFieldArgumentDefinitionLocation>,
    pub input_object_fields: IndexSet<InputObjectFieldDefinitionLocation>,
    pub directive_arguments: IndexSet<DirectiveArgumentDefinitionLocation>,
}

#[derive(Debug, Clone, Default)]
pub struct DirectiveReferencers {
    pub schema: Option<SchemaDefinitionLocation>,
    pub scalar_types: IndexSet<ScalarTypeDefinitionLocation>,
    pub object_types: IndexSet<ObjectTypeDefinitionLocation>,
    pub object_fields: IndexSet<ObjectFieldDefinitionLocation>,
    pub object_field_arguments: IndexSet<ObjectFieldArgumentDefinitionLocation>,
    pub interface_types: IndexSet<InterfaceTypeDefinitionLocation>,
    pub interface_fields: IndexSet<InterfaceFieldDefinitionLocation>,
    pub interface_field_arguments: IndexSet<InterfaceFieldArgumentDefinitionLocation>,
    pub union_types: IndexSet<UnionTypeDefinitionLocation>,
    pub enum_types: IndexSet<EnumTypeDefinitionLocation>,
    pub enum_values: IndexSet<EnumValueDefinitionLocation>,
    pub input_object_types: IndexSet<InputObjectTypeDefinitionLocation>,
    pub input_object_fields: IndexSet<InputObjectFieldDefinitionLocation>,
    pub directive_arguments: IndexSet<DirectiveArgumentDefinitionLocation>,
}

pub fn referencers(schema: &Schema) -> Referencers {
    let mut referencers: Referencers = Default::default();

    // Shallow pass to populate data structures for types/directives.
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
    if let Some(ref schema_definition) = schema.schema_definition {
        for directive_reference in schema_definition.directives.iter() {
            referencers
                .directives
                .get_mut(&directive_reference.name)
                .unwrap_or_else(|| {
                    panic!(
                        "Schema definition's directive application \"@{}\" does not refer to an existing directive.",
                        directive_reference.name,
                    )
                })
                .schema = Some(SchemaDefinitionLocation);
        }
        if let Some(ref query_root_type) = schema_definition.query {
            referencers
                .object_types
                .get_mut(query_root_type.deref())
                .unwrap_or_else(|| {
                    panic!(
                        "Query root type \"{}\" does not refer to an existing object type.",
                        query_root_type.deref()
                    )
                })
                .schema_roots
                .insert(SchemaRootDefinitionLocation {
                    root_kind: SchemaRootDefinitionKind::Query,
                });
        }
        if let Some(ref mutation_root_type) = schema_definition.mutation {
            referencers
                .object_types
                .get_mut(mutation_root_type.deref())
                .unwrap_or_else(|| {
                    panic!(
                        "Mutation root type \"{}\" does not refer to an existing object type.",
                        mutation_root_type.deref()
                    )
                })
                .schema_roots
                .insert(SchemaRootDefinitionLocation {
                    root_kind: SchemaRootDefinitionKind::Mutation,
                });
        }
        if let Some(ref subscription_root_type) = schema_definition.subscription {
            referencers
                .object_types
                .get_mut(subscription_root_type.deref())
                .unwrap_or_else(|| {
                    panic!(
                        "Subscription root type \"{}\" does not refer to an existing object type.",
                        subscription_root_type.deref()
                    )
                })
                .schema_roots
                .insert(SchemaRootDefinitionLocation {
                    root_kind: SchemaRootDefinitionKind::Subscription,
                });
        }
    }
    for (type_name, type_) in schema.types.iter() {
        match type_ {
            ExtendedType::Scalar(type_) => {
                let location = ScalarTypeDefinitionLocation {
                    type_name: type_name.clone(),
                };
                for directive_reference in type_.directives.iter() {
                    referencers
                        .directives
                        .get_mut(&directive_reference.name)
                        .unwrap_or_else(|| {
                            panic!(
                                "Scalar type \"{}\"'s directive application \"@{}\" does not refer to an existing directive.",
                                location,
                                directive_reference.name,
                            )
                        })
                        .scalar_types
                        .insert(location.clone());
                }
            }
            ExtendedType::Object(type_) => {
                let location = ObjectTypeDefinitionLocation {
                    type_name: type_name.clone(),
                };
                for directive_reference in type_.directives.iter() {
                    referencers
                        .directives
                        .get_mut(&directive_reference.name)
                        .unwrap_or_else(|| {
                            panic!(
                                "Object type \"{}\"'s directive application \"@{}\" does not refer to an existing directive.",
                                location,
                                directive_reference.name,
                            )
                        })
                        .object_types
                        .insert(location.clone());
                }
                for interface_type_reference in type_.implements_interfaces.iter() {
                    referencers
                        .interface_types
                        .get_mut(interface_type_reference.deref())
                        .unwrap_or_else(|| {
                            panic!(
                                "Object type \"{}\"'s implements clause \"{}\" does not refer to an existing interface.",
                                location,
                                interface_type_reference.deref(),
                            )
                        })
                        .object_types
                        .insert(location.clone());
                }
                for (field_name, field) in type_.fields.iter() {
                    let location = ObjectFieldDefinitionLocation {
                        type_name: type_name.clone(),
                        field_name: field_name.clone(),
                    };
                    for directive_reference in field.directives.iter() {
                        referencers
                            .directives
                            .get_mut(&directive_reference.name)
                            .unwrap_or_else(|| {
                                panic!(
                                    "Object field \"{}\"'s directive application \"@{}\" does not refer to an existing directive.",
                                    location,
                                    directive_reference.name,
                                )
                            })
                            .object_fields
                            .insert(location.clone());
                    }
                    let output_type_reference = field.ty.inner_named_type();
                    match schema.types.get(output_type_reference) {
                        Some(ExtendedType::Scalar(_)) => {
                            referencers
                                .scalar_types
                                .get_mut(output_type_reference)
                                .unwrap()
                                .object_fields
                                .insert(location.clone());
                        }
                        Some(ExtendedType::Object(_)) => {
                            referencers
                                .object_types
                                .get_mut(output_type_reference)
                                .unwrap()
                                .object_fields
                                .insert(location.clone());
                        }
                        Some(ExtendedType::Interface(_)) => {
                            referencers
                                .interface_types
                                .get_mut(output_type_reference)
                                .unwrap()
                                .object_fields
                                .insert(location.clone());
                        }
                        Some(ExtendedType::Union(_)) => {
                            referencers
                                .union_types
                                .get_mut(output_type_reference)
                                .unwrap()
                                .object_fields
                                .insert(location.clone());
                        }
                        Some(ExtendedType::Enum(_)) => {
                            referencers
                                .enum_types
                                .get_mut(output_type_reference)
                                .unwrap()
                                .object_fields
                                .insert(location.clone());
                        }
                        _ => {
                            panic!(
                                "Object field \"{}\"'s inner type \"{}\" does not refer to an existing output type.",
                                location,
                                output_type_reference.deref(),
                            )
                        }
                    }
                    for argument in field.arguments.iter() {
                        let location = ObjectFieldArgumentDefinitionLocation {
                            type_name: type_name.clone(),
                            field_name: field_name.clone(),
                            argument_name: argument.name.clone(),
                        };
                        for directive_reference in argument.directives.iter() {
                            referencers
                                .directives
                                .get_mut(&directive_reference.name)
                                .unwrap_or_else(|| {
                                    panic!(
                                        "Object field argument \"{}\"'s directive application \"@{}\" does not refer to an existing directive.",
                                        location,
                                        directive_reference.name,
                                    )
                                })
                                .object_field_arguments
                                .insert(location.clone());
                        }
                        let input_type_reference = argument.ty.inner_named_type();
                        match schema.types.get(input_type_reference) {
                            Some(ExtendedType::Scalar(_)) => {
                                referencers
                                    .scalar_types
                                    .get_mut(input_type_reference)
                                    .unwrap()
                                    .object_field_arguments
                                    .insert(location.clone());
                            }
                            Some(ExtendedType::Enum(_)) => {
                                referencers
                                    .enum_types
                                    .get_mut(input_type_reference)
                                    .unwrap()
                                    .object_field_arguments
                                    .insert(location.clone());
                            }
                            Some(ExtendedType::InputObject(_)) => {
                                referencers
                                    .input_object_types
                                    .get_mut(input_type_reference)
                                    .unwrap()
                                    .object_field_arguments
                                    .insert(location.clone());
                            }
                            _ => {
                                panic!(
                                    "Object field argument \"{}\"'s inner type \"{}\" does not refer to an existing input type.",
                                    location,
                                    input_type_reference.deref(),
                                )
                            }
                        }
                    }
                }
            }
            ExtendedType::Interface(type_) => {
                let location = InterfaceTypeDefinitionLocation {
                    type_name: type_name.clone(),
                };
                for directive_reference in type_.directives.iter() {
                    referencers
                        .directives
                        .get_mut(&directive_reference.name)
                        .unwrap_or_else(|| {
                            panic!(
                                "Interface type \"{}\"'s directive application \"@{}\" does not refer to an existing directive.",
                                location,
                                directive_reference.name,
                            )
                        })
                        .interface_types
                        .insert(location.clone());
                }
                for interface_type_reference in type_.implements_interfaces.iter() {
                    referencers
                        .interface_types
                        .get_mut(interface_type_reference.deref())
                        .unwrap_or_else(|| {
                            panic!(
                                "Interface type \"{}\"'s implements clause \"{}\" does not refer to an existing interface.",
                                location,
                                interface_type_reference.deref(),
                            )
                        })
                        .interface_types
                        .insert(location.clone());
                }
                for (field_name, field) in type_.fields.iter() {
                    let location = InterfaceFieldDefinitionLocation {
                        type_name: type_name.clone(),
                        field_name: field_name.clone(),
                    };
                    for directive_reference in field.directives.iter() {
                        referencers
                            .directives
                            .get_mut(&directive_reference.name)
                            .unwrap_or_else(|| {
                                panic!(
                                    "Interface field \"{}\"'s directive application \"@{}\" does not refer to an existing directive.",
                                    location,
                                    directive_reference.name,
                                )
                            })
                            .interface_fields
                            .insert(location.clone());
                    }
                    let output_type_reference = field.ty.inner_named_type();
                    match schema.types.get(output_type_reference) {
                        Some(ExtendedType::Scalar(_)) => {
                            referencers
                                .scalar_types
                                .get_mut(output_type_reference)
                                .unwrap()
                                .interface_fields
                                .insert(location.clone());
                        }
                        Some(ExtendedType::Object(_)) => {
                            referencers
                                .object_types
                                .get_mut(output_type_reference)
                                .unwrap()
                                .interface_fields
                                .insert(location.clone());
                        }
                        Some(ExtendedType::Interface(_)) => {
                            referencers
                                .interface_types
                                .get_mut(output_type_reference)
                                .unwrap()
                                .interface_fields
                                .insert(location.clone());
                        }
                        Some(ExtendedType::Union(_)) => {
                            referencers
                                .union_types
                                .get_mut(output_type_reference)
                                .unwrap()
                                .interface_fields
                                .insert(location.clone());
                        }
                        Some(ExtendedType::Enum(_)) => {
                            referencers
                                .enum_types
                                .get_mut(output_type_reference)
                                .unwrap()
                                .interface_fields
                                .insert(location.clone());
                        }
                        _ => {
                            panic!(
                                "Interface field \"{}\"'s inner type \"{}\" does not refer to an existing output type.",
                                location,
                                output_type_reference.deref(),
                            )
                        }
                    }
                    for argument in field.arguments.iter() {
                        let location = InterfaceFieldArgumentDefinitionLocation {
                            type_name: type_name.clone(),
                            field_name: field_name.clone(),
                            argument_name: argument.name.clone(),
                        };
                        for directive_reference in argument.directives.iter() {
                            referencers
                                .directives
                                .get_mut(&directive_reference.name)
                                .unwrap_or_else(|| {
                                    panic!(
                                        "Interface field argument \"{}\"'s directive application \"@{}\" does not refer to an existing directive.",
                                        location,
                                        directive_reference.name,
                                    )
                                })
                                .interface_field_arguments
                                .insert(location.clone());
                        }
                        let input_type_reference = argument.ty.inner_named_type();
                        match schema.types.get(input_type_reference) {
                            Some(ExtendedType::Scalar(_)) => {
                                referencers
                                    .scalar_types
                                    .get_mut(input_type_reference)
                                    .unwrap()
                                    .interface_field_arguments
                                    .insert(location.clone());
                            }
                            Some(ExtendedType::Enum(_)) => {
                                referencers
                                    .enum_types
                                    .get_mut(input_type_reference)
                                    .unwrap()
                                    .interface_field_arguments
                                    .insert(location.clone());
                            }
                            Some(ExtendedType::InputObject(_)) => {
                                referencers
                                    .input_object_types
                                    .get_mut(input_type_reference)
                                    .unwrap()
                                    .interface_field_arguments
                                    .insert(location.clone());
                            }
                            _ => {
                                panic!(
                                    "Interface field argument \"{}\"'s inner type \"{}\" does not refer to an existing input type.",
                                    location,
                                    input_type_reference.deref(),
                                )
                            }
                        }
                    }
                }
            }
            ExtendedType::Union(type_) => {
                let location = UnionTypeDefinitionLocation {
                    type_name: type_name.clone(),
                };
                for directive_reference in type_.directives.iter() {
                    referencers
                        .directives
                        .get_mut(&directive_reference.name)
                        .unwrap_or_else(|| {
                            panic!(
                                "Union type \"{}\"'s directive application \"@{}\" does not refer to an existing directive.",
                                location,
                                directive_reference.name,
                            )
                        })
                        .union_types
                        .insert(location.clone());
                }
                for member in type_.members.iter() {
                    referencers
                        .object_types
                        .get_mut(member.deref())
                        .unwrap_or_else(|| {
                            panic!(
                                "Union type \"{}\"'s member \"{}\" does not refer to an existing object type.",
                                location,
                                member.deref()
                            )
                        })
                        .union_types
                        .insert(location.clone());
                }
            }
            ExtendedType::Enum(type_) => {
                let location = EnumTypeDefinitionLocation {
                    type_name: type_name.clone(),
                };
                for directive_reference in type_.directives.iter() {
                    referencers
                        .directives
                        .get_mut(&directive_reference.name)
                        .unwrap_or_else(|| {
                            panic!(
                                "Enum type \"{}\"'s directive application \"@{}\" does not refer to an existing directive.",
                                location,
                                directive_reference.name,
                            )
                        })
                        .enum_types
                        .insert(location.clone());
                }
                for (value_name, value) in type_.values.iter() {
                    let location = EnumValueDefinitionLocation {
                        type_name: type_name.clone(),
                        value_name: value_name.clone(),
                    };
                    for directive_reference in value.directives.iter() {
                        referencers
                            .directives
                            .get_mut(&directive_reference.name)
                            .unwrap_or_else(|| {
                                panic!(
                                    "Enum value \"{}\"'s directive application \"@{}\" does not refer to an existing directive.",
                                    location,
                                    directive_reference.name,
                                )
                            })
                            .enum_values
                            .insert(location.clone());
                    }
                }
            }
            ExtendedType::InputObject(type_) => {
                let location = InputObjectTypeDefinitionLocation {
                    type_name: type_name.clone(),
                };
                for directive_reference in type_.directives.iter() {
                    referencers
                        .directives
                        .get_mut(&directive_reference.name)
                        .unwrap_or_else(|| {
                            panic!(
                                "Input object type \"{}\"'s directive application \"@{}\" does not refer to an existing directive.",
                                location,
                                directive_reference.name,
                            )
                        })
                        .input_object_types
                        .insert(location.clone());
                }
                for (field_name, field) in type_.fields.iter() {
                    let location = InputObjectFieldDefinitionLocation {
                        type_name: type_name.clone(),
                        field_name: field_name.clone(),
                    };
                    for directive_reference in field.directives.iter() {
                        referencers
                            .directives
                            .get_mut(&directive_reference.name)
                            .unwrap_or_else(|| {
                                panic!(
                                    "Input object field \"{}\"'s directive application \"@{}\" does not refer to an existing directive.",
                                    location,
                                    directive_reference.name,
                                )
                            })
                            .input_object_fields
                            .insert(location.clone());
                    }
                    let input_type_reference = field.ty.inner_named_type();
                    match schema.types.get(input_type_reference) {
                        Some(ExtendedType::Scalar(_)) => {
                            referencers
                                .scalar_types
                                .get_mut(input_type_reference)
                                .unwrap()
                                .input_object_fields
                                .insert(location.clone());
                        }
                        Some(ExtendedType::Enum(_)) => {
                            referencers
                                .enum_types
                                .get_mut(input_type_reference)
                                .unwrap()
                                .input_object_fields
                                .insert(location.clone());
                        }
                        Some(ExtendedType::InputObject(_)) => {
                            referencers
                                .input_object_types
                                .get_mut(input_type_reference)
                                .unwrap()
                                .input_object_fields
                                .insert(location.clone());
                        }
                        _ => {
                            panic!(
                                "Input object field \"{}\"'s inner type \"{}\" does not refer to an existing input type.",
                                location,
                                input_type_reference.deref(),
                            )
                        }
                    }
                }
            }
        }
    }
    for (directive_name, directive) in schema.directive_definitions.iter() {
        for argument in directive.arguments.iter() {
            let location = DirectiveArgumentDefinitionLocation {
                directive_name: directive_name.clone(),
                argument_name: argument.name.clone(),
            };
            for directive_reference in argument.directives.iter() {
                referencers
                    .directives
                    .get_mut(&directive_reference.name)
                    .unwrap_or_else(|| {
                        panic!(
                            "Directive argument \"{}\"'s directive application \"@{}\" does not refer to an existing directive.",
                            location,
                            directive_reference.name,
                        )
                    })
                    .directive_arguments
                    .insert(location.clone());
            }
            let input_type_reference = argument.ty.inner_named_type();
            match schema.types.get(input_type_reference) {
                Some(ExtendedType::Scalar(_)) => {
                    referencers
                        .scalar_types
                        .get_mut(input_type_reference)
                        .unwrap()
                        .directive_arguments
                        .insert(location.clone());
                }
                Some(ExtendedType::Enum(_)) => {
                    referencers
                        .enum_types
                        .get_mut(input_type_reference)
                        .unwrap()
                        .directive_arguments
                        .insert(location.clone());
                }
                Some(ExtendedType::InputObject(_)) => {
                    referencers
                        .input_object_types
                        .get_mut(input_type_reference)
                        .unwrap()
                        .directive_arguments
                        .insert(location.clone());
                }
                _ => {
                    panic!(
                        "Directive argument \"{}\"'s inner type \"{}\" does not refer to an existing input type.",
                        location,
                        input_type_reference.deref(),
                    )
                }
            }
        }
    }

    referencers
}
