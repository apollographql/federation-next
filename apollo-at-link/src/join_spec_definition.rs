use crate::argument::{
    directive_optional_boolean_argument, directive_optional_enum_argument,
    directive_optional_fieldset_argument, directive_optional_string_argument,
    directive_required_enum_argument, directive_required_string_argument,
};
use crate::federation_schema::{FederationSchemaRef, OptionLinksMetadata};
use crate::spec::{Identity, Url, Version};
use crate::spec_definition::{SpecDefinition, SpecDefinitions};
use apollo_compiler::schema::{Directive, DirectiveDefinition, EnumType, ExtendedType};
use apollo_compiler::{Node, NodeStr};
use apollo_federation_error::error::FederationError;
use lazy_static::lazy_static;

pub const JOIN_GRAPH_ENUM_NAME_IN_SPEC: &str = "Graph";
pub const JOIN_GRAPH_DIRECTIVE_NAME_IN_SPEC: &str = "graph";
pub const JOIN_TYPE_DIRECTIVE_NAME_IN_SPEC: &str = "type";
pub const JOIN_FIELD_DIRECTIVE_NAME_IN_SPEC: &str = "field";
pub const JOIN_IMPLEMENTS_DIRECTIVE_NAME_IN_SPEC: &str = "implements";
pub const JOIN_UNIONMEMBER_DIRECTIVE_NAME_IN_SPEC: &str = "unionMember";
pub const JOIN_ENUMVALUE_DIRECTIVE_NAME_IN_SPEC: &str = "enumValue";

pub const JOIN_NAME_ARGUMENT_NAME: &str = "name";
pub const JOIN_URL_ARGUMENT_NAME: &str = "url";
pub const JOIN_GRAPH_ARGUMENT_NAME: &str = "graph";
pub const JOIN_KEY_ARGUMENT_NAME: &str = "key";
pub const JOIN_EXTENSION_ARGUMENT_NAME: &str = "extension";
pub const JOIN_RESOLVABLE_ARGUMENT_NAME: &str = "resolvable";
pub const JOIN_ISINTERFACEOBJECT_ARGUMENT_NAME: &str = "isInterfaceObject";
pub const JOIN_REQUIRES_ARGUMENT_NAME: &str = "requires";
pub const JOIN_PROVIDES_ARGUMENT_NAME: &str = "provides";
pub const JOIN_TYPE_ARGUMENT_NAME: &str = "type";
pub const JOIN_EXTERNAL_ARGUMENT_NAME: &str = "external";
pub const JOIN_OVERRIDE_ARGUMENT_NAME: &str = "override";
pub const JOIN_USEROVERRIDDEN_ARGUMENT_NAME: &str = "usedOverridden";
pub const JOIN_INTERFACE_ARGUMENT_NAME: &str = "interface";
pub const JOIN_MEMBER_ARGUMENT_NAME: &str = "interface";

pub struct GraphDirectiveArguments<'a> {
    pub name: &'a NodeStr,
    pub url: &'a NodeStr,
}

pub struct TypeDirectiveArguments<'a> {
    pub graph: &'a NodeStr,
    pub key: Option<&'a NodeStr>,
    pub extension: bool,
    pub resolvable: bool,
    pub is_interface_object: bool,
}

pub struct FieldDirectiveArguments<'a> {
    pub graph: Option<&'a NodeStr>,
    pub requires: Option<&'a NodeStr>,
    pub provides: Option<&'a NodeStr>,
    pub type_: Option<&'a NodeStr>,
    pub external: Option<bool>,
    pub override_: Option<&'a NodeStr>,
    pub user_overridden: Option<bool>,
}

pub struct ImplementsDirectiveArguments<'a> {
    pub graph: &'a NodeStr,
    pub interface: &'a NodeStr,
}

pub struct UnionMemberDirectiveArguments<'a> {
    pub graph: &'a NodeStr,
    pub member: &'a NodeStr,
}

pub struct EnumValueDirectiveArguments<'a> {
    pub graph: &'a NodeStr,
}

pub struct JoinSpecDefinition {
    url: Url,
    minimum_federation_version: Option<Version>,
}

impl JoinSpecDefinition {
    pub fn new(version: Version, minimum_federation_version: Option<Version>) -> Self {
        Self {
            url: Url {
                identity: Identity::join_identity(),
                version,
            },
            minimum_federation_version,
        }
    }

