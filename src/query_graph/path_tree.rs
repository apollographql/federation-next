use crate::error::FederationError;
use crate::query_graph::graph_path::GraphPathItem;
use crate::query_graph::graph_path::OpGraphPath;
use crate::query_graph::graph_path::OpGraphPathTrigger;
use crate::query_graph::QueryGraph;
use crate::query_plan::operation::NormalizedSelectionSet;
use petgraph::graph::{EdgeIndex, NodeIndex};
use petgraph::visit::EdgeRef;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

/// A "merged" tree representation for a vector of `GraphPath`s that start at a common query graph
/// node, in which each node of the tree corresponds to a node in the query graph, and a tree's node
/// has a child for every unique pair of edge and trigger.
// PORT_NOTE: The JS codebase additionally has a property `triggerEquality`; this existed because
// Typescript doesn't have a native way of associating equality/hash functions with types, so they
// were passed around manually. This isn't the case with Rust, where we instead implement trigger
// equality via `PartialEq` and `Hash`.
#[derive(Debug)]
pub(crate) struct PathTree<TTrigger, TEdge>
where
    TTrigger: Eq + Hash,
    TEdge: Copy + Into<Option<EdgeIndex>>,
{
    /// The query graph of which this is a path tree.
    graph: Arc<QueryGraph>,
    /// The query graph node at which the path tree starts.
    node: NodeIndex,
    /// Note that `ClosedPath`s have an optimization which splits them into paths and a selection
    /// set representing a trailing query to a single subgraph at the final nodes of the paths. For
    /// such paths where this `PathTree`'s node corresponds to that final node, those selection sets
    /// are collected here. This is really an optimization to avoid unnecessary merging of selection
    /// sets when they query a single subgraph.
    local_selection_sets: Vec<Arc<NormalizedSelectionSet>>,
    /// The child `PathTree`s for this `PathTree` node. There is a child for every unique pair of
    /// edge and trigger present at this particular sub-path within the `GraphPath`s covered by this
    /// `PathTree` node.
    childs: Vec<Arc<PathTreeChild<TTrigger, TEdge>>>,
}

#[derive(Debug)]
pub(crate) struct PathTreeChild<TTrigger, TEdge>
where
    TTrigger: Eq + Hash,
    TEdge: Copy + Into<Option<EdgeIndex>>,
{
    /// The edge connecting this child to its parent.
    edge: TEdge,
    /// The trigger for the edge connecting this child to its parent.
    trigger: Arc<TTrigger>,
    /// The conditions required to be fetched if this edge is taken.
    conditions: Option<Arc<OpPathTree>>,
    /// The child `PathTree` reached by taking the edge.
    tree: Arc<PathTree<TTrigger, TEdge>>,
}

/// A `PathTree` whose triggers are operation elements (essentially meaning that the constituent
/// `GraphPath`s were guided by a GraphQL operation).
pub(crate) type OpPathTree = PathTree<OpGraphPathTrigger, Option<EdgeIndex>>;

impl OpPathTree {
    pub(crate) fn new(graph: Arc<QueryGraph>, node: NodeIndex) -> Self {
        Self {
            graph,
            node,
            local_selection_sets: Vec::new(),
            childs: Vec::new(),
        }
    }

    pub(crate) fn from_op_paths(
        graph: Arc<QueryGraph>,
        node: NodeIndex,
        paths: &[(&OpGraphPath, &Arc<NormalizedSelectionSet>)],
    ) -> Result<Self, FederationError> {
        assert!(
            !paths.is_empty(),
            "Should compute on empty paths" // FIXME: what does this mean?
        );
        Self::from_paths(
            graph,
            node,
            paths
                .iter()
                .map(|(path, selections)| (path.iter(), *selections))
                .collect::<Vec<_>>(),
        )
    }
}

