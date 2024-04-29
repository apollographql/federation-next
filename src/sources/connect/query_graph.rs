use apollo_compiler::ast::Name;
use indexmap::{map::Entry, IndexMap, IndexSet};
use petgraph::prelude::NodeIndex;

use crate::{
    error::FederationError,
    schema::{
        position::{ObjectOrInterfaceFieldDefinitionPosition, TypeDefinitionPosition},
        ValidFederationSchema,
    },
    source_aware::federated_query_graph::builder::IntraSourceQueryGraphBuilderApi,
    sources::{
        connect::ConnectFederatedScalarQueryGraphNode, SourceFederatedConcreteFieldQueryGraphEdge,
        SourceFederatedConcreteQueryGraphNode, SourceFederatedEnumQueryGraphNode,
        SourceFederatedQueryGraphBuilderApi, SourceFederatedScalarQueryGraphNode,
    },
    ValidFederationSubgraph,
};

use super::{
    models::Connector,
    selection_parser::{NamedSelection, PathSelection, Property, SubSelection},
    ConnectFederatedConcreteFieldQueryGraphEdge, ConnectFederatedConcreteQueryGraphNode,
    ConnectFederatedEnumQueryGraphNode, ConnectFederatedQueryGraphBuilder, Selection,
};

impl SourceFederatedQueryGraphBuilderApi for ConnectFederatedQueryGraphBuilder {
    fn process_subgraph_schema(
        &self,
        subgraph: ValidFederationSubgraph,
        builder: &mut impl IntraSourceQueryGraphBuilderApi,
    ) -> Result<(), FederationError> {
        // Extract the connects from the schema definition and map them to their `Connect` equivalent
        // Note: We need to keep the object_field as well as the arguments since it contains important
        // type information of the original schema.
        // TODO: Validate that we don't have connect directives on anything but object fields.
        let connectors = Connector::from_valid_schema(&subgraph.schema, subgraph.name.into())?;

        for (id, connect) in connectors {
            let ObjectOrInterfaceFieldDefinitionPosition::Object(field_def_pos) =
                id.directive.field
            else {
                unreachable!()
            };

            // Make a node for the entrypoint of this field, if not yet created
            let parent_node = builder.add_concrete_node(
                field_def_pos.type_name.clone(),
                SourceFederatedConcreteQueryGraphNode::Connect(
                    ConnectFederatedConcreteQueryGraphNode::ConnectParent {
                        subgraph_type: field_def_pos.parent().clone(),
                    },
                ),
            )?;

            // Process the field, constructing the rest of the graph for its selections
            let field_output_type_name = field_def_pos
                .get(subgraph.schema.schema())?
                .ty
                .inner_named_type();
            let field_output_type_pos = subgraph.schema.get_type(field_output_type_name.clone())?;
            let field_node = process_selection(
                connect.selection,
                field_output_type_pos,
                &subgraph.schema,
                builder,
            )?;

            // Make an edge from the parent into our new subgraph
            builder.add_concrete_field_edge(
                parent_node,
                field_node,
                field_def_pos.field_name.clone(),
                IndexSet::new(),
                SourceFederatedConcreteFieldQueryGraphEdge::Connect(
                    ConnectFederatedConcreteFieldQueryGraphEdge::Connect {
                        subgraph_field: field_def_pos,
                    },
                ),
            )?;
        }

        Ok(())
    }
}

