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
use apollo_compiler::schema::Type;
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
        // For @deprecated, explicitly writing `reason: null` disables the directive,
        // as `null` overrides the default string value.
        "deprecated"
            if directive
                .argument_by_name("reason")
                .is_some_and(|value| value.is_null()) =>
        {
            false
        }
        "deprecated" => true,
        _ => false,
    }
}

/// Remove `reason` argument from a `@deprecated` directive if it has the default value, just to match graphql-js output.
fn standardize_deprecated(directive: &mut Directive) {
    if directive.name == "deprecated"
        && directive
            .argument_by_name("reason")
            .and_then(|value| value.as_str())
            .is_some_and(|reason| reason == "No longer supported")
    {
        directive.arguments.clear();
    }
}

/// Retain only semantic directives in a directive list from the high-level schema representation.
fn retain_semantic_directives(directives: &mut apollo_compiler::schema::DirectiveList) {
    directives
        .0
        .retain(|directive| is_semantic_directive_application(directive));

    for directive in directives {
        standardize_deprecated(directive.make_mut());
    }
}

/// Retain only semantic directives in a directive list from the AST-level schema representation.
fn retain_semantic_directives_ast(directives: &mut apollo_compiler::ast::DirectiveList) {
    directives
        .0
        .retain(|directive| is_semantic_directive_application(directive));

    for directive in directives {
        standardize_deprecated(directive.make_mut());
    }
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
fn coerce_value(
    input_objects: &HashMap<Name, Node<InputObjectType>>,
    target: &mut Node<Value>,
    ty: &Type,
) {
    match target.make_mut() {
        Value::Object(object) if ty.is_named() => {
            let Some(definition) = input_objects.get(ty.inner_named_type()) else {
                unreachable!("Found an object value where a {ty} was expected, this should have been caught by validation");
            };
            for (field_name, field_definition) in definition.fields.iter() {
                match object.iter_mut().find(|(key, _value)| key == field_name) {
                    Some((_name, value)) => {
                        coerce_value(input_objects, value, &field_definition.ty);
                    }
                    None => {
                        if let Some(default_value) = &field_definition.default_value {
                            let mut value = default_value.clone();
                            // If the default value is an input object we may need to fill in
                            // its defaulted fields recursively.
                            coerce_value(input_objects, &mut value, &field_definition.ty);
                            object.push((field_name.clone(), value));
                        }
                    }
                }
            }
        }
        Value::List(list) => {
            for element in list {
                coerce_value(input_objects, element, ty.item_type());
            }
        }
        // Coerce single values (except null) to a list.
        Value::Object(_)
        | Value::String(_)
        | Value::Enum(_)
        | Value::Int(_)
        | Value::Float(_)
        | Value::Boolean(_)
            if ty.is_list() =>
        {
            coerce_value(input_objects, target, ty.item_type());
            *target.make_mut() = Value::List(vec![target.clone()]);
        }
        // Other types are either totally invalid (and rejected by validation), or do not need
        // coercion
        _ => {}
    }
}

fn corce_arguments_default_values(
    input_objects: &HashMap<Name, Node<InputObjectType>>,
    arguments: &mut Vec<Node<InputValueDefinition>>,
) {
    for arg in arguments {
        let arg = arg.make_mut();
        let Some(default_value) = &mut arg.default_value else {
            continue;
        };

        coerce_value(&input_objects, default_value, &arg.ty);
    }
}

/// For all object values written in the SDL, where the input object definition has default values,
/// add those default values to the object.
///
/// This does not affect the functionality of the schema, but it matches a behaviour in graphql-js
/// so we can compare API schema results between federation-next and JS federation. We can consider
/// removing this when we no longer rely on JS federation.
fn coerce_schema_default_values(schema: &mut Schema) {
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
                    corce_arguments_default_values(&input_objects, &mut field.arguments);
                }
            }
            ExtendedType::Interface(interface) => {
                let interface = interface.make_mut();
                for field in interface.fields.values_mut() {
                    let field = field.make_mut();
                    corce_arguments_default_values(&input_objects, &mut field.arguments);
                }
            }
            ExtendedType::InputObject(input_object) => {
                let input_object = input_object.make_mut();
                for field in input_object.fields.values_mut() {
                    let field = field.make_mut();
                    let Some(default_value) = &mut field.default_value else {
                        continue;
                    };

                    coerce_value(&input_objects, default_value, &field.ty);
                }
            }
            ExtendedType::Union(_) | ExtendedType::Scalar(_) | ExtendedType::Enum(_) => {
                // Nothing to do
            }
        }
    }

    for directive in schema.directive_definitions.values_mut() {
        let directive = directive.make_mut();
        corce_arguments_default_values(&input_objects, &mut directive.arguments);
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
    coerce_schema_default_values(&mut api_schema);

    Ok(api_schema.validate()?)
}
