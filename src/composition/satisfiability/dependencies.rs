use std::sync::Arc;

use crate::query_graph::{
    graph_path::{GraphPath, GraphPathTrigger},
    QueryGraph,
};

use apollo_compiler::{ast::Document, NodeStr};
use itertools::Itertools;
use petgraph::graph::EdgeIndex;

type TODO = usize;

impl<TTrigger, TEdge> GraphPath<TTrigger, TEdge>
where
    TTrigger: Eq + std::hash::Hash,
    Arc<TTrigger>: Into<GraphPathTrigger>,
    TEdge: Copy + Into<Option<EdgeIndex>>,
    EdgeIndex: Into<TEdge>,
{
    pub fn from_graph_root(
        _graph: Arc<QueryGraph>,
        _root_kind: TODO, /* SchemaRootKind */
    ) -> TODO /* Option<Self> */ {
        // graph
        //     .root_node_for_kind(root_kind)
        //     .map(|root| Self::new(graph, root))

        todo!()
    }
}

pub(super) fn print_subgraph_names(names: &[NodeStr]) -> String {
    print_human_readable_list(
        names.iter().map(|n| format!("\"{}\"", n)).collect(),
        None,                     // emptyValue
        Some("subgraph".into()),  // prefix
        Some("subgraphs".into()), // prefixPlural
        None,                     // lastSeparator
        None,                     // cutoff_output_length
    )
}

/// Like `joinStrings`, joins an array of string, but with a few twists, namely:
///  - If the resulting list to print is "too long", it only display a subset of the elements and use some elipsis (...). In other
///    words, this method is for case where, where the list ot print is too long, it is more useful to avoid flooding the output than
///    printing everything.
///  - it allows to prefix the whole list, and to use a different prefix for a single element than for > 1 elements.
///  - it forces the use of ',' as separator, but allow a different lastSeparator.
pub(super) fn print_human_readable_list(
    names: Vec<String>,
    _empty_value: Option<String>,
    _prefix: Option<String>,
    _prefix_plural: Option<String>,
    _last_separator: Option<String>,
    _cutoff_ouput_length: Option<u32>,
) -> String {
    names.iter().join(", ")
}

/// JS PORT NOTE: for printing "witness" operations, we actually need a printer
/// that accepts invalid selection sets.
pub(super) fn operation_to_document(_operation: TODO) -> Document {
    todo!()
}
