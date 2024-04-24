mod selection_parser;
mod spec;
mod url_path_template;

use crate::error::FederationError;
use crate::query_graph::build_query_graph::IntraSourceQueryGraphBuilderApi;
use crate::schema::position::{
    AbstractTypeDefinitionPosition, EnumTypeDefinitionPosition, ObjectFieldDefinitionPosition,
    ObjectOrInterfaceFieldDirectivePosition, ObjectOrInterfaceTypeDefinitionPosition,
    ObjectTypeDefinitionPosition, ScalarTypeDefinitionPosition,
};
use crate::sources::connect::selection_parser::Property;
use crate::sources::{FederatedLookupTailData, SourceFederatedQueryGraphBuilderApi};
use crate::ValidFederationSubgraph;
use apollo_compiler::executable::{Name, Value};
use apollo_compiler::NodeStr;
use indexmap::IndexMap;
pub use selection_parser::ApplyTo;
pub use selection_parser::ApplyToError;
pub use selection_parser::Selection;
pub use url_path_template::URLPathTemplate;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub(crate) struct ConnectId {
    subgraph: NodeStr,
    directive: ObjectOrInterfaceFieldDirectivePosition,
}

#[derive(Debug)]
pub(crate) struct ConnectFederatedQueryGraph {
    subgraph_schemas_by_name: IndexMap<NodeStr, ValidFederationSubgraph>,
    // source_directives_by_name: IndexMap<NodeStr, SourceDirectiveArguments>,
    // connect_directives_by_source: IndexMap<ConnectId, ConnectDirectiveArguments>,
}

#[derive(Debug)]
pub(crate) enum ConnectFederatedAbstractQueryGraphNode {
    SelectionRoot {
        subgraph_type: AbstractTypeDefinitionPosition,
        path_selection: Vec<Property>,
    },
    SelectionChild {
        subgraph_type: AbstractTypeDefinitionPosition,
    },
}

#[derive(Debug)]
pub(crate) enum ConnectFederatedConcreteQueryGraphNode {
    ConnectParent {
        subgraph_type: ObjectTypeDefinitionPosition,
    },
    SelectionRoot {
        subgraph_type: ObjectTypeDefinitionPosition,
        path_selection: Vec<Property>,
    },
    SelectionChild {
        subgraph_type: ObjectTypeDefinitionPosition,
    },
}

#[derive(Debug)]
pub(crate) enum ConnectFederatedEnumQueryGraphNode {
    SelectionRoot {
        subgraph_type: EnumTypeDefinitionPosition,
        path_selection: Vec<Property>,
    },
    SelectionChild {
        subgraph_type: EnumTypeDefinitionPosition,
    },
}

#[derive(Debug)]
pub(crate) enum ConnectFederatedScalarQueryGraphNode {
    SelectionRoot {
        subgraph_type: ScalarTypeDefinitionPosition,
        path_selection: Vec<Property>,
    },
    SelectionChild {
        subgraph_type: ScalarTypeDefinitionPosition,
    },
}

#[derive(Debug)]
pub(crate) struct ConnectFederatedAbstractFieldQueryGraphEdge;

#[derive(Debug)]

pub(crate) enum ConnectFederatedConcreteFieldQueryGraphEdge {
    Connect {
        subgraph_field: ObjectFieldDefinitionPosition,
    },
    Selection {
        subgraph_field: ObjectFieldDefinitionPosition,
        path_selection: Vec<Property>,
    },
}

#[derive(Debug)]
pub(crate) struct ConnectFederatedTypeConditionQueryGraphEdge;

#[derive(Debug)]
pub(crate) enum ConnectFederatedLookupQueryGraphEdge {
    ConnectParent {
        subgraph_type: ObjectOrInterfaceTypeDefinitionPosition,
    },
}

pub(crate) struct ConnectFederatedQueryGraphBuilder;

impl SourceFederatedQueryGraphBuilderApi for ConnectFederatedQueryGraphBuilder {
    fn process_subgraph_schema(
        &self,
        _subgraph: ValidFederationSubgraph,
        _builder: &mut impl IntraSourceQueryGraphBuilderApi,
    ) -> Result<Vec<FederatedLookupTailData>, FederationError> {
        todo!()
    }
}

#[derive(Debug)]
pub struct ConnectFetchNode {
    source_id: ConnectId,
    arguments: IndexMap<Name, Value>,
    selection: Selection,
}
