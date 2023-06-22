use crate::spec::FederationSpecError::{
    DirectiveCannotBeRenamed, UnsupportedFederationDirective, UnsupportedVersionError,
};
use apollo_at_link::link::{
    Import, Link, DEFAULT_IMPORT_SCALAR_NAME, DEFAULT_LINK_NAME, DEFAULT_PURPOSE_ENUM_NAME,
};
use apollo_at_link::spec::{Identity, Url, Version};
use apollo_compiler::hir::DirectiveLocation;
use apollo_encoder::{
    Argument, Directive, DirectiveDefinition, EnumDefinition, EnumValue, InputValueDefinition,
    ScalarDefinition, Type_, Value,
};
use std::sync::Arc;

use thiserror::Error;

pub const COMPOSE_DIRECTIVE_NAME: &str = "composeDirective";
pub const KEY_DIRECTIVE_NAME: &str = "key";
pub const EXTENDS_DIRECTIVE_NAME: &str = "extends";
pub const EXTERNAL_DIRECTIVE_NAME: &str = "external";
pub const INACCESSIBLE_DIRECTIVE_NAME: &str = "inaccessible";
pub const INTF_OBJECT_DIRECTIVE_NAME: &str = "interfaceObject";
pub const OVERRIDE_DIRECTIVE_NAME: &str = "override";
pub const PROVIDES_DIRECTIVE_NAME: &str = "provides";
pub const REQUIRES_DIRECTIVE_NAME: &str = "requires";
pub const SHAREABLE_DIRECTIVE_NAME: &str = "shareable";
pub const TAG_DIRECTIVE_NAME: &str = "tag";
pub const FIELDSET_SCALAR_NAME: &str = "FieldSet";

pub const FEDERATION_V1_DIRECTIVE_NAMES: [&str; 5] = [
    KEY_DIRECTIVE_NAME,
    EXTENDS_DIRECTIVE_NAME,
    EXTERNAL_DIRECTIVE_NAME,
    PROVIDES_DIRECTIVE_NAME,
    REQUIRES_DIRECTIVE_NAME,
];

pub const FEDERATION_V2_DIRECTIVE_NAMES: [&str; 11] = [
    COMPOSE_DIRECTIVE_NAME,
    KEY_DIRECTIVE_NAME,
    EXTENDS_DIRECTIVE_NAME,
    EXTERNAL_DIRECTIVE_NAME,
    INACCESSIBLE_DIRECTIVE_NAME,
    INTF_OBJECT_DIRECTIVE_NAME,
    OVERRIDE_DIRECTIVE_NAME,
    PROVIDES_DIRECTIVE_NAME,
    REQUIRES_DIRECTIVE_NAME,
    SHAREABLE_DIRECTIVE_NAME,
    TAG_DIRECTIVE_NAME,
];

const MIN_FEDERATION_VERSION: Version = Version { major: 2, minor: 0 };
const MAX_FEDERATION_VERSION: Version = Version { major: 2, minor: 4 };

#[derive(Error, Debug, PartialEq)]
pub enum FederationSpecError {
    #[error(
        "Specified specification version {specified} is outside of supported range {min}-{max}"
    )]
    UnsupportedVersionError {
        specified: String,
        min: String,
        max: String,
    },
    #[error("Unsupported federation directive import {0}")]
    UnsupportedFederationDirective(String),
    #[error("{0} directive cannot be renamed")]
    DirectiveCannotBeRenamed(String),
}

#[derive(Debug)]
pub struct FederationSpecDefinitions {
    link: Link,
    pub fieldset_scalar_name: String,
}

#[derive(Debug)]
pub struct LinkSpecDefinitions {
    link: Link,
    pub import_scalar_name: String,
    pub purpose_enum_name: String,
}

pub trait AppliedFederationLink {
    fn applied_link_directive(&self) -> Directive;
}