/// Processes a connect selection
///
/// This method creates nodes from selection parameters of a field decorated by
/// a connect directive, making sure to reuse nodes if possible.
fn process_selection(
    selection: Selection,
    field_output_type_pos: TypeDefinitionPosition,
    subgraph_schema: &ValidFederationSchema,
    builder: &mut impl IntraSourceQueryGraphBuilderApi,
) -> Result<NodeIndex<u32>, FederationError> {
    // Keep a cache to reuse nodes
    let mut node_cache: IndexMap<Name, NodeIndex<u32>> = IndexMap::new();

    // Get the type of the field
    let field_ty = field_output_type_pos.get(subgraph_schema.schema())?;

    // Custom scalars are easy, so handle them first
    if field_ty.is_scalar() && !field_ty.is_built_in() {
        // Note: the if condition checked that this is a scalar, so trying to unwrap to anything else
        // is impossible.
        let TypeDefinitionPosition::Scalar(scalar_field_ty) = field_output_type_pos else {
            unreachable!()
        };

        return builder.add_scalar_node(
            field_ty.name().clone(),
            SourceFederatedScalarQueryGraphNode::Connect(
                ConnectFederatedScalarQueryGraphNode::CustomScalarSelectionRoot {
                    subgraph_type: scalar_field_ty,
                    selection,
                },
            ),
        );
    }

    // If we aren't a custom scalar, then look at the selection to see what to attempt
    match selection {
        Selection::Path(path) => match field_output_type_pos {
            TypeDefinitionPosition::Enum(enum_type) => {
                // An enum cannot have subselections, but the structure is a linked list, so we need to collect here...
                let props = extract_props(&path)?;

                // Create the node for this enum
                builder.add_enum_node(
                    field_ty.name().clone(),
                    SourceFederatedEnumQueryGraphNode::Connect(
                        ConnectFederatedEnumQueryGraphNode::SelectionRoot {
                            subgraph_type: enum_type,
                            property_path: props,
                        },
                    ),
                )
            }
            TypeDefinitionPosition::Scalar(scalar_type) => {
                // An enum cannot have subselections, but the structure is a linked list, so we need to collect here...
                let props = extract_props(&path)?;

                // Create the node for this enum
                builder.add_scalar_node(
                    field_ty.name().clone(),
                    SourceFederatedScalarQueryGraphNode::Connect(
                        ConnectFederatedScalarQueryGraphNode::SelectionRoot {
                            subgraph_type: scalar_type,
                            property_path: props,
                        },
                    ),
                )
            }

            _ => {
                // If we don't have either of the above, then we must have a subselection
                let PathSelection::Selection(sub) = path else {
                    todo!("handle error")
                };

                // TODO: Where do the properties come from?
                process_subselection(
                    sub,
                    field_output_type_pos,
                    subgraph_schema,
                    builder,
                    &mut node_cache,
                    Some(Vec::new()),
                )
            }
        },
        Selection::Named(sub) => {
            // Make sure that we aren't selecting sub fields from simple types
            if field_ty.is_scalar() || field_ty.is_enum() {
                todo!("handle error");
            }

            // Grab what we need and return the root node
            process_subselection(
                sub,
                field_output_type_pos,
                subgraph_schema,
                builder,
                &mut node_cache,
                Some(Vec::new()),
            )
        }
    }
}

