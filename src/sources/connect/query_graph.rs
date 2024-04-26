use apollo_compiler::{
    ast::{FieldDefinition, Name},
    schema::Component,
};
use indexmap::{map::Entry, IndexMap, IndexSet};
use itertools::Itertools;
use petgraph::prelude::NodeIndex;

use crate::{
    error::FederationError,
    schema::{position::TypeDefinitionPosition, ValidFederationSchema},
    source_aware::federated_query_graph::builder::IntraSourceQueryGraphBuilderApi,
    sources::{
        connect::ConnectFederatedScalarQueryGraphNode, FederatedLookupTailData,
        SourceFederatedConcreteFieldQueryGraphEdge, SourceFederatedConcreteQueryGraphNode,
        SourceFederatedEnumQueryGraphNode, SourceFederatedQueryGraphBuilderApi,
        SourceFederatedScalarQueryGraphNode,
    },
    ValidFederationSubgraph,
};

use super::{
    selection_parser::{NamedSelection, PathSelection, Property, SubSelection},
    spec::schema::{ConnectDirectiveArguments, CONNECT_DIRECTIVE_NAME_IN_SPEC},
    ConnectFederatedConcreteFieldQueryGraphEdge, ConnectFederatedConcreteQueryGraphNode,
    ConnectFederatedEnumQueryGraphNode, ConnectFederatedQueryGraphBuilder, Selection,
};

impl SourceFederatedQueryGraphBuilderApi for ConnectFederatedQueryGraphBuilder {
    fn process_subgraph_schema(
        &self,
        subgraph: ValidFederationSubgraph,
        builder: &mut impl IntraSourceQueryGraphBuilderApi,
    ) -> Result<Vec<FederatedLookupTailData>, FederationError> {
        // Grab all connects
        // TODO: Handle renames
        let connect_references = subgraph
            .schema
            .referencers()
            .get_directive(&CONNECT_DIRECTIVE_NAME_IN_SPEC)?;

        // Extract the connects from the schema definition and map them to their `Connect` equivalent
        // Note: We need to keep the object_field as well as the arguments since it contains important
        // type information of the original schema.
        // TODO: Validate that we don't have connect directives on anything but object fields.
        let connects: Vec<_> = connect_references
            .object_fields
            .iter()
            .map(|object_field| {
                // Note: We need to keep the `object_field` around since it contains metadata needed for graph
                // creation.
                object_field
                    .get(subgraph.schema.schema())
                    .map(move |f| (object_field, f))
            })
            .map_ok(|(object_field, field)| {
                field
                    .directives
                    .iter()
                    .map(move |d| (object_field, field, d))
            })
            .flatten_ok()
            .map_ok(|(object_field, field, directive)| {
                ConnectDirectiveArguments::try_from(directive)
                    .map(move |args| (object_field, field, args))
            })
            .flatten_ok()
            .try_collect()?;

        for (object, field, args) in connects {
            // Make a node for the entrypoint of this field, if not yet created
            let parent_node = builder.add_concrete_node(
                object.type_name.clone(),
                SourceFederatedConcreteQueryGraphNode::Connect(
                    ConnectFederatedConcreteQueryGraphNode::ConnectParent {
                        subgraph_type: object.parent().clone(),
                    },
                ),
            )?;

            // Process the field, constructing the rest of the graph for its selections
            // TODO: What should we do if the selection is empty? Make a selection with all fields?
            let field_node =
                process_selection(args.selection.unwrap(), field, &subgraph.schema, builder)?;

            // Make an edge from the parent into our new subgraph
            builder.add_concrete_field_edge(
                parent_node,
                field_node,
                field.name.clone(),
                IndexSet::new(),
                SourceFederatedConcreteFieldQueryGraphEdge::Connect(
                    ConnectFederatedConcreteFieldQueryGraphEdge::Connect {
                        subgraph_field: object.clone(),
                    },
                ),
            )?;
        }

        Ok(Vec::new())
    }
}

