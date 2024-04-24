use crate::error::{FederationError, SingleFederationError};
use crate::query_plan::operation::normalized_field_selection::NormalizedField;
use crate::query_plan::operation::normalized_inline_fragment_selection::NormalizedInlineFragment;
use crate::query_plan::operation::NormalizedSelectionSet;
use crate::schema::position::{
    AbstractFieldDefinitionPosition, AbstractTypeDefinitionPosition,
    CompositeTypeDefinitionPosition, EnumTypeDefinitionPosition, FieldDefinitionPosition,
    InterfaceFieldDefinitionPosition, ObjectFieldDefinitionPosition, ObjectTypeDefinitionPosition,
    OutputTypeDefinitionPosition, ScalarTypeDefinitionPosition, SchemaRootDefinitionKind,
};
use crate::schema::ValidFederationSchema;
use crate::sources::{
    SourceFederatedAbstractFieldQueryGraphEdge, SourceFederatedAbstractQueryGraphNode,
    SourceFederatedConcreteFieldQueryGraphEdge, SourceFederatedConcreteQueryGraphNode,
    SourceFederatedEnumQueryGraphNode, SourceFederatedLookupQueryGraphEdge,
    SourceFederatedQueryGraphs, SourceFederatedScalarQueryGraphNode,
    SourceFederatedTypeConditionQueryGraphEdge, SourceId,
};
use apollo_compiler::schema::{Name, NamedType};
use apollo_compiler::NodeStr;
use indexmap::{IndexMap, IndexSet};
use petgraph::graph::{DiGraph, EdgeIndex, EdgeReference, NodeIndex};
use petgraph::Direction;
use std::fmt::{Display, Formatter};
use std::hash::Hash;

pub mod build_query_graph;
pub(crate) mod condition_resolver;
pub(crate) mod extract_subgraphs_from_supergraph;
mod field_set;
pub(crate) mod graph_path;
pub mod output;
pub(crate) mod path_tree;

pub use build_query_graph::build_federated_query_graph;

#[derive(Debug)]
pub(crate) enum FederatedQueryGraphNode {
    Abstract {
        supergraph_type: AbstractTypeDefinitionPosition,
        fields: IndexMap<AbstractFieldDefinitionPosition, IndexSet<EdgeIndex>>,
        type_conditions: IndexMap<CompositeTypeDefinitionPosition, IndexSet<EdgeIndex>>,
        lookups: IndexMap<NodeIndex, IndexSet<EdgeIndex>>,
        source_id: SourceId,
        source_data: SourceFederatedAbstractQueryGraphNode,
    },
    Concrete {
        supergraph_type: ObjectTypeDefinitionPosition,
        fields: IndexMap<ObjectFieldDefinitionPosition, IndexSet<EdgeIndex>>,
        lookups: IndexMap<NodeIndex, IndexSet<EdgeIndex>>,
        source_id: SourceId,
        source_data: SourceFederatedConcreteQueryGraphNode,
    },
    Enum {
        supergraph_type: EnumTypeDefinitionPosition,
        source_id: SourceId,
        source_data: SourceFederatedEnumQueryGraphNode,
    },
    Scalar {
        supergraph_type: ScalarTypeDefinitionPosition,
        source_id: SourceId,
        source_data: SourceFederatedScalarQueryGraphNode,
    },
}

impl FederatedQueryGraphNode {
    pub(crate) fn supergraph_type(&self) -> OutputTypeDefinitionPosition {
        match self {
            FederatedQueryGraphNode::Abstract {
                supergraph_type, ..
            } => supergraph_type.clone().into(),
            FederatedQueryGraphNode::Concrete {
                supergraph_type, ..
            } => supergraph_type.clone().into(),
            FederatedQueryGraphNode::Enum {
                supergraph_type, ..
            } => supergraph_type.clone().into(),
            FederatedQueryGraphNode::Scalar {
                supergraph_type, ..
            } => supergraph_type.clone().into(),
        }
    }

    pub(crate) fn source_id(&self) -> &SourceId {
        match self {
            FederatedQueryGraphNode::Abstract { source_id, .. } => source_id,
            FederatedQueryGraphNode::Concrete { source_id, .. } => source_id,
            FederatedQueryGraphNode::Enum { source_id, .. } => source_id,
            FederatedQueryGraphNode::Scalar { source_id, .. } => source_id,
        }
    }
}

