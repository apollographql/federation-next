//! Implements API schema generation.
use crate::error::FederationError;
use crate::link::inaccessible_spec_definition::remove_inaccessible_elements;
use crate::link::inaccessible_spec_definition::validate_inaccessible;
use crate::schema::position;
use crate::schema::FederationSchema;
use apollo_compiler::ast::Value;
use apollo_compiler::name;
use apollo_compiler::schema::Directive;
use apollo_compiler::schema::ExtendedType;
use apollo_compiler::schema::InputObjectType;
use apollo_compiler::schema::InputValueDefinition;
use apollo_compiler::schema::Name;
use apollo_compiler::validation::Valid;
use apollo_compiler::Node;
use apollo_compiler::Schema;
use std::collections::HashMap;

/// Remove types and directives imported by `@link`.
fn remove_core_feature_elements(schema: &mut FederationSchema) -> Result<(), FederationError> {
    let Some(metadata) = schema.metadata() else {
        return Ok(());
    };

    // Remove federation types and directives
    let types_for_removal = schema
        .get_types()
        .filter(|position| metadata.source_link_of_type(position.type_name()).is_some())
        .collect::<Vec<_>>();

    let directives_for_removal = schema
        .get_directive_definitions()
        .filter(|position| {
            metadata
                .source_link_of_directive(&position.directive_name)
                .is_some()
        })
        .collect::<Vec<_>>();

    // First remove children of elements that need to be removed, so there won't be outgoing
    // references from the type.
    for position in &types_for_removal {
        match position {
            position::TypeDefinitionPosition::Object(position) => {
                let object = position.get(schema.schema())?;
                let remove_children = object
                    .fields
                    .keys()
                    .map(|field_name| position.field(field_name.clone()))
                    .collect::<Vec<_>>();
                for child in remove_children {
                    child.remove(schema)?;
                }
            }
            position::TypeDefinitionPosition::Interface(position) => {
                let interface = position.get(schema.schema())?;
                let remove_children = interface
                    .fields
                    .keys()
                    .map(|field_name| position.field(field_name.clone()))
                    .collect::<Vec<_>>();
                for child in remove_children {
                    child.remove(schema)?;
                }
            }
            position::TypeDefinitionPosition::InputObject(position) => {
                let input_object = position.get(schema.schema())?;
                let remove_children = input_object
                    .fields
                    .keys()
                    .map(|field_name| position.field(field_name.clone()))
                    .collect::<Vec<_>>();
                for child in remove_children {
                    child.remove(schema)?;
                }
            }
            position::TypeDefinitionPosition::Enum(position) => {
                let enum_ = position.get(schema.schema())?;
                let remove_children = enum_
                    .values
                    .keys()
                    .map(|field_name| position.value(field_name.clone()))
                    .collect::<Vec<_>>();
                for child in remove_children {
                    child.remove(schema)?;
                }
            }
            _ => {}
        }
    }

    // TODO remove arguments first
    for position in &directives_for_removal {
        position.remove(schema)?;
    }

    for position in &types_for_removal {
        match position {
            position::TypeDefinitionPosition::Object(position) => {
                position.remove(schema)?;
            }
            position::TypeDefinitionPosition::Interface(position) => {
                position.remove(schema)?;
            }
            position::TypeDefinitionPosition::InputObject(position) => {
                position.remove(schema)?;
            }
            position::TypeDefinitionPosition::Enum(position) => {
                position.remove(schema)?;
            }
            position::TypeDefinitionPosition::Scalar(position) => {
                position.remove(schema)?;
            }
            position::TypeDefinitionPosition::Union(position) => {
                position.remove(schema)?;
            }
        }
    }

    Ok(())
}

/// Return true if a directive application is "semantic", meaning it's observable in introspection.
fn is_semantic_directive_application(directive: &Directive) -> bool {
    match directive.name.as_str() {
        "specifiedBy" => true,
        // For @deprecated, explicitly writing `url: null` disables the directive,
        // as `null` overrides the default string value.
        "deprecated"
            if directive
                .argument_by_name("url")
                .is_some_and(|value| value.is_null()) =>
        {
            false
        }
        "deprecated" => true,
        _ => false,
    }
}

