use lazy_static::lazy_static;
use regex::Regex;
use crate::error::{ErrorEnum, GraphQLError};

// PORT_NOTE: The JS code uses "as", but it's a reserved keyword in Rust, so we change it to "as_"
// here (note that Rust gives a special meaning to unused variables starting with "_").
#[derive(Debug)]
pub struct CoreImport {
    name: String,
    as_: Option<String>
}

impl CoreImport {
    pub(crate) fn new(name: String, as_: Option<String>) -> CoreImport {
        CoreImport {
            name,
            as_,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn as_(&self) -> Option<&str> {
        self.as_.as_deref()
    }
}


/// Versions are a (major, minor) number pair.
#[derive(Debug)]
pub struct FeatureVersion {
    major: u32,
    minor: u32,
}

lazy_static! {
    static ref VERSION_RE: Regex = Regex::new(r"^v(\d+)\.(\d+)$").unwrap();
}

impl FeatureVersion {
    pub(crate) fn new(major: u32, minor: u32) -> FeatureVersion {
        FeatureVersion {
            major,
            minor,
        }
    }

    pub fn major(&self) -> u32 {
        self.major.clone()
    }

    pub fn minor(&self) -> u32 {
        self.minor.clone()
    }

    // TODO: Convert example to Rust
    /// Parse a version specifier of the form "v(major).(minor)" or throw
    ///
    /// # Example
    /// ``
    /// expect(FeatureVersion.parse('v1.0')).toEqual(new FeatureVersion(1, 0))
    /// expect(FeatureVersion.parse('v0.1')).toEqual(new FeatureVersion(0, 1))
    /// expect(FeatureVersion.parse("v987.65432")).toEqual(new FeatureVersion(987, 65432))
    /// ``
    pub fn parse(input: &str) -> Result<FeatureVersion, GraphQLError> {
        let Some(caps) = VERSION_RE.captures(input) else {
            return Err(ErrorEnum::InvalidLinkIdentifier.definition().err(
                format!("Expected a version string (of the form v1.2), got {}", input),
                None,
            ));
        };
        let Ok(major) = caps[1].parse() else {
            return Err(ErrorEnum::InvalidLinkIdentifier.definition().err(
                format!("Expected an unsigned 32-bit major version, got {}", &caps[1]),
                None,
            ));
        };
        let Ok(minor) = caps[2].parse() else {
            return Err(ErrorEnum::InvalidLinkIdentifier.definition().err(
                format!("Expected an unsigned 32-bit minor version, got {}", &caps[2]),
                None,
            ));
        };
        return Ok(FeatureVersion {
            major,
            minor,
        })
    }
}

#[derive(Debug)]
pub struct FeatureUrl {
    identity: String,
    name: String,
    version: FeatureVersion,
    element: Option<String>,
}

impl FeatureUrl {
    pub(crate) fn new(
        identity: String,
        name: String,
        version: FeatureVersion,
        element: Option<String>
    ) -> FeatureUrl {
        FeatureUrl {
            identity,
            name,
            version,
            element,
        }
    }

    pub fn identity(&self) -> &str {
        &self.identity
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn version(&self) -> &FeatureVersion {
        &self.version
    }

    pub fn element(&self) -> Option<&str> {
        self.element.as_deref()
    }
}
