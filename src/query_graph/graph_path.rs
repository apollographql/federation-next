use crate::error::FederationError;
use crate::link::graphql_definition::{DeferDirectiveArguments, OperationConditional};
use crate::query_graph::path_tree::OpPathTree;
use crate::query_graph::QueryGraph;
use crate::query_plan::operation::normalized_field_selection::NormalizedField;
use crate::query_plan::operation::normalized_inline_fragment_selection::NormalizedInlineFragment;
use crate::query_plan::operation::NormalizedSelectionSet;
use crate::query_plan::QueryPlanCost;
use crate::schema::position::ObjectTypeDefinitionPosition;
use indexmap::IndexSet;
use petgraph::graph::{EdgeIndex, NodeIndex};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// An immutable path in a query graph.
///
/// A "path" here is mostly understood in the graph-theoretical sense of the term, i.e. as "a
/// connected series of edges"; a `GraphPath` is generated by traversing a query graph.
///
/// However, as query graph edges may have conditions, a `GraphPath` also records, for each edge it
/// is composed of, the set of paths (an `OpPathTree` in practice) that were taken to fulfill each
/// edge's conditions (when an edge has one).
///
/// Additionally, for each edge of the path, a `GraphPath` records the "trigger" that made the
/// traversal take that edge. In practice, the "trigger" can be seen as a way to decorate a path
/// with some additional metadata for each element of the path. In practice, that trigger is used in
/// 2 main ways (corresponding to our 2 main query graph traversals):
/// - For composition validation, the traversal of the federated query graph is driven by other
///   transitions into the supergraph API query graph (essentially, composition validation is about
///   finding, for every path in supergraph API query graph, a "matching" traversal of the federated
///   query graph). In that case, for the graph paths we build on the federated query graph, the
///   "trigger" will be one of the edge transitions from the supergraph API query graph (which,
///   granted, will be fairly similar to the one of the edge we're taking in the federated query
///   graph; in practice, triggers are more useful in the query planning case).
/// - For query planning, the traversal of the federated query graph is driven by the elements of
///   the query we are planning. Which means that the "trigger" for taking an edge in this case will
///   be an operation element (or `None`). See the specialized `OpGraphPath` that is defined for this
///   use case.
///
/// Lastly, some `GraphPath`s can actually encode `None` edges: this is used during query planning
/// in the (rare) case where the query we plan for has an inline fragment spread without type
/// condition (or a "useless" one, i.e. one that doesn't restrict the possible types anymore than
/// they already were) but with some directives. In that case, we want to preserve the information
/// about the directive (to properly rebuild query plans later) but it doesn't correspond to taking
/// any edges, so we add a `None` edge and use the trigger to store the fragment spread.
///
/// Regarding type parameters:
/// - `TTrigger`: The type of the path's "triggers", metadata that can associated to each element
///   of the path (see above for more details).
/// - `TEdge`: The type of the edge. Either `Option<EdgeIndex>` (meaning that the path may have a
///   `None` edge) or `never` (the path cannot have `None` edges).
// PORT_NOTE: The JS codebase also parameterized whether the head of the path was a root node, but
// in the Rust code we don't have a distinguished type for that case. We instead check this at
// runtime (introducing the new field `head_must_be_root`). This means the `RootPath` type in the
// JS codebase is replaced with this one.
#[derive(Debug, Clone)]
pub(crate) struct GraphPath<TTrigger, TEdge>
where
    TTrigger: Eq + Hash,
    TEdge: Copy + Into<Option<EdgeIndex>>,
{
    /// The query graph of which this is a path.
    graph: Arc<QueryGraph>,
    /// The node at which the path starts. This should be the head of the first non-`None` edge in
    /// the path if such edge exists, but if there are only `None` edges (or if there are zero
    /// edges), this will still exist (and the head and tail of the path will be the same).
    head: NodeIndex,
    /// Whether the head must be a root node for this path.
    head_must_be_root: bool,
    /// The node at which the path stops. This should be the tail of the last non-`None` edge in the
    /// path if such edge exists, but if there are only `None` edges (or if there are zero edges),
    /// this will still exist (and the head and tail of the path will be the same).
    tail: NodeIndex,
    /// The edges composing the path.
    edges: Vec<TEdge>,
    /// The triggers associated to each edge in the path.
    edge_triggers: Vec<Arc<TTrigger>>,
    /// For each edge in the path, if the edge has conditions, the set of paths that fulfill that
    /// condition.
    ///
    /// Note that no matter which kind of traversal we are doing (composition or query planning),
    /// fulfilling the conditions is always driven by the conditions themselves, and since
    /// conditions are a GraphQL result set, the resulting set of paths are an `OpGraphPath` (and
    /// since they start at the edge's head node, we use the `OpPathTree` representation for that
    /// set of paths).
    edge_conditions: Vec<Option<Arc<OpPathTree>>>,
    /// Information about the last subgraph-entering edge in this path, which is used to eliminate
    /// some non-optimal paths. (This is reset when encountering a `@defer` application.)
    last_subgraph_entering_edge_info: Option<SubgraphEnteringEdgeInfo>,
    /// As part of an optimization, we keep track of when one path "overrides" other paths by
    /// creating an ID, and storing that ID in the paths to track the "overrides" relationship (not
    /// to be confused with the `@override` directive, which is completely separate).
    ///
    /// This array stores the IDs associated with this path.
    own_path_ids: Arc<IndexSet<u64>>,
    /// This array stores the IDs of paths that override this one. (See docs for `own_path_ids` for
    /// more info).
    overriding_path_ids: Arc<IndexSet<u64>>,
    /// Names of all the possible runtime types the tail of the path can be.
    runtime_types_of_tail: Vec<ObjectTypeDefinitionPosition>,
    /// If the last edge in the `edges` array was a `DownCast` transition, then the runtime types
    /// before that edge.
    runtime_types_before_tail_if_last_is_cast: Option<Vec<ObjectTypeDefinitionPosition>>,
    /// If the trigger of the last edge in the `edges` array was an operation element with a
    /// `@defer` application, then the arguments of that application.
    defer_on_tail: Option<DeferDirectiveArguments>,
}

