use std::{collections::BTreeMap, path::Path, sync::Arc};

use apollo_compiler::hir::TypeSystem;
use apollo_compiler::{ApolloCompiler, FileId, HirDatabase, InputDatabase, Source};
use apollo_encoder::{Document, SchemaDefinition};

use apollo_at_link::link::LinkError;
use apollo_at_link::{
    link::{self, DEFAULT_LINK_NAME},
    spec::Identity,
};
#[allow(unused)]
use database::{SubgraphDatabase, SubgraphRootDatabase};

use crate::spec::{
    AppliedFederationLink, FederationSpecDefinitions, FederationSpecError, LinkSpecDefinitions,
    FEDERATION_V2_DIRECTIVE_NAMES,
};

mod database;
mod spec;

// TODO: we need a strategy for errors. All (or almost all) federation errors have a code in
// particular (and the way we deal with this in typescript, having all errors declared in one place
// with descriptions that allow to autogenerate doc is kind of useful (if not perfect)), so we
// probably want some additional crate specific for errors and use that here.
#[derive(Debug)]
pub struct SubgraphError {
    pub msg: String,
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
    pub db: SubgraphRootDatabase,
}

impl Subgraph {
    pub fn new(name: &str, url: &str, schema_str: &str) -> Self {
        let mut db = SubgraphRootDatabase::default();
        db.set_recursion_limit(None);
        db.set_token_limit(None);
        db.set_type_system_hir_input(None);
        db.set_source_files(vec![]);

        // TODO: should be added theoretically.
        //self.add_implicit_types();

        let file_id = FileId::new();
        let mut sources = db.source_files();
        sources.push(file_id);
        let path: &Path = name.as_ref();
        db.set_input(file_id, Source::schema(path.to_owned(), schema_str));
        db.set_source_files(sources);

        // TODO: ideally, we'd want Subgraph to always represent a valid subgraph: we don't want
        // every possible method that receive a subgraph to have to worry if the underlying schema
        // is actually not a subgraph at all: we want the type-system to help carry known
        // guarantees. This imply we should run validation here (both graphQL ones, but also
        // subgraph specific ones).
        // This also mean we would ideally want `db` to not export any mutable methods (like
        // set_input), but not sure what that entail/how doable that is currently.

        Self {
            name: name.to_string(),
            url: url.to_string(),
            db,
        }
    }

    pub fn parse_and_expand(
        name: &str,
        url: &str,
        schema_str: &str,
    ) -> Result<Self, SubgraphError> {
        let mut compiler = ApolloCompiler::new();
        compiler.add_type_system(schema_str, "schema.graphqls");

        let type_system = compiler.db.type_system();
        let mut imported_federation_definitions: Option<FederationSpecDefinitions> = None;
        let mut imported_link_definitions: Option<LinkSpecDefinitions> = None;
        let link_directives = type_system
            .definitions
            .schema
            .directives_by_name(DEFAULT_LINK_NAME);

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
        let missing_definitions_document = Self::populate_missing_type_definitions(
            &type_system,
            imported_federation_definitions,
            imported_link_definitions,
        )?;
        let missing_definitions = missing_definitions_document.to_string();

        // validate generated schema
        compiler.add_type_system(&missing_definitions, "federation.graphqls");
        let diagnostics = compiler.validate();
        let mut errors = diagnostics.iter().filter(|d| d.data.is_error()).peekable();

        if errors.peek().is_none() {
            Ok(Subgraph::new(
                name,
                url,
                format!("{}\n\n{}", schema_str, missing_definitions).as_str(),
            ))
        } else {
            let errors = errors
                .map(|d| d.to_string())
                .collect::<Vec<String>>()
                .join("");
            Err(SubgraphError { msg: errors })
        }
    }

