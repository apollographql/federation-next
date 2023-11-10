use crate::error::{FederationError, SingleFederationError};
use apollo_compiler::ast::Value;
use apollo_compiler::schema::{Directive, Name};
use apollo_compiler::{Node, NodeStr};
use std::ops::Deref;

pub(crate) fn directive_optional_enum_argument(
    application: &Node<Directive>,
    name: &Name,
) -> Result<Option<Name>, FederationError> {
    match application.arguments.iter().find(|a| a.name == *name) {
        Some(a) => match a.value.deref() {
            Value::Enum(name) => Ok(Some(name.clone())),
            Value::Null => Ok(None),
            _ => Err(SingleFederationError::Internal {
                message: format!(
                    "Argument \"{}\" of directive \"@{}\" must be an enum value.",
                    name, application.name
                ),
            }
            .into()),
        },
        None => Ok(None),
    }
}

pub(crate) fn directive_required_enum_argument(
    application: &Node<Directive>,
    name: &Name,
) -> Result<Name, FederationError> {
    directive_optional_enum_argument(application, name)?.ok_or_else(|| {
        SingleFederationError::Internal {
            message: format!(
                "Required argument \"{}\" of directive \"@{}\" was not present.",
                name, application.name
            ),
        }
        .into()
    })
}

pub(crate) fn directive_optional_string_argument(
    application: &Node<Directive>,
    name: &Name,
) -> Result<Option<NodeStr>, FederationError> {
    match application.arguments.iter().find(|a| a.name == *name) {
        Some(a) => match a.value.deref() {
            Value::String(name) => Ok(Some(name.clone())),
            Value::Null => Ok(None),
            _ => Err(SingleFederationError::Internal {
                message: format!(
                    "Argument \"{}\" of directive \"@{}\" must be a string.",
                    name, application.name
                ),
            }
            .into()),
        },
        None => Ok(None),
    }
}

pub(crate) fn directive_required_string_argument(
    application: &Node<Directive>,
    name: &Name,
) -> Result<NodeStr, FederationError> {
    directive_optional_string_argument(application, name)?.ok_or_else(|| {
        SingleFederationError::Internal {
            message: format!(
                "Required argument \"{}\" of directive \"@{}\" was not present.",
                name, application.name
            ),
        }
        .into()
    })
}

pub(crate) fn directive_optional_fieldset_argument(
    application: &Node<Directive>,
    name: &Name,
) -> Result<Option<NodeStr>, FederationError> {
    let argument = application.arguments.iter().find(|a| a.name == *name);
    match argument {
        Some(argument) => match argument.value.deref() {
            Value::String(name) => Ok(Some(name.clone())),
            Value::Null => Ok(None),
            _ => Err(SingleFederationError::InvalidGraphQL {
                message: format!("Invalid value for argument \"{}\": must be a string.", name),
            }
            .into()),
        },
        None => Ok(None),
    }
}

#[allow(dead_code)]
pub(crate) fn directive_required_fieldset_argument(
    application: &Node<Directive>,
    name: &Name,
) -> Result<NodeStr, FederationError> {
    directive_optional_fieldset_argument(application, name)?.ok_or_else(|| {
        SingleFederationError::Internal {
            message: format!(
                "Required argument \"{}\" of directive \"@{}\" was not present.",
                name, application.name
            ),
        }
        .into()
    })
}

pub(crate) fn directive_optional_boolean_argument(
    application: &Node<Directive>,
    name: &Name,
) -> Result<Option<bool>, FederationError> {
    match application.arguments.iter().find(|a| a.name == *name) {
        Some(a) => match a.value.deref() {
            Value::Boolean(value) => Ok(Some(*value)),
            Value::Null => Ok(None),
            _ => Err(SingleFederationError::Internal {
                message: format!(
                    "Argument \"{}\" of directive \"@{}\" must be a boolean.",
                    name, application.name
                ),
            }
            .into()),
        },
        None => Ok(None),
    }
}

#[allow(dead_code)]
pub(crate) fn directive_required_boolean_argument(
    application: &Node<Directive>,
    name: &Name,
) -> Result<bool, FederationError> {
    directive_optional_boolean_argument(application, name)?.ok_or_else(|| {
        SingleFederationError::Internal {
            message: format!(
                "Required argument \"{}\" of directive \"@{}\" was not present.",
                name, application.name
            ),
        }
        .into()
    })
}