    pub fn graph_enum_definition<'a, 'schema, T: AsRef<OptionLinksMetadata>>(
        &'a self,
        schema: &'a FederationSchemaRef<'schema, T>,
    ) -> &'schema Node<EnumType> {
        let type_ = self
            .type_definition(schema, JOIN_GRAPH_ENUM_NAME_IN_SPEC)
            .unwrap_or_else(|| panic!("Unexpectedly could not find join spec in schema"));
        if let ExtendedType::Enum(ref type_) = type_ {
            type_
        } else {
            panic!(
                "Unexpectedly found non-enum for join spec's \"{}\" enum definition",
                JOIN_GRAPH_ENUM_NAME_IN_SPEC
            )
        }
    }

    pub fn graph_directive_definition<'a, 'schema, T: AsRef<OptionLinksMetadata>>(
        &'a self,
        schema: &'a FederationSchemaRef<'schema, T>,
    ) -> &'schema Node<DirectiveDefinition> {
        self.directive_definition(schema, JOIN_GRAPH_DIRECTIVE_NAME_IN_SPEC)
            .unwrap_or_else(|| panic!("Unexpectedly could not find join spec in schema"))
    }

    pub fn graph_directive_arguments<'a, 'schema>(
        &'a self,
        application: &'schema Node<Directive>,
    ) -> GraphDirectiveArguments<'schema> {
        GraphDirectiveArguments {
            name: directive_required_string_argument(application, JOIN_NAME_ARGUMENT_NAME),
            url: directive_required_string_argument(application, JOIN_URL_ARGUMENT_NAME),
        }
    }

    pub fn type_directive_definition<'a, 'schema, T: AsRef<OptionLinksMetadata>>(
        &'a self,
        schema: &'a FederationSchemaRef<'schema, T>,
    ) -> &'schema Node<DirectiveDefinition> {
        self.directive_definition(schema, JOIN_TYPE_DIRECTIVE_NAME_IN_SPEC)
            .unwrap_or_else(|| {
                panic!(
                    "Unexpectedly could not find join spec's \"@{}\" directive definition",
                    JOIN_TYPE_DIRECTIVE_NAME_IN_SPEC
                )
            })
    }

    pub fn type_directive_arguments<'a, 'schema>(
        &'a self,
        application: &'schema Node<Directive>,
    ) -> Result<TypeDirectiveArguments<'schema>, FederationError> {
        Ok(TypeDirectiveArguments {
            graph: directive_required_enum_argument(application, JOIN_GRAPH_ARGUMENT_NAME),
            key: directive_optional_fieldset_argument(application, JOIN_KEY_ARGUMENT_NAME)?,
            extension: directive_optional_boolean_argument(
                application,
                JOIN_EXTENSION_ARGUMENT_NAME,
            )
            .unwrap_or(false),
            resolvable: directive_optional_boolean_argument(
                application,
                JOIN_RESOLVABLE_ARGUMENT_NAME,
            )
            .unwrap_or(true),
            is_interface_object: directive_optional_boolean_argument(
                application,
                JOIN_ISINTERFACEOBJECT_ARGUMENT_NAME,
            )
            .unwrap_or(false),
        })
    }

    pub fn field_directive_definition<'a, 'schema, T: AsRef<OptionLinksMetadata>>(
        &'a self,
        schema: &'a FederationSchemaRef<'schema, T>,
    ) -> &'schema Node<DirectiveDefinition> {
        self.directive_definition(schema, JOIN_FIELD_DIRECTIVE_NAME_IN_SPEC)
            .unwrap_or_else(|| {
                panic!(
                    "Unexpectedly could not find join spec's \"@{}\" directive definition",
                    JOIN_FIELD_DIRECTIVE_NAME_IN_SPEC
                )
            })
    }

    pub fn field_directive_arguments<'a, 'schema>(
        &'a self,
        application: &'schema Node<Directive>,
    ) -> Result<FieldDirectiveArguments<'schema>, FederationError> {
        Ok(FieldDirectiveArguments {
            graph: directive_optional_enum_argument(application, JOIN_GRAPH_ARGUMENT_NAME),
            requires: directive_optional_fieldset_argument(
                application,
                JOIN_REQUIRES_ARGUMENT_NAME,
            )?,
            provides: directive_optional_fieldset_argument(
                application,
                JOIN_PROVIDES_ARGUMENT_NAME,
            )?,
            type_: directive_optional_string_argument(application, JOIN_TYPE_ARGUMENT_NAME),
            external: directive_optional_boolean_argument(application, JOIN_EXTERNAL_ARGUMENT_NAME),
            override_: directive_optional_string_argument(application, JOIN_OVERRIDE_ARGUMENT_NAME),
            user_overridden: directive_optional_boolean_argument(
                application,
                JOIN_USEROVERRIDDEN_ARGUMENT_NAME,
            ),
        })
    }

    pub fn implements_directive_definition<'a, 'schema, T: AsRef<OptionLinksMetadata>>(
        &'a self,
        schema: &'a FederationSchemaRef<'schema, T>,
    ) -> Option<&'schema Node<DirectiveDefinition>> {
        if *self.version() < (Version { major: 0, minor: 2 }) {
            return None;
        }
        Some(
            self.directive_definition(schema, JOIN_IMPLEMENTS_DIRECTIVE_NAME_IN_SPEC)
                .unwrap_or_else(|| {
                    panic!(
                        "Unexpectedly could not find join spec's \"@{}\" directive definition",
                        JOIN_IMPLEMENTS_DIRECTIVE_NAME_IN_SPEC
                    )
                }),
        )
    }

    pub fn implements_directive_arguments<'a, 'schema>(
        &'a self,
        application: &'schema Node<Directive>,
    ) -> ImplementsDirectiveArguments<'schema> {
        ImplementsDirectiveArguments {
            graph: directive_required_enum_argument(application, JOIN_GRAPH_ARGUMENT_NAME),
            interface: directive_required_string_argument(
                application,
                JOIN_INTERFACE_ARGUMENT_NAME,
            ),
        }
    }

    pub fn union_member_directive_definition<'a, 'schema, T: AsRef<OptionLinksMetadata>>(
        &'a self,
        schema: &'a FederationSchemaRef<'schema, T>,
    ) -> Option<&'schema Node<DirectiveDefinition>> {
        if *self.version() < (Version { major: 0, minor: 3 }) {
            return None;
        }
        Some(
            self.directive_definition(schema, JOIN_UNIONMEMBER_DIRECTIVE_NAME_IN_SPEC)
                .unwrap_or_else(|| {
                    panic!(
                        "Unexpectedly could not find join spec's \"@{}\" directive definition",
                        JOIN_UNIONMEMBER_DIRECTIVE_NAME_IN_SPEC
                    )
                }),
        )
    }

    pub fn union_member_directive_arguments<'a, 'schema>(
        &'a self,
        application: &'schema Node<Directive>,
    ) -> UnionMemberDirectiveArguments<'schema> {
        UnionMemberDirectiveArguments {
            graph: directive_required_enum_argument(application, JOIN_GRAPH_ARGUMENT_NAME),
            member: directive_required_string_argument(application, JOIN_MEMBER_ARGUMENT_NAME),
        }
    }

    pub fn enum_value_directive_definition<'a, 'schema, T: AsRef<OptionLinksMetadata>>(
        &'a self,
        schema: &'a FederationSchemaRef<'schema, T>,
    ) -> Option<&'schema Node<DirectiveDefinition>> {
        if *self.version() < (Version { major: 0, minor: 3 }) {
            return None;
        }
        Some(
            self.directive_definition(schema, JOIN_ENUMVALUE_DIRECTIVE_NAME_IN_SPEC)
                .unwrap_or_else(|| {
                    panic!(
                        "Unexpectedly could not find join spec's \"@{}\" directive definition",
                        JOIN_ENUMVALUE_DIRECTIVE_NAME_IN_SPEC
                    )
                }),
        )
    }

    pub fn enum_value_directive_arguments<'a, 'schema>(
        &'a self,
        application: &'schema Node<Directive>,
    ) -> EnumValueDirectiveArguments<'schema> {
        EnumValueDirectiveArguments {
            graph: directive_required_enum_argument(application, JOIN_GRAPH_ARGUMENT_NAME),
        }
    }
}

impl SpecDefinition for JoinSpecDefinition {
    fn url(&self) -> &Url {
        &self.url
    }

    fn minimum_federation_version(&self) -> Option<&Version> {
        self.minimum_federation_version.as_ref()
    }
}

lazy_static! {
    pub static ref JOIN_VERSIONS: SpecDefinitions<JoinSpecDefinition> = {
        let mut definitions = SpecDefinitions::new(Identity::join_identity());
        definitions.add(JoinSpecDefinition::new(
            Version { major: 0, minor: 1 },
            None,
        ));
        definitions.add(JoinSpecDefinition::new(
            Version { major: 0, minor: 2 },
            None,
        ));
        definitions.add(JoinSpecDefinition::new(
            Version { major: 0, minor: 3 },
            None,
        ));
        definitions.add(JoinSpecDefinition::new(
            Version { major: 0, minor: 1 },
            Some(Version { major: 2, minor: 0 }),
        ));
        definitions
    };
}