/// Retain only semantic directives in a directive list from the high-level schema representation.
fn retain_semantic_directives(directives: &mut apollo_compiler::schema::DirectiveList) {
    directives
        .0
        .retain(|directive| is_semantic_directive_application(directive));
}

/// Retain only semantic directives in a directive list from the AST-level schema representation.
fn retain_semantic_directives_ast(directives: &mut apollo_compiler::ast::DirectiveList) {
    directives
        .0
        .retain(|directive| is_semantic_directive_application(directive));
}

/// Remove non-semantic directive applications from the schema representation.
/// This only keeps directive applications that are observable in introspection.
fn remove_non_semantic_directives(schema: &mut Schema) {
    let root_definitions = schema.schema_definition.make_mut();
    retain_semantic_directives(&mut root_definitions.directives);

    for ty in schema.types.values_mut() {
        match ty {
            ExtendedType::Object(object) => {
                let object = object.make_mut();
                retain_semantic_directives(&mut object.directives);
                for field in object.fields.values_mut() {
                    let field = field.make_mut();
                    retain_semantic_directives_ast(&mut field.directives);
                    for arg in &mut field.arguments {
                        let arg = arg.make_mut();
                        retain_semantic_directives_ast(&mut arg.directives);
                    }
                }
            }
            ExtendedType::Interface(interface) => {
                let interface = interface.make_mut();
                retain_semantic_directives(&mut interface.directives);
                for field in interface.fields.values_mut() {
                    let field = field.make_mut();
                    retain_semantic_directives_ast(&mut field.directives);
                    for arg in &mut field.arguments {
                        let arg = arg.make_mut();
                        retain_semantic_directives_ast(&mut arg.directives);
                    }
                }
            }
            ExtendedType::InputObject(input_object) => {
                let input_object = input_object.make_mut();
                retain_semantic_directives(&mut input_object.directives);
                for field in input_object.fields.values_mut() {
                    let field = field.make_mut();
                    retain_semantic_directives_ast(&mut field.directives);
                }
            }
            ExtendedType::Union(union_) => {
                let union_ = union_.make_mut();
                retain_semantic_directives(&mut union_.directives);
            }
            ExtendedType::Scalar(scalar) => {
                let scalar = scalar.make_mut();
                retain_semantic_directives(&mut scalar.directives);
            }
            ExtendedType::Enum(enum_) => {
                let enum_ = enum_.make_mut();
                retain_semantic_directives(&mut enum_.directives);
                for value in enum_.values.values_mut() {
                    let value = value.make_mut();
                    retain_semantic_directives_ast(&mut value.directives);
                }
            }
        }
    }

    for directive in schema.directive_definitions.values_mut() {
        let directive = directive.make_mut();
        for arg in &mut directive.arguments {
            let arg = arg.make_mut();
            retain_semantic_directives_ast(&mut arg.directives);
        }
    }
}

/// Recursively assign default values in input object values.
fn propagate_default_input_fields_in_value(
    input_objects: &HashMap<Name, Node<InputObjectType>>,
    target: &mut Value,
    definition: &InputObjectType,
) {
    match target {
        Value::Object(object) => {
            for (field_name, field_definition) in definition.fields.iter() {
                match object.iter_mut().find(|(key, _value)| key == field_name) {
                    Some((_name, value)) => {
                        let Some(input_object) =
                            input_objects.get(field_definition.ty.inner_named_type())
                        else {
                            // Not an input object type.
                            continue;
                        };

                        propagate_default_input_fields_in_value(
                            input_objects,
                            value.make_mut(),
                            input_object,
                        );
                    }
                    None => {
                        if let Some(default_value) = &field_definition.default_value {
                            let mut value = default_value.clone();
                            // If the default value is an input object we may need to fill in
                            // its defaulted fields recursively.
                            if let Some(input_object) =
                                input_objects.get(field_definition.ty.inner_named_type())
                            {
                                propagate_default_input_fields_in_value(
                                    input_objects,
                                    value.make_mut(),
                                    input_object,
                                );
                            }
                            object.push((field_name.clone(), value));
                        }
                    }
                }
            }
        }
        Value::List(list) => {
            for element in list {
                propagate_default_input_fields_in_value(
                    input_objects,
                    element.make_mut(),
                    definition,
                );
            }
        }
        _ => {}
    }
}