#[derive(Debug, Clone)]
pub(crate) struct SubgraphEnteringEdgeInfo {
    /// The index within the `edges` array.
    index: usize,
    /// The cost of resolving the conditions for this edge.
    conditions_cost: QueryPlanCost,
}

/// The item type for [`GraphPath::iter`]
pub(crate) type GraphPathItem<'path, TTrigger, TEdge> =
    (TEdge, &'path Arc<TTrigger>, &'path Option<Arc<OpPathTree>>);

/// A `GraphPath` whose triggers are operation elements (essentially meaning that the path has been
/// guided by a GraphQL operation).
// PORT_NOTE: As noted in the docs for `GraphPath`, we omit a type parameter for the root node,
// whose constraint is instead checked at runtime. This means the `OpRootPath` type in the JS
// codebase is replaced with this one.
pub(crate) type OpGraphPath = GraphPath<OpGraphPathTrigger, Option<EdgeIndex>>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum OpGraphPathTrigger {
    Field(NormalizedField),
    InlineFragment(NormalizedInlineFragment),
    Context(OpGraphPathContext),
}

/// A path of operation elements within a GraphQL operation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct OpPath(Vec<Arc<OpPathElement>>);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum OpPathElement {
    Field(NormalizedField),
    InlineFragment(NormalizedInlineFragment),
}

/// Records, as we walk a path within a GraphQL operation, important directives encountered
/// (currently `@include` and `@skip` with their conditions).
#[derive(Debug, Clone, Eq, Default)]
pub(crate) struct OpGraphPathContext {
    /// A list of conditionals (e.g. `[{ kind: Include, value: true}, { kind: Skip, value: $foo }]`)
    /// in the reverse order in which they were applied (so the first element is the inner-most
    /// applied include/skip).
    conditionals: Vec<Arc<OperationConditional>>,
}

