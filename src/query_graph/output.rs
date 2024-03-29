// Output module for query graphs
// - Corresponds to the `graphviz` and `mermaid` modules from the JS federation.

use apollo_compiler::NodeStr;
use crate::query_graph::{
    QueryGraph, QueryGraphNode, QueryGraphEdge,
};
use petgraph::graph::{DiGraph, EdgeIndex};
use petgraph::stable_graph::StableGraph;
use petgraph::dot::{Dot, Config};

type InnerGraph = DiGraph<QueryGraphNode, QueryGraphEdge>;
type StableInnerGraph = StableGraph<QueryGraphNode, QueryGraphEdge>;

//////////////////////////////////////////////////////////////////////////////
// GraphViz output for QueryGraph

fn label_edge(edge: &QueryGraphEdge) -> String {
    let label = edge.to_string();
    if label.is_empty() {
        String::new()
    }
    else {
        format!("label=\"{}\"", edge)
    }
}

fn label_node(node: &QueryGraphNode) -> String {
    format!("label=\"{}\"", node.type_)
}

pub fn to_dot(graph: &QueryGraph) -> String {
    if graph.sources.len() > 1 {
        return to_dot_federated(graph)
    }

    // Note: Use label_edge/label_node as `attr_getters` in order to create custom label
    //       strings, instead of the default labeling.
    let config = [Config::NodeNoLabel, Config::EdgeNoLabel];
    Dot::with_attr_getters( &graph.graph, &config
                          , &(|_, er| label_edge(er.weight()))
                          , &(|_, (_, node)| label_node(node))
                          ).to_string()
}

fn to_dot_federated(graph: &QueryGraph) -> String {

    fn edge_within_cluster( graph: &StableInnerGraph, cluster_name: &NodeStr, edge_index: EdgeIndex ) -> bool {
        match graph.edge_endpoints(edge_index) {
            None => false,

            Some((n1, n2)) => {
                graph[n1].source == *cluster_name
                && graph[n2].source == *cluster_name
            }
        }
    }

    fn edge_across_clusters( graph: &StableInnerGraph, edge_index: EdgeIndex ) -> bool {
        match graph.edge_endpoints(edge_index) {
            None => false,

            Some((n1, n2)) => {
                graph[n1].source != graph[n2].source
            }
        }
    }

    fn label_cluster_node(node: &QueryGraphNode) -> String {
        format!("label=\"{}@{}\"", node.type_, node.source)
    }

    // Build a stable graph, so we can derive subgraph clusters with the same indices.
    let stable_graph = StableGraph::from(graph.graph.clone());
    let cluster_dot_config = [Config::NodeNoLabel, Config::EdgeNoLabel, Config::GraphContentOnly];

    let mut dot_str : String = format!("digraph \"{}\" {{\n", graph.name());

    // Subgraph clusters
    for (cluster_name,_) in graph.sources.iter() {
        if cluster_name == graph.name() {
            continue; // skip non-subgraph nodes
        }

        let filtered_graph : StableInnerGraph
            = stable_graph.filter_map(
                |_i, n| if n.source == *cluster_name {Some(n.clone())} else {None},
                |i, e| if edge_within_cluster(&stable_graph, cluster_name, i) {Some(e.clone())} else {None}
            );
        let s = Dot::with_attr_getters( &filtered_graph, &cluster_dot_config
                                      , &(|_, er| label_edge(er.weight()))
                                      , &(|_, (_, node)| label_cluster_node(node))
                                     ).to_string();

        dot_str.push_str(&format!("  subgraph \"cluster_{}\" {{\n", &cluster_name));
        dot_str.push_str(&format!("    label = \"Subgraph \\\"{}\\\"\";\n", &cluster_name));
        dot_str.push_str("    color = \"black;\"\n");
        dot_str.push_str("    style = \"\"\n");
        dot_str.push_str(&s);
        dot_str.push_str("  }\n");
    }

    // Supergraph nodes
    for i in stable_graph.node_indices() {
        let node = &stable_graph[i];
        if node.source == graph.name() {
            dot_str.push_str(&format!("  {} [{}]\n", i.index(), label_node(&node)));
        }
    }

    // Supergraph edges
    for i in stable_graph.edge_indices()
    {
        if edge_across_clusters(&stable_graph, i) {
            if let Some((n1, n2)) = stable_graph.edge_endpoints(i) {
                let edge = &stable_graph[i];
                dot_str.push_str(&format!("  {} -> {} [{}]\n", n1.index(), n2.index(), label_edge(edge)));
            }
        }
    }

    dot_str.push_str("}");
    dot_str
}