impl Display for FederatedQueryGraphNode {
    fn fmt(&self, _f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
        // write!(f, "{}({})", self.supergraph_type, self.source_id)?;
        // if let Some(provide_id) = self.provide_id {
        //     write!(f, "-{}", provide_id)?;
        // }
        // if self.root_kind.is_some() {
        //     write!(f, "*")?;
        // }
        // Ok(())
    }
}

#[derive(Debug)]
pub(crate) enum FederatedQueryGraphEdge {
    AbstractField {
        supergraph_field: AbstractFieldDefinitionPosition,
        self_conditions: Option<ConditionNormalizedSelectionSet>,
        matches_concrete_options: bool,
        source_id: SourceId,
        source_data: Option<SourceFederatedAbstractFieldQueryGraphEdge>,
    },
    ConcreteField {
        supergraph_field: ObjectFieldDefinitionPosition,
        self_conditions: Option<ConditionNormalizedSelectionSet>,
        source_id: SourceId,
        source_data: Option<SourceFederatedConcreteFieldQueryGraphEdge>,
    },
    TypeCondition {
        supergraph_type: CompositeTypeDefinitionPosition,
        source_id: SourceId,
        source_data: Option<SourceFederatedTypeConditionQueryGraphEdge>,
    },
    Lookup {
        supergraph_type: ObjectTypeDefinitionPosition,
        self_conditions: Option<ConditionNormalizedSelectionSet>,
        source_id: SourceId,
        source_data: Option<SourceFederatedLookupQueryGraphEdge>,
    },
}

impl Display for FederatedQueryGraphEdge {
    fn fmt(&self, _f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
        // if matches!(
        //     self.transition,
        //     QueryGraphEdgeTransition::SubgraphEnteringTransition
        // ) && self.conditions.is_none()
        // {
        //     return Ok(());
        // }
        // if let Some(conditions) = &self.conditions {
        //     write!(f, "{} ⊢ {}", conditions, self.transition)
        // } else {
        //     self.transition.fmt(f)
        // }
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub(crate) struct SelfConditionIndex(usize);

#[derive(Debug)]
pub(crate) struct ConditionNormalizedSelectionSet(NormalizedSelectionSet);

#[derive(Debug, Clone, PartialEq, Eq, Hash, derive_more::From, derive_more::IsVariant)]
pub(crate) enum QueryGraphNodeType {
    SchemaType(OutputTypeDefinitionPosition),
    FederatedRootType(SchemaRootDefinitionKind),
}

impl Display for QueryGraphNodeType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            QueryGraphNodeType::SchemaType(pos) => pos.fmt(f),
            QueryGraphNodeType::FederatedRootType(root_kind) => {
                write!(f, "[{root_kind}]")
            }
        }
    }
}

/// The type of query graph edge "transition".
///
/// An edge transition encodes what the edge corresponds to, in the underlying GraphQL schema.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum QueryGraphEdgeTransition {
    /// A field edge, going from (a node for) the field parent type to the field's (base) type.
    FieldCollection {
        /// The name of the schema containing the field.
        source: NodeStr,
        /// The object/interface field being collected.
        field_definition_position: FieldDefinitionPosition,
        /// Whether this field is part of an @provides.
        is_part_of_provides: bool,
    },
    /// A downcast edge, going from a composite type (object, interface, or union) to another
    /// composite type that intersects that type (i.e. has at least one possible runtime object type
    /// in common with it).
    Downcast {
        /// The name of the schema containing the from/to types.
        source: NodeStr,
        /// The parent type of the type condition, i.e. the type of the selection set containing
        /// the type condition.
        from_type_position: CompositeTypeDefinitionPosition,
        /// The type of the type condition, i.e. the type coming after "... on".
        to_type_position: CompositeTypeDefinitionPosition,
    },
    /// A key edge (only found in federated query graphs) going from an entity type in a particular
    /// subgraph to the same entity type but in another subgraph. Key transition edges _must_ have
    /// `conditions` corresponding to the key fields.
    KeyResolution,
    /// A root type edge (only found in federated query graphs) going from a root type (query,
    /// mutation or subscription) of a subgraph to the (same) root type of another subgraph. It
    /// encodes the fact that if a subgraph field returns a root type, any subgraph can be queried
    /// from there.
    RootTypeResolution {
        /// The kind of schema root resolved.
        root_kind: SchemaRootDefinitionKind,
    },
    /// A subgraph-entering edge, which is a special case only used for edges coming out of the root
    /// nodes of "federated" query graphs. It does not correspond to any physical GraphQL elements
    /// but can be understood as the fact that the router is always free to start querying any of
    /// the subgraph services as needed.
    SubgraphEnteringTransition,
    /// A "fake" downcast edge (only found in federated query graphs) going from an @interfaceObject
    /// type to an implementation. This encodes the fact that an @interfaceObject type "stands-in"
    /// for any possible implementations (in the supergraph) of the corresponding interface. It is
    /// "fake" because the corresponding edge stays on the @interfaceObject type (this is also why
    /// the "to type" is only a name: that to/casted type does not actually exist in the subgraph
    /// in which the corresponding edge will be found).
    InterfaceObjectFakeDownCast {
        /// The name of the schema containing the from type.
        source: NodeStr,
        /// The parent type of the type condition, i.e. the type of the selection set containing
        /// the type condition.
        from_type_position: CompositeTypeDefinitionPosition,
        /// The type of the type condition, i.e. the type coming after "... on".
        to_type_name: Name,
    },
}

