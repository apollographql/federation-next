use crate::error::{FederationError, SingleFederationError};
use crate::link::spec::{Identity, Url, Version};
use crate::link::Link;
use crate::schema::FederationSchema;
use apollo_compiler::schema::{DirectiveDefinition, ExtendedType};
use apollo_compiler::{Node, NodeStr};
use std::collections::btree_map::Keys;
use std::collections::BTreeMap;
use std::sync::Arc;

pub(crate) trait SpecDefinition {
    fn url(&self) -> &Url;
    fn minimum_federation_version(&self) -> Option<&Version>;

    fn identity(&self) -> &Identity {
        &self.url().identity
    }

    fn version(&self) -> &Version {
        &self.url().version
    }

    fn is_spec_directive_name(
        &self,
        schema: &FederationSchema,
        name_in_schema: &str,
    ) -> Result<bool, FederationError> {
        let Some(ref metadata) = schema.metadata() else {
            return Err(SingleFederationError::Internal {
                message: "Schema is not a core schema (add @link first)".to_owned(),
            }
            .into());
        };
        Ok(metadata
            .source_link_of_directive(name_in_schema)
            .map(|e| e.link.url.identity == *self.identity())
            .unwrap_or(false))
    }

    fn is_spec_type_name(
        &self,
        schema: &FederationSchema,
        name_in_schema: &str,
    ) -> Result<bool, FederationError> {
        let Some(ref metadata) = schema.metadata() else {
            return Err(SingleFederationError::Internal {
                message: "Schema is not a core schema (add @link first)".to_owned(),
            }
            .into());
        };
        Ok(metadata
            .source_link_of_type(name_in_schema)
            .map(|e| e.link.url.identity == *self.identity())
            .unwrap_or(false))
    }

    fn directive_name_in_schema(
        &self,
        schema: &FederationSchema,
        name_in_spec: &str,
    ) -> Result<Option<String>, FederationError> {
        Ok(self
            .link_in_schema(schema)?
            .map(|link| link.directive_name_in_schema(name_in_spec)))
    }

    fn type_name_in_schema(
        &self,
        schema: &FederationSchema,
        name_in_spec: &str,
    ) -> Result<Option<String>, FederationError> {
        Ok(self
            .link_in_schema(schema)?
            .map(|link| link.type_name_in_schema(name_in_spec)))
    }

    fn directive_definition<'schema>(
        &self,
        schema: &'schema FederationSchema,
        name_in_spec: &str,
    ) -> Result<Option<&'schema Node<DirectiveDefinition>>, FederationError> {
        match self.directive_name_in_schema(schema, name_in_spec)? {
            Some(name) => schema
                .schema()
                .directive_definitions
                .get(&NodeStr::new(&name))
                .ok_or_else(|| {
                    SingleFederationError::Internal {
                        message: format!(
                            "Unexpectedly could not find spec directive \"@{}\" in schema",
                            name
                        ),
                    }
                    .into()
                })
                .map(Some),
            None => Ok(None),
        }
    }

    fn type_definition<'schema>(
        &self,
        schema: &'schema FederationSchema,
        name_in_spec: &str,
    ) -> Result<Option<&'schema ExtendedType>, FederationError> {
        match self.type_name_in_schema(schema, name_in_spec)? {
            Some(name) => schema
                .schema()
                .types
                .get(&NodeStr::new(&name))
                .ok_or_else(|| {
                    SingleFederationError::Internal {
                        message: format!(
                            "Unexpectedly could not find spec type \"{}\" in schema",
                            name
                        ),
                    }
                    .into()
                })
                .map(Some),
            None => Ok(None),
        }
    }

    fn link_in_schema(
        &self,
        schema: &FederationSchema,
    ) -> Result<Option<Arc<Link>>, FederationError> {
        let Some(ref metadata) = schema.metadata() else {
            return Err(SingleFederationError::Internal {
                message: "Schema is not a core schema (add @link first)".to_owned(),
            }
            .into());
        };
        Ok(metadata.for_identity(self.identity()))
    }

    fn to_string(&self) -> String {
        self.url().to_string()
    }
}

pub(crate) struct SpecDefinitions<T: SpecDefinition> {
    identity: Identity,
    definitions: BTreeMap<Version, T>,
}

impl<T: SpecDefinition> SpecDefinitions<T> {
    pub(crate) fn new(identity: Identity) -> Self {
        Self {
            identity,
            definitions: BTreeMap::new(),
        }
    }

    pub(crate) fn add(&mut self, definition: T) -> Result<(), FederationError> {
        if *definition.identity() != self.identity {
            return Err(SingleFederationError::Internal {
                message: format!(
                    "Cannot add definition for {} to the versions of definitions for {}",
                    definition.to_string(),
                    self.identity
                ),
            }
            .into());
        }
        if self.definitions.contains_key(definition.version()) {
            return Ok(());
        }
        self.definitions
            .insert(definition.version().clone(), definition);
        Ok(())
    }

    pub(crate) fn find(&self, requested: &Version) -> Option<&T> {
        self.definitions.get(requested)
    }

    pub(crate) fn versions(&self) -> Keys<Version, T> {
        self.definitions.keys()
    }
}

pub(crate) fn spec_definitions<T: SpecDefinition>(
    spec_definitions: &'static Result<SpecDefinitions<T>, FederationError>,
) -> Result<&'static SpecDefinitions<T>, FederationError> {
    match spec_definitions {
        Ok(spec_definitions) => Ok(spec_definitions),
        Err(error) => Err(error.clone()),
    }
}
