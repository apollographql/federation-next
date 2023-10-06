use crate::federation_schema::{FederationSchemaRef, OptionLinksMetadata};
use crate::link::Link;
use crate::spec::{Identity, Url, Version};
use apollo_compiler::schema::{DirectiveDefinition, ExtendedType};
use apollo_compiler::{Node, NodeStr};
use std::collections::btree_map::Keys;
use std::collections::BTreeMap;
use std::sync::Arc;

pub trait SpecDefinition {
    fn url(&self) -> &Url;
    fn minimum_federation_version(&self) -> Option<&Version>;

    fn identity(&self) -> &Identity {
        &self.url().identity
    }

    fn version(&self) -> &Version {
        &self.url().version
    }

    fn is_spec_directive_name<T: AsRef<OptionLinksMetadata>>(
        &self,
        schema: &FederationSchemaRef<T>,
        name_in_schema: &str,
    ) -> bool {
        let Some(ref metadata) = schema.metadata() else {
            panic!("Schema is not a core schema (add @link first)");
        };
        metadata
            .source_link_of_directive(name_in_schema)
            .map(|e| e.link.url.identity == *self.identity())
            .unwrap_or(false)
    }

    fn is_spec_type_name<T: AsRef<OptionLinksMetadata>>(
        &self,
        schema: &FederationSchemaRef<T>,
        name_in_schema: &str,
    ) -> bool {
        let Some(ref metadata) = schema.metadata() else {
            panic!("Schema is not a core schema (add @link first)");
        };
        metadata
            .source_link_of_type(name_in_schema)
            .map(|e| e.link.url.identity == *self.identity())
            .unwrap_or(false)
    }

    fn directive_name_in_schema<T: AsRef<OptionLinksMetadata>>(
        &self,
        schema: &FederationSchemaRef<T>,
        name_in_spec: &str,
    ) -> Option<String> {
        self.link_in_schema(schema)
            .map(|link| link.directive_name_in_schema(name_in_spec))
    }

    fn type_name_in_schema<T: AsRef<OptionLinksMetadata>>(
        &self,
        schema: &FederationSchemaRef<T>,
        name_in_spec: &str,
    ) -> Option<String> {
        self.link_in_schema(schema)
            .map(|link| link.type_name_in_schema(name_in_spec))
    }

    fn directive_definition<'a, 'schema, T: AsRef<OptionLinksMetadata>>(
        &'a self,
        schema: &'a FederationSchemaRef<'schema, T>,
        name_in_spec: &'a str,
    ) -> Option<&'schema Node<DirectiveDefinition>> {
        self.directive_name_in_schema(schema, name_in_spec)
            .map(|name| {
                schema
                    .schema
                    .directive_definitions
                    .get(&NodeStr::new(&name))
                    .unwrap_or_else(|| {
                        panic!(
                            "Unexpectedly could not find spec directive \"@{}\" in schema",
                            name
                        )
                    })
            })
    }

    fn type_definition<'a, 'schema, T: AsRef<OptionLinksMetadata>>(
        &'a self,
        schema: &'a FederationSchemaRef<'schema, T>,
        name_in_spec: &'a str,
    ) -> Option<&'schema ExtendedType> {
        self.type_name_in_schema(schema, name_in_spec).map(|name| {
            schema
                .schema
                .types
                .get(&NodeStr::new(&name))
                .unwrap_or_else(|| {
                    panic!("Unexpectedly could not find spec type \"{}\" in schema", name)
                })
        })
    }

    fn link_in_schema<T: AsRef<OptionLinksMetadata>>(
        &self,
        schema: &FederationSchemaRef<T>,
    ) -> Option<Arc<Link>> {
        let Some(ref metadata) = schema.metadata() else {
            panic!("Schema is not a core schema (add @link first)");
        };
        metadata.for_identity(self.identity())
    }

    fn to_string(&self) -> String {
        self.url().to_string()
    }
}

pub struct SpecDefinitions<T: SpecDefinition> {
    identity: Identity,
    definitions: BTreeMap<Version, T>,
}

impl<T: SpecDefinition> SpecDefinitions<T> {
    pub fn new(identity: Identity) -> Self {
        Self {
            identity,
            definitions: BTreeMap::new(),
        }
    }

    pub fn add(&mut self, definition: T) {
        assert_eq!(
            *definition.identity(),
            self.identity,
            "Cannot add definition for {} to the versions of definitions for {}",
            definition.to_string(),
            self.identity
        );
        if self.definitions.contains_key(definition.version()) {
            return;
        }
        self.definitions
            .insert(definition.version().clone(), definition);
    }

    pub fn find(&self, requested: &Version) -> Option<&T> {
        self.definitions.get(requested)
    }

    pub fn versions(&self) -> Keys<Version, T> {
        self.definitions.keys()
    }
}
