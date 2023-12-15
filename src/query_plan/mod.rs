use apollo_compiler::executable::{
    Field, InlineFragment, Name, OperationType, Selection, SelectionSet,
};
use apollo_compiler::validation::Valid;
use apollo_compiler::{ExecutableDocument, NodeStr};
use std::sync::Arc;

pub mod generate;
pub mod operation;
pub mod query_planner;

pub struct QueryPlan {
    node: Option<TopLevelPlanNode>,
}

pub enum TopLevelPlanNode {
    Subscription(SubscriptionNode),
    Fetch(FetchNode),
    Sequence(SequenceNode),
    Parallel(ParallelNode),
    Flatten(FlattenNode),
    Defer(DeferNode),
    Condition(ConditionNode),
}

pub struct SubscriptionNode {
    primary: FetchNode,
    rest: Option<PlanNode>,
}

pub enum PlanNode {
    Fetch(Arc<FetchNode>),
    Sequence(Arc<SequenceNode>),
    Parallel(Arc<ParallelNode>),
    Flatten(Arc<FlattenNode>),
    Defer(Arc<DeferNode>),
    Condition(Arc<ConditionNode>),
}

pub struct FetchNode {
    service_name: NodeStr,
    /// Optional identifier for the fetch for defer support. All fetches of a given plan will be
    /// guaranteed to have a unique `id`.
    id: Option<NodeStr>,
    /// If query planner defer support is enabled _and_ the subgrpah named `service_name` supports
    /// defer, then this boolean says whether `operation` contains some @defer. Unset otherwise.
    has_defers: Option<bool>,
    variable_usages: Vec<Name>,
    /// `Selection`s in apollo-rs _can_ have a `FragmentSpread`, but this `Selection` is
    /// specifically typing the `requires` key in a built query plan, where there can't be
    /// `FragmentSpread`.
    // PORT_NOTE: This was its own type in the JS codebase, but it's likely simpler to just have the
    // constraint be implicit for router instead of creating a new type.
    requires: Vec<Selection>,
    // PORT_NOTE: We don't serialize the "operation" string in this struct, as these query plan
    // nodes are meant for direct consumption by router (without any serdes), so we leave the
    // question of whether it needs to be serialized to router.
    operation_document: Valid<ExecutableDocument>,
    operation_name: Option<NodeStr>,
    operation_kind: OperationType,
    /// Optionally describe a number of "rewrites" that query plan executors should apply to the
    /// data that is sent as the input of this fetch. Note that such rewrites should only impact the
    /// inputs of the fetch they are applied to (meaning that, as those inputs are collected from
    /// the current in-memory result, the rewrite should _not_ impact said in-memory results, only
    /// what is sent in the fetch).
    input_rewrites: Vec<FetchDataRewrite>,
    /// Similar to `input_rewrites`, but for optional "rewrites" to apply to the data that is
    /// received from a fetch (and before it is applied to the current in-memory results).
    output_rewrites: Vec<FetchDataRewrite>,
}

pub struct SequenceNode {
    nodes: Vec<PlanNode>,
}

pub struct ParallelNode {
    nodes: Vec<PlanNode>,
}

pub struct FlattenNode {
    path: Vec<FetchDataPathElement>,
    node: PlanNode,
}

/// A `DeferNode` corresponds to one or more `@defer` applications at the same level of "nestedness"
/// in the planned query.
///
/// It contains a "primary block" and a vector of "deferred blocks". The "primary block" represents
/// the part of the query that is _not_ deferred (so the part of the query up until we reach the
/// @defer(s) this handles), while each "deferred block" correspond to the deferred part of one of
/// the @defer(s) handled by the node.
///
/// Note that `DeferNode`s are only generated if defer support is enabled for the query planner.
/// Also note that if said support is enabled, then `DeferNode`s are always generated if the query
/// has a @defer application, even if in some cases generated plan may not "truly" defer the
/// underlying fetches (i.e. in cases where `deferred[*].node` are all undefined). This currently
/// happens because some specific cases of defer cannot be handled, but could later also happen if
/// we implement more advanced server-side heuristics to decide if deferring is judicious or not.
/// This allows the executor of the plan to consistently send a defer-abiding multipart response to
/// the client.
pub struct DeferNode {
    /// The "primary" part of a defer, that is the non-deferred part (though could be deferred
    /// itself for a nested defer).
    primary: PrimaryDeferBlock,
    /// The "deferred" parts of the defer (note that it's a vector). Each of those deferred elements
    /// will correspond to a different chunk of the response to the client (after the initial
    /// on-deferred one that is).
    deferred: Vec<DeferredDeferBlock>,
}

