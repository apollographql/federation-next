use crate::spec::{
    AppliedFederationLink, FederationSpecDefinitions, FederationSpecError, LinkSpecDefinitions,
    FEDERATION_V2_DIRECTIVE_NAMES,
};
use apollo_at_link::link::LinkError;
use apollo_at_link::link::{self, DEFAULT_LINK_NAME};
use apollo_at_link::spec::Identity;
use apollo_compiler::Schema;
use indexmap::map::Entry;
use std::collections::BTreeMap;
use std::fmt::Formatter;
use std::sync::Arc;

pub mod database;
mod spec;

// TODO: we need a strategy for errors. All (or almost all) federation errors have a code in
// particular (and the way we deal with this in typescript, having all errors declared in one place
// with descriptions that allow to autogenerate doc is kind of useful (if not perfect)), so we
// probably want some additional crate specific for errors and use that here.
#[derive(Debug)]
pub struct SubgraphError {
    pub msg: String,
}

impl From<apollo_compiler::Diagnostics> for SubgraphError {
    fn from(value: apollo_compiler::Diagnostics) -> Self {
        SubgraphError {
            msg: value.to_string_no_color(),
        }
    }
}

impl From<LinkError> for SubgraphError {
    fn from(value: LinkError) -> Self {
        SubgraphError {
            msg: value.to_string(),
        }
    }
}

impl From<FederationSpecError> for SubgraphError {
    fn from(value: FederationSpecError) -> Self {
        SubgraphError {
            msg: value.to_string(),
        }
    }
}

pub struct Subgraph {
    pub name: String,
    pub url: String,
    pub schema: Schema,
}

impl Subgraph {
    pub fn new(name: &str, url: &str, schema_str: &str) -> Self {
        let schema = Schema::parse(schema_str, name);

        // TODO: ideally, we'd want Subgraph to always represent a valid subgraph: we don't want
        // every possible method that receive a subgraph to have to worry if the underlying schema
        // is actually not a subgraph at all: we want the type-system to help carry known
        // guarantees. This imply we should run validation here (both graphQL ones, but also
        // subgraph specific ones).
        // This also mean we would ideally want `schema` to not export any mutable methods
        // but not sure what that entail/how doable that is currently.

        Self {
            name: name.to_string(),
            url: url.to_string(),
            schema,
        }
    }

    pub fn parse_and_expand(
        name: &str,
        url: &str,
        schema_str: &str,
    ) -> Result<Self, SubgraphError> {
        let mut schema = Schema::parse(schema_str, name);

        let mut imported_federation_definitions: Option<FederationSpecDefinitions> = None;
        let mut imported_link_definitions: Option<LinkSpecDefinitions> = None;
        let link_directives = schema
            .schema_definition
            .directives
            .get_all(DEFAULT_LINK_NAME);

        for directive in link_directives {
            let link_directive = link::Link::from_directive_application(directive)?;
            if link_directive
                .url
                .identity
                .eq(&Identity::federation_identity())
            {
                if imported_federation_definitions.is_some() {
                    return Err(SubgraphError { msg: "invalid graphql schema - multiple @link imports for the federation specification are not supported".to_owned() });
                }

                imported_federation_definitions =
                    Some(FederationSpecDefinitions::from_link(link_directive)?);
            } else if link_directive.url.identity.eq(&Identity::link_identity()) {
                // user manually imported @link specification
                if imported_link_definitions.is_some() {
                    return Err(SubgraphError { msg: "invalid graphql schema - multiple @link imports for the link specification are not supported".to_owned() });
                }

                imported_link_definitions = Some(LinkSpecDefinitions::new(link_directive));
            }
        }

        // generate additional schema definitions
        Self::populate_missing_type_definitions(
            &mut schema,
            imported_federation_definitions,
            imported_link_definitions,
        )?;
        schema.validate()?;
        Ok(Self {
            name: name.to_owned(),
            url: url.to_owned(),
            schema,
        })
    }

