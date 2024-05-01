use std::{collections::HashMap, sync::Arc};

use apollo_compiler::{execution::GraphQLError, NodeStr};

use crate::query_graph::QueryGraph;

use super::{diagnostics::CompositionHint, ValidationState};

type TODO = usize;

pub(super) struct ValidationTraversal {}

impl ValidationTraversal {
    pub(super) fn new(
        _supergraph_schema: TODO, // Schema
        _supergraph_api: Arc<QueryGraph>,
        _federated_query_graph: Arc<QueryGraph>,
    ) -> Self {
        Self {}
    }

    pub(super) fn validate(
        &mut self,
    ) -> Result<Vec<CompositionHint>, (Vec<GraphQLError>, Vec<CompositionHint>)> {
        todo!()
    }

    fn handle_state(&mut self, _state: &ValidationState) {}
}

struct VertexVisit {
    subgraphs: Vec<NodeStr>,
    override_conditions: HashMap<NodeStr, bool>,
}

/// `maybe_superset` is a superset (or equal) if it contains all of `other`'s
/// subgraphs and all of `other`'s labels (with matching conditions).
fn is_superset_or_equal(maybe_superset: &VertexVisit, other: &VertexVisit) -> bool {
    let include_all_subgraphs = other
        .subgraphs
        .iter()
        .all(|subgraph| maybe_superset.subgraphs.contains(subgraph));

    let includes_all_override_conditions =
        other.override_conditions.iter().all(|(label, condition)| {
            maybe_superset
                .override_conditions
                .get(label)
                .map_or(false, |c| c == condition)
        });

    include_all_subgraphs && includes_all_override_conditions
}