impl QueryGraphEdgeTransition {
    pub(crate) fn collect_operation_elements(&self) -> bool {
        match self {
            QueryGraphEdgeTransition::FieldCollection { .. } => true,
            QueryGraphEdgeTransition::Downcast { .. } => true,
            QueryGraphEdgeTransition::KeyResolution => false,
            QueryGraphEdgeTransition::RootTypeResolution { .. } => false,
            QueryGraphEdgeTransition::SubgraphEnteringTransition => false,
            QueryGraphEdgeTransition::InterfaceObjectFakeDownCast { .. } => true,
        }
    }
}

impl Display for QueryGraphEdgeTransition {
    fn fmt(&self, _f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
        // match self {
        //     QueryGraphEdgeTransition::FieldCollection {
        //         field_definition_position,
        //         ..
        //     } => {
        //         write!(f, "{}", field_definition_position.field_name())
        //     }
        //     QueryGraphEdgeTransition::Downcast {
        //         to_type_position, ..
        //     } => {
        //         write!(f, "... on {}", to_type_position.type_name())
        //     }
        //     QueryGraphEdgeTransition::KeyResolution => {
        //         write!(f, "key()")
        //     }
        //     QueryGraphEdgeTransition::RootTypeResolution { root_kind } => {
        //         write!(f, "{}()", root_kind)
        //     }
        //     QueryGraphEdgeTransition::SubgraphEnteringTransition => {
        //         write!(f, "∅")
        //     }
        //     QueryGraphEdgeTransition::InterfaceObjectFakeDownCast { to_type_name, .. } => {
        //         write!(f, "... on {}", to_type_name)
        //     }
        // }
    }
}

#[derive(Debug)]
pub struct FederatedQueryGraph {
    graph: DiGraph<FederatedQueryGraphNode, FederatedQueryGraphEdge>,
    supergraph_types_to_nodes: IndexMap<NamedType, IndexSet<NodeIndex>>,
    supergraph_root_kinds_to_nodes: IndexMap<SchemaRootDefinitionKind, NodeIndex>,
    self_conditions: Vec<NormalizedSelectionSet>,
    non_trivial_followup_edges: IndexMap<EdgeIndex, IndexSet<EdgeIndex>>,
    source_data: SourceFederatedQueryGraphs,
}

impl FederatedQueryGraph {
    pub(crate) fn name(&self) -> &str {
        todo!()
        // &self.current_source
    }

    pub(crate) fn graph(&self) -> &DiGraph<FederatedQueryGraphNode, FederatedQueryGraphEdge> {
        &self.graph
    }

    pub(crate) fn node_weight(
        &self,
        node: NodeIndex,
    ) -> Result<&FederatedQueryGraphNode, FederationError> {
        self.graph.node_weight(node).ok_or_else(|| {
            SingleFederationError::Internal {
                message: "Node unexpectedly missing".to_owned(),
            }
            .into()
        })
    }

    fn node_weight_mut(
        &mut self,
        node: NodeIndex,
    ) -> Result<&mut FederatedQueryGraphNode, FederationError> {
        self.graph.node_weight_mut(node).ok_or_else(|| {
            SingleFederationError::Internal {
                message: "Node unexpectedly missing".to_owned(),
            }
            .into()
        })
    }