impl<TTrigger, TEdge> PathTree<TTrigger, TEdge>
where
    TTrigger: Eq + Hash,
    TEdge: Copy + PartialEq + Into<Option<EdgeIndex>>,
{
    fn from_paths<'paths>(
        graph: Arc<QueryGraph>,
        node: NodeIndex,
        paths: Vec<(
            impl Iterator<Item = GraphPathItem<'paths, TTrigger, TEdge>>,
            &'paths Arc<NormalizedSelectionSet>,
        )>,
    ) -> Result<Self, FederationError>
    where
        TTrigger: 'paths,
        TEdge: 'paths,
    {
        // Map `EdgeIndex` IDs within the graph for edges going out of `node`
        // to consecutive positions/indices within `Vec`s we’re about to create.
        let edges_positions: HashMap<EdgeIndex, usize> = graph
            .edges(node)
            .enumerate()
            .map(|(position, edge)| (edge.id(), position))
            .collect();
        let edge_count = edges_positions.len();

        // We store "null" edges at `edge_count` index
        //
        // `impl Trait` in `type` alias is not stable,
        // so the alternative to a complex type annotation is no type annotation:
        #[allow(clippy::type_complexity)]
        let mut for_edge_position: Vec<
            Vec<(
                &Arc<TTrigger>,
                Option<Arc<OpPathTree>>,
                Vec<(
                    /* impl Iterator<Item = GraphPathItem<…>> */ _,
                    &'paths Arc<NormalizedSelectionSet>,
                )>,
            )>,
        > = std::iter::repeat_with(Vec::new)
            .take(edge_count + 1)
            .collect();

        let mut order = Vec::with_capacity(edge_count + 1);
        let mut total_childs = 0;
        let mut local_selection_sets = Vec::new();
        for (mut path_iter, selection) in paths {
            let Some((generic_edge, trigger, conditions)) = path_iter.next() else {
                local_selection_sets.push(selection.clone());
                continue;
            };
            let position;
            let new_node;
            if let Some(edge) = generic_edge.into() {
                position = edges_positions[&edge];
                let (_source, target) = graph.edge_endpoints(edge)?;
                new_node = target;
            } else {
                position = edge_count;
                new_node = node;
            };
            let for_position = &mut for_edge_position[position];
            if !for_position.is_empty() {
                if let Some((_, existing_conditions, new_paths)) = for_position
                    .iter_mut()
                    .find(|(existing_trigger, _, _)| *existing_trigger == trigger)
                {
                    if let Some(existing) = existing_conditions {
                        if let Some(cond) = &conditions {
                            *existing = existing.merge_if_not_equal(cond)
                        }
                    } else {
                        *existing_conditions = conditions.cloned()
                    }
                    new_paths.push((path_iter, selection))
                    // Note that as we merge, we don't create a new child
                } else {
                    for_position.push((trigger, conditions.cloned(), vec![(path_iter, selection)]));
                    total_childs += 1;
                }
            } else {
                // First time we see someone from that position, record the order
                order.push((position, generic_edge, new_node));
                for_edge_position[position] =
                    vec![(trigger, conditions.cloned(), vec![(path_iter, selection)])];
                total_childs += 1;
            }
        }

        let mut childs = Vec::with_capacity(total_childs);
        for (position, generic_edge, new_node) in order {
            for (trigger, conditions, sub_path_and_selections) in
                std::mem::take(&mut for_edge_position[position])
            {
                childs.push(Arc::new(PathTreeChild {
                    edge: generic_edge,
                    trigger: (*trigger).clone(),
                    conditions: conditions.clone(),
                    tree: Arc::new(Self::from_paths(
                        graph.clone(),
                        new_node,
                        sub_path_and_selections,
                    )?),
                }))
            }
        }
        assert_eq!(childs.len(), total_childs);
        Ok(Self {
            graph,
            node,
            local_selection_sets,
            childs,
        })
    }

    fn merge_if_not_equal(self: &Arc<Self>, other: &Arc<Self>) -> Arc<Self> {
        if self.equals_same_root(other) {
            self.clone()
        } else {
            self.merge(other)
        }
    }

    /// May have false negatives (see comment about `Arc::ptr_eq`)
    fn equals_same_root(self: &Arc<Self>, other: &Arc<Self>) -> bool {
        Arc::ptr_eq(self, other)
            || self.childs.iter().zip(&other.childs).all(|(a, b)| {
                a.edge == b.edge
                    // `Arc::ptr_eq` instead of `==` is faster and good enough.
                    // This method is all about avoid unnecessary merging
                    // when we suspect conditions trees have been build from the exact same inputs.
                    && Arc::ptr_eq(&a.trigger, &b.trigger)
                    && match (&a.conditions, &b.conditions) {
                        (None, None) => true,
                        (Some(cond_a), Some(cond_b)) => cond_a.equals_same_root(cond_b),
                        _ => false,
                    }
                    && a.tree.equals_same_root(&b.tree)
            })
    }

    fn merge(self: &Arc<Self>, other: &Arc<Self>) -> Arc<Self> {
        if Arc::ptr_eq(self, other) {
            return self.clone();
        }
        assert!(
            Arc::ptr_eq(&self.graph, &other.graph),
            "Cannot merge path tree build on another graph"
        );
        assert_eq!(
            self.node, other.node,
            "Cannot merge path trees rooted different nodes"
        );
        if other.childs.is_empty() {
            return self.clone();
        }
        if self.childs.is_empty() {
            return other.clone();
        }

        let mut count_to_add = 0;
        let merge_indices: Vec<_> = other
            .childs
            .iter()
            .map(|other_child| {
                let position = self.childs.iter().position(|self_child| {
                    self_child.edge == other_child.edge && self_child.trigger == other_child.trigger
                });
                if position.is_none() {
                    count_to_add += 1
                }
                position
            })
            .collect();
        let expected_new_len = self.childs.len() + count_to_add;
        let mut childs = Vec::with_capacity(expected_new_len);
        childs.extend(self.childs.iter().cloned());
        for (other_child, merge_index) in other.childs.iter().zip(merge_indices) {
            if let Some(i) = merge_index {
                let child = &mut childs[i];
                *child = Arc::new(PathTreeChild {
                    edge: child.edge,
                    trigger: child.trigger.clone(),
                    conditions: match (&child.conditions, &other_child.conditions) {
                        (Some(a), Some(b)) => Some(a.merge_if_not_equal(b)),
                        (Some(a), None) => Some(a.clone()),
                        (None, Some(b)) => Some(b.clone()),
                        (None, None) => None,
                    },
                    tree: child.tree.merge(&other_child.tree),
                })
            } else {
                childs.push(other_child.clone())
            }
        }
        assert_eq!(childs.len(), expected_new_len);

        Arc::new(Self {
            graph: self.graph.clone(),
            node: self.node,
            local_selection_sets: self
                .local_selection_sets
                .iter()
                .chain(&other.local_selection_sets)
                .cloned()
                .collect(),
            childs,
        })
    }
}