/// The primary block of a `DeferNode`.
pub struct PrimaryDeferBlock {
    /// The part of the original query that "selects" the data to send in that primary response
    /// once the plan in `node` completes). Note that if the parent `DeferNode` is nested, then it
    /// must come inside the `DeferredNode` in which it is nested, and in that case this
    /// sub-selection will start at that parent `DeferredNode.query_path`. Note that this can be
    /// `None` in the rare case that everything in the original query is deferred (which is not very
    /// useful  in practice, but not disallowed by the @defer spec at the moment).
    sub_selection: Option<SelectionSet>,
    /// The plan to get all the data for the primary block. Same notes as for subselection: usually
    /// defined, but can be undefined in some corner cases where nothing is to be done in the
    /// primary block.
    node: Option<PlanNode>,
}

/// A deferred block of a `DeferNode`.
pub struct DeferredDeferBlock {
    /// References one or more fetch node(s) (by `id`) within `DeferNode.primary.node`. The plan of
    /// this deferred part should not be started until all such fetches return.
    depends: Vec<DeferredDependency>,
    /// The optional defer label.
    label: Option<NodeStr>,
    /// Path, in the query, to the `@defer` application this corresponds to. The `sub_selection`
    /// starts at this `query_path`.
    query_path: Vec<QueryPathElement>,
    /// The part of the original query that "selects" the data to send in the deferred response
    /// (once the plan in `node` completes). Will be set _unless_ `node` is a `DeferNode` itself.
    sub_selection: Option<SelectionSet>,
    /// The plan to get all the data for this deferred block. Usually set, but can be `None` for a
    /// `@defer` application where everything has been fetched in the "primary block" (i.e. when
    /// this deferred block only exists to expose what should be send to the upstream client in a
    /// deferred response), but without declaring additional fetches. This happens for @defer
    /// applications that cannot be handled through the query planner and where the defer cannot be
    /// passed through to the subgraph).
    node: Option<PlanNode>,
}

pub struct DeferredDependency {
    /// A `FetchNode` ID.
    id: NodeStr,
    /// If the `FetchNode` pointed to by `id` has `has_defers` as `true`, and this field is set (to
    /// the label of one of the defers of said `FetchNode`), then this dependency is strictly for
    /// the defers with that label.
    defer_label: Option<NodeStr>,
}

pub struct ConditionNode {
    condition_variable: Name,
    if_clause: Option<PlanNode>,
    else_clause: Option<PlanNode>,
}

/// The type of rewrites currently supported on the input/output data of fetches.
///
/// A rewrite usually identifies some sub-part of the data and some action to perform on that
/// sub-part.
pub enum FetchDataRewrite {
    ValueSetter(FetchDataValueSetter),
    KeyRenamer(FetchDataKeyRenamer),
}

/// A rewrite that sets a value at the provided path of the data it is applied to.
pub struct FetchDataValueSetter {
    /// Path to the value that is set by this "rewrite".
    path: Vec<FetchDataPathElement>,
    /// The value to set at `path`. Note that the query planner currently only uses string values,
    /// but that may change in the future.
    set_value_to: serde_json::Value,
}

/// A rewrite that renames the key at the provided path of the data it is applied to.
pub struct FetchDataKeyRenamer {
    /// Path to the key that is renamed by this "rewrite".
    path: Vec<FetchDataPathElement>,
    /// The key to rename to at `path`.
    rename_key_to: NodeStr,
}

/// Vectors of this element match path(s) to a value in fetch data. Each element is (1) a key in
/// object data, (2) _any_ index in array data (often serialized as `@`), or (3) a typename
/// constraint on the object data at that point in the path(s) (a path should only match for objects
/// whose `__typename` is the provided type).
///
/// It's possible for vectors of this element to match no paths in fetch data, e.g. if an object key
/// doesn't exist, or if an object's `__typename` doesn't equal the provided one. If this occurs,
/// then query plan execution should not execute the instruction this path is associated with.
///
/// The path starts at the top of the data it is applied to. So for instance, for fetch data inputs,
/// the path starts at the root of the object representing those inputs.
///
/// Note that the `@` is currently optional in some contexts, as query plan execution may assume
/// upon encountering array data in a path that it should match the remaining path to the array's
/// elements.
pub enum FetchDataPathElement {
    Key(NodeStr),
    AnyIndex,
    TypenameEquals(NodeStr),
}

/// Vectors of this element match a path in a query. Each element is (1) a field in a query, or (2)
/// an inline fragment in a query.
pub enum QueryPathElement {
    Field(Field),
    InlineFragment(InlineFragment),
}
