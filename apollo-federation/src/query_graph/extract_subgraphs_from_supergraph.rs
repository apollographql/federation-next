use crate::schema::FederationSchema;
use crate::link::federation_spec_definition::{FederationSpecDefinition, FEDERATION_VERSIONS};
use crate::link::join_spec_definition::{
    FieldDirectiveArguments, JoinSpecDefinition, TypeDirectiveArguments, JOIN_VERSIONS,
};
use crate::link::link_spec_definition::LinkSpecDefinition;
use crate::schema::location::{DirectiveDefinitionLocation, EnumTypeDefinitionLocation, EnumValueDefinitionLocation, InputObjectFieldDefinitionLocation, InputObjectTypeDefinitionLocation, InterfaceFieldDefinitionLocation, InterfaceTypeDefinitionLocation, ObjectFieldDefinitionLocation, ObjectTypeDefinitionLocation, ScalarTypeDefinitionLocation, SchemaRootDefinitionKind, SchemaRootDefinitionLocation, TypeDefinitionLocation, UnionTypeDefinitionLocation};
use crate::link::spec::{Identity, Version};
use crate::link::spec_definition::{spec_definitions, SpecDefinition};
use apollo_compiler::ast::FieldDefinition;
use apollo_compiler::schema::{
    Component, ComponentOrigin, ComponentStr, DirectiveDefinition, DirectiveLocation, Directives,
    EnumType, EnumValueDefinition, ExtendedType, ExtensionId, InputObjectType,
    InputValueDefinition, InterfaceType, Name, NamedType, ObjectType, ScalarType, Type, UnionType,
};
use apollo_compiler::{Node, NodeStr, Schema};
use crate::error::{FederationError, SingleFederationError};
use indexmap::{IndexMap, IndexSet};
use lazy_static::lazy_static;
use std::collections::BTreeMap;
use std::ops::Deref;

// Assumes the given schema has been validated.
#[allow(dead_code)]
fn extract_subgraphs_from_supergraph(
    supergraph_schema: Schema,
    validate_extracted_subgraphs: Option<bool>,
) -> Result<FederationSubgraphs, FederationError> {
    let validate_extracted_subgraphs = validate_extracted_subgraphs.unwrap_or(true);
    let (supergraph_schema, link_spec_definition, join_spec_definition) =
        validate_supergraph(supergraph_schema)?;
    let is_fed_1 = *join_spec_definition.version() == Version { major: 0, minor: 1 };
    let (mut subgraphs, federation_spec_definitions, graph_enum_value_name_to_subgraph_name) =
        collect_empty_subgraphs(&supergraph_schema, join_spec_definition)?;

    let mut filtered_types: Vec<&NamedType> = Vec::new();
    for type_name in supergraph_schema
        .schema()
        .types
        .keys() {
        if !join_spec_definition.is_spec_type_name(&supergraph_schema, type_name)?
            && !link_spec_definition.is_spec_type_name(&supergraph_schema, type_name)? {
            filtered_types.push(type_name);
        }
    }
    if is_fed_1 {
        // Handle Fed 1 supergraphs eventually, the extraction logic is gnarly
        todo!()
    } else {
        extract_subgraphs_from_fed_2_supergraph(
            &supergraph_schema,
            &mut subgraphs,
            &graph_enum_value_name_to_subgraph_name,
            &federation_spec_definitions,
            join_spec_definition,
            &filtered_types,
        )?;
    }

    for graph_enum_value in graph_enum_value_name_to_subgraph_name.keys() {
        let subgraph = get_subgraph(
            &mut subgraphs,
            &graph_enum_value_name_to_subgraph_name,
            graph_enum_value,
        )?;
        let federation_spec_definition = federation_spec_definitions.get(graph_enum_value).unwrap();
        add_federation_operations(subgraph, federation_spec_definition)?;
        if validate_extracted_subgraphs {
            let Some(diagnostics) = subgraph.schema.schema().validate().err() else {
                continue;
            };
            // TODO: Implement maybeDumpSubgraphSchema() for better error messaging
            if is_fed_1 {
                // See message above about Fed 1 supergraphs
                todo!()
            } else {
                return Err(
                    SingleFederationError::InvalidFederationSupergraph {
                        message: format!(
                            "Unexpected error extracting {} from the supergraph: this is either a bug, or the supergraph has been corrupted.\n\nDetails:\n{}",
                            subgraph.name,
                            diagnostics.to_string_no_color()
                        ),
                    }.into()
                );
            }
        }
    }

    Ok(subgraphs)
}

fn validate_supergraph(
    supergraph_schema: Schema,
) -> Result<
    (
        FederationSchema,
        &'static LinkSpecDefinition,
        &'static JoinSpecDefinition,
    ),
    FederationError,
> {
    let supergraph_schema = FederationSchema::new(supergraph_schema)?;
    let Some(metadata) = supergraph_schema.metadata() else {
        return Err(SingleFederationError::InvalidFederationSupergraph {
            message: "Invalid supergraph: must be a core schema".to_owned(),
        }.into());
    };
    let link_spec_definition = metadata.link_spec_definition()?;
    let Some(join_link) = metadata.for_identity(&Identity::join_identity()) else {
        return Err(SingleFederationError::InvalidFederationSupergraph {
            message: "Invalid supergraph: must use the join spec".to_owned()
        }.into());
    };
    let Some(join_spec_definition) = spec_definitions(JOIN_VERSIONS.deref())?.find(&join_link.url.version) else {
        return Err(SingleFederationError::InvalidFederationSupergraph {
            message: format!(
                "Invalid supergraph: uses unsupported join spec version {} (supported versions: {})",
                spec_definitions(JOIN_VERSIONS.deref())?.versions().map( | v| v.to_string()).collect:: < Vec<String> > ().join(", "),
                join_link.url.version,
            ),
        }.into());
    };
    Ok((
        supergraph_schema,
        link_spec_definition,
        join_spec_definition,
    ))
}

fn collect_empty_subgraphs<'schema>(
    supergraph_schema: &'schema FederationSchema,
    join_spec_definition: &JoinSpecDefinition,
) -> Result<
    (
        FederationSubgraphs,
        IndexMap<&'schema Name, &'static FederationSpecDefinition>,
        IndexMap<&'schema Name, NodeStr>,
    ),
    FederationError,
> {
    let mut subgraphs = FederationSubgraphs::new();
    let graph_directive = join_spec_definition.graph_directive_definition(&supergraph_schema)?;
    let graph_enum = join_spec_definition.graph_enum_definition(&supergraph_schema)?;
    let mut federation_spec_definitions = IndexMap::new();
    let mut graph_enum_value_name_to_subgraph_name = IndexMap::new();
    for (enum_value_name, enum_value_definition) in graph_enum.values.iter() {
        let graph_application = enum_value_definition
            .directives
            .iter()
            .find(|d| d.name == graph_directive.name)
            .ok_or_else(|| {
                SingleFederationError::InvalidFederationSupergraph {
                    message: format!(
                        "Value \"{}\" of join__Graph enum has no @join__graph directive",
                        enum_value_name
                    ),
                }
            })?;
        let graph_arguments = join_spec_definition.graph_directive_arguments(graph_application)?;
        let subgraph = FederationSubgraph {
            name: graph_arguments.name.as_str().to_owned(),
            url: graph_arguments.url.as_str().to_owned(),
            schema: new_empty_fed_2_subgraph_schema()?,
        };
        let federation_link = &subgraph
            .schema
            .metadata()
            .as_ref()
            .and_then(|metadata| metadata.for_identity(&Identity::federation_identity()))
            .ok_or_else(|| {
                SingleFederationError::InvalidFederationSupergraph {
                    message: "Subgraph unexpectedly does not use federation spec".to_owned(),
                }
            })?;
        let federation_spec_definition =
            spec_definitions(FEDERATION_VERSIONS.deref())?
                .find(&federation_link.url.version)
                .ok_or_else(|| {
                    SingleFederationError::InvalidFederationSupergraph {
                        message: "Subgraph unexpectedly does not use a supported federation spec version".to_owned(),
                    }
                })?;
        subgraphs.add(subgraph)?;
        graph_enum_value_name_to_subgraph_name.insert(enum_value_name, graph_arguments.name);
        federation_spec_definitions.insert(enum_value_name, federation_spec_definition);
    }
    Ok((
        subgraphs,
        federation_spec_definitions,
        graph_enum_value_name_to_subgraph_name,
    ))
}

