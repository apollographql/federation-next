use crate::federation_schema::{FederationSchemaRef, OptionLinksMetadata};
use crate::spec::{Identity, Url, Version};
use crate::spec_definition::{SpecDefinition, SpecDefinitions};
use apollo_compiler::schema::{Directive, DirectiveDefinition, Value};
use apollo_compiler::{Node, NodeStr};
use apollo_compiler::ast::Argument;
use lazy_static::lazy_static;

pub const FEDERATION_KEY_DIRECTIVE_NAME_IN_SPEC: &str = "key";
pub const FEDERATION_INTERFACEOBJECT_DIRECTIVE_NAME_IN_SPEC: &str = "interfaceObject";
pub const FEDERATION_EXTERNAL_DIRECTIVE_NAME_IN_SPEC: &str = "external";
pub const FEDERATION_REQUIRES_DIRECTIVE_NAME_IN_SPEC: &str = "requires";
pub const FEDERATION_PROVIDES_DIRECTIVE_NAME_IN_SPEC: &str = "provides";
pub const FEDERATION_SHAREABLE_DIRECTIVE_NAME_IN_SPEC: &str = "shareable";
pub const FEDERATION_OVERRIDE_DIRECTIVE_NAME_IN_SPEC: &str = "override";

pub const FEDERATION_FIELDS_ARGUMENT_NAME: &str = "fields";
pub const FEDERATION_RESOLVABLE_ARGUMENT_NAME: &str = "resolvable";
pub const FEDERATION_REASON_ARGUMENT_NAME: &str = "reason";
pub const FEDERATION_FROM_ARGUMENT_NAME: &str = "from";

pub struct FederationSpecDefinition {
    url: Url,
}

impl FederationSpecDefinition {
    pub fn new(version: Version) -> Self {
        Self {
            url: Url {
                identity: Identity::join_identity(),
                version,
            },
        }
    }