impl PartialEq for OpGraphPathContext {
    fn eq(&self, _other: &Self) -> bool {
        todo!()
    }
}

impl Hash for OpGraphPathContext {
    fn hash<H: Hasher>(&self, _state: &mut H) {
        todo!()
    }
}

/// A vector of graph paths that are being considered simultaneously by the query planner as an
/// option for a path within a GraphQL operation. These arise since the edge to take in a query
/// graph may depend on outcomes that are only known at query plan execution time, and we account
/// for this by splitting a path into multiple paths (one for each possible outcome). The common
/// example is abstract types, where we may end up taking a different edge depending on the runtime
/// type (e.g. during type explosion).
pub(crate) struct SimultaneousPaths(pub(crate) Vec<Arc<OpGraphPath>>);

/// One of the options for an `OpenBranch` (see the documentation of that struct for details). This
/// includes
pub(crate) struct SimultaneousPathsWithLazyIndirectPaths {
    paths: SimultaneousPaths,
    context: OpGraphPathContext,
}

/// One of the options for a `ClosedBranch` (see the documentation of that struct for details). Note
/// there is an optimization here, in that if some ending section of the path within the GraphQL
/// operation can be satisfied by a query to a single subgraph, then we just record that selection
/// set, and the `SimultaneousPaths` ends at the node at which that query is made instead of a node
/// for the leaf field. The selection set gets copied "as-is" into the `FetchNode`, and also avoids
/// extra `GraphPath` creation and work during `PathTree` merging.
pub(crate) struct ClosedPath {
    pub(crate) paths: SimultaneousPaths,
    pub(crate) selection_set: Option<Arc<NormalizedSelectionSet>>,
}

/// A list of the options generated during query planning for a specific "closed branch", which is a
/// full/closed path in a GraphQL operation (i.e. one that ends in a leaf field).
pub(crate) struct ClosedBranch(pub(crate) Vec<Arc<ClosedPath>>);

/// A list of the options generated during query planning for a specific "open branch", which is a
/// partial/open path in a GraphQL operation (i.e. one that does not end in a leaf field).
pub(crate) struct OpenBranch(Vec<SimultaneousPathsWithLazyIndirectPaths>);

impl<TTrigger, TEdge> GraphPath<TTrigger, TEdge>
where
    TTrigger: Eq + Hash,
    TEdge: Copy + Into<Option<EdgeIndex>>,
{
    pub(crate) fn iter(&self) -> impl Iterator<Item = GraphPathItem<'_, TTrigger, TEdge>> {
        debug_assert_eq!(self.edges.len(), self.edge_triggers.len());
        debug_assert_eq!(self.edges.len(), self.edge_conditions.len());
        self.edges
            .iter()
            .copied()
            .zip(&self.edge_triggers)
            .zip(&self.edge_conditions)
            .map(|((edge, trigger), condition)| (edge, trigger, condition))
    }
}

impl OpGraphPath {
    pub(crate) fn is_overridden_by(&self, other: &Self) -> bool {
        self.overriding_path_ids
            .iter()
            .any(|overriding_id| other.own_path_ids.contains(overriding_id))
    }

    pub(crate) fn subgraph_jumps(&self) -> Result<u32, FederationError> {
        self.subgraph_jumps_at_idx(0)
    }

    pub(crate) fn subgraph_jumps_at_idx(&self, start_index: usize) -> Result<u32, FederationError> {
        self.edges[start_index..]
            .iter()
            .flatten()
            .try_fold(0, |sum, &edge_index| {
                let (start, end) = self.graph.edge_endpoints(edge_index)?;
                let start = self.graph.node_weight(start)?;
                let end = self.graph.node_weight(end)?;
                let changes_subgraph = start.source != end.source;
                Ok(sum + if changes_subgraph { 1 } else { 0 })
            })
    }
}