    fn populate_missing_type_definitions(
        schema: &mut Schema,
        imported_federation_definitions: Option<FederationSpecDefinitions>,
        imported_link_definitions: Option<LinkSpecDefinitions>,
    ) -> Result<(), SubgraphError> {
        // populate @link spec definitions
        let link_spec_definitions = match imported_link_definitions {
            Some(definitions) => definitions,
            None => {
                // need to apply default @link directive for link spec on schema
                let defaults = LinkSpecDefinitions::default();
                schema
                    .schema_definition
                    .make_mut()
                    .directives
                    .push(defaults.applied_link_directive().into());
                defaults
            }
        };
        Self::populate_missing_link_definitions(schema, link_spec_definitions)?;

        // populate @link federation spec definitions
        let fed_definitions = match imported_federation_definitions {
            Some(definitions) => definitions,
            None => {
                // federation v1 schema or user does not import federation spec
                // need to apply default @link directive for federation spec on schema
                let defaults = FederationSpecDefinitions::default()?;
                schema
                    .schema_definition
                    .make_mut()
                    .directives
                    .push(defaults.applied_link_directive().into());
                defaults
            }
        };
        Self::populate_missing_federation_definitions(schema, fed_definitions)
    }

    fn populate_missing_link_definitions(
        schema: &mut Schema,
        link_spec_definitions: LinkSpecDefinitions,
    ) -> Result<(), SubgraphError> {
        schema
            .types
            .entry(link_spec_definitions.purpose_enum_name.as_str().into())
            .or_insert_with(|| link_spec_definitions.link_purpose_enum_definition().into());
        schema
            .types
            .entry(link_spec_definitions.import_scalar_name.as_str().into())
            .or_insert_with(|| link_spec_definitions.import_scalar_definition().into());
        schema
            .directive_definitions
            .entry(DEFAULT_LINK_NAME.into())
            .or_insert_with(|| link_spec_definitions.link_directive_definition().into());
        Ok(())
    }

    fn populate_missing_federation_definitions(
        schema: &mut Schema,
        fed_definitions: FederationSpecDefinitions,
    ) -> Result<(), SubgraphError> {
        schema
            .types
            .entry(fed_definitions.fieldset_scalar_name.as_str().into())
            .or_insert_with(|| fed_definitions.fieldset_scalar_definition().into());

        for directive_name in FEDERATION_V2_DIRECTIVE_NAMES {
            let namespaced_directive_name =
                fed_definitions.namespaced_type_name(directive_name, true);
            if let Entry::Vacant(entry) = schema
                .directive_definitions
                .entry(namespaced_directive_name.as_str().into())
            {
                let directive_definition = fed_definitions.directive_definition(
                    directive_name,
                    &Some(namespaced_directive_name.to_owned()),
                )?;
                entry.insert(directive_definition.into());
            }
        }
        Ok(())
    }
}

impl std::fmt::Debug for Subgraph {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, r#"name: {}, urL: {}"#, self.name, self.url)
    }
}

pub struct Subgraphs {
    subgraphs: BTreeMap<String, Arc<Subgraph>>,
}

#[allow(clippy::new_without_default)]
impl Subgraphs {
    pub fn new() -> Self {
        Subgraphs {
            subgraphs: BTreeMap::new(),
        }
    }