// TODO: Use the JS/programmatic approach instead of hard-coding definitions.
pub(crate) fn new_empty_fed_2_subgraph_schema() -> Result<FederationSchema, FederationError> {
    FederationSchema::new(Schema::parse(
        r#"
    extend schema
        @link(url: "https://specs.apollo.dev/link/v1.0")
        @link(url: "https://specs.apollo.dev/federation/v2.5")

    directive @link(url: String, as: String, for: link__Purpose, import: [link__Import]) repeatable on SCHEMA

    scalar link__Import

    enum link__Purpose {
        """
        \`SECURITY\` features provide metadata necessary to securely resolve fields.
        """
        SECURITY

        """
        \`EXECUTION\` features provide metadata necessary for operation execution.
        """
        EXECUTION
    }

    directive @federation__key(fields: federation__FieldSet!, resolvable: Boolean = true) repeatable on OBJECT | INTERFACE

    directive @federation__requires(fields: federation__FieldSet!) on FIELD_DEFINITION

    directive @federation__provides(fields: federation__FieldSet!) on FIELD_DEFINITION

    directive @federation__external(reason: String) on OBJECT | FIELD_DEFINITION

    directive @federation__tag(name: String!) repeatable on FIELD_DEFINITION | OBJECT | INTERFACE | UNION | ARGUMENT_DEFINITION | SCALAR | ENUM | ENUM_VALUE | INPUT_OBJECT | INPUT_FIELD_DEFINITION | SCHEMA

    directive @federation__extends on OBJECT | INTERFACE

    directive @federation__shareable on OBJECT | FIELD_DEFINITION

    directive @federation__inaccessible on FIELD_DEFINITION | OBJECT | INTERFACE | UNION | ARGUMENT_DEFINITION | SCALAR | ENUM | ENUM_VALUE | INPUT_OBJECT | INPUT_FIELD_DEFINITION

    directive @federation__override(from: String!) on FIELD_DEFINITION

    directive @federation__composeDirective(name: String) repeatable on SCHEMA

    directive @federation__interfaceObject on OBJECT

    directive @federation__authenticated on FIELD_DEFINITION | OBJECT | INTERFACE | SCALAR | ENUM

    directive @federation__requiresScopes(scopes: [[federation__Scope!]!]!) on FIELD_DEFINITION | OBJECT | INTERFACE | SCALAR | ENUM

    scalar federation__FieldSet

    scalar federation__Scope
    "#,
        "subgraph.graphql",
    ))
}

struct TypeInfo<'schema> {
    name: &'schema NamedType,
    // HashMap<subgraph_enum_value: String, is_interface_object: bool>
    subgraph_info: IndexMap<Name, bool>,
}

struct TypeInfos<'schema> {
    object_types: Vec<TypeInfo<'schema>>,
    interface_types: Vec<TypeInfo<'schema>>,
    union_types: Vec<TypeInfo<'schema>>,
    enum_types: Vec<TypeInfo<'schema>>,
    input_object_types: Vec<TypeInfo<'schema>>,
}

fn extract_subgraphs_from_fed_2_supergraph<'subgraph, 'schema>(
    supergraph_schema: &'schema FederationSchema,
    subgraphs: &'subgraph mut FederationSubgraphs,
    graph_enum_value_name_to_subgraph_name: &IndexMap<&'schema Name, NodeStr>,
    federation_spec_definitions: &IndexMap<&Name, &'static FederationSpecDefinition>,
    join_spec_definition: &'static JoinSpecDefinition,
    filtered_types: &Vec<&'schema NamedType>,
) -> Result<(), FederationError> {
    let TypeInfos {
        object_types,
        interface_types,
        union_types,
        enum_types,
        input_object_types,
    } = add_all_empty_subgraph_types(
        supergraph_schema,
        subgraphs,
        graph_enum_value_name_to_subgraph_name,
        federation_spec_definitions,
        join_spec_definition,
        filtered_types,
    )?;

    extract_object_type_content(
        supergraph_schema,
        subgraphs,
        graph_enum_value_name_to_subgraph_name,
        federation_spec_definitions,
        join_spec_definition,
        &object_types,
    )?;
    extract_interface_type_content(
        supergraph_schema,
        subgraphs,
        graph_enum_value_name_to_subgraph_name,
        federation_spec_definitions,
        join_spec_definition,
        &interface_types,
    )?;
    extract_union_type_content(
        supergraph_schema,
        subgraphs,
        graph_enum_value_name_to_subgraph_name,
        join_spec_definition,
        &union_types,
    )?;
    extract_enum_type_content(
        supergraph_schema,
        subgraphs,
        graph_enum_value_name_to_subgraph_name,
        join_spec_definition,
        &enum_types,
    )?;
    extract_input_object_type_content(
        supergraph_schema,
        subgraphs,
        graph_enum_value_name_to_subgraph_name,
        join_spec_definition,
        &input_object_types,
    )?;

    // We add all the "executable" directive definitions from the supergraph to each subgraphs, as
    // those may be part of a query and end up in any subgraph fetches. We do this "last" to make
    // sure that if one of the directives uses a type for an argument, that argument exists. Note
    // that we don't bother with non-executable directive definitions at the moment since we
    // don't extract their applications. It might become something we need later, but we don't so
    // far. Accordingly, we skip any potentially applied directives in the argument of the copied
    // definition, because we haven't copied type-system directives.
    let all_executable_directive_definitions = supergraph_schema
        .schema()
        .directive_definitions
        .values()
        .filter_map(|directive_definition| {
            let executable_locations = directive_definition
                .locations
                .iter()
                .filter(|location| EXECUTABLE_DIRECTIVE_LOCATIONS.contains(*location))
                .map(|location| location.clone())
                .collect::<Vec<_>>();
            if executable_locations.is_empty() {
                return None;
            }
            Some(Node::new(DirectiveDefinition {
                description: None,
                name: directive_definition.name.clone(),
                arguments: directive_definition
                    .arguments
                    .iter()
                    .map(|argument| {
                        Node::new(InputValueDefinition {
                            description: None,
                            name: argument.name.clone(),
                            ty: argument.ty.clone(),
                            default_value: argument.default_value.clone(),
                            directives: Default::default(),
                        })
                    })
                    .collect::<Vec<_>>(),
                repeatable: directive_definition.repeatable,
                locations: executable_locations,
            }))
        })
        .collect::<Vec<_>>();
    for subgraph in subgraphs.subgraphs.values_mut() {
        // TODO: removeInactiveProvidesAndRequires(subgraph.schema)
        remove_unused_types_from_subgraph(subgraph)?;
        for definition in all_executable_directive_definitions.iter() {
            DirectiveDefinitionLocation {
                directive_name: definition.name.clone()
            }.insert(
                &mut subgraph.schema,
                definition.clone()
            )?;
        }
    }

    Ok(())
}

