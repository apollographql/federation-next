use std::{collections::BTreeMap, path::Path, sync::Arc};

use apollo_compiler::{FileId, InputDatabase, Source};
#[allow(unused)]
use database::{SubgraphDatabase, SubgraphRootDatabase};

mod database;

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
}