macro_rules! applied_specification {
    ($($t:ty),+) => {
        $(impl AppliedFederationLink for $t {
            /// @link(url: "https://specs.apollo.dev/federation/v2.3", import: ["@key"])
            fn applied_link_directive(&self) -> Directive {
                let mut applied_link_directive = Directive::new(DEFAULT_LINK_NAME.to_owned());
                applied_link_directive.arg(Argument::new(
                    "url".to_owned(),
                    Value::String(self.link.url.to_string()),
                ));
                let imports = self
                    .link
                    .imports
                    .iter()
                    .map(|i| {
                        if i.alias.is_some() {
                            Value::Object(vec![
                                ("name".to_string(), Value::String(i.element.to_owned())),
                                ("as".to_string(), Value::String(i.imported_display_name())),
                            ])
                        } else {
                            Value::String(i.imported_display_name())
                        }
                    })
                    .collect::<Vec<Value>>();
                applied_link_directive.arg(Argument::new("import".to_owned(), Value::List(imports)));
                if let Some(spec_alias) = &self.link.spec_alias {
                    applied_link_directive.arg(Argument::new(
                        "as".to_owned(),
                        Value::String(spec_alias.to_owned()),
                    ))
                }
                if let Some(purpose) = &self.link.purpose {
                    applied_link_directive.arg(Argument::new(
                        "for".to_owned(),
                        Value::Enum(purpose.to_string()),
                    ))
                }
                applied_link_directive
            }
        })+
    }
}

applied_specification!(FederationSpecDefinitions, LinkSpecDefinitions);

impl FederationSpecDefinitions {
    pub fn from_link(link: Link) -> Result<Self, FederationSpecError> {
        if !link
            .url
            .version
            .satisfies_range(&MIN_FEDERATION_VERSION, &MAX_FEDERATION_VERSION)
        {
            Err(UnsupportedVersionError {
                specified: link.url.version.to_string(),
                min: MIN_FEDERATION_VERSION.to_string(),
                max: MAX_FEDERATION_VERSION.to_string(),
            })
        } else {
            let fieldset_scalar_name = link.type_name_in_schema(FIELDSET_SCALAR_NAME);
            Ok(Self {
                link,
                fieldset_scalar_name,
            })
        }
    }

    pub fn default() -> Result<Self, FederationSpecError> {
        Self::from_link(Link {
            url: Url {
                identity: Identity::federation_identity(),
                version: MAX_FEDERATION_VERSION,
            },
            imports: FEDERATION_V1_DIRECTIVE_NAMES
                .iter()
                .map(|i| {
                    Arc::new(Import {
                        element: i.to_string(),
                        alias: None,
                        is_directive: true,
                    })
                })
                .collect::<Vec<Arc<Import>>>(),
            purpose: None,
            spec_alias: None,
        })
    }

    pub fn namespaced_type_name(&self, name: &str, is_directive: bool) -> String {
        if is_directive {
            self.link.directive_name_in_schema(name)
        } else {
            self.link.type_name_in_schema(name)
        }
    }

    pub fn directive_definition(
        &self,
        name: &str,
        alias: &Option<String>,
    ) -> Result<DirectiveDefinition, FederationSpecError> {
        match name {
            COMPOSE_DIRECTIVE_NAME => Ok(self.compose_directive_definition(alias)),
            KEY_DIRECTIVE_NAME => Ok(self.key_directive_definition(alias)),
            EXTENDS_DIRECTIVE_NAME => Ok(self.extends_directive_definition(alias)),
            EXTERNAL_DIRECTIVE_NAME => Ok(self.external_directive_definition(alias)),
            INACCESSIBLE_DIRECTIVE_NAME => self.inaccessible_directive_definition(alias),
            INTF_OBJECT_DIRECTIVE_NAME => Ok(self.interface_object_directive_definition(alias)),
            OVERRIDE_DIRECTIVE_NAME => Ok(self.override_directive_definition(alias)),
            PROVIDES_DIRECTIVE_NAME => Ok(self.provides_directive_definition(alias)),
            REQUIRES_DIRECTIVE_NAME => Ok(self.requires_directive_definition(alias)),
            SHAREABLE_DIRECTIVE_NAME => Ok(self.shareable_directive_definition(alias)),
            TAG_DIRECTIVE_NAME => self.tag_directive_definition(alias),
            _ => Err(UnsupportedFederationDirective(name.to_string())),
        }
    }

    /// scalar FieldSet
    pub fn fieldset_scalar_definition(&self) -> ScalarDefinition {
        ScalarDefinition::new(self.namespaced_type_name(FIELDSET_SCALAR_NAME, false))
    }

    fn fields_argument_definition(&self) -> InputValueDefinition {
        InputValueDefinition::new(
            "fields".to_owned(),
            Type_::NonNull {
                ty: Box::new(Type_::NamedType {
                    name: self.namespaced_type_name(FIELDSET_SCALAR_NAME, false),
                }),
            },
        )
    }

