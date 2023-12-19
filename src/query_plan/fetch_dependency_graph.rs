use super::operation::NormalizedSelection;
use crate::error::FederationError;
use crate::query_graph::graph_path::OpPath;
use crate::query_graph::QueryGraph;
use crate::query_plan::conditions::Conditions;
use crate::query_plan::fetch_dependency_graph_processor::RebasedFragments;
use crate::query_plan::operation::NormalizedSelectionSet;
use crate::query_plan::query_planner::QueryPlannerConfig;
use crate::query_plan::{FetchDataPathElement, FetchDataRewrite, QueryPlanCost};
use crate::schema::position::{CompositeTypeDefinitionPosition, SchemaRootDefinitionKind};
use crate::schema::ValidFederationSchema;
use apollo_compiler::executable::VariableDefinition;
use apollo_compiler::{Node, NodeStr};
use indexmap::{IndexMap, IndexSet};
use petgraph::stable_graph::{NodeIndex, StableDiGraph};
use std::sync::Arc;

/// Represents a subgraph fetch of a query plan.
// PORT_NOTE: The JS codebase called this `FetchGroup`, but this naming didn't make it apparent that
// this was a node in a fetch dependency graph, so we've renamed it accordingly.
//
// The JS codebase additionally has a property named `subgraphAndMergeAtKey` that was used as a
// precomputed map key, but this isn't necessary in Rust since we can use `PartialEq`/`Eq`/`Hash`.
#[derive(Debug, Clone)]
pub(crate) struct FetchDependencyGraphNode {
    /// The subgraph this fetch is queried against.
    pub(crate) subgraph_name: NodeStr,
    /// Which root operation kind the fetch should have.
    root_kind: SchemaRootDefinitionKind,
    /// The parent type of the fetch's selection set. For fetches against the root, this is the
    /// subgraph's root operation type for the corresponding root kind, but for entity fetches this
    /// will be the subgraph's entity union type.
    parent_type: CompositeTypeDefinitionPosition,
    /// The selection set to be fetched from the subgraph, along with memoized conditions.
    selection_set: FetchSelectionSet,
    /// Whether this fetch uses the federation `_entities` field and correspondingly is against the
    /// subgraph's entity union type (sometimes called a "key" fetch).
    is_entity_fetch: bool,
    /// The inputs to be passed into `_entities` field, if this is an entity fetch.
    inputs: Arc<FetchInputs>,
    /// Input rewrites for query plan execution to perform prior to executing the fetch.
    input_rewrites: Arc<Vec<Arc<FetchDataRewrite>>>,
    /// As query plan execution runs, it accumulates fetch data into a response object. This is the
    /// path at which to merge in the data for this particular fetch.
    merge_at: Arc<Vec<Arc<FetchDataPathElement>>>,
    /// The fetch ID generation, if one is necessary (used when handling `@defer`).
    id: Option<u64>,
    /// The label of the `@defer` block this fetch appears in, if any.
    defer_ref: Option<NodeStr>,
    /// The cached computation of this fetch's cost, if it's been done already.
    cached_cost: Option<QueryPlanCost>,
    /// Set in some code paths to indicate that the selection set of the group should not be
    /// optimized away even if it "looks" useless.
    must_preserve_selection_set: bool,
    /// If true, then we skip an expensive computation during `is_useless()`. (This partially
    /// caches that computation.)
    is_known_useful: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct FetchSelectionSet {
    /// The selection set to be fetched from the subgraph.
    pub(crate) selection_set: Arc<NormalizedSelectionSet>,
    /// The conditions determining whether the fetch should be executed (which must be recomputed
    /// from the selection set when it changes).
    pub(crate) conditions: Arc<Conditions>,
}

// PORT_NOTE: The JS codebase additionally has a property `onUpdateCallback`. This was only ever
// used to update `isKnownUseful` in `FetchGroup`, and it's easier to handle this there than try
// to pass in a callback in Rust.
#[derive(Debug, Clone)]
pub(crate) struct FetchInputs {
    /// The selection sets to be used as input to `_entities`, separated per parent type.
    selection_sets_per_parent_type:
        IndexMap<CompositeTypeDefinitionPosition, Arc<NormalizedSelectionSet>>,
    /// The supergraph schema (primarily used for validation of added selection sets).
    supergraph_schema: ValidFederationSchema,
}

/// Represents a dependency between two subgraph fetches, namely that the tail/child depends on the
/// head/parent executing first.
#[derive(Debug, Clone)]
pub(crate) struct FetchDependencyGraphEdge {
    /// The operation path of the tail/child _relative_ to the head/parent. This information is
    /// maintained in case we want/need to merge groups into each other. This can roughly be thought
    /// of similarly to `merge_at` in the child, but is relative to the start of the parent. It can
    /// be `None`, which either means we don't know the relative path, or that the concept of a
    /// relative path doesn't make sense in this context. E.g. there is case where a child's
    /// `merge_at` can be shorter than its parent's, in which case the `path` (which is essentially
    /// `child.merge_at - parent.merge_at`), does not make sense (or rather, it's negative, which we
    /// cannot represent). The gist is that `None` for the `path` means that no assumption should be
    /// made, and that any merge logic using said path should bail.
    path: Option<Arc<OpPath>>,
}

/// A directed acyclic graph (DAG) of fetches (a.k.a. fetch groups) and their dependencies.
///
/// In the graph, two fetches are connected if one of them (the parent/head) must be performed
/// strictly before the other one (the child/tail).
#[derive(Debug, Clone)]
pub(crate) struct FetchDependencyGraph {
    /// The supergraph schema that generated the federated query graph.
    supergraph_schema: ValidFederationSchema,
    /// The federated query graph that generated the fetches. (This also contains the subgraph
    /// schemas.)
    federated_query_graph: Arc<QueryGraph>,
    /// The nodes/edges of the fetch dependency graph. Note that this must be a stable graph since
    /// we remove nodes/edges during optimizations.
    graph: StableDiGraph<Arc<FetchDependencyGraphNode>, Arc<FetchDependencyGraphEdge>>,
    /// The root nodes by subgraph name, representing the fetches against root operation types of
    /// the subgraphs.
    root_nodes_by_subgraph: Arc<IndexMap<NodeStr, IndexSet<NodeIndex>>>,
    /// Tracks metadata about deferred blocks and their dependencies on one another.
    defer_tracking: Arc<DeferTracking>,
    /// The initial fetch ID generation (used when handling `@defer`).
    starting_id_generation: u64,
    /// The current fetch ID generation (used when handling `@defer`).
    fetch_id_generation: u64,
    /// Whether this fetch dependency graph has undergone a transitive reduction.
    is_reduced: bool,
    /// Whether this fetch dependency graph has undergone optimization (e.g. transitive reduction,
    /// removing empty/useless fetches, merging fetches with the same subgraph/path).
    is_optimized: bool,
}

// TODO: Write docstrings
#[derive(Debug, Clone)]
pub(crate) struct DeferTracking {
    top_level_deferred: IndexSet<NodeStr>,
    deferred: IndexMap<NodeStr, Vec<DeferredInfo>>,
    primary_selection: Option<Arc<NormalizedSelectionSet>>,
}

// TODO: Write docstrings
#[derive(Debug, Clone)]
pub(crate) struct DeferredInfo {
    pub(crate) label: NodeStr,
    pub(crate) path: FetchDependencyGraphPath,
    pub(crate) sub_selection: NormalizedSelectionSet,
    pub(crate) deferred: IndexSet<NodeStr>,
    pub(crate) dependencies: IndexSet<NodeStr>,
}

// TODO: Write docstrings
#[derive(Debug, Clone)]
pub(crate) struct FetchDependencyGraphPath {
    pub(crate) full_path: OpPath,
    pub(crate) path_in_node: OpPath,
    pub(crate) response_path: Vec<FetchDataPathElement>,
}

impl FetchDependencyGraphNode {
    pub(crate) fn cost(&mut self) -> Result<QueryPlanCost, FederationError> {
        if self.cached_cost.is_none() {
            self.cached_cost = Some(self.selection_set.selection_set.cost(1)?)
        }
        Ok(self.cached_cost.unwrap())
    }