fn propagate_default_input_fields_in_arguments(
    input_objects: &HashMap<Name, Node<InputObjectType>>,
    arguments: &mut Vec<Node<InputValueDefinition>>,
) {
    for arg in arguments {
        let arg = arg.make_mut();
        let Some(default_value) = &mut arg.default_value else {
            continue;
        };

        let ty = arg.ty.inner_named_type();
        let Some(input_object) = input_objects.get(ty) else {
            continue;
        };

        propagate_default_input_fields_in_value(
            &input_objects,
            default_value.make_mut(),
            input_object,
        );
    }
}

/// For all object values written in the SDL, where the input object definition has default values,
/// add those default values to the object.
///
/// This does not affect the functionality of the schema, but it matches a behaviour in graphql-js
/// so we can compare API schema results between federation-next and JS federation. We can consider
/// removing this when we no longer rely on JS federation.
fn propagate_default_input_fields(schema: &mut Schema) {
    // Keep a copy of the input objects so we can mutate the schema while walking it.
    let input_objects = schema
        .types
        .iter()
        .filter_map(|(name, ty)| {
            if let ExtendedType::InputObject(input_object) = ty {
                Some((name.clone(), input_object.clone()))
            } else {
                None
            }
        })
        .collect::<HashMap<_, _>>();

    for ty in schema.types.values_mut() {
        match ty {
            ExtendedType::Object(object) => {
                let object = object.make_mut();
                for field in object.fields.values_mut() {
                    let field = field.make_mut();
                    propagate_default_input_fields_in_arguments(
                        &input_objects,
                        &mut field.arguments,
                    );
                }
            }
            ExtendedType::Interface(interface) => {
                let interface = interface.make_mut();
                for field in interface.fields.values_mut() {
                    let field = field.make_mut();
                    propagate_default_input_fields_in_arguments(
                        &input_objects,
                        &mut field.arguments,
                    );
                }
            }
            ExtendedType::InputObject(input_object) => {
                let input_object = input_object.make_mut();
                for field in input_object.fields.values_mut() {
                    let field = field.make_mut();
                    let Some(default_value) = &mut field.default_value else {
                        continue;
                    };

                    let ty = field.ty.inner_named_type();
                    let Some(input_object) = input_objects.get(ty) else {
                        continue;
                    };

                    propagate_default_input_fields_in_value(
                        &input_objects,
                        default_value.make_mut(),
                        input_object,
                    );
                }
            }
            ExtendedType::Union(_) | ExtendedType::Scalar(_) | ExtendedType::Enum(_) => {
                // Nothing to do
            }
        }
    }

    for directive in schema.directive_definitions.values_mut() {
        let directive = directive.make_mut();
        propagate_default_input_fields_in_arguments(&input_objects, &mut directive.arguments);
    }
}

pub fn to_api_schema(schema: FederationSchema) -> Result<Valid<Schema>, FederationError> {
    let mut api_schema = schema;

    // As we compute the API schema of a supergraph, we want to ignore explicit definitions of `@defer` and `@stream` because
    // those correspond to the merging of potential definitions from the subgraphs, but whether the supergraph API schema
    // supports defer or not is unrelated to whether subgraphs support it.
    if let Some(defer) = api_schema.get_directive_definition(&name!("defer")) {
        defer.remove(&mut api_schema)?;
    }
    if let Some(stream) = api_schema.get_directive_definition(&name!("stream")) {
        stream.remove(&mut api_schema)?;
    }

    validate_inaccessible(&api_schema)?;
    remove_inaccessible_elements(&mut api_schema)?;

    remove_core_feature_elements(&mut api_schema)?;

    let mut api_schema = api_schema.into_inner();
    remove_non_semantic_directives(&mut api_schema);
    // To match the graphql-js output, we propagate default values declared on fields in
    // input object definitions to their usage sites.
    propagate_default_input_fields(&mut api_schema);

    Ok(api_schema.validate()?)
}