fn add_all_empty_subgraph_types<'subgraph, 'schema>(
    supergraph_schema: &'schema FederationSchema,
    subgraphs: &'subgraph mut FederationSubgraphs,
    graph_enum_value_name_to_subgraph_name: &IndexMap<&'schema Name, NodeStr>,
    federation_spec_definitions: &IndexMap<&Name, &'static FederationSpecDefinition>,
    join_spec_definition: &'static JoinSpecDefinition,
    filtered_types: &Vec<&'schema NamedType>,
) -> Result<TypeInfos<'schema>, FederationError> {
    let type_directive_definition =
        join_spec_definition.type_directive_definition(supergraph_schema)?;

    let mut object_types: Vec<TypeInfo> = Vec::new();
    let mut interface_types: Vec<TypeInfo> = Vec::new();
    let mut union_types: Vec<TypeInfo> = Vec::new();
    let mut enum_types: Vec<TypeInfo> = Vec::new();
    let mut input_object_types: Vec<TypeInfo> = Vec::new();

    for type_name in filtered_types {
        let type_ = supergraph_schema.schema().types.get(*type_name).ok_or_else(|| {
            SingleFederationError::Internal {
                message: format!("Type \"{}\" missing from schema", type_name),
            }
        })?;
        let mut type_directive_applications = Vec::new();
        for directive in type_.directives().iter() {
            if directive.name != type_directive_definition.name {
                continue;
            }
            type_directive_applications
                .push(join_spec_definition.type_directive_arguments(directive)?);
        }
        match type_ {
            ExtendedType::Scalar(_) => {
                // Scalar are a bit special in that they don't have any sub-component, so we don't
                // track them beyond adding them to the proper subgraphs. It's also simple because
                // there is no possible key so there is exactly one @join__type application for each
                // subgraph having the scalar (and most arguments cannot be present).
                for type_directive_application in type_directive_applications {
                    let subgraph = get_subgraph(
                        subgraphs,
                        graph_enum_value_name_to_subgraph_name,
                        &type_directive_application.graph,
                    )?;
                    let loc = ScalarTypeDefinitionLocation { type_name: (*type_name).clone() };
                    loc.pre_insert(&mut subgraph.schema)?;
                    loc.insert(
                        &mut subgraph.schema,
                        Node::new(ScalarType {
                            description: None,
                            directives: Default::default(),
                        })
                    )?;
                }
            }
            ExtendedType::Object(_) => {
                object_types.push(add_empty_type(
                    type_name,
                    type_,
                    &type_directive_applications,
                    subgraphs,
                    graph_enum_value_name_to_subgraph_name,
                    federation_spec_definitions,
                )?);
            }
            ExtendedType::Interface(_) => {
                interface_types.push(add_empty_type(
                    type_name,
                    type_,
                    &type_directive_applications,
                    subgraphs,
                    graph_enum_value_name_to_subgraph_name,
                    federation_spec_definitions,
                )?);
            }
            ExtendedType::Union(_) => {
                union_types.push(add_empty_type(
                    type_name,
                    type_,
                    &type_directive_applications,
                    subgraphs,
                    graph_enum_value_name_to_subgraph_name,
                    federation_spec_definitions,
                )?);
            }
            ExtendedType::Enum(_) => {
                enum_types.push(add_empty_type(
                    type_name,
                    type_,
                    &type_directive_applications,
                    subgraphs,
                    graph_enum_value_name_to_subgraph_name,
                    federation_spec_definitions,
                )?);
            }
            ExtendedType::InputObject(_) => {
                input_object_types.push(add_empty_type(
                    type_name,
                    type_,
                    &type_directive_applications,
                    subgraphs,
                    graph_enum_value_name_to_subgraph_name,
                    federation_spec_definitions,
                )?);
            }
        }
    }

    Ok(TypeInfos {
        object_types,
        interface_types,
        union_types,
        enum_types,
        input_object_types,
    })
}

fn add_empty_type<'subgraph, 'schema>(
    type_name: &'schema NamedType,
    type_: &'schema ExtendedType,
    type_directive_applications: &Vec<TypeDirectiveArguments>,
    subgraphs: &'subgraph mut FederationSubgraphs,
    graph_enum_value_name_to_subgraph_name: &IndexMap<&'schema Name, NodeStr>,
    federation_spec_definitions: &IndexMap<&Name, &'static FederationSpecDefinition>,
) -> Result<TypeInfo<'schema>, FederationError> {
    // In fed2, we always mark all types with `@join__type` but making sure.
    if type_directive_applications.is_empty() {
        return Err(
            SingleFederationError::InvalidFederationSupergraph {
                message: format!(
                    "Missing @join__type on \"{}\"",
                    type_name
                ),
            }.into()
        );
    }
    let mut type_info = TypeInfo {
        name: type_name,
        subgraph_info: IndexMap::new(),
    };
    for type_directive_application in type_directive_applications {
        let subgraph = get_subgraph(
            subgraphs,
            graph_enum_value_name_to_subgraph_name,
            &type_directive_application.graph,
        )?;
        let federation_spec_definition = federation_spec_definitions
            .get(&type_directive_application.graph)
            .ok_or_else(|| {
                SingleFederationError::Internal {
                    message: format!(
                        "Missing federation spec info for subgraph enum value \"{}\"",
                        type_directive_application.graph
                    ),
                }
            })?;

        if type_info
            .subgraph_info
            .contains_key(&type_directive_application.graph)
        {
            if let Some(key) = &type_directive_application.key {
                let mut key_directive =
                    Component::new(federation_spec_definition.key_directive(
                        &subgraph.schema,
                        key.clone(),
                        type_directive_application.resolvable,
                    )?);
                if type_directive_application.extension {
                    key_directive.origin =
                        ComponentOrigin::Extension(ExtensionId::new(&key_directive.node))
                }
                match subgraph.schema.schema().types.get(type_name).ok_or_else(|| {
                    SingleFederationError::Internal {
                        message: format!(
                            "Missing type \"{}\" from subgraph despite it being in type_info",
                            type_name
                        ),
                    }
                })?
                {
                    ExtendedType::Scalar(_) => {
                        return Err(
                            SingleFederationError::Internal {
                                message: "\"add_empty_type()\" shouldn't be called for scalars".to_owned(),
                            }.into()
                        );
                    }
                    ExtendedType::Object(_) => {
                        ObjectTypeDefinitionLocation { type_name: type_name.clone() }.insert_directive(
                            &mut subgraph.schema,
                            key_directive,
                        )?;
                    }
                    ExtendedType::Interface(_) => {
                        InterfaceTypeDefinitionLocation { type_name: type_name.clone() }.insert_directive(
                            &mut subgraph.schema,
                            key_directive,
                        )?;
                    }
                    ExtendedType::Union(_) => {
                        UnionTypeDefinitionLocation { type_name: type_name.clone() }.insert_directive(
                            &mut subgraph.schema,
                            key_directive,
                        )?;
                    }
                    ExtendedType::Enum(_) => {
                        EnumTypeDefinitionLocation { type_name: type_name.clone() }.insert_directive(
                            &mut subgraph.schema,
                            key_directive,
                        )?;
                    }
                    ExtendedType::InputObject(_) => {
                        InputObjectTypeDefinitionLocation { type_name: type_name.clone() }.insert_directive(
                            &mut subgraph.schema,
                            key_directive,
                        )?;
                    }
                };
            }
        } else {
            let mut is_interface_object = false;
            match type_ {
                ExtendedType::Scalar(_) => {
                    return Err(
                        SingleFederationError::Internal {
                            message: "\"add_empty_type()\" shouldn't be called for scalars".to_owned(),
                        }.into()
                    );
                }
                ExtendedType::Object(_) => {
                    let loc = ObjectTypeDefinitionLocation { type_name: type_name.clone() };
                    loc.pre_insert(&mut subgraph.schema)?;
                    loc.insert(
                        &mut subgraph.schema,
                        Node::new(ObjectType {
                            description: None,
                            implements_interfaces: Default::default(),
                            directives: Default::default(),
                            fields: Default::default(),
                        })
                    )?;
                    if type_name == "Query" {
                        let loc = SchemaRootDefinitionLocation { root_kind: SchemaRootDefinitionKind::Query };
                        if loc.try_get(subgraph.schema.schema()).is_none() {
                            loc.insert(
                                &mut subgraph.schema,
                                ComponentStr::new(type_name),
                            )?;
                        }
                    } else if type_name == "Mutation" {
                        let loc = SchemaRootDefinitionLocation { root_kind: SchemaRootDefinitionKind::Mutation };
                        if loc.try_get(subgraph.schema.schema()).is_none() {
                            loc.insert(
                                &mut subgraph.schema,
                                ComponentStr::new(type_name),
                            )?;
                        }
                    } else if type_name == "Subscription" {
                        let loc = SchemaRootDefinitionLocation { root_kind: SchemaRootDefinitionKind::Subscription };
                        if loc.try_get(subgraph.schema.schema()).is_none() {
                            loc.insert(
                                &mut subgraph.schema,
                                ComponentStr::new(type_name),
                            )?;
                        }
                    }
                }
                ExtendedType::Interface(_) => {
                    if type_directive_application.is_interface_object {
                        is_interface_object = true;
                        let interface_object_directive = federation_spec_definition
                            .interface_object_directive(&subgraph.schema, )?;
                        let loc = ObjectTypeDefinitionLocation { type_name: type_name.clone() };
                        loc.pre_insert(&mut subgraph.schema)?;
                        loc.insert(
                            &mut subgraph.schema,
                            Node::new(ObjectType {
                                description: None,
                                implements_interfaces: Default::default(),
                                directives: Directives(vec![Component::new(
                                    interface_object_directive,
                                )]),
                                fields: Default::default(),
                            })
                        )?;
                    } else {
                        let loc = InterfaceTypeDefinitionLocation { type_name: type_name.clone() };
                        loc.pre_insert(&mut subgraph.schema)?;
                        loc.insert(
                            &mut subgraph.schema,
                            Node::new(InterfaceType {
                                description: None,
                                implements_interfaces: Default::default(),
                                directives: Default::default(),
                                fields: Default::default(),
                            })
                        )?;
                    }
                }
                ExtendedType::Union(_) => {
                    let loc = UnionTypeDefinitionLocation { type_name: type_name.clone() };
                    loc.pre_insert(&mut subgraph.schema)?;
                    loc.insert(
                        &mut subgraph.schema,
                        Node::new(UnionType {
                            description: None,
                            directives: Default::default(),
                            members: Default::default(),
                        })
                    )?;
                }
                ExtendedType::Enum(_) => {
                    let loc = EnumTypeDefinitionLocation { type_name: type_name.clone() };
                    loc.pre_insert(&mut subgraph.schema)?;
                    loc.insert(
                        &mut subgraph.schema,
                        Node::new(EnumType {
                            description: None,
                            directives: Default::default(),
                            values: Default::default(),
                        })
                    )?;
                },
                ExtendedType::InputObject(_) => {
                    let loc = InputObjectTypeDefinitionLocation { type_name: type_name.clone() };
                    loc.pre_insert(&mut subgraph.schema)?;
                    loc.insert(
                        &mut subgraph.schema,
                        Node::new(InputObjectType {
                            description: None,
                            directives: Default::default(),
                            fields: Default::default(),
                        })
                    )?;
                }
            };
            type_info.subgraph_info.insert(
                type_directive_application.graph.clone(),
                is_interface_object,
            );
        };
    }

    Ok(type_info)
}

