use crate::query_graph::graph_path::{ClosedBranch, OpenBranch, SimultaneousPaths};
use crate::query_graph::path_tree::OpPathTree;
use crate::query_graph::QueryGraph;
use crate::query_plan::fetch_dependency_graph::FetchDependencyGraph;
use crate::query_plan::fetch_dependency_graph_processor::{
    FetchDependencyGraphToCostProcessor, FetchDependencyGraphToQueryPlanProcessor,
};
use crate::query_plan::operation::{NormalizedOperation, NormalizedSelection};
use crate::query_plan::query_planner::QueryPlannerConfig;
use crate::query_plan::QueryPlanCost;
use crate::schema::position::{AbstractTypeDefinitionPosition, SchemaRootDefinitionKind};
use crate::schema::ValidFederationSchema;
use indexmap::IndexSet;
use petgraph::graph::NodeIndex;
use std::sync::Arc;

// PORT_NOTE: Named `PlanningParameters` in the JS codebase, but there was no particular reason to
// leave out to the `Query` prefix, so it's been added for consistency. Similar to `GraphPath`, we
// don't have a distinguished type for when the head is a root vertex, so we instead check this at
// runtime (introducing the new field `head_must_be_root`).
pub(crate) struct QueryPlanningParameters {
    /// The supergraph schema that generated the federated query graph.
    supergraph_schema: ValidFederationSchema,
    /// The federated query graph used for query planning.
    federated_query_graph: Arc<QueryGraph>,
    /// The operation to be query planned.
    operation: Arc<NormalizedOperation>,
    /// A processor for converting fetch dependency graphs to query plans.
    processor: FetchDependencyGraphToQueryPlanProcessor,
    /// The query graph node at which query planning begins.
    head: NodeIndex,
    /// Whether the head must be a root node for query planning.
    head_must_be_root: bool,
    /// A set of the names of interface or union types that have inconsistent "runtime types" across
    /// subgraphs.
    // PORT_NOTE: Named `inconsistentAbstractTypesRuntimes` in the JS codebase, which was slightly
    // confusing.
    abstract_types_with_inconsistent_runtime_types: Arc<IndexSet<AbstractTypeDefinitionPosition>>,
    /// The configuration for the query planner.
    config: Arc<QueryPlannerConfig>,
    // TODO: When `PlanningStatistics` is ported, add a field for it.
}

// PORT_NOTE: The JS codebase also had a field `conditionResolver`, but this was only ever used once
// during construction, so we don't store it in the struct itself.
pub(crate) struct QueryPlanningTraversal {
    /// The parameters given to query planning.
    parameters: QueryPlanningParameters,
    /// The root kind of the operation.
    root_kind: SchemaRootDefinitionKind,
    /// True if query planner `@defer` support is enabled and the operation contains some `@defer`
    /// application.
    has_defers: bool,
    /// The initial fetch ID generation (used when handling `@defer`).
    starting_id_generation: u64,
    /// A processor for converting fetch dependency graphs to cost.
    cost_processor: FetchDependencyGraphToCostProcessor,
    /// True if this query planning is at top-level (note that query planning can recursively start
    /// further query planning).
    is_top_level: bool,
    /// The stack of open branches left to plan, along with state indicating the next selection to
    /// plan for them.
    // PORT_NOTE: The `stack` in the JS codebase only contained one selection per stack entry, but
    // to avoid having to clone the `OpenBranch` structures (which loses the benefits of indirect
    // path caching), we create a multi-level-stack here, where the top-level stack is over open
    // branches and the sub-stack is over selections.
    open_branches: Vec<OpenBranchAndSelections>,
    /// The closed branches that have been planned.
    closed_branches: Vec<ClosedBranch>,
    /// The best plan found as a result of query planning.
    best_plan: Option<BestQueryPlanInfo>,
}

struct OpenBranchAndSelections {
    /// The options for this open branch.
    open_branch: OpenBranch,
    /// A stack of the remaining selections to plan from the node this open branch ends on.
    selections: Vec<NormalizedSelection>,
}

struct BestQueryPlanInfo {
    /// The fetch dependency graph for this query plan.
    fetch_dependency_graph: FetchDependencyGraph,
    /// The path tree for the closed branch options chosen for this query plan.
    path_tree: OpPathTree,
    /// The cost of this query plan.
    cost: QueryPlanCost,
}

impl QueryPlanningTraversal {
    fn compute_best_plan_from_closed_branches(&mut self) {
        if self.closed_branches.is_empty() {
            return;
        }
        self.prune_closed_branches();
        self.sort_options_in_closed_branches();
        self.reduce_options_if_needed();

        todo!() // the rest of the owl
    }

    /// Remove closed branches that are known to be overridden by others.
    ///
    /// We've computed all branches and need to compare all the possible plans to pick the best.
    /// Note however that "all the possible plans" is essentially a cartesian product of all
    /// the closed branches options, and if a lot of branches have multiple options, this can
    /// exponentially explode.
    /// So first, we check if we can preemptively prune some branches based on
    /// those branches having options that are known to be overriden by other ones.
    fn prune_closed_branches(&mut self) {
        for branch in &mut self.closed_branches {
            if branch.0.len() <= 1 {
                continue;
            }

            let mut pruned = ClosedBranch(Vec::new());
            for (i, to_check) in branch.0.iter().enumerate() {
                if !Self::option_is_overriden(i, &to_check.paths, branch) {
                    pruned.0.push(to_check.clone());
                }
            }

            *branch = pruned
        }
    }