fn process_subselection(
    sub: SubSelection,
    field_output_type_pos: TypeDefinitionPosition,
    subgraph_schema: &ValidFederationSchema,
    builder: &mut impl IntraSourceQueryGraphBuilderApi,
    node_cache: &mut IndexMap<Name, NodeIndex<u32>>,
    properties_path: Option<Vec<Property>>,
) -> Result<NodeIndex<u32>, FederationError> {
    // Reference for working with the entry API
    // let parent_node = match node_cache.entry(&object.type_name) {
    //     Entry::Occupied(e) => e.into_mut(),
    //     Entry::Vacant(e) => {
    //         let node = builder.add_concrete_node(
    //             object.type_name.clone(),
    //             SourceFederatedConcreteQueryGraphNode::Connect(
    //                 ConnectFederatedConcreteQueryGraphNode::ConnectParent {
    //                     subgraph_type: object.parent().clone(),
    //                 },
    //             ),
    //         )?;

    //         e.insert(node)
    //     }
    // };

    // Get the type of the field
    let field_ty = field_output_type_pos.get(subgraph_schema.schema())?;

    // For milestone 1 we don't need to support anything other than objects...
    let TypeDefinitionPosition::Object(object_pos) = field_output_type_pos else {
        todo!("handle error");
    };
    let object_type = object_pos.get(subgraph_schema.schema())?;

    // Create the root node for this object
    let object_node = builder.add_concrete_node(
        field_ty.name().clone(),
        SourceFederatedConcreteQueryGraphNode::Connect(
            properties_path
                .map(
                    |props| ConnectFederatedConcreteQueryGraphNode::SelectionRoot {
                        subgraph_type: object_pos.clone(),
                        property_path: props,
                    },
                )
                .unwrap_or(ConnectFederatedConcreteQueryGraphNode::SelectionChild {
                    subgraph_type: object_pos.clone(),
                }),
        ),
    )?;

    // Handle all named selections
    for selection in sub.selections {
        // Make sure that we have a field on the object type that matches the alias (or the name itself)
        let alias = selection.name();
        let Some(selection_field) = object_type.fields.get(alias) else {
            todo!("handle error");
        };
        let selection_type =
            subgraph_schema.get_type(selection_field.ty.inner_named_type().clone())?;
        let selection_extended_type = selection_type.get(subgraph_schema.schema())?;

        // Now add sub type info to the graph
        match selection_type {
            TypeDefinitionPosition::Scalar(ref scalar) => {
                // Custom scalars need to be handled differently
                if !selection_extended_type.is_built_in() {
                    todo!("handle error");
                }

                // A scalar cannot have sub selections, so enforce that now
                if matches!(
                    selection,
                    NamedSelection::Field(_, _, Some(_))
                        | NamedSelection::Quoted(_, _, Some(_))
                        | NamedSelection::Path(_, PathSelection::Selection(_))
                        | NamedSelection::Group(_, _)
                ) {
                    todo!("handle error");
                }

                // Create the scalar node (or grab it from the cache)
                let scalar_node = match node_cache.entry(scalar.type_name.clone()) {
                    Entry::Occupied(e) => e.into_mut(),
                    Entry::Vacant(e) => {
                        let node = builder.add_scalar_node(
                            scalar.type_name.clone(),
                            SourceFederatedScalarQueryGraphNode::Connect(
                                ConnectFederatedScalarQueryGraphNode::SelectionChild {
                                    subgraph_type: scalar.clone(),
                                },
                            ),
                        )?;

                        e.insert(node)
                    }
                };

                // Link the field to the object node
                builder.add_concrete_field_edge(
                    object_node,
                    *scalar_node,
                    selection_field.name.clone(),
                    IndexSet::new(),
                    SourceFederatedConcreteFieldQueryGraphEdge::Connect(
                        ConnectFederatedConcreteFieldQueryGraphEdge::CustomScalarPathSelection {
                            subgraph_field: object_pos.field(field_ty.name().clone()),
                            path_selection: PathSelection::Empty,
                        },
                    ),
                )?;
            }
            TypeDefinitionPosition::Object(_) => todo!(),
            TypeDefinitionPosition::Interface(_) => todo!(),
            TypeDefinitionPosition::Union(_) => todo!(),
            TypeDefinitionPosition::Enum(_) => todo!(),
            TypeDefinitionPosition::InputObject(_) => todo!(),
        }
    }

    // Handle the optional star selection
    if let Some(_star) = sub.star {
        //
    }

    Ok(object_node)
}