/// Processes a connect selection
///
/// This method creates nodes from selection parameters of a field decorated by
/// a connect directive, making sure to reuse nodes if possible.
fn process_selection(
    selection: Selection,
    field: &Component<FieldDefinition>,
    subgraph_schema: &ValidFederationSchema,
    builder: &mut impl IntraSourceQueryGraphBuilderApi,
) -> Result<NodeIndex<u32>, FederationError> {
    // Keep a cache to reuse nodes
    let mut node_cache: IndexMap<Name, NodeIndex<u32>> = IndexMap::new();

    // Get the type of the field
    let field_pos = subgraph_schema.get_type(field.ty.inner_named_type().clone())?;
    let field_ty = field_pos.get(subgraph_schema.schema())?;

    // Custom scalars are easy, so handle them first
    if field_ty.is_scalar() && !field_ty.is_built_in() {
        // Note: the if condition checked that this is a scalar, so trying to unwrap to anything else
        // is impossible.
        let TypeDefinitionPosition::Scalar(scalar_field_ty) = field_pos else {
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
        Selection::Path(path) => match field_pos {
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
                    field,
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
                field,
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
    field: &Component<FieldDefinition>,
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
    let field_pos = subgraph_schema.get_type(field.ty.inner_named_type().clone())?;
    let field_ty = field_pos.get(subgraph_schema.schema())?;

    // For milestone 1 we don't need to support anything other than objects...
    let TypeDefinitionPosition::Object(object_pos) = field_pos else {
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
    use insta::{assert_debug_snapshot, assert_snapshot};

    use crate::{
        schema::ValidFederationSchema, sources::SourceFederatedQueryGraphBuilderApi,
        ValidFederationSubgraph,
    };

    use super::ConnectFederatedQueryGraphBuilder;

    #[test]
    fn it_creates_a_connect_graph() {
        let federated_builder = ConnectFederatedQueryGraphBuilder;
        let mut mock_builder = mock::MockSourceQueryGraphBuilder::new();
        let subgraph = parse_schema(
            r#"
            extend schema
             @link(url: "https://specs.apollo.dev/connect/v0.1", import: ["@connect", "@source"])
             @source(
               name: "json"
               http: {
                 baseURL: "https://jsonplaceholder.typicode.com/",
                 headers: [
                   {
                     name: "X-Auth-Token",
                     as: "AuthToken"
                   },
                   {
                     name: "user-agent",
                     value: "Firefox"
                   },
                   { name: "X-From-Env" }
                 ]
               }
             )

            type Query {
              users: [User]
               @connect(
                 source: "json"
                 http: { GET: "/users" }
                 selection: "id name"
               )

              posts: [Post]
               @connect(
                 source: "json"
                 http: { GET: "/posts" }
                 selection: "id title body"
               )
            }

            type User {
              id: ID!
              name: String
            }

            type Post {
              id: ID!
              title: String
              body: String
            }
        "#,
        );

        // Make sure that the tail data is correct
        let results = federated_builder
            .process_subgraph_schema(subgraph, &mut mock_builder)
            .unwrap();
        assert_debug_snapshot!(results, @"[]");

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

    /// Helper for creating a subgraph from a schema
    fn parse_schema(schema_src: &str) -> ValidFederationSubgraph {
        let schema_with_directives = format!(
            r#"
                {schema_src}
                {}
                {}
            "#,
            constants::TEMP_FEDERATION_DEFINITIONS,
            constants::TEMP_SOURCE_DEFINITIONS
        );
        let schema = Schema::parse(schema_with_directives, "schema.graphql").unwrap();
        let schema = ValidFederationSchema::new(schema.validate().unwrap()).unwrap();

        ValidFederationSubgraph {
            name: "test".to_string(),
            url: "https://example.com/placeholder".to_string(),
            schema,
        }
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
        }
    }

    mod constants {
        pub static TEMP_FEDERATION_DEFINITIONS: &str = r#"
            directive @link(url: String, as: String, for: link__Purpose, import: [link__Import]) repeatable on SCHEMA
            scalar link__Import

            enum link__Purpose {
              """
              \`SECURITY\` features provide metadata necessary to securely resolve fields.
              """
              SECURITY

              """
              \`EXECUTION\` features provide metadata necessary for operation execution.
              """
              EXECUTION
            }
        "#;

        pub static TEMP_SOURCE_DEFINITIONS: &str = r#"
            """
            Defines a connector as the implementation of a field.

            Exactly one of {http} must be present.
            """
            directive @connect(
              """
              Optionally connects a @source directive for shared connector configuration.
              Must match the `name:` argument of a @source directive in this schema.
              """
              source: String

              """
              Defines HTTP configuration for this connector.
              """
              http: ConnectHTTP

              """
              Uses the JSONSelection syntax to define a mapping of connector response
              to GraphQL schema.
              """
              selection: JSONSelection

              """
              Marks this connector as a canonical resolver for an entity (uniquely
              identified domain model.) If true, the connector must be defined on a
              field of the Query type.
              """
              entity: Boolean = false
            ) on FIELD_DEFINITION

            """
            HTTP configuration for a connector.

            Exactly one of {GET,POST,PATCH,PUT,DELETE} must be present.
            """
            input ConnectHTTP {
              """
              URL template for GET requests to an HTTP endpoint.

              Can be a full URL or a partial path. If it's a partial path, it will
              be appended to an associated `baseURL` from the related @source.
              """
              GET: URLPathTemplate

              "Same as GET but for POST requests"
              POST: URLPathTemplate

              "Same as GET but for PATCH requests"
              PATCH: URLPathTemplate

              "Same as GET but for PUT requests"
              PUT: URLPathTemplate

              "Same as GET but for DELETE requests"
              DELETE: URLPathTemplate

              """
              Define a request body using JSONSelection. Selections can include
              values from field arguments using `$args.argName` and from fields on the
              parent type using `$this.fieldName`.
              """
              body: JSONSelection

              """
              Configuration for headers to attach to the request.

              Takes precedence over headers defined on the associated @source.
              """
              headers: [HTTPHeaderMapping!]
            }

            """
            At most one of {as,value} can be present.
            """
            input HTTPHeaderMapping {
              "The name of the incoming HTTP header to propagate to the endpoint"
              name: String!

              "If present, this defines the name of the header in the endpoint request"
              as: String

              "If present, this defines values for the headers in the endpoint request"
              value: [String]
            }

            """
            Defines connector configuration for reuse across multiple connectors.

            Exactly one of {http} must be present.
            """
            directive @source(
              name: String!

              http: SourceHTTP
            ) on SCHEMA

            """
            Common HTTP configuration for connectors.
            """
            input SourceHTTP {
              """
              If the URL path template in a connector is not a valid URL, it will be appended
              to this URL. Must be a valid URL.
              """
              baseURL: String!

              """
              Common headers from related connectors.
              """
              headers: [HTTPHeaderMapping!]
            }

            """
            A string containing a "JSON Selection", which defines a mapping from one JSON-like
            shape to another JSON-like shape.

            Example: ".data { id: user_id name account: { id: account_id } }"
            """
            scalar JSONSelection @specifiedBy(url: "...")

            """
            A string that declares a URL path with values interpolated inside `{}`.

            Example: "/product/{$this.id}/reviews?count={$args.count}"
            """
            scalar URLPathTemplate @specifiedBy(url: "...")
        "#;
    }
}
