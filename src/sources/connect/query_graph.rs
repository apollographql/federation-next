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
                // Create the node for this enum
                builder.add_enum_node(
                    field_ty.name().clone(),
                    SourceFederatedEnumQueryGraphNode::Connect(
                        ConnectFederatedEnumQueryGraphNode::SelectionRoot {
                            subgraph_type: enum_type,
                            property_path: path.collect_paths(),
                        },
                    ),
                )
            }
            TypeDefinitionPosition::Scalar(scalar_type) => {
                // Create the node for this enum
                builder.add_scalar_node(
                    field_ty.name().clone(),
                    SourceFederatedScalarQueryGraphNode::Connect(
                        ConnectFederatedScalarQueryGraphNode::SelectionRoot {
                            subgraph_type: scalar_type,
                            property_path: path.collect_paths(),
                        },
                    ),
                )
            }

            _ => {
                // If we don't have either of the above, then we must have a subselection
                let Some(sub) = path.next_subselection() else {
                    todo!("handle error");
                };

                process_subselection(
                    sub,
                    field_output_type_pos,
                    subgraph_schema,
                    builder,
                    &mut node_cache,
                    Some(path.collect_paths()),
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
                &sub,
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
    sub: &SubSelection,
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
    let field_type_pos = object_pos.field(field_ty.name().clone());

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
    for selection in sub.selections.iter() {
        // Make sure that we have a field on the object type that matches the alias (or the name itself)
        let alias = selection.name();
        let Some(selection_field) = object_type.fields.get(alias) else {
            todo!("handle error");
        };
        let selection_type =
            subgraph_schema.get_type(selection_field.ty.inner_named_type().clone())?;
        let subgraph_field_pos = field_type_pos.clone();

        // Extract the property chain for this selection
        let properties = selection.property_path();
        let next_subselection = selection.next_subselection();

        // Now add sub type info to the graph
        match selection_type {
            TypeDefinitionPosition::Enum(ref r#enum) => {
                // An enum cannot have sub selections, so enforce that now
                if next_subselection.is_some() {
                    todo!("handle error");
                }

                // Create the scalar node (or grab it from the cache)
                let enum_node = match node_cache.entry(r#enum.type_name.clone()) {
                    Entry::Occupied(e) => e.into_mut(),
                    Entry::Vacant(e) => {
                        let node = builder.add_enum_node(
                            r#enum.type_name.clone(),
                            SourceFederatedEnumQueryGraphNode::Connect(
                                ConnectFederatedEnumQueryGraphNode::SelectionChild {
                                    subgraph_type: r#enum.clone(),
                                },
                            ),
                        )?;

                        e.insert(node)
                    }
                };

                // Link the field to the object node
                builder.add_concrete_field_edge(
                    object_node,
                    *enum_node,
                    selection_field.name.clone(),
                    IndexSet::new(),
                    SourceFederatedConcreteFieldQueryGraphEdge::Connect(
                        ConnectFederatedConcreteFieldQueryGraphEdge::Selection {
                            subgraph_field: subgraph_field_pos,
                            property_path: properties,
                        },
                    ),
                )?;
            }
            TypeDefinitionPosition::Scalar(ref scalar) => {
                // Custom scalars need to be handled differently
                if next_subselection.is_some() {
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
                        ConnectFederatedConcreteFieldQueryGraphEdge::Selection {
                            subgraph_field: subgraph_field_pos,
                            property_path: properties,
                        },
                    ),
                )?;
            }

            // The other types must be composite
            other => {
                // Since the type must be composite, there HAS to be a subselection
                let Some(subselection) = next_subselection else {
                    todo!("handle error");
                };

                let subselection_node = process_subselection(
                    subselection,
                    other,
                    subgraph_schema,
                    builder,
                    node_cache,
                    None,
                )?;

                // Link the field to the object node
                builder.add_concrete_field_edge(
                    object_node,
                    subselection_node,
                    selection_field.name.clone(),
                    IndexSet::new(),
                    SourceFederatedConcreteFieldQueryGraphEdge::Connect(
                        ConnectFederatedConcreteFieldQueryGraphEdge::Selection {
                            subgraph_field: subgraph_field_pos,
                            property_path: properties,
                        },
                    ),
                )?;
            }
        }
    }

    // Handle the optional star selection
    if let Some(_star) = sub.star.as_ref() {
        //
    }

    Ok(object_node)
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

    fn get_subgraphs(supergraph_sdl: &str) -> ValidFederationSubgraphs {
        let schema = Schema::parse(supergraph_sdl, "supergraph.graphql").unwrap();
        let supergraph_schema = FederationSchema::new(schema).unwrap();
        extract_subgraphs_from_supergraph(&supergraph_schema, Some(true)).unwrap()
    }

    #[test]
    fn it_handles_a_simple_schema() {
        let federated_builder = ConnectFederatedQueryGraphBuilder;
        let mut mock_builder = mock::MockSourceQueryGraphBuilder::new();
        let subgraphs = get_subgraphs(include_str!("./tests/schemas/simple.graphql"));
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

    #[test]
    fn it_handles_an_aliased_schema() {
        let federated_builder = ConnectFederatedQueryGraphBuilder;
        let mut mock_builder = mock::MockSourceQueryGraphBuilder::new();
        let subgraphs = get_subgraphs(include_str!("./tests/schemas/aliasing.graphql"));
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
            1 -> 3 [ label = "name: .username" ]
            0 -> 1 [ label = "users" ]
            5 -> 6 [ label = "id" ]
            5 -> 7 [ label = "title: .\"body title\"" ]
            5 -> 7 [ label = "body: .summary" ]
            4 -> 5 [ label = "posts" ]
        }
        "###
        );
    }

    #[test]
    fn it_handles_a_cyclical_schema() {
        let federated_builder = ConnectFederatedQueryGraphBuilder;
        let mut mock_builder = mock::MockSourceQueryGraphBuilder::new();
        let subgraphs = get_subgraphs(include_str!("./tests/schemas/cyclical.graphql"));
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
            4 [ label = "Node: User" ]
            5 [ label = "Node: Query" ]
            6 [ label = "Node: User" ]
            7 [ label = "Scalar: ID" ]
            8 [ label = "Scalar: String" ]
            9 [ label = "Node: User" ]
            1 -> 2 [ label = "id" ]
            1 -> 3 [ label = "name" ]
            4 -> 2 [ label = "id" ]
            1 -> 4 [ label = "friends" ]
            0 -> 1 [ label = "me" ]
            6 -> 7 [ label = "id" ]
            6 -> 8 [ label = "name" ]
            9 -> 7 [ label = "id" ]
            6 -> 9 [ label = "friends" ]
            5 -> 6 [ label = "user" ]
        }
        "###
        );
    }

    #[test]
    fn it_handles_a_nested_schema() {
        let federated_builder = ConnectFederatedQueryGraphBuilder;
        let mut mock_builder = mock::MockSourceQueryGraphBuilder::new();
        let subgraphs = get_subgraphs(include_str!("./tests/schemas/nested.graphql"));
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
            3 [ label = "Node: UserInfo" ]
            4 [ label = "Scalar: String" ]
            5 [ label = "Node: UserAddress" ]
            6 [ label = "Scalar: Int" ]
            7 [ label = "Node: UserAvatar" ]
            1 -> 2 [ label = "id" ]
            3 -> 4 [ label = "name: .\"user full name\"" ]
            5 -> 4 [ label = "street: .street_line" ]
            5 -> 4 [ label = "state" ]
            5 -> 6 [ label = "zip" ]
            3 -> 5 [ label = "address: .addresses.main.address" ]
            7 -> 4 [ label = "large" ]
            7 -> 4 [ label = "thumbnail" ]
            3 -> 7 [ label = "avatar" ]
            1 -> 3 [ label = "info: .user_info" ]
            0 -> 1 [ label = "user" ]
        }
        "###
        );
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
                connect::{
                    selection_parser::Property, ConnectFederatedConcreteFieldQueryGraphEdge,
                },
                SourceFederatedAbstractFieldQueryGraphEdge,
                SourceFederatedConcreteFieldQueryGraphEdge, SourceFederatedConcreteQueryGraphNode,
                SourceFederatedEnumQueryGraphNode, SourceFederatedQueryGraph,
                SourceFederatedScalarQueryGraphNode, SourceFederatedSourceEnteringQueryGraphEdge,
                SourceFederatedTypeConditionQueryGraphEdge, SourceId,
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
            path: Vec<Property>,
        }
        impl Display for MockEdge {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.field_name)?;

                // Short out if we have no path to display
                if self.path.is_empty() {
                    return Ok(());
                }

                // Helper for checking name equality of a property
                fn is_name_eq(prop: &Property, other: &str) -> bool {
                    match prop {
                        Property::Field(f) => f == other,
                        Property::Quoted(q) => q == other,

                        // No string name will be equal to a number
                        Property::Index(_) => false,
                    }
                }

                // Short out if we have the same path as the identifier
                if self.path.len() == 1 && is_name_eq(&self.path[0], self.field_name.as_str()) {
                    return Ok(());
                }

                // Show the equivalent JSON path, if different from the field itself
                write!(f, ": ")?;
                for path in &self.path {
                    write!(f, "{path}")?;
                }

                Ok(())
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
                let SourceFederatedConcreteFieldQueryGraphEdge::Connect(data) = source_data else {
                    unreachable!()
                };

                let path = match data {
                    ConnectFederatedConcreteFieldQueryGraphEdge::Selection {
                        property_path,
                        ..
                    } => property_path,
                    _ => Vec::new(),
                };
                Ok(self.graph.add_edge(
                    head,
                    tail,
                    MockEdge {
                        field_name: supergraph_field_name,
                        path,
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

            fn source_query_graph(
                &mut self,
            ) -> Result<&mut SourceFederatedQueryGraph, FederationError> {
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

            fn add_source_entering_edge(
                &mut self,
                _tail: NodeIndex,
                _self_conditions: Option<SelfConditionIndex>,
                _source_data: SourceFederatedSourceEnteringQueryGraphEdge,
            ) -> Result<EdgeIndex, FederationError> {
                todo!()
            }
        }
    }
}
