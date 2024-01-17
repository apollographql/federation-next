//! Implements API schema generation.
use crate::error::FederationError;
use crate::link::inaccessible_spec_definition::remove_inaccessible_elements;
use crate::link::inaccessible_spec_definition::validate_inaccessible;
use crate::schema::position;
use crate::schema::FederationSchema;
use apollo_compiler::name;
use apollo_compiler::validation::Valid;
use apollo_compiler::Schema;

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
    crate::compat::make_print_schema_compatible(&mut api_schema);

    Ok(api_schema.validate()?)
}
