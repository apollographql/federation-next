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
        - dependencies:
            - [ ] simpleValidationConditionResolver

- [ ] QueryGraph
- [ ] RootPath<Transition> (replaced with GraphPath?)
- [ ] GraphPath
    - [ ] .fromGraphRoot
    - [ ] .tailPossibleRuntimeTypes
- [ ] TransitionPathWithLazyIndirectPaths
    - dependencies:
        - [ ] IndirectPaths
        - [ ] advancePathWithNonCollectingAndTypePreservingTransitions
- [ ] SchemaRootKind
- [ ] ConditionResolver
- [ ] Subgraph
- [ ] operationToDocument
- [ ] Operation
- [ ] Schema (is this FederatedSchema?)
*/

use std::sync::Arc;

use apollo_compiler::{
    ast::{DirectiveDefinition, FieldDefinition},
    execution::GraphQLError,
};

use crate::{composition::satisfiability::traversal::ValidationTraversal, query_graph::QueryGraph};

use self::diagnostics::CompositionHint;

mod dependencies;
mod diagnostics;
mod state;
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
