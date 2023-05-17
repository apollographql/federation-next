use std::sync::Arc;

use apollo_at_link::{
    database::{AtLinkDatabase, AtLinkStorage},
    link::Link,
    spec::{Identity, APOLLO_SPEC_DOMAIN}
};
use apollo_compiler::{
    database::{db::Upcast, AstStorage, HirStorage, InputStorage},
    hir::{SelectionSet, Directive, Value}, HirDatabase
};

// TODO: we should define this as part as some more generic "FederationSpec" definition, but need
// to define the ground work for that in `apollo-at-link` first.
pub fn federation_link_identity() -> Identity {
    Identity {
        domain: APOLLO_SPEC_DOMAIN.to_string(),
        name: "federation".to_string(),
    }
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct Key {
    pub type_name: String,
    // TODO: this should _not_ be an Option below; but we don't know how to build the SelectionSet,
    // so until we have a solution, we use None to have code that compiles. 
    selections: Option<Arc<SelectionSet>>,
}

impl Key {
    // TODO: same remark as above: not meant to be `Option`
    pub fn selections(&self) -> Option<Arc<SelectionSet>> {
        return self.selections.clone();
    }

    pub(crate) fn from_directive_application(type_name: &str, directive: &Directive) -> Option<Key> {
        let fields_arg = directive
            .arguments()
            .iter()
            .find(|arg| arg.name() == "fields")
            .map(|arg| arg.value());
        if let Some(Value::String(_value)) = fields_arg {
            Some(Key {
                type_name: type_name.to_string(),
                // TODO: obviously not what we want.
                selections: None
            })
        } else {
            None
        }
    }
}

/// Database used for valid federation 2 subgraphs.
///
/// Note: technically, federation 1 subgraphs are still accepted as input of
/// composition. However, there is some pre-composition steps that "massage" 
/// the input schema to transform them in fully valid federation 2 subgraphs,
/// so the subgraphs seen by composition and query planning are always fully
/// valid federation 2 ones, and this is what this database handles.
/// Note2: This does assumes that whichever way an implementation of this
/// trait is created, some validation that the underlying schema is a valid
/// federation subgraph (so valid graphql, link to the federation spec, and
/// pass additional federation validations). If this is not the case, most
/// of the methods here will panic.
#[salsa::query_group(SubgraphStorage)]
pub trait SubgraphDatabase: AtLinkDatabase + HirDatabase {
    fn federation_link(&self) -> Arc<Link>;

    /// The name of the @key directive in this subgraph.
    /// This will either return 'federation__key' if the `@key` directive is not imported,
    /// or whatever never it is imported under otherwise. Commonly, this would just be `key`.
    fn key_directive_name(&self) -> String;

    fn keys(&self, type_name: String) -> Vec<Key>;
}

fn federation_link(db: &dyn SubgraphDatabase) -> Arc<Link> {
    db.links_metadata()
        .for_identity(&federation_link_identity())
        .expect("The presence of the federation link should have been validated on construction")
}

fn key_directive_name(db: &dyn SubgraphDatabase) -> String {
    db.federation_link().directive_name_in_schema("key")

}

fn keys(db: &dyn SubgraphDatabase, type_name: String) -> Vec<Key> {
    let key_name = db.key_directive_name();
    if let Some(type_def) = db.find_type_definition_by_name(type_name.clone()) {
        type_def.directives_by_name(&key_name).filter_map(|directive| Key::from_directive_application(&type_name, directive)).collect()
    } else {
        vec![]
    }
}

#[salsa::database(InputStorage, AstStorage, HirStorage, AtLinkStorage, SubgraphStorage)]
#[derive(Default)]
pub struct SubgraphRootDatabase {
    pub storage: salsa::Storage<SubgraphRootDatabase>,
}

impl salsa::Database for SubgraphRootDatabase {}

impl salsa::ParallelDatabase for SubgraphRootDatabase {
    fn snapshot(&self) -> salsa::Snapshot<SubgraphRootDatabase> {
        salsa::Snapshot::new(SubgraphRootDatabase {
            storage: self.storage.snapshot(),
        })
    }
}

impl Upcast<dyn HirDatabase> for SubgraphRootDatabase {
    fn upcast(&self) -> &(dyn HirDatabase + 'static) {
        self
    }
}