fn extract_object_type_content<'schema>(
    supergraph_schema: &'schema FederationSchema,
    subgraphs: &mut FederationSubgraphs,
    graph_enum_value_name_to_subgraph_name: &IndexMap<&'schema Name, NodeStr>,
    federation_spec_definitions: &IndexMap<&Name, &'static FederationSpecDefinition>,
    join_spec_definition: &JoinSpecDefinition,
    info: &Vec<TypeInfo<'schema>>,
) -> Result<(), FederationError> {
    let field_directive_definition =
        join_spec_definition.field_directive_definition(supergraph_schema)?;
    // join__implements was added in join 0.2, and this method does not run for join 0.1, so it
    // should be defined.
    let implements_directive_definition = join_spec_definition
        .implements_directive_definition(supergraph_schema)?
        .ok_or_else(|| {
            SingleFederationError::InvalidFederationSupergraph {
                message: "@join__implements should exist for a fed2 supergraph".to_owned(),
            }
        })?;

    for TypeInfo {
        name: type_name,
        subgraph_info,
    } in info.iter()
    {
        let loc = ObjectTypeDefinitionLocation { type_name: (*type_name).clone() };
        let type_ = loc.get(&supergraph_schema.schema())?;

        for directive in type_.directives.iter() {
            if directive.name != implements_directive_definition.name {
                continue;
            }
            let implements_directive_application =
                join_spec_definition.implements_directive_arguments(directive)?;
            if !subgraph_info.contains_key(&implements_directive_application.graph) {
                return Err(
                    SingleFederationError::InvalidFederationSupergraph {
                        message: format!(
                            "@join__implements cannot exist on \"{}\" for subgraph \"{}\" without type-level @join__type",
                            type_name,
                            implements_directive_application.graph,
                        ),
                    }.into()
                );
            }
            let subgraph = get_subgraph(
                subgraphs,
                graph_enum_value_name_to_subgraph_name,
                &implements_directive_application.graph,
            )?;
            loc.insert_implements_interface(
                &mut subgraph.schema,
                implements_directive_application.interface.clone(),
            )?;
        }

        for (field_name, field) in type_.fields.iter() {
            let mut field_directive_applications = Vec::new();
            for directive in field.directives.iter() {
                if directive.name != field_directive_definition.name {
                    continue;
                }
                field_directive_applications
                    .push(join_spec_definition.field_directive_arguments(directive)?);
            }
            if field_directive_applications.is_empty() {
                // In a fed2 subgraph, no @join__field means that the field is in all the subgraphs
                // in which the type is.
                let is_shareable = subgraph_info.len() > 1;
                for graph_enum_value in subgraph_info.keys() {
                    let subgraph = get_subgraph(
                        subgraphs,
                        graph_enum_value_name_to_subgraph_name,
                        graph_enum_value,
                    )?;
                    let federation_spec_definition =
                        federation_spec_definitions.get(graph_enum_value).ok_or_else(|| {
                            SingleFederationError::InvalidFederationSupergraph {
                                message: "Subgraph unexpectedly does not use federation spec".to_owned(),
                            }
                        })?;
                    add_subgraph_field(
                        field_name,
                        field,
                        type_name,
                        subgraph,
                        federation_spec_definition,
                        is_shareable,
                        None,
                    )?;
                }
            } else {
                let is_shareable = field_directive_applications
                    .iter()
                    .filter(|field_directive_application| {
                        !field_directive_application.external.unwrap_or(false)
                            && !field_directive_application.user_overridden.unwrap_or(false)
                    })
                    .count()
                    > 1;

                for field_directive_application in &field_directive_applications {
                    let Some(graph_enum_value) = &field_directive_application.graph else {
                        // We use a @join__field with no graph to indicates when a field in the
                        // supergraph does not come directly from any subgraph and there is thus
                        // nothing to do to "extract" it.
                        continue;
                    };
                    let subgraph = get_subgraph(
                        subgraphs,
                        graph_enum_value_name_to_subgraph_name,
                        graph_enum_value,
                    )?;
                    let federation_spec_definition =
                        federation_spec_definitions.get(graph_enum_value).ok_or_else(|| {
                            SingleFederationError::InvalidFederationSupergraph {
                                message: "Subgraph unexpectedly does not use federation spec".to_owned(),
                            }
                        })?;
                    if !subgraph_info.contains_key(graph_enum_value) {
                        return Err(
                            SingleFederationError::InvalidFederationSupergraph {
                                message: format!(
                                    "@join__field cannot exist on {}.{} for subgraph {} without type-level @join__type",
                                    type_name,
                                    field_name,
                                    graph_enum_value,
                                ),
                            }.into()
                        );
                    }
                    add_subgraph_field(
                        field_name,
                        field,
                        type_name,
                        subgraph,
                        federation_spec_definition,
                        is_shareable,
                        Some(field_directive_application),
                    )?;
                }
            }
        }
    }

    Ok(())
}