    pub(crate) fn edge_weight(
        &self,
        edge: EdgeIndex,
    ) -> Result<&FederatedQueryGraphEdge, FederationError> {
        self.graph.edge_weight(edge).ok_or_else(|| {
            SingleFederationError::Internal {
                message: "Edge unexpectedly missing".to_owned(),
            }
            .into()
        })
    }

    fn edge_weight_mut(
        &mut self,
        edge: EdgeIndex,
    ) -> Result<&mut FederatedQueryGraphEdge, FederationError> {
        self.graph.edge_weight_mut(edge).ok_or_else(|| {
            SingleFederationError::Internal {
                message: "Edge unexpectedly missing".to_owned(),
            }
            .into()
        })
    }

    pub(crate) fn edge_endpoints(
        &self,
        edge: EdgeIndex,
    ) -> Result<(NodeIndex, NodeIndex), FederationError> {
        self.graph.edge_endpoints(edge).ok_or_else(|| {
            SingleFederationError::Internal {
                message: "Edge unexpectedly missing".to_owned(),
            }
            .into()
        })
    }

    pub(crate) fn schema(&self) -> Result<&ValidFederationSchema, FederationError> {
        todo!()
        // self.schema_by_source(&self.current_source)
    }

    pub(crate) fn schema_by_source(
        &self,
        _source: &str,
    ) -> Result<&ValidFederationSchema, FederationError> {
        todo!()
        // self.sources.get(source).ok_or_else(|| {
        //     SingleFederationError::Internal {
        //         message: "Schema unexpectedly missing".to_owned(),
        //     }
        //     .into()
        // })
    }

    pub(crate) fn sources(&self) -> impl Iterator<Item = &ValidFederationSchema> {
        // TODO (couldn't use todo!() here due to impl return)
        // self.sources.values()
        vec![].into_iter()
    }

    pub(crate) fn types_to_nodes(
        &self,
    ) -> Result<&IndexMap<NamedType, IndexSet<NodeIndex>>, FederationError> {
        todo!()
        // self.types_to_nodes_by_source(&self.current_source)
    }

    fn types_to_nodes_by_source(
        &self,
        _source: &str,
    ) -> Result<&IndexMap<NamedType, IndexSet<NodeIndex>>, FederationError> {
        todo!()
        // self.types_to_nodes_by_source.get(source).ok_or_else(|| {
        //     SingleFederationError::Internal {
        //         message: "Types-to-nodes map unexpectedly missing".to_owned(),
        //     }
        //     .into()
        // })
    }

    fn types_to_nodes_mut(
        &mut self,
    ) -> Result<&mut IndexMap<NamedType, IndexSet<NodeIndex>>, FederationError> {
        todo!()
        // self.types_to_nodes_by_source
        //     .get_mut(&self.current_source)
        //     .ok_or_else(|| {
        //         SingleFederationError::Internal {
        //             message: "Types-to-nodes map unexpectedly missing".to_owned(),
        //         }
        //         .into()
        //     })
    }

    pub(crate) fn root_kinds_to_nodes(
        &self,
    ) -> Result<&IndexMap<SchemaRootDefinitionKind, NodeIndex>, FederationError> {
        todo!()
        // self.root_kinds_to_nodes_by_source
        //     .get(&self.current_source)
        //     .ok_or_else(|| {
        //         SingleFederationError::Internal {
        //             message: "Root-kinds-to-nodes map unexpectedly missing".to_owned(),
        //         }
        //         .into()
        //     })
    }

    fn root_kinds_to_nodes_mut(
        &mut self,
    ) -> Result<&mut IndexMap<SchemaRootDefinitionKind, NodeIndex>, FederationError> {
        todo!()
        // self.root_kinds_to_nodes_by_source
        //     .get_mut(&self.current_source)
        //     .ok_or_else(|| {
        //         SingleFederationError::Internal {
        //             message: "Root-kinds-to-nodes map unexpectedly missing".to_owned(),
        //         }
        //         .into()
        //     })
    }

    pub(crate) fn non_trivial_followup_edges(&self) -> &IndexMap<EdgeIndex, IndexSet<EdgeIndex>> {
        &self.non_trivial_followup_edges
    }

    /// All outward edges from the given node (including self-key and self-root-type-resolution
    /// edges). Primarily used by `@defer`, when needing to re-enter a subgraph for a deferred
    /// section.
    pub(crate) fn out_edges_with_federation_self_edges(
        &self,
        node: NodeIndex,
    ) -> impl Iterator<Item = EdgeReference<FederatedQueryGraphEdge>> {
        self.graph.edges_directed(node, Direction::Outgoing)
    }

