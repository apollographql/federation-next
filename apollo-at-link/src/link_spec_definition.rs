use lazy_static::lazy_static;
use crate::spec::{Identity, Url, Version};
use crate::spec_definition::{SpecDefinition, SpecDefinitions};

pub struct LinkSpecDefinition {
    url: Url,
    minimum_federation_version: Option<Version>,
}

impl LinkSpecDefinition {
    pub fn new(
        version: Version,
        minimum_federation_version: Option<Version>,
        identity: Identity,
    ) -> Self {
        Self {
            url: Url {
                identity,
                version,
            },
            minimum_federation_version,
        }
    }
}

impl SpecDefinition for LinkSpecDefinition {
    fn url(&self) -> &Url {
        &self.url
    }

    fn minimum_federation_version(&self) -> Option<&Version> {
        self.minimum_federation_version.as_ref()
    }
}

lazy_static! {
    pub static ref CORE_VERSIONS: SpecDefinitions<LinkSpecDefinition> = {
        let mut definitions = SpecDefinitions::new(Identity::core_identity());
        definitions.add(LinkSpecDefinition::new(
            Version { major: 0, minor: 1 },
            None,
            Identity::core_identity(),
        ));
        definitions.add(LinkSpecDefinition::new(
            Version { major: 0, minor: 2 },
            Some(Version { major: 2, minor: 0 }),
            Identity::core_identity(),
        ));
        definitions
    };

    pub static ref LINK_VERSIONS: SpecDefinitions<LinkSpecDefinition> = {
        let mut definitions = SpecDefinitions::new(Identity::core_identity());
        definitions.add(LinkSpecDefinition::new(
            Version { major: 1, minor: 0 },
            Some(Version { major: 2, minor: 0 }),
            Identity::link_identity(),
        ));
        definitions
    };
}