/// Attempt to extract all properties from a path selection
///
/// Note: This will fail if any of the subsequent paths are not also [PathSelection]
/// which should be impossible since it is constructed manually this way.
// TODO: Update subselection to not be a linked list of parent types...
fn extract_props(path: &PathSelection) -> Result<Vec<Property>, FederationError> {
    // TODO: Can this be cyclical?
    let mut current_path = path;
    let mut results = Vec::new();
    loop {
        match current_path {
            PathSelection::Path(prop, next) => {
                results.push(prop.clone());
                current_path = &next;
            }
            PathSelection::Empty => break,

            // TODO: We need to error out if we find a SubSelection since we only expect properties.
            // This might happen if the user tries to write a subselection for a type that does not support it
            PathSelection::Selection(_) => {
                todo!("handle error")
            }
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use apollo_compiler::Schema;
    use insta::assert_snapshot;

    use crate::{
        query_graph::extract_subgraphs_from_supergraph::extract_subgraphs_from_supergraph,
        schema::FederationSchema, sources::SourceFederatedQueryGraphBuilderApi,
        ValidFederationSubgraphs,
    };

    use super::ConnectFederatedQueryGraphBuilder;

    static SIMPLE_SUPERGRAPH: &str = include_str!("./tests/schemas/simple.graphql");

    fn get_subgraphs(supergraph_sdl: &str) -> ValidFederationSubgraphs {
        let schema = Schema::parse(supergraph_sdl, "supergraph.graphql").unwrap();
        let supergraph_schema = FederationSchema::new(schema).unwrap();
        extract_subgraphs_from_supergraph(&supergraph_schema, Some(true)).unwrap()
    }

    #[test]
    fn it_creates_a_connect_graph() {
        let federated_builder = ConnectFederatedQueryGraphBuilder;
        let mut mock_builder = mock::MockSourceQueryGraphBuilder::new();
        let subgraphs = get_subgraphs(SIMPLE_SUPERGRAPH);
        let (_, subgraph) = subgraphs.into_iter().next().unwrap();

        // Make sure that the tail data is correct
        federated_builder
            .process_subgraph_schema(subgraph, &mut mock_builder)
            .unwrap();

        // Make sure that our graph makes sense
        let as_dot = mock_builder.into_dot();
        assert_snapshot!(as_dot, @r###"
        digraph {
            0 [ label = "Node: Query" ]
            1 [ label = "Node: User" ]
            2 [ label = "Scalar: ID" ]
            3 [ label = "Scalar: String" ]
            4 [ label = "Node: Query" ]
            5 [ label = "Node: Post" ]
            6 [ label = "Scalar: ID" ]
            7 [ label = "Scalar: String" ]
            1 -> 2 [ label = "id" ]
            1 -> 3 [ label = "name" ]
            0 -> 1 [ label = "users" ]
            5 -> 6 [ label = "id" ]
            5 -> 7 [ label = "title" ]
            5 -> 7 [ label = "body" ]
            4 -> 5 [ label = "posts" ]
        }
        "###);
    }

    mod mock {
        use std::fmt::Display;

        use apollo_compiler::ast::{Name, NamedType};
        use indexmap::IndexSet;
        use petgraph::{
            dot::Dot,
            prelude::{EdgeIndex, NodeIndex},
            Graph,
        };

        use crate::{
            error::FederationError,
            source_aware::federated_query_graph::{
                builder::IntraSourceQueryGraphBuilderApi, SelfConditionIndex,
            },
            sources::{
                SourceFederatedAbstractFieldQueryGraphEdge,
                SourceFederatedConcreteFieldQueryGraphEdge, SourceFederatedConcreteQueryGraphNode,
                SourceFederatedEnumQueryGraphNode, SourceFederatedQueryGraphs,
                SourceFederatedScalarQueryGraphNode, SourceFederatedTypeConditionQueryGraphEdge,
                SourceId,
            },
        };

        /// A mock query Node
        struct MockNode {
            prefix: String,
            type_name: NamedType,
        }
        impl Display for MockNode {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}: {}", self.prefix, self.type_name)
            }
        }

        /// A mock query edge
        struct MockEdge {
            field_name: Name,
        }
        impl Display for MockEdge {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.field_name)
            }
        }

        /// Mock implementation of [IntraSourceQueryGraphBuilder]
        pub struct MockSourceQueryGraphBuilder {
            graph: Graph<MockNode, MockEdge>,
        }
        impl MockSourceQueryGraphBuilder {
            pub fn new() -> Self {
                Self {
                    graph: Graph::new(),
                }
            }

            pub fn into_dot(self) -> String {
                Dot::with_config(&self.graph, &[]).to_string()
            }
        }

        impl IntraSourceQueryGraphBuilderApi for MockSourceQueryGraphBuilder {
            // We only support concrete types for now
            fn add_concrete_node(
                &mut self,
                supergraph_type_name: NamedType,
                source_data: SourceFederatedConcreteQueryGraphNode,
            ) -> Result<NodeIndex, FederationError> {
                let SourceFederatedConcreteQueryGraphNode::Connect(_data) = source_data else {
                    unreachable!()
                };

                Ok(self.graph.add_node(MockNode {
                    prefix: "Node".to_string(),
                    type_name: supergraph_type_name,
                }))
            }

            fn add_concrete_field_edge(
                &mut self,
                head: NodeIndex,
                tail: NodeIndex,
                supergraph_field_name: Name,
                _self_conditions: IndexSet<SelfConditionIndex>,
                source_data: SourceFederatedConcreteFieldQueryGraphEdge,
            ) -> Result<EdgeIndex, FederationError> {
                let SourceFederatedConcreteFieldQueryGraphEdge::Connect(_data) = source_data else {
                    unreachable!()
                };

                Ok(self.graph.add_edge(
                    head,
                    tail,
                    MockEdge {
                        field_name: supergraph_field_name,
                    },
                ))
            }

            fn add_scalar_node(
                &mut self,
                supergraph_type_name: NamedType,
                _source_data: SourceFederatedScalarQueryGraphNode,
            ) -> Result<NodeIndex, FederationError> {
                Ok(self.graph.add_node(MockNode {
                    prefix: "Scalar".to_string(),
                    type_name: supergraph_type_name,
                }))
            }

            // ---------------------------------
            // -- Everything below is todo!() --
            // ---------------------------------

            fn source_query_graph(&mut self) -> &mut SourceFederatedQueryGraphs {
                todo!()
            }

            fn add_and_set_current_source(
                &mut self,
                _source: SourceId,
            ) -> Result<(), FederationError> {
                todo!()
            }

            fn get_current_source(&self) -> Result<SourceId, FederationError> {
                todo!()
            }

            fn add_self_condition(
                &mut self,
                _supergraph_type_name: NamedType,
                _field_set: &str,
            ) -> Result<SelfConditionIndex, FederationError> {
                todo!()
            }

            fn add_abstract_node(
                &mut self,
                _supergraph_type_name: NamedType,
                _source_data: SourceFederatedAbstractFieldQueryGraphEdge,
            ) -> Result<NodeIndex, FederationError> {
                todo!()
            }

            fn add_enum_node(
                &mut self,
                _supergraph_type_name: NamedType,
                _source_data: SourceFederatedEnumQueryGraphNode,
            ) -> Result<NodeIndex, FederationError> {
                todo!()
            }

            fn add_abstract_field_edge(
                &mut self,
                _head: NodeIndex,
                _tail: NodeIndex,
                _supergraph_field_name: Name,
                _self_conditions: IndexSet<SelfConditionIndex>,
                _source_data: SourceFederatedAbstractFieldQueryGraphEdge,
            ) -> Result<EdgeIndex, FederationError> {
                todo!()
            }

            fn add_type_condition_edge(
                &mut self,
                _head: NodeIndex,
                _tail: NodeIndex,
                _source_data: SourceFederatedTypeConditionQueryGraphEdge,
            ) -> Result<EdgeIndex, FederationError> {
                todo!()
            }

            fn is_for_query_planning(&self) -> bool {
                todo!()
            }

            fn add_source_enter_edge(
                &mut self,
                _tail: NodeIndex,
                _self_conditions: Option<SelfConditionIndex>,
                _source_data: crate::sources::SourceFederatedSourceEnterQueryGraphEdge,
            ) -> Result<EdgeIndex, FederationError> {
                todo!()
            }
        }
    }
}