fn extract_interface_type_content<'subgraph, 'schema>(
    supergraph_schema: &'schema FederationSchema,
    subgraphs: &mut FederationSubgraphs,
    graph_enum_value_name_to_subgraph_name: &IndexMap<&'schema Name, NodeStr>,
    federation_spec_definitions: &IndexMap<&Name, &'static FederationSpecDefinition>,
    join_spec_definition: &JoinSpecDefinition,
    info: &Vec<TypeInfo<'schema>>,
) -> Result<(), FederationError> {
    let field_directive_definition =
        join_spec_definition.field_directive_definition(supergraph_schema)?;
    // join_implements was added in join 0.2, and this method does not run for join 0.1, so it
    // should be defined.
    let implements_directive_definition = join_spec_definition
        .implements_directive_definition(supergraph_schema)?
        .ok_or_else(|| {
            SingleFederationError::InvalidFederationSupergraph {
                message: "@join__implements should exist for a fed2 supergraph".to_owned(),
            }
        })?;

    for TypeInfo {
        name: type_name,
        subgraph_info,
    } in info.iter()
    {
        let loc = InterfaceTypeDefinitionLocation { type_name: (*type_name).clone() };
        let type_ = loc.get(&supergraph_schema.schema())?;

        for directive in type_.directives.iter() {
            if directive.name != implements_directive_definition.name {
                continue;
            }
            let implements_directive_application =
                join_spec_definition.implements_directive_arguments(directive)?;
            let is_interface_object = *subgraph_info.get(&implements_directive_application.graph).ok_or_else(|| {
                SingleFederationError::InvalidFederationSupergraph {
                    message: format!(
                        "@join__implements cannot exist on {} for subgraph {} without type-level @join__type",
                        type_name,
                        implements_directive_application.graph,
                    ),
                }
            })?;
            let subgraph = get_subgraph(
                subgraphs,
                graph_enum_value_name_to_subgraph_name,
                &implements_directive_application.graph,
            )?;
            match subgraph
                .schema
                .schema()
                .types
                .get(*type_name)
                .ok_or_else(|| {
                    SingleFederationError::Internal {
                        message: format!(
                            "Missing type \"{}\" from subgraph despite it being in type_info",
                            type_name
                        ),
                    }
                })?
            {
                ExtendedType::Object(_) => {
                    if !is_interface_object {
                        return Err(
                            SingleFederationError::Internal {
                                message: "\"extract_interface_type_content()\" encountered an unexpected interface object type in subgraph".to_owned(),
                            }.into()
                        );
                    }
                    let loc = ObjectTypeDefinitionLocation { type_name: (*type_name).clone() };
                    loc.insert_implements_interface(
                        &mut subgraph.schema,
                        implements_directive_application.interface.clone()
                    )?;
                }
                ExtendedType::Interface(_) => {
                    if is_interface_object {
                        return Err(
                            SingleFederationError::Internal {
                                message: "\"extract_interface_type_content()\" encountered an interface type in subgraph that should have been an interface object".to_owned(),
                            }.into()
                        );
                    }
                    loc.insert_implements_interface(
                        &mut subgraph.schema,
                        implements_directive_application.interface.clone()
                    )?;
                }
                _ => {
                    return Err(
                        SingleFederationError::Internal {
                            message: "\"extract_interface_type_content()\" encountered non-object/interface type in subgraph".to_owned(),
                        }.into()
                    );
                }
            };
        }

        for (field_name, field) in type_.fields.iter() {
            let mut field_directive_applications = Vec::new();
            for directive in field.directives.iter() {
                if directive.name != field_directive_definition.name {
                    continue;
                }
                field_directive_applications
                    .push(join_spec_definition.field_directive_arguments(directive)?);
            }
            if field_directive_applications.is_empty() {
                // In a fed2 subgraph, no @join__field means that the field is in all the subgraphs
                // in which the type is.
                for graph_enum_value in subgraph_info.keys() {
                    let subgraph = get_subgraph(
                        subgraphs,
                        graph_enum_value_name_to_subgraph_name,
                        graph_enum_value,
                    )?;
                    let federation_spec_definition =
                        federation_spec_definitions.get(graph_enum_value).ok_or_else(|| {
                            SingleFederationError::InvalidFederationSupergraph {
                                message: "Subgraph unexpectedly does not use federation spec".to_owned(),
                            }
                        })?;
                    add_subgraph_field(
                        field_name,
                        field,
                        type_name,
                        subgraph,
                        federation_spec_definition,
                        false,
                        None,
                    )?;
                }
            } else {
                for field_directive_application in &field_directive_applications {
                    let Some(graph_enum_value) = &field_directive_application.graph else {
                        // We use a @join__field with no graph to indicates when a field in the
                        // supergraph does not come directly from any subgraph and there is thus
                        // nothing to do to "extract" it.
                        continue;
                    };
                    let subgraph = get_subgraph(
                        subgraphs,
                        graph_enum_value_name_to_subgraph_name,
                        &graph_enum_value,
                    )?;
                    let federation_spec_definition =
                        federation_spec_definitions.get(graph_enum_value).ok_or_else(|| {
                            SingleFederationError::InvalidFederationSupergraph {
                                message: "Subgraph unexpectedly does not use federation spec".to_owned(),
                            }
                        })?;
                    if !subgraph_info.contains_key(graph_enum_value) {
                        return Err(
                            SingleFederationError::InvalidFederationSupergraph {
                                message: format!(
                                    "@join__field cannot exist on {}.{} for subgraph {} without type-level @join__type",
                                    type_name,
                                    field_name,
                                    graph_enum_value,
                                ),
                            }.into()
                        );
                    }
                    add_subgraph_field(
                        field_name,
                        field,
                        type_name,
                        subgraph,
                        federation_spec_definition,
                        false,
                        Some(field_directive_application),
                    )?;
                }
            }
        }
    }

    Ok(())
}

fn extract_union_type_content<'subgraph, 'schema>(
    supergraph_schema: &'schema FederationSchema,
    subgraphs: &mut FederationSubgraphs,
    graph_enum_value_name_to_subgraph_name: &IndexMap<&'schema Name, NodeStr>,
    join_spec_definition: &JoinSpecDefinition,
    info: &Vec<TypeInfo<'schema>>,
) -> Result<(), FederationError> {
    // This was added in join 0.3, so it can genuinely be None.
    let union_member_directive_definition =
        join_spec_definition.union_member_directive_definition(supergraph_schema)?;

    // Note that union members works a bit differently from fields or enum values, and this because
    // we cannot have directive applications on type members. So the `join_unionMember` directive
    // applications are on the type itself, and they mention the member that they target.
    for TypeInfo {
        name: type_name,
        subgraph_info,
    } in info.iter()
    {
        let loc = UnionTypeDefinitionLocation { type_name: (*type_name).clone() };
        let type_ = loc.get(&supergraph_schema.schema())?;

        let mut union_member_directive_applications = Vec::new();
        if let Some(union_member_directive_definition) = union_member_directive_definition {
            for directive in type_.directives.iter() {
                if directive.name != union_member_directive_definition.name {
                    continue;
                }
                union_member_directive_applications
                    .push(join_spec_definition.union_member_directive_arguments(directive)?);
            }
        }
        if union_member_directive_applications.is_empty() {
            // No @join__unionMember; every member should be added to every subgraph having the
            // union (at least as long as the subgraph has the member itself).
            for graph_enum_value in subgraph_info.keys() {
                let subgraph = get_subgraph(
                    subgraphs,
                    graph_enum_value_name_to_subgraph_name,
                    graph_enum_value,
                )?;
                // Note that object types in the supergraph are guaranteed to be object types in
                // subgraphs.
                let subgraph_members = type_
                    .members
                    .iter()
                    .filter(|member| subgraph.schema.schema().types.contains_key((*member).deref()))
                    .map(|member| member.clone())
                    .collect::<Vec<_>>();
                for member in subgraph_members {
                    loc.insert_member(
                        &mut subgraph.schema,
                        member.node
                    )?;
                }
            }
        } else {
            for union_member_directive_application in &union_member_directive_applications {
                let subgraph = get_subgraph(
                    subgraphs,
                    graph_enum_value_name_to_subgraph_name,
                    &union_member_directive_application.graph,
                )?;
                if !subgraph_info.contains_key(&union_member_directive_application.graph) {
                    return Err(
                        SingleFederationError::InvalidFederationSupergraph {
                            message: format!(
                                "@join__unionMember cannot exist on {} for subgraph {} without type-level @join__type",
                                type_name,
                                union_member_directive_application.graph,
                            ),
                        }.into()
                    );
                }
                // Note that object types in the supergraph are guaranteed to be object types in
                // subgraphs. We also know that the type must exist in this case (we don't generate
                // broken @join__unionMember).
                loc.insert_member(
                    &mut subgraph.schema,
                    union_member_directive_application.member.clone()
                )?;
            }
        }
    }

    Ok(())
}