    fn populate_missing_type_definitions(
        type_system: &Arc<TypeSystem>,
        imported_federation_definitions: Option<FederationSpecDefinitions>,
        imported_link_definitions: Option<LinkSpecDefinitions>,
    ) -> Result<Document, SubgraphError> {
        let mut missing_definitions_document = Document::new();
        let mut schema_extension: Option<SchemaDefinition> = None;
        // populate @link spec definitions
        let link_spec_definitions = match imported_link_definitions {
            Some(definitions) => definitions,
            None => {
                // need to apply default @link directive for link spec on schema
                let defaults = LinkSpecDefinitions::default();
                let mut extension = SchemaDefinition::new();
                extension.directive(defaults.applied_link_directive());
                extension.extend();
                schema_extension = Some(extension);
                defaults
            }
        };
        Self::populate_missing_link_definitions(
            &mut missing_definitions_document,
            type_system,
            link_spec_definitions,
        )?;

        // populate @link federation spec definitions
        let fed_definitions = match imported_federation_definitions {
            Some(definitions) => definitions,
            None => {
                // federation v1 schema or user does not import federation spec
                // need to apply default @link directive for federation spec on schema
                let defaults = FederationSpecDefinitions::default()?;
                let mut extension = match schema_extension {
                    Some(ext) => ext,
                    None => SchemaDefinition::new(),
                };
                extension.directive(defaults.applied_link_directive());
                extension.extend();
                schema_extension = Some(extension);
                defaults
            }
        };
        Self::populate_missing_federation_definitions(
            &mut missing_definitions_document,
            type_system,
            fed_definitions,
        )?;

        // add schema extension if needed
        if let Some(extension) = schema_extension {
            missing_definitions_document.schema(extension);
        }
        Ok(missing_definitions_document)
    }

    fn populate_missing_link_definitions(
        missing_definitions_document: &mut Document,
        type_system: &Arc<TypeSystem>,
        link_spec_definitions: LinkSpecDefinitions,
    ) -> Result<(), SubgraphError> {
        if !type_system
            .type_definitions_by_name
            .contains_key(&link_spec_definitions.purpose_enum_name)
        {
            missing_definitions_document
                .enum_(link_spec_definitions.link_purpose_enum_definition());
        }
        if !type_system
            .type_definitions_by_name
            .contains_key(&link_spec_definitions.import_scalar_name)
        {
            missing_definitions_document.scalar(link_spec_definitions.import_scalar_definition());
        }
        if !type_system
            .definitions
            .directives
            .contains_key(DEFAULT_LINK_NAME)
        {
            missing_definitions_document
                .directive(link_spec_definitions.link_directive_definition());
        }
        Ok(())
    }

    fn populate_missing_federation_definitions(
        missing_definitions_document: &mut Document,
        type_system: &Arc<TypeSystem>,
        fed_definitions: FederationSpecDefinitions,
    ) -> Result<(), SubgraphError> {
        if !type_system
            .type_definitions_by_name
            .contains_key(&fed_definitions.fieldset_scalar_name)
        {
            missing_definitions_document.scalar(fed_definitions.fieldset_scalar_definition());
        }

        for directive_name in FEDERATION_V2_DIRECTIVE_NAMES {
            let namespaced_directive_name =
                fed_definitions.namespaced_type_name(directive_name, true);
            if !type_system
                .type_definitions_by_name
                .contains_key(&namespaced_directive_name)
            {
                let directive_definition = fed_definitions.directive_definition(
                    directive_name,
                    &Some(namespaced_directive_name.to_owned()),
                )?;
                missing_definitions_document.directive(directive_definition);
            }
        }
        Ok(())
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
        let keys = subgraph.db.keys("T".to_string());
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
        assert!(subgraph
            .db
            .type_system()
            .type_definitions_by_name
            .contains_key("T"));
        assert!(subgraph
            .db
            .type_system()
            .definitions
            .directives
            .contains_key("key"));
        assert!(subgraph
            .db
            .type_system()
            .definitions
            .directives
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
        assert!(subgraph
            .db
            .type_system()
            .definitions
            .directives
            .contains_key("myKey"));
        assert!(subgraph
            .db
            .type_system()
            .definitions
            .directives
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
        assert!(subgraph
            .db
            .type_system()
            .definitions
            .directives
            .contains_key("key"));
        assert!(subgraph
            .db
            .type_system()
            .definitions
            .directives
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
            .db
            .type_system()
            .type_definitions_by_name
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
        assert!(subgraph
            .db
            .type_system()
            .type_definitions_by_name
            .contains_key("T"));
        assert!(subgraph
            .db
            .type_system()
            .definitions
            .directives
            .contains_key("key"));
        Ok(())
    }
}
