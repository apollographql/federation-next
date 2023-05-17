use std::sync::Arc;

use apollo_at_link::{
    database::{AtLinkDatabase, AtLinkStorage},
    link::Link,
    spec::{Identity, APOLLO_SPEC_DOMAIN}
};
use apollo_compiler::{
    database::{db::Upcast, AstStorage, HirStorage, InputStorage},
    HirDatabase
};
use apollo_subgraph::Subgraphs;

use crate::SupergraphError;

// TODO: we should define this as part as some more generic "JoinSpec" definition, but need
// to define the ground work for that in `apollo-at-link` first.
pub fn join_link_identity() -> Identity {
    Identity {
        domain: APOLLO_SPEC_DOMAIN.to_string(),
        name: "join".to_string(),
    }
}

#[salsa::query_group(SupergraphStorage)]
pub trait SupergraphDatabase: AtLinkDatabase + HirDatabase {
    fn join_link(&self) -> Arc<Link>;

    // TODO: this currently either _has to_ be transparent, or we can implement `salsa::ParallelDatabase` below because
    // `Subgraph` is not `Sync` due to the underlying `db` and would need to be
    // (`SubgraphRootDatabase` does implement `salsa::ParallelDatabase` but I think that only makes
    // it `Send`, not `Sync`). Need to figure this all out (not that having this transparent is
    // necessarily a huge deal per se; but having subgraphs not `Sync` may ultimately be, at least
    // if we want the query planner to be `Sync`). We may need to use snapshots somewhere ...
    #[salsa::transparent]
    fn extract_subgraphs(&self) -> Result<Subgraphs, SupergraphError>;
}

fn join_link(db: &dyn SupergraphDatabase) -> Arc<Link> {
    db.links_metadata()
        .for_identity(&join_link_identity())
        .expect("The presence of the join link should have been validated on construction")
}

fn extract_subgraphs(_db: &dyn SupergraphDatabase) -> Result<Subgraphs, SupergraphError> {
    // TODO
    Ok(Subgraphs::new())
}


#[salsa::database(InputStorage, AstStorage, HirStorage, AtLinkStorage, SupergraphStorage)]
#[derive(Default)]
pub struct SupergraphRootDatabase {
    pub storage: salsa::Storage<SupergraphRootDatabase>,
}

impl salsa::Database for SupergraphRootDatabase {}

impl salsa::ParallelDatabase for SupergraphRootDatabase {
    fn snapshot(&self) -> salsa::Snapshot<SupergraphRootDatabase> {
        salsa::Snapshot::new(SupergraphRootDatabase {
            storage: self.storage.snapshot(),
        })
    }
}

impl Upcast<dyn HirDatabase> for SupergraphRootDatabase {
    fn upcast(&self) -> &(dyn HirDatabase + 'static) {
        self
    }
}