    /// directive @composeDirective(name: String!) repeatable on SCHEMA
    fn compose_directive_definition(&self, alias: &Option<String>) -> DirectiveDefinition {
        let compose_directive_name = alias
            .as_deref()
            .unwrap_or(COMPOSE_DIRECTIVE_NAME)
            .to_owned();
        let mut compose_directive = DirectiveDefinition::new(compose_directive_name);
        compose_directive.arg(InputValueDefinition::new(
            "name".to_owned(),
            Type_::NonNull {
                ty: Box::new(Type_::NamedType {
                    name: "String".to_owned(),
                }),
            },
        ));

        compose_directive.location(DirectiveLocation::Schema.to_string());
        compose_directive
    }

    /// directive @key(fields: FieldSet!, resolvable: Boolean = true) repeatable on OBJECT | INTERFACE
    fn key_directive_definition(&self, alias: &Option<String>) -> DirectiveDefinition {
        let key_directive_name = alias.as_deref().unwrap_or(KEY_DIRECTIVE_NAME).to_owned();
        let mut key_directive = DirectiveDefinition::new(key_directive_name);

        key_directive.arg(self.fields_argument_definition());
        let mut resolvable_arg = InputValueDefinition::new(
            "resolvable".to_owned(),
            Type_::NamedType {
                name: "Boolean".to_owned(),
            },
        );
        resolvable_arg.default_value("true".to_owned());
        key_directive.arg(resolvable_arg);

        key_directive.repeatable();
        key_directive.location(DirectiveLocation::Object.to_string());
        key_directive.location(DirectiveLocation::Interface.to_string());
        key_directive
    }

    /// directive @extends on OBJECT | INTERFACE
    fn extends_directive_definition(&self, alias: &Option<String>) -> DirectiveDefinition {
        let extends_directive_name = alias
            .as_deref()
            .unwrap_or(EXTENDS_DIRECTIVE_NAME)
            .to_owned();
        let mut extends_directive = DirectiveDefinition::new(extends_directive_name);
        extends_directive.location(DirectiveLocation::Object.to_string());
        extends_directive.location(DirectiveLocation::Interface.to_string());
        extends_directive
    }

    /// directive @external on OBJECT | FIELD_DEFINITION
    fn external_directive_definition(&self, alias: &Option<String>) -> DirectiveDefinition {
        let external_directive_name = alias
            .as_deref()
            .unwrap_or(EXTERNAL_DIRECTIVE_NAME)
            .to_owned();
        let mut external_directive = DirectiveDefinition::new(external_directive_name);
        external_directive.location(DirectiveLocation::Object.to_string());
        external_directive.location(DirectiveLocation::FieldDefinition.to_string());
        external_directive
    }

    /// directive @inaccessible on
    ///   | ARGUMENT_DEFINITION
    ///   | ENUM
    ///   | ENUM_VALUE
    ///   | FIELD_DEFINITION
    ///   | INPUT_FIELD_DEFINITION
    ///   | INPUT_OBJECT
    ///   | INTERFACE
    ///   | OBJECT
    ///   | SCALAR
    ///   | UNION
    fn inaccessible_directive_definition(
        &self,
        alias: &Option<String>,
    ) -> Result<DirectiveDefinition, FederationSpecError> {
        if alias.is_some()
            && !alias.as_ref().unwrap().eq(&format!(
                "{}__inaccessible",
                self.link.spec_name_in_schema()
            ))
        {
            return Err(DirectiveCannotBeRenamed(
                INACCESSIBLE_DIRECTIVE_NAME.to_owned(),
            ));
        }

        let inaccessible_directive_name = INACCESSIBLE_DIRECTIVE_NAME.to_owned();
        let mut inaccessible_directive = DirectiveDefinition::new(inaccessible_directive_name);
        inaccessible_directive.location(DirectiveLocation::ArgumentDefinition.to_string());
        inaccessible_directive.location(DirectiveLocation::Enum.to_string());
        inaccessible_directive.location(DirectiveLocation::EnumValue.to_string());
        inaccessible_directive.location(DirectiveLocation::FieldDefinition.to_string());
        inaccessible_directive.location(DirectiveLocation::InputFieldDefinition.to_string());
        inaccessible_directive.location(DirectiveLocation::InputObject.to_string());
        inaccessible_directive.location(DirectiveLocation::Interface.to_string());
        inaccessible_directive.location(DirectiveLocation::Object.to_string());
        inaccessible_directive.location(DirectiveLocation::Scalar.to_string());
        inaccessible_directive.location(DirectiveLocation::Union.to_string());
        Ok(inaccessible_directive)
    }