    /// The outward edges from the given node, minus self-key and self-root-type-resolution edges,
    /// as they're rarely useful (currently only used by `@defer`).
    pub(crate) fn out_edges(
        &self,
        _node: NodeIndex,
    ) -> impl Iterator<Item = EdgeReference<FederatedQueryGraphEdge>> {
        // TODO (couldn't use todo!() here due to impl return)
        // self.graph
        //     .edges_directed(node, Direction::Outgoing)
        //     .filter(|edge_ref| {
        //         !(edge_ref.source() == edge_ref.target()
        //             && matches!(
        //                 edge_ref.weight().transition,
        //                 QueryGraphEdgeTransition::KeyResolution
        //                     | QueryGraphEdgeTransition::RootTypeResolution { .. }
        //             ))
        //     })
        // self.sources.values()
        vec![].into_iter()
    }

    pub(crate) fn edge_for_field(
        &self,
        _node: NodeIndex,
        _field: &NormalizedField,
    ) -> Option<EdgeIndex> {
        todo!()
        // let mut candidates = self.out_edges(node).filter_map(|edge_ref| {
        //     let edge_weight = edge_ref.weight();
        //     let QueryGraphEdgeTransition::FieldCollection {
        //         field_definition_position,
        //         ..
        //     } = &edge_weight.transition
        //     else {
        //         return None;
        //     };
        //     // We explicitly avoid comparing parent type's here, to allow interface object
        //     // fields to match operation fields with the same name but differing types.
        //     if field.data().field_position.field_name() == field_definition_position.field_name() {
        //         Some(edge_ref.id())
        //     } else {
        //         None
        //     }
        // });
        // if let Some(candidate) = candidates.next() {
        //     // PORT_NOTE: The JS codebase used an assertion rather than a debug assertion here. We
        //     // consider it unlikely for there to be more than one candidate given all the code paths
        //     // that create edges, so we've downgraded this to a debug assertion.
        //     debug_assert!(
        //         candidates.next().is_none(),
        //         "Unexpectedly found multiple candidates",
        //     );
        //     Some(candidate)
        // } else {
        //     None
        // }
    }

    pub(crate) fn edge_for_inline_fragment(
        &self,
        _node: NodeIndex,
        _inline_fragment: &NormalizedInlineFragment,
    ) -> Option<EdgeIndex> {
        todo!()
        // let Some(type_condition_pos) = &inline_fragment.data().type_condition_position else {
        //     // No type condition means the type hasn't changed, meaning there is no edge to take.
        //     return None;
        // };
        // let mut candidates = self.out_edges(node).filter_map(|edge_ref| {
        //     let edge_weight = edge_ref.weight();
        //     let QueryGraphEdgeTransition::Downcast {
        //         to_type_position, ..
        //     } = &edge_weight.transition
        //     else {
        //         return None;
        //     };
        //     // We explicitly avoid comparing type kinds, to allow interface object types to
        //     // match operation inline fragments (where the supergraph type kind is interface,
        //     // but the subgraph type kind is object).
        //     if type_condition_pos.type_name() == to_type_position.type_name() {
        //         Some(edge_ref.id())
        //     } else {
        //         None
        //     }
        // });
        // if let Some(candidate) = candidates.next() {
        //     // PORT_NOTE: The JS codebase used an assertion rather than a debug assertion here. We
        //     // consider it unlikely for there to be more than one candidate given all the code paths
        //     // that create edges, so we've downgraded this to a debug assertion.
        //     debug_assert!(
        //         candidates.next().is_none(),
        //         "Unexpectedly found multiple candidates",
        //     );
        //     Some(candidate)
        // } else {
        //     None
        // }
    }