fn extract_enum_type_content<'subgraph, 'schema>(
    supergraph_schema: &'schema FederationSchema,
    subgraphs: &mut FederationSubgraphs,
    graph_enum_value_name_to_subgraph_name: &IndexMap<&'schema Name, NodeStr>,
    join_spec_definition: &JoinSpecDefinition,
    info: &Vec<TypeInfo<'schema>>,
) -> Result<(), FederationError> {
    // This was added in join 0.3, so it can genuinely be None.
    let enum_value_directive_definition =
        join_spec_definition.enum_value_directive_definition(supergraph_schema)?;

    for TypeInfo {
        name: type_name,
        subgraph_info,
    } in info.iter()
    {
        let loc = EnumTypeDefinitionLocation { type_name: (*type_name).clone() };
        let type_ = loc.get(&supergraph_schema.schema())?;

        for (value_name, value) in type_.values.iter() {
            let mut enum_value_directive_applications = Vec::new();
            if let Some(enum_value_directive_definition) = enum_value_directive_definition {
                for directive in value.directives.iter() {
                    if directive.name != enum_value_directive_definition.name {
                        continue;
                    }
                    enum_value_directive_applications
                        .push(join_spec_definition.enum_value_directive_arguments(directive)?);
                }
            }
            if enum_value_directive_applications.is_empty() {
                for graph_enum_value in subgraph_info.keys() {
                    let subgraph = get_subgraph(
                        subgraphs,
                        graph_enum_value_name_to_subgraph_name,
                        graph_enum_value,
                    )?;
                    EnumValueDefinitionLocation {
                        type_name: (*type_name).clone(),
                        value_name: value_name.clone(),
                    }.insert(
                        &mut subgraph.schema,
                        Component::new(EnumValueDefinition {
                            description: None,
                            value: value_name.clone(),
                            directives: Default::default(),
                        }),
                    )?;
                }
            } else {
                for enum_value_directive_application in &enum_value_directive_applications {
                    let subgraph = get_subgraph(
                        subgraphs,
                        graph_enum_value_name_to_subgraph_name,
                        &enum_value_directive_application.graph,
                    )?;
                    if !subgraph_info.contains_key(&enum_value_directive_application.graph) {
                        return Err(
                            SingleFederationError::InvalidFederationSupergraph {
                                message: format!(
                                    "@join__enumValue cannot exist on {}.{} for subgraph {} without type-level @join__type",
                                    type_name,
                                    value_name,
                                    enum_value_directive_application.graph,
                                ),
                            }.into()
                        );
                    }
                    EnumValueDefinitionLocation {
                        type_name: (*type_name).clone(),
                        value_name: value_name.clone(),
                    }.insert(
                        &mut subgraph.schema,
                        Component::new(EnumValueDefinition {
                            description: None,
                            value: value_name.clone(),
                            directives: Default::default(),
                        }),
                    )?;
                }
            }
        }
    }

    Ok(())
}

fn extract_input_object_type_content<'subgraph, 'schema>(
    supergraph_schema: &'schema FederationSchema,
    subgraphs: &mut FederationSubgraphs,
    graph_enum_value_name_to_subgraph_name: &IndexMap<&'schema Name, NodeStr>,
    join_spec_definition: &JoinSpecDefinition,
    info: &Vec<TypeInfo<'schema>>,
) -> Result<(), FederationError> {
    let field_directive_definition =
        join_spec_definition.field_directive_definition(supergraph_schema)?;

    for TypeInfo {
        name: type_name,
        subgraph_info,
    } in info.iter()
    {
        let loc = InputObjectTypeDefinitionLocation { type_name: (*type_name).clone() };
        let type_ = loc.get(&supergraph_schema.schema())?;

        for (input_field_name, input_field) in type_.fields.iter() {
            let mut field_directive_applications = Vec::new();
            for directive in input_field.directives.iter() {
                if directive.name != field_directive_definition.name {
                    continue;
                }
                field_directive_applications
                    .push(join_spec_definition.field_directive_arguments(directive)?);
            }
            if field_directive_applications.is_empty() {
                for graph_enum_value in subgraph_info.keys() {
                    let subgraph = get_subgraph(
                        subgraphs,
                        graph_enum_value_name_to_subgraph_name,
                        graph_enum_value,
                    )?;
                    add_subgraph_input_field(
                        input_field_name,
                        input_field,
                        type_name,
                        subgraph,
                        None,
                    )?;
                }
            } else {
                for field_directive_application in &field_directive_applications {
                    let Some(graph_enum_value) = &field_directive_application.graph else {
                        // We use a @join__field with no graph to indicates when a field in the
                        // supergraph does not come directly from any subgraph and there is thus
                        // nothing to do to "extract" it.
                        continue;
                    };
                    let subgraph = get_subgraph(
                        subgraphs,
                        graph_enum_value_name_to_subgraph_name,
                        graph_enum_value,
                    )?;
                    if !subgraph_info.contains_key(graph_enum_value) {
                        return Err(
                            SingleFederationError::InvalidFederationSupergraph {
                                message: format!(
                                    "@join__field cannot exist on {}.{} for subgraph {} without type-level @join__type",
                                    type_name,
                                    input_field_name,
                                    graph_enum_value,
                                ),
                            }.into()
                        );
                    }
                    add_subgraph_input_field(
                        input_field_name,
                        input_field,
                        type_name,
                        subgraph,
                        Some(field_directive_application),
                    )?;
                }
            }
        }
    }

    Ok(())
}

