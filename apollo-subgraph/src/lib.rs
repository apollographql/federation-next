use std::{collections::BTreeMap, path::Path, sync::Arc};

use apollo_compiler::{ApolloCompiler, FileId, HirDatabase, InputDatabase, Source};
use apollo_encoder::{Document, SchemaDefinition};

use apollo_at_link::{
    link::{self, DEFAULT_LINK_NAME},
    spec::Identity,
};
#[allow(unused)]
use database::{SubgraphDatabase, SubgraphRootDatabase};

use crate::spec::{FederationSpecDefinitions, LinkSpecDefinitions, FEDERATED_DIRECTIVE_NAMES};

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

        let mut missing_definitions_document = Document::new();

        let mut link_directive_applied_on_schema: bool = false;
        let mut imports_link_spec: bool = false;
        let mut imports_federation_spec: bool = false;

        let type_system = compiler.db.type_system();
        let mut link_spec_definitions = LinkSpecDefinitions::default();
        let link_directives = type_system
            .definitions
            .schema
            .directives_by_name(DEFAULT_LINK_NAME);
        for directive in link_directives {
            link_directive_applied_on_schema = true;

            let link_directive = match link::Link::from_directive_application(directive) {
                Ok(link) => link,
                Err(e) => return Err(SubgraphError { msg: e.to_string() }),
            };
            if link_directive
                .url
                .identity
                .eq(&Identity::federation_identity())
            {
                if imports_federation_spec {
                    return Err(SubgraphError { msg: "invalid graphql schema - multiple @link imports for the federation specification are not supported".to_owned() });
                } else {
                    imports_federation_spec = true;
                }

                let federation_definitions = match FederationSpecDefinitions::new(link_directive) {
                    Ok(definitions) => definitions,
                    Err(e) => return Err(SubgraphError { msg: e.to_string() }),
                };
                if !type_system
                    .type_definitions_by_name
                    .contains_key(&federation_definitions.fieldset_scalar_name())
                {
                    missing_definitions_document
                        .scalar(federation_definitions.fieldset_scalar_definition());
                }

                for directive_name in FEDERATED_DIRECTIVE_NAMES {
                    let namespaced_directive_name =
                        federation_definitions.federated_directive_name(directive_name);
                    if !type_system
                        .type_definitions_by_name
                        .contains_key(&namespaced_directive_name)
                    {
                        let directive_definition = match federation_definitions
                            .federated_directive_definition(
                                directive_name.to_owned(),
                                &Some(namespaced_directive_name.to_owned()),
                            ) {
                            Ok(definition) => definition,
                            Err(e) => return Err(SubgraphError { msg: e.to_string() }),
                        };
                        missing_definitions_document.directive(directive_definition);
                    }
                }
            } else if link_directive.url.identity.eq(&Identity::link_identity()) {
                if imports_link_spec {
                    return Err(SubgraphError { msg: "invalid graphql schema - multiple @link imports for the link specification are not supported".to_owned() });
                } else {
                    imports_link_spec = true;
                }

                link_spec_definitions = LinkSpecDefinitions {
                    link: link_directive,
                };
            }
        }

        if link_directive_applied_on_schema {
            if !type_system
                .type_definitions_by_name
                .contains_key(&link_spec_definitions.link_purpose_enum_name())
            {
                missing_definitions_document
                    .enum_(link_spec_definitions.link_purpose_enum_definition());
            }
            if !type_system
                .type_definitions_by_name
                .contains_key(&link_spec_definitions.import_scalar_name())
            {
                missing_definitions_document
                    .scalar(link_spec_definitions.import_scalar_definition());
            }
            if !type_system
                .definitions
                .directives
                .contains_key(DEFAULT_LINK_NAME)
            {
                missing_definitions_document
                    .directive(link_spec_definitions.link_directive_definition());
            }

            if !imports_link_spec {
                // need to apply @link directive on schema extension
                let mut schema_extension = SchemaDefinition::new();
                schema_extension.directive(link_spec_definitions.applied_link_directive());
                schema_extension.extend();
                missing_definitions_document.schema(schema_extension);
            }
        }

        let missing_definitions = missing_definitions_document.to_string();

        // validate generated schema
        compiler.add_type_system(&missing_definitions, "federation.graphql");
        let diagnostics = compiler.validate();
        let mut errors = diagnostics.iter().filter(|d| d.data.is_error()).peekable();

        return if errors.peek().is_none() {
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
        };
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

        let subgraph = match Subgraph::parse_and_expand("S1", "http://s1", schema) {
            Ok(graph) => graph,
            Err(e) => {
                println!("{}", e.msg);
                return Err(String::from(
                    "failed to parse and expand the subgraph, see errors above for details",
                ));
            }
        };
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
            t: T
        }

        type T @key(fields: "id") {
            id: ID!
            x: Int
        }
        "#;

        let subgraph = match Subgraph::parse_and_expand("S1", "http://s1", schema) {
            Ok(graph) => graph,
            Err(e) => {
                println!("{}", e.msg);
                return Err(String::from(
                    "failed to parse and expand the subgraph, see errors above for details",
                ));
            }
        };
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
            .contains_key("myKey"));
        assert!(subgraph
            .db
            .type_system()
            .definitions
            .directives
            .contains_key("federation__requires"));
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

        let subgraph = match Subgraph::parse_and_expand("S1", "http://s1", schema) {
            Ok(graph) => graph,
            Err(e) => {
                println!("{}", e.msg);
                return Err(String::from(
                    "failed to parse and expand the subgraph, see errors above for details",
                ));
            }
        };
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
            .contains_key("fed__requires"));
        Ok(())
    }

    #[test]
    fn can_parse_and_expand_preserves_user_definitions() -> Result<(), String> {
        let schema = r#"
        extend schema
          @link(url: "https://specs.apollo.dev/link/v1.0", import: ["Import", "Purpose"])
          @link(url: "https://specs.apollo.dev/federation/v2.3", import: [ "@key" ], as: "fed" )

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

        let subgraph = match Subgraph::parse_and_expand("S1", "http://s1", schema) {
            Ok(graph) => graph,
            Err(e) => {
                println!("{}", e.msg);
                return Err(String::from(
                    "failed to parse and expand the subgraph, see errors above for details",
                ));
            }
        };
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
}