    /// Given the possible runtime types at the head of the given edge, returns the possible runtime
    /// types after traversing the edge.
    // PORT_NOTE: Named `updateRuntimeTypes` in the JS codebase.
    pub(crate) fn advance_possible_runtime_types(
        &self,
        _possible_runtime_types: &IndexSet<ObjectTypeDefinitionPosition>,
        _edge: Option<EdgeIndex>,
    ) -> Result<IndexSet<ObjectTypeDefinitionPosition>, FederationError> {
        todo!()
        // let Some(edge) = edge else {
        //     return Ok(possible_runtime_types.clone());
        // };
        //
        // let edge_weight = self.edge_weight(edge)?;
        // let (_, tail) = self.edge_endpoints(edge)?;
        // let tail_weight = self.node_weight(tail)?;
        // let QueryGraphNodeType::SchemaType(tail_type_pos) = &tail_weight.type_ else {
        //     return Err(FederationError::internal(
        //         "Unexpectedly encountered federation root node as tail node.",
        //     ));
        // };
        // return match &edge_weight.transition {
        //     QueryGraphEdgeTransition::FieldCollection {
        //         source,
        //         field_definition_position,
        //         ..
        //     } => {
        //         let Ok(_): Result<CompositeTypeDefinitionPosition, _> =
        //             tail_type_pos.clone().try_into()
        //         else {
        //             return Ok(IndexSet::new());
        //         };
        //         let schema = self.schema_by_source(source)?;
        //         let mut new_possible_runtime_types = IndexSet::new();
        //         for possible_runtime_type in possible_runtime_types {
        //             let field_pos =
        //                 possible_runtime_type.field(field_definition_position.field_name().clone());
        //             let Some(field) = field_pos.try_get(schema.schema()) else {
        //                 continue;
        //             };
        //             let field_type_pos: CompositeTypeDefinitionPosition = schema
        //                 .get_type(field.ty.inner_named_type().clone())?
        //                 .try_into()?;
        //             new_possible_runtime_types
        //                 .extend(schema.possible_runtime_types(field_type_pos)?);
        //         }
        //         Ok(new_possible_runtime_types)
        //     }
        //     QueryGraphEdgeTransition::Downcast {
        //         source,
        //         to_type_position,
        //         ..
        //     } => Ok(self
        //         .schema_by_source(source)?
        //         .possible_runtime_types(to_type_position.clone())?
        //         .intersection(possible_runtime_types)
        //         .cloned()
        //         .collect()),
        //     QueryGraphEdgeTransition::KeyResolution => {
        //         let tail_type_pos: CompositeTypeDefinitionPosition =
        //             tail_type_pos.clone().try_into()?;
        //         Ok(self
        //             .schema_by_source(&tail_weight.source)?
        //             .possible_runtime_types(tail_type_pos)?)
        //     }
        //     QueryGraphEdgeTransition::RootTypeResolution { .. } => {
        //         let OutputTypeDefinitionPosition::Object(tail_type_pos) = tail_type_pos.clone()
        //         else {
        //             return Err(FederationError::internal(
        //                 "Unexpectedly encountered non-object root operation type.",
        //             ));
        //         };
        //         Ok(IndexSet::from([tail_type_pos]))
        //     }
        //     QueryGraphEdgeTransition::SubgraphEnteringTransition => {
        //         let OutputTypeDefinitionPosition::Object(tail_type_pos) = tail_type_pos.clone()
        //         else {
        //             return Err(FederationError::internal(
        //                 "Unexpectedly encountered non-object root operation type.",
        //             ));
        //         };
        //         Ok(IndexSet::from([tail_type_pos]))
        //     }
        //     QueryGraphEdgeTransition::InterfaceObjectFakeDownCast { .. } => {
        //         Ok(possible_runtime_types.clone())
        //     }
        // };
    }

    pub(crate) fn get_locally_satisfiable_key(
        &self,
        _node: NodeIndex,
    ) -> Result<Option<NormalizedSelectionSet>, FederationError> {
        todo!()
    }

    pub(crate) fn is_cross_subgraph_edge(&self, _edge: EdgeIndex) -> Result<bool, FederationError> {
        todo!()
        // let (head, tail) = self.edge_endpoints(edge)?;
        // let head_weight = self.node_weight(head)?;
        // let tail_weight = self.node_weight(tail)?;
        // Ok(head_weight.source != tail_weight.source)
    }

    pub(crate) fn is_provides_edge(&self, _edge: EdgeIndex) -> Result<bool, FederationError> {
        todo!()
        // let edge_weight = self.edge_weight(edge)?;
        // let QueryGraphEdgeTransition::FieldCollection {
        //     is_part_of_provides,
        //     ..
        // } = &edge_weight.transition
        // else {
        //     return Ok(false);
        // };
        // Ok(*is_part_of_provides)
    }

    pub(crate) fn has_an_implementation_with_provides(
        &self,
        _source: &NodeStr,
        _interface_field_definition_position: InterfaceFieldDefinitionPosition,
    ) -> Result<bool, FederationError> {
        todo!()
    }
}