fn add_subgraph_field<'subgraph, 'schema>(
    field_name: &'schema Name,
    field: &'schema FieldDefinition,
    type_name: &'schema NamedType,
    subgraph: &'subgraph mut FederationSubgraph,
    federation_spec_definition: &'static FederationSpecDefinition,
    is_shareable: bool,
    field_directive_application: Option<&FieldDirectiveArguments>,
) -> Result<(), FederationError> {
    let field_directive_application =
        field_directive_application.unwrap_or_else(|| &FieldDirectiveArguments {
            graph: None,
            requires: None,
            provides: None,
            type_: None,
            external: None,
            override_: None,
            user_overridden: None,
        });
    let subgraph_field_type = match &field_directive_application.type_ {
        Some(t) => decode_type(t)?,
        None => field.ty.clone(),
    };
    let mut subgraph_field = FieldDefinition {
        description: None,
        name: field_name.clone(),
        arguments: vec![],
        ty: subgraph_field_type,
        directives: Default::default(),
    };

    for argument in &field.arguments {
        subgraph_field
            .arguments
            .push(Node::new(InputValueDefinition {
                description: None,
                name: argument.name.clone(),
                ty: argument.ty.clone(),
                default_value: argument.default_value.clone(),
                directives: Default::default(),
            }))
    }
    if let Some(requires) = &field_directive_application.requires {
        subgraph_field
            .directives
            .push(Node::new(federation_spec_definition.requires_directive(
                &subgraph.schema,
                requires.clone(),
            )?));
    }
    if let Some(provides) = &field_directive_application.provides {
        subgraph_field
            .directives
            .push(Node::new(federation_spec_definition.provides_directive(
                &subgraph.schema,
                provides.clone(),
            )?));
    }
    let external = field_directive_application.external.unwrap_or(false);
    if external {
        subgraph_field.directives.push(Node::new(
            federation_spec_definition.external_directive(&subgraph.schema, None)?,
        ));
    }
    let user_overridden = field_directive_application.user_overridden.unwrap_or(false);
    if user_overridden {
        subgraph_field
            .directives
            .push(Node::new(federation_spec_definition.external_directive(
                &subgraph.schema,
                Some(NodeStr::new("[overridden]")),
            )?));
    }
    if let Some(override_) = &field_directive_application.override_ {
        subgraph_field
            .directives
            .push(Node::new(federation_spec_definition.override_directive(
                &subgraph.schema,
                override_.clone(),
            )?));
    }
    if is_shareable && !external && !user_overridden {
        subgraph_field
            .directives
            .push(Node::new(federation_spec_definition.shareable_directive(
                &subgraph.schema,
            )?));
    }

    match subgraph.schema.schema().types.get(type_name).ok_or_else(|| {
        SingleFederationError::Internal {
            message: format!(
                "Missing type \"{}\" from subgraph despite it being in type_info",
                type_name
            ),
        }
    })? {
        ExtendedType::Object(_) => {
            ObjectFieldDefinitionLocation {
                type_name: type_name.clone(),
                field_name: field_name.clone(),
            }.insert(
                &mut subgraph.schema,
                Component::from(subgraph_field),
            )?;
        }
        ExtendedType::Interface(_) => {
            InterfaceFieldDefinitionLocation {
                type_name: type_name.clone(),
                field_name: field_name.clone(),
            }.insert(
                &mut subgraph.schema,
                Component::from(subgraph_field),
            )?;
        }
        _ => {
            return Err(
                SingleFederationError::Internal {
                    message: "\"add_subgraph_field()\" encountered non-object/interface type in subgraph".to_owned(),
                }.into()
            );
        }
    };

    Ok(())
}

fn add_subgraph_input_field<'subgraph, 'schema>(
    input_field_name: &'schema Name,
    input_field: &'schema InputValueDefinition,
    type_name: &'schema NamedType,
    subgraph: &'subgraph mut FederationSubgraph,
    field_directive_application: Option<&FieldDirectiveArguments>,
) -> Result<(), FederationError> {
    let field_directive_application =
        field_directive_application.unwrap_or_else(|| &FieldDirectiveArguments {
            graph: None,
            requires: None,
            provides: None,
            type_: None,
            external: None,
            override_: None,
            user_overridden: None,
        });
    let subgraph_input_field_type = match &field_directive_application.type_ {
        Some(t) => Node::new(decode_type(t)?),
        None => input_field.ty.clone(),
    };
    let subgraph_input_field = InputValueDefinition {
        description: None,
        name: input_field_name.clone(),
        ty: subgraph_input_field_type,
        default_value: input_field.default_value.clone(),
        directives: Default::default(),
    };

    InputObjectFieldDefinitionLocation {
        type_name: type_name.clone(),
        field_name: input_field_name.clone(),
    }.insert(
        &mut subgraph.schema,
        Component::from(subgraph_input_field),
    )?;

    Ok(())
}

// TODO: Ask apollo-rs for type-reference parsing function, similar to graphql-js
fn decode_type(type_: &str) -> Result<Type, FederationError> {
    // Detect if type string is trying to end the field/type in the hack below.
    if type_.chars().any(|c| c == '}' || c == ':') {
        return Err(SingleFederationError::InvalidGraphQL {
            message: format!("Cannot parse type \"{}\"", type_),
        }.into());
    }
    let schema = Schema::parse(format!("type Query {{ field: {} }}", type_), "temp.graphql");
    let Some(ExtendedType::Object(dummy_type)) = schema.types.get("Query") else {
        return Err(SingleFederationError::InvalidGraphQL {
            message: format!("Cannot parse type \"{}\"", type_),
        }.into());
    };
    let Some(dummy_field) = dummy_type.fields.get("field") else {
        return Err(SingleFederationError::InvalidGraphQL {
            message: format!("Cannot parse type \"{}\"", type_),
        }.into());
    };
    Ok(dummy_field.ty.clone())
}

fn get_subgraph<'subgraph, 'schema>(
    subgraphs: &'subgraph mut FederationSubgraphs,
    graph_enum_value_name_to_subgraph_name: &IndexMap<&'schema Name, NodeStr>,
    graph_enum_value: &'schema NodeStr,
) -> Result<&'subgraph mut FederationSubgraph, FederationError> {
    let subgraph_name = graph_enum_value_name_to_subgraph_name
        .get(graph_enum_value)
        .ok_or_else(|| {
            SingleFederationError::Internal {
                message: format!(
                    "Invalid graph enum_value \"{}\": does not match an enum value defined in the @join__Graph enum",
                    graph_enum_value,
                ),
            }
        })?;
    subgraphs.get_mut(subgraph_name).ok_or_else(|| {
        SingleFederationError::Internal {
            message: "All subgraphs should have been created by \"collect_empty_subgraphs()\"".to_owned(),
        }.into()
    })
}

pub(crate) struct FederationSubgraph {
    pub(crate) name: String,
    pub(crate) url: String,
    pub(crate) schema: FederationSchema,
}

pub(crate) struct FederationSubgraphs {
    subgraphs: BTreeMap<String, FederationSubgraph>,
}

impl FederationSubgraphs {
    pub(crate) fn new() -> Self {
        FederationSubgraphs {
            subgraphs: BTreeMap::new(),
        }
    }

    pub(crate) fn add(&mut self, subgraph: FederationSubgraph) -> Result<(), FederationError> {
        if self.subgraphs.contains_key(&subgraph.name) {
            return Err(SingleFederationError::InvalidFederationSupergraph {
                message: format!("A subgraph named \"{}\" already exists", subgraph.name),
            }.into());
        }
        self.subgraphs.insert(subgraph.name.clone(), subgraph);
        Ok(())
    }

    pub(crate) fn get(&self, name: &str) -> Option<&FederationSubgraph> {
        self.subgraphs.get(name)
    }

    pub(crate) fn get_mut(&mut self, name: &str) -> Option<&mut FederationSubgraph> {
        self.subgraphs.get_mut(name)
    }
}

lazy_static! {
    static ref EXECUTABLE_DIRECTIVE_LOCATIONS: IndexSet<DirectiveLocation> = {
        let mut locations = IndexSet::new();
        locations.insert(DirectiveLocation::Query);
        locations.insert(DirectiveLocation::Mutation);
        locations.insert(DirectiveLocation::Subscription);
        locations.insert(DirectiveLocation::Field);
        locations.insert(DirectiveLocation::FragmentDefinition);
        locations.insert(DirectiveLocation::FragmentSpread);
        locations.insert(DirectiveLocation::InlineFragment);
        locations.insert(DirectiveLocation::VariableDefinition);
        locations
    };
}

