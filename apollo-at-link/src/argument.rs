use apollo_compiler::ast::Value;
use apollo_compiler::schema::{Directive, Name};
use apollo_compiler::{Node, NodeStr};
use apollo_federation_error::error::{FederationDirectiveErrorCategory, FederationError};
use std::ops::Deref;

pub fn directive_optional_enum_argument<'directive, 'a>(
    application: &'directive Node<Directive>,
    name: &'a str,
) -> Option<&'directive Name> {
    application
        .arguments
        .iter()
        .find(|a| *a.name == *name)
        .and_then(|a| match a.value.deref() {
            Value::Enum(name) => Some(name),
            Value::Null => None,
            _ => {
                panic!(
                    "Argument \"{}\" of directive \"@{}\" must be an enum value.",
                    name, application.name
                )
            }
        })
}

pub fn directive_required_enum_argument<'directive, 'a>(
    application: &'directive Node<Directive>,
    name: &'a str,
) -> &'directive Name {
    directive_optional_enum_argument(application, name).unwrap_or_else(|| {
        panic!(
            "Required argument \"{}\" of directive \"@{}\" was not present.",
            name, application.name
        )
    })
}

pub fn directive_optional_string_argument<'directive, 'a>(
    application: &'directive Node<Directive>,
    name: &'a str,
) -> Option<&'directive NodeStr> {
    application
        .arguments
        .iter()
        .find(|a| *a.name == *name)
        .and_then(|a| match a.value.deref() {
            Value::String(name) => Some(name),
            Value::Null => None,
            _ => {
                panic!(
                    "Argument \"{}\" of directive \"@{}\" must be a string.",
                    name, application.name
                )
            }
        })
}

pub fn directive_required_string_argument<'directive, 'a>(
    application: &'directive Node<Directive>,
    name: &'a str,
) -> &'directive NodeStr {
    directive_optional_string_argument(application, name).unwrap_or_else(|| {
        panic!(
            "Required argument \"{}\" of directive \"@{}\" was not present.",
            name, application.name
        )
    })
}

pub fn directive_optional_fieldset_argument<'directive, 'a>(
    application: &'directive Node<Directive>,
    name: &'a str,
) -> Result<Option<&'directive NodeStr>, FederationError> {
    let argument = application.arguments.iter().find(|a| *a.name == *name);
    match argument {
        Some(argument) => match argument.value.deref() {
            Value::String(name) => Ok(Some(name)),
            Value::Null => Ok(None),
            _ => Err(FederationDirectiveErrorCategory::DirectiveInvalidFields
                .definition()
                .get(application.name.as_str().to_owned())
                .err(
                    format!("Invalid value for argument \"{}\": must be a string.", name),
                    Some(vec![application.clone().into()]),
                )
                .into()),
        },
        None => Ok(None),
    }
}

pub fn directive_required_fieldset_argument<'directive, 'a>(
    application: &'directive Node<Directive>,
    name: &'a str,
) -> Result<&'directive NodeStr, FederationError> {
    Ok(
        directive_optional_fieldset_argument(application, name)?
            .unwrap_or_else(|| {
                panic!(
                    "Required argument \"{}\" of directive \"@{}\" was not present.",
                    name, application.name
                )
            }),
    )
}

pub fn directive_optional_boolean_argument(
    application: &Node<Directive>,
    name: &str,
) -> Option<bool> {
    application
        .arguments
        .iter()
        .find(|a| *a.name == *name)
        .and_then(|a| match a.value.deref() {
            Value::Boolean(value) => Some(value.clone()),
            Value::Null => None,
            _ => {
                panic!(
                    "Argument \"{}\" of directive \"@{}\" must be a boolean.",
                    name, application.name
                )
            }
        })
}

pub fn directive_required_boolean_argument(
    application: &Node<Directive>,
    name: &str,
) -> bool {
    directive_optional_boolean_argument(application, name).unwrap_or_else(|| {
        panic!(
            "Required argument \"{}\" of directive \"@{}\" was not present.",
            name, application.name
        )
    })
}