    pub fn key_directive_definition<'a, 'schema, T: AsRef<OptionLinksMetadata>>(
        &'a self,
        schema: &'a FederationSchemaRef<'schema, T>,
    ) -> &'schema Node<DirectiveDefinition> {
        self.directive_definition(schema, FEDERATION_KEY_DIRECTIVE_NAME_IN_SPEC)
            .unwrap_or_else(|| {
                panic!(
                    "Unexpectedly could not find join spec's \"@{}\" directive definition",
                    FEDERATION_KEY_DIRECTIVE_NAME_IN_SPEC
                )
            })
    }

    pub fn key_directive<T: AsRef<OptionLinksMetadata>>(
        &self,
        schema: &FederationSchemaRef<T>,
        fields: NodeStr,
        resolvable: bool,
    ) -> Directive {
        let name_in_schema = self
            .directive_name_in_schema(schema, FEDERATION_KEY_DIRECTIVE_NAME_IN_SPEC)
            .unwrap_or_else(|| panic!("Unexpectedly could not find federation spec in schema"));
        Directive {
            name: NodeStr::new(&name_in_schema),
            arguments: vec![
                Node::new(Argument {
                    name: NodeStr::new(FEDERATION_FIELDS_ARGUMENT_NAME),
                    value: Node::new(Value::String(fields)),
                }),
                Node::new(Argument {
                    name: NodeStr::new(FEDERATION_RESOLVABLE_ARGUMENT_NAME),
                    value: Node::new(Value::Boolean(resolvable)),
                }),
            ],
        }
    }

    pub fn interface_object_directive<T: AsRef<OptionLinksMetadata>>(
        &self,
        schema: &FederationSchemaRef<T>,
    ) -> Directive {
        assert!(
            *self.version() >= Version { major: 2, minor: 3 },
            "Must be using federation >= v2.3 to use interface object",
        );
        let name_in_schema = self
            .directive_name_in_schema(schema, FEDERATION_INTERFACEOBJECT_DIRECTIVE_NAME_IN_SPEC)
            .unwrap_or_else(|| panic!("Unexpectedly could not find federation spec in schema"));
        Directive {
            name: NodeStr::new(&name_in_schema),
            arguments: Vec::new(),
        }
    }

    pub fn external_directive<T: AsRef<OptionLinksMetadata>>(
        &self,
        schema: &FederationSchemaRef<T>,
        reason: Option<NodeStr>,
    ) -> Directive {
        let name_in_schema = self
            .directive_name_in_schema(schema, FEDERATION_EXTERNAL_DIRECTIVE_NAME_IN_SPEC)
            .unwrap_or_else(|| panic!("Unexpectedly could not find federation spec in schema"));
        let mut arguments = Vec::new();
        if let Some(reason) = reason {
            arguments.push(
                Node::new(Argument {
                    name: NodeStr::new(FEDERATION_REASON_ARGUMENT_NAME),
                    value: Node::new(Value::String(reason)),
                }),
            )
        }
        Directive {
            name: NodeStr::new(&name_in_schema),
            arguments,
        }
    }

    pub fn requires_directive<T: AsRef<OptionLinksMetadata>>(
        &self,
        schema: &FederationSchemaRef<T>,
        fields: NodeStr,
    ) -> Directive {
        let name_in_schema = self
            .directive_name_in_schema(schema, FEDERATION_REQUIRES_DIRECTIVE_NAME_IN_SPEC)
            .unwrap_or_else(|| panic!("Unexpectedly could not find federation spec in schema"));
        Directive {
            name: NodeStr::new(&name_in_schema),
            arguments: vec![
                Node::new(Argument {
                    name: NodeStr::new(FEDERATION_FIELDS_ARGUMENT_NAME),
                    value: Node::new(Value::String(fields)),
                }),
            ],
        }
    }

    pub fn provides_directive<T: AsRef<OptionLinksMetadata>>(
        &self,
        schema: &FederationSchemaRef<T>,
        fields: NodeStr,
    ) -> Directive {
        let name_in_schema = self
            .directive_name_in_schema(schema, FEDERATION_PROVIDES_DIRECTIVE_NAME_IN_SPEC)
            .unwrap_or_else(|| panic!("Unexpectedly could not find federation spec in schema"));
        Directive {
            name: NodeStr::new(&name_in_schema),
            arguments: vec![
                Node::new(Argument {
                    name: NodeStr::new(FEDERATION_FIELDS_ARGUMENT_NAME),
                    value: Node::new(Value::String(fields)),
                }),
            ],
        }
    }

    pub fn shareable_directive<T: AsRef<OptionLinksMetadata>>(
        &self,
        schema: &FederationSchemaRef<T>,
    ) -> Directive {
        let name_in_schema = self
            .directive_name_in_schema(schema, FEDERATION_SHAREABLE_DIRECTIVE_NAME_IN_SPEC)
            .unwrap_or_else(|| panic!("Unexpectedly could not find federation spec in schema"));
        Directive {
            name: NodeStr::new(&name_in_schema),
            arguments: Vec::new(),
        }
    }

    pub fn override_directive<T: AsRef<OptionLinksMetadata>>(
        &self,
        schema: &FederationSchemaRef<T>,
        from: NodeStr,
    ) -> Directive {
        let name_in_schema = self
            .directive_name_in_schema(schema, FEDERATION_OVERRIDE_DIRECTIVE_NAME_IN_SPEC)
            .unwrap_or_else(|| panic!("Unexpectedly could not find federation spec in schema"));
        Directive {
            name: NodeStr::new(&name_in_schema),
            arguments: vec![
                Node::new(Argument {
                    name: NodeStr::new(FEDERATION_FROM_ARGUMENT_NAME),
                    value: Node::new(Value::String(from)),
                }),
            ],
        }
    }

}

impl SpecDefinition for FederationSpecDefinition {
    fn url(&self) -> &Url {
        &self.url
    }

    fn minimum_federation_version(&self) -> Option<&Version> {
        None
    }
}

lazy_static! {
    pub static ref FEDERATION_VERSIONS: SpecDefinitions<FederationSpecDefinition> = {
        let mut definitions = SpecDefinitions::new(Identity::federation_identity());
        definitions.add(FederationSpecDefinition::new(Version {
            major: 2,
            minor: 0,
        }));
        definitions.add(FederationSpecDefinition::new(Version {
            major: 2,
            minor: 1,
        }));
        definitions.add(FederationSpecDefinition::new(Version {
            major: 2,
            minor: 2,
        }));
        definitions.add(FederationSpecDefinition::new(Version {
            major: 2,
            minor: 3,
        }));
        definitions.add(FederationSpecDefinition::new(Version {
            major: 2,
            minor: 4,
        }));
        definitions.add(FederationSpecDefinition::new(Version {
            major: 2,
            minor: 5,
        }));
        definitions
    };
}