fn remove_unused_types_from_subgraph(subgraph: &mut FederationSubgraph) -> Result<(), FederationError> {
    // We now do an additional path on all types because we sometimes added types to subgraphs
    // without being sure that the subgraph had the type in the first place (especially with the
    // join 0.1 spec), and because we later might not have added any fields/members to said type,
    // they may be empty (indicating they clearly didn't belong to the subgraph in the first) and we
    // need to remove them. Note that need to do this _after_ the `add_external_fields()` call above
    // since it may have added (external) fields to some of the types.
    let mut type_definition_locations: Vec<TypeDefinitionLocation> = Vec::new();
    for (type_name, type_) in subgraph.schema.schema().types.iter() {
        match type_ {
            ExtendedType::Object(type_) => {
                if type_.fields.is_empty() {
                    type_definition_locations.push(
                        ObjectTypeDefinitionLocation {
                            type_name: type_name.clone(),
                        }
                        .into(),
                    );
                }
            }
            ExtendedType::Interface(type_) => {
                if type_.fields.is_empty() {
                    type_definition_locations.push(
                        InterfaceTypeDefinitionLocation {
                            type_name: type_name.clone(),
                        }
                        .into(),
                    );
                }
            }
            ExtendedType::Union(type_) => {
                if type_.members.is_empty() {
                    type_definition_locations.push(
                        UnionTypeDefinitionLocation {
                            type_name: type_name.clone(),
                        }
                        .into(),
                    );
                }
            }
            ExtendedType::InputObject(type_) => {
                if type_.fields.is_empty() {
                    type_definition_locations.push(
                        InputObjectTypeDefinitionLocation {
                            type_name: type_name.clone(),
                        }
                        .into(),
                    );
                }
            }
            _ => {}
        }
    }

    // Note that we have to use remove_recursive() or this could leave the subgraph invalid. But if
    // the type was not in this subgraph, nothing that depends on it should be either.
    for location in type_definition_locations {
        match location {
            TypeDefinitionLocation::ObjectTypeDefinitionLocation(location) => {
                location.remove_recursive(&mut subgraph.schema)?;
            }
            TypeDefinitionLocation::InterfaceTypeDefinitionLocation(location) => {
                location.remove_recursive(&mut subgraph.schema)?;
            }
            TypeDefinitionLocation::UnionTypeDefinitionLocation(location) => {
                location.remove_recursive(&mut subgraph.schema)?;
            }
            TypeDefinitionLocation::InputObjectTypeDefinitionLocation(location) => {
                location.remove_recursive(&mut subgraph.schema)?;
            }
            _ => panic!("Encountered type kind that shouldn't have been removed"),
        }
    }

    Ok(())
}

const FEDERATION_ANY_TYPE_NAME: &str = "_Any";
const FEDERATION_SERVICE_TYPE_NAME: &str = "_Service";
const FEDERATION_SDL_FIELD_NAME: &str = "sdl";
const FEDERATION_ENTITY_TYPE_NAME: &str = "_Entity";
const FEDERATION_SERVICE_FIELD_NAME: &str = "_service";
const FEDERATION_ENTITIES_FIELD_NAME: &str = "_entities";
const FEDERATION_REPRESENTATIONS_ARGUMENTS_NAME: &str = "representations";

fn add_federation_operations(
    subgraph: &mut FederationSubgraph,
    federation_spec_definition: &'static FederationSpecDefinition,
) -> Result<(), FederationError> {
    // TODO: Use the JS/programmatic approach of checkOrAdd() instead of hard-coding the adds.
    let any_type_loc = ScalarTypeDefinitionLocation {
        type_name: NodeStr::new(FEDERATION_ANY_TYPE_NAME),
    };
    any_type_loc.pre_insert(&mut subgraph.schema)?;
    any_type_loc.insert(
        &mut subgraph.schema,
        Node::new(ScalarType {
            description: None,
            directives: Default::default(),
        })
    )?;
    let mut service_fields = IndexMap::new();
    service_fields.insert(
        NodeStr::new(FEDERATION_SDL_FIELD_NAME),
        Component::new(FieldDefinition {
            description: None,
            name: NodeStr::new(FEDERATION_SDL_FIELD_NAME),
            arguments: Vec::new(),
            ty: Type::Named(NodeStr::new("String")),
            directives: Default::default(),
        }),
    );
    let service_type_loc = ObjectTypeDefinitionLocation {
        type_name: NodeStr::new(FEDERATION_SERVICE_TYPE_NAME),
    };
    service_type_loc.pre_insert(&mut subgraph.schema)?;
    service_type_loc.insert(
        &mut subgraph.schema,
        Node::new(ObjectType {
            description: None,
            implements_interfaces: Default::default(),
            directives: Default::default(),
            fields: service_fields,
        })
    )?;
    let key_directive = federation_spec_definition
        .key_directive_definition(&subgraph.schema)?;
    let entity_members = subgraph
        .schema
        .schema()
        .types
        .iter()
        .filter_map(|(type_name, type_)| {
            let ExtendedType::Object(type_) = type_ else {
                return None;
            };
            if !type_
                .directives
                .iter()
                .any(|d| d.name == key_directive.name)
            {
                return None;
            }
            Some(ComponentStr::new(type_name))
        })
        .collect::<IndexSet<_>>();
    let is_entity_type = !entity_members.is_empty();
    if is_entity_type {
        let entity_type_loc = UnionTypeDefinitionLocation {
            type_name: NodeStr::new(FEDERATION_ENTITY_TYPE_NAME),
        };
        entity_type_loc.pre_insert(&mut subgraph.schema)?;
        entity_type_loc.insert(
            &mut subgraph.schema,
            Node::new(UnionType {
                description: None,
                directives: Default::default(),
                members: entity_members,
            })
        )?;
    }

    let query_root_loc = SchemaRootDefinitionLocation { root_kind: SchemaRootDefinitionKind::Query };
    if query_root_loc.try_get(&subgraph.schema.schema()).is_none() {
        let default_query_type_loc = ObjectTypeDefinitionLocation {
            type_name: NodeStr::new("Query")
        };
        default_query_type_loc.pre_insert(&mut subgraph.schema)?;
        default_query_type_loc.insert(
            &mut subgraph.schema,
            Node::new(ObjectType {
                description: None,
                implements_interfaces: Default::default(),
                directives: Default::default(),
                fields: Default::default(),
            })
        )?;
        query_root_loc.insert(
            &mut subgraph.schema,
            ComponentStr::new("Query"),
        )?;
    }

    let query_root_type_name = query_root_loc.get(&subgraph.schema.schema())?.node.clone();
    let entity_field_loc = ObjectFieldDefinitionLocation {
        type_name: query_root_type_name.clone(),
        field_name: NodeStr::new(FEDERATION_ENTITIES_FIELD_NAME),
    };
    if is_entity_type {
        entity_field_loc.insert(
            &mut subgraph.schema,
            Component::new(FieldDefinition {
                description: None,
                name: NodeStr::new(FEDERATION_ENTITIES_FIELD_NAME),
                arguments: vec![Node::new(InputValueDefinition {
                    description: None,
                    name: NodeStr::new(FEDERATION_REPRESENTATIONS_ARGUMENTS_NAME),
                    ty: Node::new(Type::NonNullList(Box::new(Type::NonNullNamed(
                        NodeStr::new(FEDERATION_ANY_TYPE_NAME),
                    )))),
                    default_value: None,
                    directives: Default::default(),
                })],
                ty: Type::NonNullList(Box::new(Type::Named(NodeStr::new(
                    FEDERATION_ENTITY_TYPE_NAME,
                )))),
                directives: Default::default(),
            })
        )?;
    } else {
        entity_field_loc.remove(&mut subgraph.schema)?;
    }

    ObjectFieldDefinitionLocation {
        type_name: query_root_type_name.clone(),
        field_name: NodeStr::new(FEDERATION_SERVICE_FIELD_NAME),
    }.insert(
        &mut subgraph.schema,
        Component::new(FieldDefinition {
            description: None,
            name: NodeStr::new(FEDERATION_SERVICE_FIELD_NAME),
            arguments: Vec::new(),
            ty: Type::NonNullNamed(NodeStr::new(FEDERATION_SERVICE_TYPE_NAME)),
            directives: Default::default(),
        })
    )?;

    Ok(())
}