    fn option_is_overriden(
        index: usize,
        to_check: &SimultaneousPaths,
        all_options: &ClosedBranch,
    ) -> bool {
        all_options
            .0
            .iter()
            .enumerate()
            // Don’t compare `to_check` with itself
            .filter(|&(i, _)| i != index)
            .any(|(_i, option)| {
                to_check
                    .0
                    .iter()
                    .all(|p| option.paths.0.iter().any(|o| p.is_overridden_by(o)))
            })
    }

    /// We now sort the options within each branch,
    /// putting those with the least amount of subgraph jumps first.
    /// The idea is that for each branch taken individually,
    /// the option with the least jumps is going to be the most efficient,
    /// and while it is not always the case that the best plan is built for those individual bests,
    /// they are still statistically more likely to be part of the best plan.
    /// So putting them first has 2 benefits for the rest of this method:
    ///
    /// 1. if we end up cutting some options of a branch below
    ///    (due to having too many possible plans),
    ///    we'll cut the last option first (we `pop()`),
    ///    so better cut what it the least likely to be good.
    /// 2. when we finally generate the plan,
    ///    we use the cost of previously computed plans to cut computation early when possible
    ///    (see `generate_all_plans_and_find_best`),
    ///    so there is a premium in generating good plans early (it cuts more computation),
    ///    and putting those more-likely-to-be-good options first helps this.
    fn sort_options_in_closed_branches(&mut self) {
        for branch in &mut self.closed_branches {
            branch.0.sort_by_key(|closed_path| {
                closed_path
                    .paths
                    .0
                    .iter()
                    .map(|path| path.subgraph_jumps())
                    .sum::<u32>()
            })
        }
    }

    /// Look at how many plans we'd have to generate and if it's "too much"
    /// reduce it to something manageable by arbitrarilly throwing out options.
    /// This effectively means that when a query has too many options,
    /// we give up on always finding the "best" query plan in favor of an "ok" query plan.
    ///
    /// TODO: currently, when we need to reduce options, we do so somewhat arbitrarilly.
    /// More precisely, we reduce the branches with the most options first
    /// and then drop the last option of the branch,
    /// repeating until we have a reasonable number of plans to consider.
    /// The sorting we do about help making this slightly more likely to be a good choice,
    /// but there is likely more "smarts" we could add to this.
    fn reduce_options_if_needed(&mut self) {
        // We sort branches by those that have the most options first.
        self.closed_branches
            .sort_by(|b1, b2| b1.0.len().cmp(&b2.0.len()).reverse());
        let mut plan_count = self
            .closed_branches
            .iter()
            .try_fold(1, |product, branch| {
                if branch.0.is_empty() {
                    // This would correspond to not being to find *any* path
                    // for a particular queried field,
                    // which means we have no plan for the overall query.
                    // Now, this shouldn't happen in practice if composition validation
                    // has been run successfully (and is not buggy),
                    // since the goal of composition validation
                    // is exactly to ensure we can never run into this path.
                    // In any case, we will throw later if that happens,
                    // but let's just return the proper result here, which is no plan at all.
                    None
                } else {
                    Some(product * branch.0.len())
                }
            })
            .unwrap_or(0);
        // debug!("Query has {plan_count} possible plans");

        let max_evaluated_plans = self.parameters.config.debug.max_evaluated_plans as usize;
        loop {
            let first_branch = &mut self.closed_branches[0];
            if plan_count <= max_evaluated_plans || first_branch.0.len() <= 1 {
                break;
            }
            // we remove the right-most option of the first branch,
            // and them move that branch to it's new place.
            let prev_size = first_branch.0.len();
            first_branch.0.pop();
            plan_count -= plan_count / prev_size;
            self.reorder_first_branch();
            // Note that if firstBranch is our only branch, it's fine,
            // we'll continue to remove options from it (but that is beyond unlikely).

            // debug!("Reduced plans to consider to {plan_count} plans");
        }
    }

    /// Moves the first closed branch to after any branch having more options.
    /// This method assumes that closed branches are sorted by decreasing number of options
    /// _except_ for the first element which may be out of order,
    /// and this method restore that order.
    fn reorder_first_branch(&mut self) {
        let first_branch_len = self.closed_branches[0].0.len();
        let i = self
            .closed_branches
            .iter()
            .skip(1)
            .position(|branch| branch.0.len() <= first_branch_len)
            .unwrap_or(self.closed_branches.len());
        // `i` is the smallest index of an element
        // having the same number or less options than the first one,
        // so we switch that first branch with the element "before" `i` (which has more elements).
        self.closed_branches.swap(0, i - 1)

        // FIXME: doesn’t this leave the element foremly at `i - 1` out of place?
        // Shouldn’t the whole range be shifted?
        //
        // It may be easier to do a full sort again, since it’s documented as:
        //
        // https://doc.rust-lang.org/std/primitive.slice.html#method.sort_by
        // > The current algorithm is an adaptive, iterative merge sort inspired by timsort.
        // > It is designed to be very fast in cases where the slice is nearly sorted
    }
}