    pub fn add(&mut self, subgraph: Subgraph) -> Result<(), SubgraphError> {
        if self.subgraphs.contains_key(&subgraph.name) {
            return Err(SubgraphError {
                msg: format!("A subgraph named {} already exists", subgraph.name),
            });
        }
        self.subgraphs
            .insert(subgraph.name.clone(), Arc::new(subgraph));
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<Arc<Subgraph>> {
        self.subgraphs.get(name).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::keys;

    #[test]
    fn can_inspect_a_type_key() {
        // TODO: no schema expansion currently, so need to having the `@link` to `link` and the
        // @link directive definition for @link-bootstrapping to work. Also, we should
        // theoretically have the @key directive definition added too (but validation is not
        // wired up yet, so we get away without). Point being, this is just some toy code at
        // the moment.

        let schema = r#"
          extend schema
            @link(url: "https://specs.apollo.dev/link/v1.0", import: ["Import"])
            @link(url: "https://specs.apollo.dev/federation/v2.3", import: ["@key"])

          type Query {
            t: T
          }

          type T @key(fields: "id") {
            id: ID!
            x: Int
          }

          enum link__Purpose {
            SECURITY
            EXECUTION
          }

          scalar Import

          directive @link(url: String, as: String, import: [Import], for: link__Purpose) repeatable on SCHEMA
        "#;

        let subgraph = Subgraph::new("S1", "http://s1", schema);
        let keys = keys(&subgraph.schema, "T");
        assert_eq!(keys.len(), 1);
        assert_eq!(keys.get(0).unwrap().type_name, "T");

        // TODO: no accessible selection yet.
    }

    #[test]
    fn can_parse_and_expand() -> Result<(), String> {
        let schema = r#"
        extend schema
          @link(url: "https://specs.apollo.dev/federation/v2.3", import: [ "@key" ])

        type Query {
            t: T
        }

        type T @key(fields: "id") {
            id: ID!
            x: Int
        }
        "#;

        let subgraph = Subgraph::parse_and_expand("S1", "http://s1", schema).map_err(|e| {
            println!("{}", e.msg);
            String::from("failed to parse and expand the subgraph, see errors above for details")
        })?;
        assert!(subgraph.schema.types.contains_key("T"));
        assert!(subgraph.schema.directive_definitions.contains_key("key"));
        assert!(subgraph
            .schema
            .directive_definitions
            .contains_key("federation__requires"));
        Ok(())
    }

    #[test]
    fn can_parse_and_expand_with_renames() -> Result<(), String> {
        let schema = r#"
        extend schema
          @link(url: "https://specs.apollo.dev/federation/v2.3", import: [ { name: "@key", as: "@myKey" }, "@provides" ])

        type Query {
            t: T @provides(fields: "x")
        }

        type T @myKey(fields: "id") {
            id: ID!
            x: Int
        }
        "#;

        let subgraph = Subgraph::parse_and_expand("S1", "http://s1", schema).map_err(|e| {
            println!("{}", e.msg);
            String::from("failed to parse and expand the subgraph, see errors above for details")
        })?;
        assert!(subgraph.schema.directive_definitions.contains_key("myKey"));
        assert!(subgraph
            .schema
            .directive_definitions
            .contains_key("provides"));
        Ok(())
    }

    #[test]
    fn can_parse_and_expand_with_namespace() -> Result<(), String> {
        let schema = r#"
        extend schema
          @link(url: "https://specs.apollo.dev/federation/v2.3", import: [ "@key" ], as: "fed" )

        type Query {
            t: T
        }

        type T @key(fields: "id") {
            id: ID!
            x: Int
        }
        "#;

        let subgraph = Subgraph::parse_and_expand("S1", "http://s1", schema).map_err(|e| {
            println!("{}", e.msg);
            String::from("failed to parse and expand the subgraph, see errors above for details")
        })?;
        assert!(subgraph.schema.directive_definitions.contains_key("key"));
        assert!(subgraph
            .schema
            .directive_definitions
            .contains_key("fed__requires"));
        Ok(())
    }

    #[test]
    fn can_parse_and_expand_preserves_user_definitions() -> Result<(), String> {
        let schema = r#"
        extend schema
          @link(url: "https://specs.apollo.dev/link/v1.0", import: ["Import", "Purpose"])
          @link(url: "https://specs.apollo.dev/federation/v2.3", import: [ "@key" ])

        type Query {
            t: T
        }

        type T @key(fields: "id") {
            id: ID!
            x: Int
        }

        enum Purpose {
            SECURITY
            EXECUTION
        }

        scalar Import

        directive @link(url: String, as: String, import: [Import], for: Purpose) repeatable on SCHEMA
        "#;

        let subgraph = Subgraph::parse_and_expand("S1", "http://s1", schema).map_err(|e| {
            println!("{}", e.msg);
            String::from("failed to parse and expand the subgraph, see errors above for details")
        })?;
        assert!(subgraph
            .schema
            .directive_definitions
            .contains_key("Purpose"));
        Ok(())
    }

    #[test]
    fn can_parse_and_expand_works_with_fed_v1() -> Result<(), String> {
        let schema = r#"
        type Query {
            t: T
        }

        type T @key(fields: "id") {
            id: ID!
            x: Int
        }
        "#;

        let subgraph = Subgraph::parse_and_expand("S1", "http://s1", schema).map_err(|e| {
            println!("{}", e.msg);
            String::from("failed to parse and expand the subgraph, see errors above for details")
        })?;
        assert!(subgraph.schema.types.contains_key("T"));
        assert!(subgraph.schema.directive_definitions.contains_key("key"));
        Ok(())
    }

    #[test]
    fn can_parse_and_expand_will_fail_when_importing_same_spec_twice() {
        let schema = r#"
        extend schema
          @link(url: "https://specs.apollo.dev/federation/v2.3", import: [ "@key" ] )
          @link(url: "https://specs.apollo.dev/federation/v2.3", import: [ "@provides" ] )

        type Query {
            t: T
        }

        type T @key(fields: "id") {
            id: ID!
            x: Int
        }
        "#;

        let result = Subgraph::parse_and_expand("S1", "http://s1", schema)
            .expect_err("importing same specification twice should fail");
        assert_eq!("invalid graphql schema - multiple @link imports for the federation specification are not supported", result.msg);
    }
}