    /// directive @interfaceObject on OBJECT
    fn interface_object_directive_definition(&self, alias: &Option<String>) -> DirectiveDefinition {
        let interface_object_name = alias
            .as_deref()
            .unwrap_or(INTF_OBJECT_DIRECTIVE_NAME)
            .to_owned();
        let mut interface_object_directive = DirectiveDefinition::new(interface_object_name);
        interface_object_directive.location(DirectiveLocation::Object.to_string());
        interface_object_directive
    }

    /// directive @override(from: String!) on FIELD_DEFINITION
    fn override_directive_definition(&self, alias: &Option<String>) -> DirectiveDefinition {
        let override_directive_name = alias
            .as_deref()
            .unwrap_or(OVERRIDE_DIRECTIVE_NAME)
            .to_owned();
        let mut override_directive = DirectiveDefinition::new(override_directive_name);
        override_directive.location(DirectiveLocation::FieldDefinition.to_string());

        override_directive.arg(InputValueDefinition::new(
            "from".to_owned(),
            Type_::NonNull {
                ty: Box::new(Type_::NamedType {
                    name: "String".to_owned(),
                }),
            },
        ));
        override_directive
    }

    /// directive @provides(fields: FieldSet!) on FIELD_DEFINITION
    fn provides_directive_definition(&self, alias: &Option<String>) -> DirectiveDefinition {
        let provides_directive_name = alias
            .as_deref()
            .unwrap_or(PROVIDES_DIRECTIVE_NAME)
            .to_owned();
        let mut provides_directive = DirectiveDefinition::new(provides_directive_name);
        provides_directive.arg(self.fields_argument_definition());
        provides_directive.location(DirectiveLocation::FieldDefinition.to_string());
        provides_directive
    }

    /// directive @requires(fields: FieldSet!) on FIELD_DEFINITION
    fn requires_directive_definition(&self, alias: &Option<String>) -> DirectiveDefinition {
        let requires_directive_name = alias
            .as_deref()
            .unwrap_or(REQUIRES_DIRECTIVE_NAME)
            .to_owned();
        let mut requires_directive = DirectiveDefinition::new(requires_directive_name);
        requires_directive.arg(self.fields_argument_definition());
        requires_directive.location(DirectiveLocation::FieldDefinition.to_string());
        requires_directive
    }

    /// directive @shareable repeatable on FIELD_DEFINITION | OBJECT
    fn shareable_directive_definition(&self, alias: &Option<String>) -> DirectiveDefinition {
        let shareable_directive_name = alias
            .as_deref()
            .unwrap_or(SHAREABLE_DIRECTIVE_NAME)
            .to_owned();
        let mut shareable_directive = DirectiveDefinition::new(shareable_directive_name);
        shareable_directive.repeatable();
        shareable_directive.location(DirectiveLocation::FieldDefinition.to_string());
        shareable_directive.location(DirectiveLocation::Object.to_string());
        shareable_directive
    }

    /// directive @tag(name: String!) repeatable on
    ///   | ARGUMENT_DEFINITION
    ///   | ENUM
    ///   | ENUM_VALUE
    ///   | FIELD_DEFINITION
    ///   | INPUT_FIELD_DEFINITION
    ///   | INPUT_OBJECT
    ///   | INTERFACE
    ///   | OBJECT
    ///   | SCALAR
    ///   | UNION
    fn tag_directive_definition(
        &self,
        alias: &Option<String>,
    ) -> Result<DirectiveDefinition, FederationSpecError> {
        if alias.is_some()
            && !alias
                .as_ref()
                .unwrap()
                .eq(&format!("{}__tag", self.link.spec_name_in_schema()))
        {
            return Err(DirectiveCannotBeRenamed(TAG_DIRECTIVE_NAME.to_owned()));
        }

        let tag_directive_name = TAG_DIRECTIVE_NAME.to_owned();
        let mut tag_directive = DirectiveDefinition::new(tag_directive_name);
        tag_directive.repeatable();
        tag_directive.location(DirectiveLocation::ArgumentDefinition.to_string());
        tag_directive.location(DirectiveLocation::Enum.to_string());
        tag_directive.location(DirectiveLocation::EnumValue.to_string());
        tag_directive.location(DirectiveLocation::FieldDefinition.to_string());
        tag_directive.location(DirectiveLocation::InputFieldDefinition.to_string());
        tag_directive.location(DirectiveLocation::InputObject.to_string());
        tag_directive.location(DirectiveLocation::Interface.to_string());
        tag_directive.location(DirectiveLocation::Object.to_string());
        tag_directive.location(DirectiveLocation::Scalar.to_string());
        tag_directive.location(DirectiveLocation::Union.to_string());
        Ok(tag_directive)
    }
}