    // TODO: https://github.com/apollographql/federation/blob/f69a0694b95/query-planner-js/src/buildPlan.ts#L1518-L1573
    pub(crate) fn to_plan_node(
        &self,
        _config: &QueryPlannerConfig,
        _handled_conditions: &Conditions,
        _variable_definitions: &[Node<VariableDefinition>],
        _fragments: Option<&RebasedFragments>,
        _op_name: Option<String>,
    ) -> Option<super::PlanNode> {
        todo!()
    }
}

impl NormalizedSelectionSet {
    pub(crate) fn cost(&self, depth: QueryPlanCost) -> Result<QueryPlanCost, FederationError> {
        // The cost is essentially the number of elements in the selection,
        // but we make deep element cost a tiny bit more,
        // mostly to make things a tad more deterministic
        // (typically, if we have an interface with a single implementation,
        // then we can have a choice between a query plan that type-explode a field of the interface
        // and one that doesn't, and both will be almost identical,
        // except that the type-exploded field will be a different depth;
        // by favoring lesser depth in that case, we favor not type-exploding).
        self.selections.values().try_fold(0, |sum, selection| {
            let subselections = match selection {
                NormalizedSelection::Field(field) => field.selection_set.as_ref(),
                NormalizedSelection::InlineFragment(inline) => Some(&inline.selection_set),
                NormalizedSelection::FragmentSpread(_) => {
                    return Err(FederationError::internal(
                        "unexpected fragment spread in FetchDependencyGraphNode",
                    ))
                }
            };
            let subselections_cost = if let Some(selection_set) = subselections {
                selection_set.cost(depth + 1)?
            } else {
                0
            };
            Ok(sum + depth + subselections_cost)
        })
    }
}
