/*
- [ ] ValidationError (maybe unnecessary?)
- [ ] satisfiabilityError
    - [ ] displayReasons
- [ ] subgraphNodes (maybe unnecessary?)
    - dependencies:
        - [ ] addSubgraphToASTNode
- [ ] shareableFieldNonIntersectingRuntimeTypesError
- [ ] shareableFieldMismatchedRuntimeTypesHint
        - dependencies:
            - [ ] printHumanReadableList
            - [ ] printSubgraphNames
- [ ] buildWitnessOperation
- [ ] buildWitnessNextStep
- [ ] buildWitnessField
- [ ] generateWitnessValue
- [ ] validateGraphComposition
- [x] computeSubgraphPaths (unused)
- [ ] initialSubgraphPaths
    - dependencies:
        - [ ] SchemaRootKind
        - [ ] federatedGraphRootTypeName
        - [ ] GraphPath.fromGraphRoot
- [ ] possibleRuntimeTypeNamesSorted
- [x] extractValidationError (unused)
- [ ] ValidationContext
    - [ ] constructor
        - dependencies:
            - [ ] validateSupergraph
            - [ ] joinSpec.typeDirective
            - [ ] joinSpec.fieldDirective
    - [ ] isShareable
- [ ] ValidationState
    - [ ] initial
        - dependencies:
            - [ ] TransitionPathWithLazyIndirectPaths.initial
            - [ ] ConditionResolver
    - [ ] validateTransition
        - dependencies:
            - [ ] Edge
    - [ ] currentSubgraphNames
    - [ ] currentSubgraphs
    - [x] toString
- [x] isSupersetOrEqual
- [x] VertexVisit
- [ ] ValidationTraversal
    - [ ] constructor
    - [ ] validate
    - [ ] handleState

- [ ] QueryGraph
- [ ] RootPath<Transition> (replaced with GraphPath?)
- [ ] GraphPath
    - [ ] .fromGraphRoot
    - [ ] .tailPossibleRuntimeTypes
- [ ] TransitionPathWithLazyIndirectPaths
- [ ] SchemaRootKind
- [ ] ConditionResolver
- [ ] Subgraph
- [ ] operationToDocument
- [ ] Operation
- [ ] Schema (is this FederatedSchema?)
*/

use std::{
    collections::HashMap,
    fmt::{self, Display, Formatter},
    sync::Arc,
};

use apollo_compiler::{
    ast::{DirectiveDefinition, FieldDefinition},
    execution::GraphQLError,
    NodeStr,
};
use itertools::Itertools;

use crate::{composition::satisfiability::traversal::ValidationTraversal, query_graph::QueryGraph};

use self::diagnostics::CompositionHint;

mod dependencies;
mod diagnostics;
mod traversal;

type TODO = usize;
static _TODO: TODO = 0;

pub(crate) fn validate_graph_composition(
    supergraph_schema: TODO, // Schema
    supergraph_api: Arc<QueryGraph>,
    federated_query_graph: Arc<QueryGraph>,
) -> Result<Vec<CompositionHint>, (Vec<GraphQLError>, Vec<CompositionHint>)> {
    ValidationTraversal::new(supergraph_schema, supergraph_api, federated_query_graph).validate()
}

struct ValidationState {
    /// Path in the supergraph corresponding to the current state.
    supergraph_path: TODO, // RootPath<Transition>

    /// All the possible paths we could be in the subgraph.
    subgraph_paths: Vec<TODO>, // TransitionPathWithLazyIndirectPaths<RootVertix>[]

    /// When we encounter an `@override`n field with a label condition, we record
    /// its value (T/F) as we traverse the graph. This allows us to ignore paths
    /// that can never be taken by the query planner (i.e. a path where the
    /// condition is T in one case and F in another).
    selected_override_conditions: HashMap<NodeStr, bool>,
}

impl ValidationState {
    fn initial(
        _supergraph_api: Arc<QueryGraph>,
        kind: TODO, // SchemaRootKind
        federated_query_graph: Arc<QueryGraph>,
        _condition_resolver: TODO, // ConditionResolver
        _override_conditions: HashMap<NodeStr, bool>,
    ) -> Result<Self, TODO> {
        Ok(Self {
            supergraph_path: _TODO, // GraphPath::from_graph_root(_supergraph_api, _kind),
            subgraph_paths: initial_subgraph_paths(kind, federated_query_graph)?, // .map(p => TransitionPathWithLazyIndirectPaths.initial(p, _condition_resolver, _override_conditions)),
            selected_override_conditions: Default::default(),
        })
    }

    /// Validates that the current state can always be advanced for the provided supergraph edge, and returns the updated state if
    /// so.
    ///
    /// @param supergraphEdge - the edge to try to advance from the current state.
    /// @return an object with `error` set if the state _cannot_ be properly advanced (and if so, `state` and `hint` will be `undefined`).
    ///  If the state can be successfully advanced, then `state` contains the updated new state. This *can* be `undefined` to signal
    ///  that the state _can_ be successfully advanced (no error) but is guaranteed to yield no results (in other words, the edge corresponds
    ///  to a type condition for which there cannot be any runtime types), in which case not further validation is necessary "from that branch".
    ///  Additionally, when the state can be successfully advanced, an `hint` can be optionally returned.
    ///
    fn validate_transition(
        &self,
        _context: &ValidationContext,
        _supergraph_edge: TODO, // Edge
    ) -> Result<(Self, CompositionHint), GraphQLError> {
        todo!()
    }

    fn current_subgraph_names(&self) -> Vec<NodeStr> {
        todo!()
    }

    fn current_subgraphs(&self) -> Vec<TODO> /* (name: NodeStr, subgraph: Subgraph)[] */ {
        todo!()
    }
}

impl Display for ValidationState {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let supergraph_path = self.supergraph_path;
        let subgraph_paths = self.subgraph_paths.iter().map(|p| p.to_string()).join(", ");
        write!(f, "{supergraph_path} <=> [{subgraph_paths}]")
    }
}

fn initial_subgraph_paths(
    _kind: TODO, // SchemaRootKind
    _subgraphs: Arc<QueryGraph>,
) -> Result<Vec<TODO>, TODO> /* RootPath<Transition>[], can error */ {
    todo!()
}

fn possible_runtime_type_names_sorted(_path: TODO /* RootPath<Transition> */) -> Vec<String> {
    todo!()
}

struct ValidationContext {
    supergraph_schema: TODO, // Schema
    join_type_directive: DirectiveDefinition,
    join_field_directive: DirectiveDefinition,
}

impl ValidationContext {
    fn new(_supergraph_schema: TODO /* Schema */) -> Self {
        // const [_, joinSpec] = validateSupergraph(supergraphSchema);
        // this.joinTypeDirective = joinSpec.typeDirective(supergraphSchema);
        // this.joinFieldDirective = joinSpec.fieldDirective(supergraphSchema);
        todo!()
    }

    /// A field is shareable if either:
    ///     1) there is not join__field, but multiple join__type
    ///     2) there is more than one join__field where the field is neither external nor overriden.
    // JS PORT NOTE: we need the field parent type, so this should be a different type
    fn is_shareable(&self, _field: FieldDefinition) -> bool {
        todo!()
    }
}