impl LinkSpecDefinitions {
    pub fn new(link: Link) -> Self {
        let import_scalar_name = link.type_name_in_schema(DEFAULT_IMPORT_SCALAR_NAME);
        let purpose_enum_name = link.type_name_in_schema(DEFAULT_PURPOSE_ENUM_NAME);
        Self {
            link,
            import_scalar_name,
            purpose_enum_name,
        }
    }

    pub fn default() -> Self {
        let link = Link {
            url: Url {
                identity: Identity::link_identity(),
                version: Version { major: 1, minor: 0 },
            },
            imports: vec![Arc::new(Import {
                element: "Import".to_owned(),
                is_directive: false,
                alias: None,
            })],
            purpose: None,
            spec_alias: None,
        };
        Self::new(link)
    }

    ///   scalar Import
    pub fn import_scalar_definition(&self) -> ScalarDefinition {
        ScalarDefinition::new(self.import_scalar_name.to_owned())
    }

    ///   enum link__Purpose {
    ///     SECURITY
    ///     EXECUTION
    ///   }
    pub fn link_purpose_enum_definition(&self) -> EnumDefinition {
        let mut link_purpose_enum_definition =
            EnumDefinition::new(self.purpose_enum_name.to_owned());
        link_purpose_enum_definition.value(EnumValue::new("SECURITY".to_owned()));
        link_purpose_enum_definition.value(EnumValue::new("EXECUTION".to_owned()));
        link_purpose_enum_definition
    }

    ///   directive @link(url: String, as: String, import: [Import], for: link__Purpose) repeatable on SCHEMA
    pub fn link_directive_definition(&self) -> DirectiveDefinition {
        let mut link_directive_definition = DirectiveDefinition::new(DEFAULT_LINK_NAME.to_owned());
        link_directive_definition.arg(InputValueDefinition::new(
            "url".to_owned(),
            Type_::NonNull {
                ty: Box::new(Type_::NamedType {
                    name: "String".to_owned(),
                }),
            },
        ));
        link_directive_definition.arg(InputValueDefinition::new(
            "as".to_owned(),
            Type_::NamedType {
                name: "String".to_owned(),
            },
        ));
        link_directive_definition.arg(InputValueDefinition::new(
            "import".to_owned(),
            Type_::List {
                ty: Box::new(Type_::NamedType {
                    name: self.import_scalar_name.to_owned(),
                }),
            },
        ));
        link_directive_definition.arg(InputValueDefinition::new(
            "for".to_owned(),
            Type_::NamedType {
                name: self.purpose_enum_name.to_owned(),
            },
        ));
        link_directive_definition.repeatable();
        link_directive_definition.location(DirectiveLocation::Schema.to_string());
        link_directive_definition
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::federation_link_identity;

    #[test]
    fn handle_unsupported_federation_version() {
        FederationSpecDefinitions::from_link(Link {
            url: Url {
                identity: federation_link_identity(),
                version: Version {
                    major: 99,
                    minor: 99,
                },
            },
            spec_alias: None,
            imports: vec![],
            purpose: None,
        })
        .expect_err("federation version 99 is not yet supported");
    }

    #[test]
    fn tag_directive_cannot_be_renamed() {
        let definitions = FederationSpecDefinitions::from_link(Link {
            url: Url {
                identity: federation_link_identity(),
                version: Version { major: 2, minor: 3 },
            },
            spec_alias: None,
            imports: vec![Arc::new(Import {
                element: "tag".to_string(),
                is_directive: false,
                alias: Some("myTag".to_string()),
            })],
            purpose: None,
        })
        .unwrap();
        definitions
            .tag_directive_definition(&Some("myTag".to_owned()))
            .expect_err("we shouldn't be able to rename @tag directive");
    }

    #[test]
    fn inaccessible_directive_cannot_be_renamed() {
        let definitions = FederationSpecDefinitions::from_link(Link {
            url: Url {
                identity: federation_link_identity(),
                version: Version { major: 2, minor: 3 },
            },
            spec_alias: None,
            imports: vec![Arc::new(Import {
                element: "inaccessible".to_string(),
                is_directive: false,
                alias: Some("hidden".to_string()),
            })],
            purpose: None,
        })
        .unwrap();
        definitions
            .inaccessible_directive_definition(&Some("hidden".to_owned()))
            .expect_err("we shouldn't be able to rename @inaccessible directive");
    }
}
