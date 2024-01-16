use crate::error::FederationError;
use crate::error::SingleFederationError::Internal;
use crate::schema::position::{
    CompositeTypeDefinitionPosition, FieldDefinitionPosition, InterfaceTypeDefinitionPosition,
    SchemaRootDefinitionKind,
};
use crate::schema::ValidFederationSchema;
use apollo_compiler::ast::{Argument, DirectiveList, Name, OperationType};
use apollo_compiler::executable::{
    Field, Fragment, FragmentSpread, InlineFragment, Operation, Selection, SelectionSet,
    VariableDefinition,
};
use apollo_compiler::{name, Node};
use indexmap::{IndexMap, IndexSet};
use linked_hash_map::{Entry, LinkedHashMap};
use std::ops::Deref;
use std::sync::{atomic, Arc};

const TYPENAME_FIELD: Name = name!("__typename");

// Global storage for the counter used to uniquely identify selections
static NEXT_ID: atomic::AtomicUsize = atomic::AtomicUsize::new(1);

// opaque wrapper of the unique selection ID type
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) struct SelectionId(usize);

impl SelectionId {
    fn new() -> Self {
        // atomically increment global counter
        Self(NEXT_ID.fetch_add(1, atomic::Ordering::AcqRel))
    }
}

/// An analogue of the apollo-compiler type `Operation` with these changes:
/// - Stores the schema that the operation is queried against.
/// - Swaps `operation_type` with `root_kind` (using the analogous federation-next type).
/// - Encloses collection types in `Arc`s to facilitate cheaper cloning.
/// - Stores the fragments used by this operation (the executable document the operation was taken
///   from may contain other fragments that are not used by this operation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedOperation {
    pub(crate) schema: ValidFederationSchema,
    pub(crate) root_kind: SchemaRootDefinitionKind,
    pub(crate) name: Option<Name>,
    pub(crate) variables: Arc<Vec<Node<VariableDefinition>>>,
    pub(crate) directives: Arc<DirectiveList>,
    pub(crate) selection_set: NormalizedSelectionSet,
    pub(crate) fragments: Arc<IndexMap<Name, Node<NormalizedFragment>>>,
}

/// An analogue of the apollo-compiler type `SelectionSet` with these changes:
/// - For the type, stores the schema and the position in that schema instead of just the
///   `NamedType`.
/// - Stores selections in a map so they can be normalized efficiently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NormalizedSelectionSet {
    pub(crate) schema: ValidFederationSchema,
    pub(crate) type_position: CompositeTypeDefinitionPosition,
    pub(crate) selections: Arc<NormalizedSelectionMap>,
}

/// A "normalized" selection map is an optimized representation of a selection set which does not
/// contain selections with the same selection "key". Selections that do have the same key are
/// merged during the normalization process. By storing a selection set as a map, we can efficiently
/// merge/join multiple selection sets.
///
/// Note that this must be a `LinkedHashMap` so that removals don't change the order.
pub(crate) type NormalizedSelectionMap = LinkedHashMap<NormalizedSelectionKey, NormalizedSelection>;

/// A selection "key" (unrelated to the federation `@key` directive) is an identifier of a selection
/// (field, inline fragment, or fragment spread) that is used to determine whether two selections
/// can be merged.
///
/// In order to merge two selections they need to
/// * reference the same field/inline fragment
/// * specify the same directives
/// * directives have to be applied in the same order
/// * directive arguments order does not matter (they get automatically sorted by their names).
/// * selection cannot specify @defer directive
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum NormalizedSelectionKey {
    Field {
        // field alias (if specified) or field name in the resulting selection set
        response_name: Name,
        // directives applied on the field
        directives: Arc<DirectiveList>,
        // optional unique selection ID used to distinguish fields that cannot be merged (set if field is deferred)
        deferred_id: Option<SelectionId>,
    },
    FragmentSpread {
        // fragment name
        name: Name,
        // directives applied on the fragment spread
        directives: Arc<DirectiveList>,
        // optional unique selection ID used to distinguish spreads that cannot be merged (set if fragment spread is deferred)
        deferred_id: Option<SelectionId>,
    },
    InlineFragment {
        // optional type condition of a fragment
        type_condition: Option<Name>,
        // directives applied on a fragment
        directives: Arc<DirectiveList>,
        // optional unique selection ID used to distinguish fragments that cannot be merged (set if inline fragment is deferred)
        deferred_id: Option<SelectionId>,
    },
}

/// An analogue of the apollo-compiler type `Selection` that stores our other selection analogues
/// instead of the apollo-compiler types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NormalizedSelection {
    Field(Arc<NormalizedFieldSelection>),
    FragmentSpread(Arc<NormalizedFragmentSpreadSelection>),
    InlineFragment(Arc<NormalizedInlineFragmentSelection>),
}

impl NormalizedSelection {
    fn directives(&self) -> &Arc<DirectiveList> {
        match self {
            NormalizedSelection::Field(field_selection) => &field_selection.field.directives,
            NormalizedSelection::FragmentSpread(fragment_spread_selection) => {
                &fragment_spread_selection.directives
            }
            NormalizedSelection::InlineFragment(inline_fragment_selection) => {
                &inline_fragment_selection.inline_fragment.directives
            }
        }
    }
}

/// An analogue of the apollo-compiler type `Fragment` with these changes:
/// - Stores the type condition explicitly, which means storing the schema and position (in
///   apollo-compiler, this is in the `SelectionSet`).
/// - Encloses collection types in `Arc`s to facilitate cheaper cloning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NormalizedFragment {
    pub(crate) schema: ValidFederationSchema,
    pub(crate) name: Name,
    pub(crate) type_condition_position: CompositeTypeDefinitionPosition,
    pub(crate) directives: Arc<DirectiveList>,
    pub(crate) selection_set: NormalizedSelectionSet,
}

/// An analogue of the apollo-compiler type `Field` with these changes:
/// - Makes the selection set optional. This is because `NormalizedSelectionSet` requires a type of
///   `CompositeTypeDefinitionPosition`, which won't exist for fields returning a non-composite type
///   (scalars and enums).
/// - Stores the field data (other than the selection set) in `NormalizedField`, to facilitate
///   operation paths and graph paths.
/// - For the field definition, stores the schema and the position in that schema instead of just
///   the `FieldDefinition` (which contains no references to the parent type or schema).
/// - Encloses collection types in `Arc`s to facilitate cheaper cloning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NormalizedFieldSelection {
    pub(crate) field: NormalizedField,
    pub(crate) selection_set: Option<NormalizedSelectionSet>,
    pub(crate) sibling_typename: Option<Name>,
}

/// The non-selection-set data of `NormalizedFieldSelection`, used with operation paths and graph
/// paths.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct NormalizedField {
    pub(crate) schema: ValidFederationSchema,
    pub(crate) field_position: FieldDefinitionPosition,
    pub(crate) alias: Option<Name>,
    pub(crate) arguments: Arc<Vec<Node<Argument>>>,
    pub(crate) directives: Arc<DirectiveList>,
    selection_id: SelectionId,
}

impl NormalizedField {
    fn name(&self) -> &Name {
        self.field_position.field_name()
    }

    fn response_name(&self) -> Name {
        self.alias.clone().unwrap_or_else(|| self.name().clone())
    }
}

/// An analogue of the apollo-compiler type `FragmentSpread` with these changes:
/// - Stores the schema (may be useful for directives).
/// - Encloses collection types in `Arc`s to facilitate cheaper cloning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NormalizedFragmentSpreadSelection {
    pub(crate) schema: ValidFederationSchema,
    pub(crate) fragment_name: Name,
    pub(crate) directives: Arc<DirectiveList>,
    selection_id: SelectionId,
}

/// An analogue of the apollo-compiler type `InlineFragment` with these changes:
/// - Stores the inline fragment data (other than the selection set) in `NormalizedInlineFragment`,
///   to facilitate operation paths and graph paths.
/// - For the type condition, stores the schema and the position in that schema instead of just
///   the `NamedType`.
/// - Stores the parent type explicitly, which means storing the position (in apollo-compiler, this
///   is in the parent selection set).
/// - Encloses collection types in `Arc`s to facilitate cheaper cloning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NormalizedInlineFragmentSelection {
    pub(crate) inline_fragment: NormalizedInlineFragment,
    pub(crate) selection_set: NormalizedSelectionSet,
}

/// The non-selection-set data of `NormalizedInlineFragmentSelection`, used with operation paths and
/// graph paths.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct NormalizedInlineFragment {
    pub(crate) schema: ValidFederationSchema,
    pub(crate) parent_type_position: CompositeTypeDefinitionPosition,
    pub(crate) type_condition_position: Option<CompositeTypeDefinitionPosition>,
    pub(crate) directives: Arc<DirectiveList>,
    selection_id: SelectionId,
}

impl NormalizedSelectionSet {
    /// Normalize this selection set (merging selections with the same keys), with the following
    /// additional transformations:
    /// - Expand fragment spreads into inline fragments.
    /// - Remove `__schema` or `__type` introspection fields, as these shouldn't be handled by query
    ///   planning.
    /// - Hoist fragment spreads/inline fragments into their parents if they have no directives and
    ///   their parent type matches.
    ///
    /// Note this function asserts that the type of the selection set is a composite type (i.e. this
    /// isn't the empty selection set of some leaf field), and will return error if this is not the
    /// case.
    pub(crate) fn normalize_and_expand_fragments(
        selection_set: &SelectionSet,
        fragments: &IndexMap<Name, Node<Fragment>>,
        schema: &ValidFederationSchema,
        inline_fragment_spreads: bool,
    ) -> Result<NormalizedSelectionSet, FederationError> {
        let type_position: CompositeTypeDefinitionPosition =
            schema.get_type(selection_set.ty.clone())?.try_into()?;
        let mut normalized_selections = vec![];
        NormalizedSelectionSet::normalize_selections(
            &selection_set.selections,
            &type_position,
            &mut normalized_selections,
            fragments,
            schema,
            inline_fragment_spreads,
        )?;
        let mut merged = NormalizedSelectionSet {
            schema: schema.clone(),
            type_position,
            selections: Arc::new(LinkedHashMap::new()),
        };
        merged.merge_pairs_into(normalized_selections)?;
        Ok(merged)
    }

    /// A helper function for normalizing a list of selections into a destination.
    fn normalize_selections(
        selections: &[Selection],
        parent_type_position: &CompositeTypeDefinitionPosition,
        destination: &mut Vec<(NormalizedSelectionKey, NormalizedSelection)>,
        fragments: &IndexMap<Name, Node<Fragment>>,
        schema: &ValidFederationSchema,
        inline_fragment_spreads: bool,
    ) -> Result<(), FederationError> {
        for selection in selections {
            match selection {
                Selection::Field(field_selection) => {
                    let Some(normalized_field_selection) =
                        NormalizedFieldSelection::normalize_and_expand_fragments(
                            field_selection,
                            parent_type_position,
                            fragments,
                            schema,
                            inline_fragment_spreads,
                        )?
                    else {
                        continue;
                    };
                    let key: NormalizedSelectionKey = (&normalized_field_selection).into();
                    destination.push((
                        key,
                        NormalizedSelection::Field(Arc::new(normalized_field_selection)),
                    ));
                }
                Selection::FragmentSpread(fragment_spread_selection) => {
                    let Some(fragment) = fragments.get(&fragment_spread_selection.fragment_name)
                    else {
                        return Err(Internal {
                            message: format!(
                                "Fragment spread referenced non-existent fragment \"{}\"",
                                fragment_spread_selection.fragment_name,
                            ),
                        }
                        .into());
                    };

                    if inline_fragment_spreads {
                        // We can hoist/collapse named fragments if their type condition is on the
                        // parent type and they don't have any directives.
                        if fragment.type_condition() == parent_type_position.type_name()
                            && fragment_spread_selection.directives.is_empty()
                        {
                            NormalizedSelectionSet::normalize_selections(
                                &fragment.selection_set.selections,
                                parent_type_position,
                                destination,
                                fragments,
                                schema,
                                inline_fragment_spreads,
                            )?;
                        } else {
                            let normalized_inline_fragment_selection =
                                NormalizedFragmentSpreadSelection::normalize_and_expand_fragments(
                                    fragment_spread_selection,
                                    parent_type_position,
                                    fragments,
                                    schema,
                                )?;
                            let key: NormalizedSelectionKey =
                                (&normalized_inline_fragment_selection).into();
                            destination.push((
                                key,
                                NormalizedSelection::InlineFragment(Arc::new(
                                    normalized_inline_fragment_selection,
                                )),
                            ));
                        }
                    } else {
                        // if we don't expand fragments, we just convert FragmentSpread to NormalizedFragmentSpreadSelection
                        let normalized_fragment_spread =
                            NormalizedFragmentSpreadSelection::normalize(
                                fragment_spread_selection,
                                schema,
                            );
                        let key: NormalizedSelectionKey = (&normalized_fragment_spread).into();
                        destination.push((
                            key,
                            NormalizedSelection::FragmentSpread(Arc::new(
                                normalized_fragment_spread,
                            )),
                        ));
                    }
                }
                Selection::InlineFragment(inline_fragment_selection) => {
                    let is_on_parent_type =
                        if let Some(type_condition) = &inline_fragment_selection.type_condition {
                            type_condition == parent_type_position.type_name()
                        } else {
                            true
                        };
                    // We can hoist/collapse inline fragments if their type condition is on the
                    // parent type (or they have no type condition) and they don't have any
                    // directives.
                    //
                    // PORT_NOTE: The JS codebase didn't hoist inline fragments, only fragment
                    // spreads (presumably because named fragments would commonly be on the same
                    // type as their fragment spread usages). It should be fine to also hoist inline
                    // fragments though if we notice they're similarly useless (and presumably later
                    // transformations in the JS codebase would take care of this).
                    if is_on_parent_type && inline_fragment_selection.directives.is_empty() {
                        NormalizedSelectionSet::normalize_selections(
                            &inline_fragment_selection.selection_set.selections,
                            parent_type_position,
                            destination,
                            fragments,
                            schema,
                            inline_fragment_spreads,
                        )?;
                    } else {
                        let normalized_inline_fragment_selection =
                            NormalizedInlineFragmentSelection::normalize_and_expand_fragments(
                                inline_fragment_selection,
                                parent_type_position,
                                fragments,
                                schema,
                                inline_fragment_spreads,
                            )?;
                        let key: NormalizedSelectionKey =
                            (&normalized_inline_fragment_selection).into();
                        destination.push((
                            key,
                            NormalizedSelection::InlineFragment(Arc::new(
                                normalized_inline_fragment_selection,
                            )),
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    /// Merges the given normalized selection sets into this one.
    pub(crate) fn merge_into(
        &mut self,
        others: Vec<NormalizedSelectionSet>,
    ) -> Result<(), FederationError> {
        if !others.is_empty() {
            let mut pairs = vec![];
            for other in others {
                if other.schema != self.schema {
                    return Err(Internal {
                        message: "Cannot merge selection sets from different schemas".to_owned(),
                    }
                    .into());
                }
                if other.type_position != self.type_position {
                    return Err(Internal {
                        message: format!(
                            "Cannot merge selection set for type \"{}\" into a selection set for type \"{}\"",
                            other.type_position,
                            self.type_position,
                        ),
                    }.into());
                }
                let selections = Arc::try_unwrap(other.selections)
                    .unwrap_or_else(|selections| selections.deref().clone());
                for pair in selections {
                    pairs.push(pair);
                }
            }
            self.merge_pairs_into(pairs)?;
        }
        Ok(())
    }

    /// A helper function for merging a vector of (key, selection) pairs into this one.
    fn merge_pairs_into(
        &mut self,
        others: Vec<(NormalizedSelectionKey, NormalizedSelection)>,
    ) -> Result<(), FederationError> {
        if !others.is_empty() {
            let mut fields = IndexMap::new();
            let mut fragment_spreads = IndexMap::new();
            let mut inline_fragments = IndexMap::new();
            for (other_key, other_selection) in others {
                match Arc::make_mut(&mut self.selections).entry(other_key.clone()) {
                    Entry::Occupied(existing) => match existing.get() {
                        NormalizedSelection::Field(self_field_selection) => {
                            let NormalizedSelection::Field(other_field_selection) = other_selection
                            else {
                                return Err(Internal {
                                        message: format!(
                                            "Field selection key for field \"{}\" references non-field selection",
                                            self_field_selection.field.field_position,
                                        ),
                                    }.into());
                            };
                            let other_field_selection = Arc::try_unwrap(other_field_selection)
                                .unwrap_or_else(|selection| selection.deref().clone());
                            fields
                                .entry(other_key)
                                .or_insert_with(Vec::new)
                                .push(other_field_selection);
                        }
                        NormalizedSelection::FragmentSpread(self_fragment_spread_selection) => {
                            let NormalizedSelection::FragmentSpread(
                                other_fragment_spread_selection,
                            ) = other_selection
                            else {
                                return Err(Internal {
                                        message: format!(
                                            "Fragment spread selection key for fragment \"{}\" references non-field selection",
                                            self_fragment_spread_selection.fragment_name,
                                        ),
                                    }.into());
                            };
                            let other_fragment_spread_selection =
                                Arc::try_unwrap(other_fragment_spread_selection)
                                    .unwrap_or_else(|selection| selection.deref().clone());
                            fragment_spreads
                                .entry(other_key)
                                .or_insert_with(Vec::new)
                                .push(other_fragment_spread_selection);
                        }
                        NormalizedSelection::InlineFragment(self_inline_fragment_selection) => {
                            let NormalizedSelection::InlineFragment(
                                other_inline_fragment_selection,
                            ) = other_selection
                            else {
                                return Err(Internal {
                                        message: format!(
                                            "Inline fragment selection key under parent type \"{}\" {}references non-field selection",
                                            self_inline_fragment_selection.inline_fragment.parent_type_position,
                                            self_inline_fragment_selection.inline_fragment.type_condition_position.clone()
                                                .map_or_else(
                                                    String::new,
                                                    |cond| format!("(type condition: {}) ", cond),
                                                ),
                                        ),
                                    }.into());
                            };
                            let other_inline_fragment_selection =
                                Arc::try_unwrap(other_inline_fragment_selection)
                                    .unwrap_or_else(|selection| selection.deref().clone());
                            inline_fragments
                                .entry(other_key)
                                .or_insert_with(Vec::new)
                                .push(other_inline_fragment_selection);
                        }
                    },
                    Entry::Vacant(vacant) => {
                        vacant.insert(other_selection);
                    }
                }
            }
            for (key, self_selection) in Arc::make_mut(&mut self.selections).iter_mut() {
                match self_selection {
                    NormalizedSelection::Field(self_field_selection) => {
                        if let Some(other_field_selections) = fields.remove(key) {
                            Arc::make_mut(self_field_selection)
                                .merge_into(other_field_selections)?;
                        }
                    }
                    NormalizedSelection::FragmentSpread(self_fragment_spread_selection) => {
                        if let Some(other_fragment_spread_selections) = fragment_spreads.remove(key)
                        {
                            Arc::make_mut(self_fragment_spread_selection)
                                .merge_into(other_fragment_spread_selections)?;
                        }
                    }
                    NormalizedSelection::InlineFragment(self_inline_fragment_selection) => {
                        if let Some(other_inline_fragment_selections) = inline_fragments.remove(key)
                        {
                            Arc::make_mut(self_inline_fragment_selection)
                                .merge_into(other_inline_fragment_selections)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Modifies the provided selection set to optimize the handling of __typename selections for query planning.
    ///
    /// __typename information can always be provided by any subgraph declaring that type. While this data can be
    /// theoretically fetched from multiple sources, in practice it doesn't really matter which subgraph we use
    /// for the __typename and we should just get it from the same source as the one that was used to resolve
    /// other fields.
    ///
    /// In most cases, selecting __typename won't be a problem as query planning algorithm ignores "obviously"
    /// inefficient paths. Typically, querying the __typename of an entity is generally ok because when looking at
    /// a path, the query planning algorithm always favor getting a field "locally" if it can (which it always can
    /// for __typename) and ignore alternative that would jump subgraphs.
    ///
    /// When querying a __typename after a @shareable field, query planning algorithm would consider getting the
    /// __typename from EACH version of the @shareable field. This unnecessarily explodes the number of possible
    /// query plans with some useless options and results in degraded performance. Since the number of possible
    /// plans doubles for every field for which there is a choice, eliminating unnecessary choices improves query
    /// planning performance.
    ///
    /// It is unclear how to do this cleanly with the current planning algorithm, so this method is a workaround
    /// so we can efficiently generate query plans. In order to prevent the query planner from spending time
    /// exploring those useless __typename options, we "remove" the unnecessary __typename selections from the
    /// operation. Since we need to ensure that the __typename field will still need to be queried, we "tag"
    /// one of the "sibling" selections (using "attachement") to remember that __typename needs to be added
    /// back eventually. The core query planning algorithm will ignore that tag, and because __typename has been
    /// otherwise removed, we'll save any related work. As we build the final query plan, we'll check back for
    /// those "tags" and add back the __typename selections. As this only happen after the query planning
    /// algorithm has computed all choices, we achieve our goal of not considering useless choices due to
    /// __typename. Do note that if __typename is the "only" selection of some selection set, then we leave it
    /// untouched, and let the query planning algorithm treat it as any other field. We have no other choice in
    /// that case, and that's actually what we want.
    pub(crate) fn optimize_sibling_typenames(
        &mut self,
        interface_types_with_interface_objects: &IndexSet<InterfaceTypeDefinitionPosition>,
    ) -> Result<(), FederationError> {
        let is_interface_object =
            interface_types_with_interface_objects.contains(&InterfaceTypeDefinitionPosition {
                type_name: self.type_position.type_name().clone(),
            });
        let mut typename_field_key: Option<NormalizedSelectionKey> = None;
        let mut sibling_field_key: Option<NormalizedSelectionKey> = None;

        let mutable_selection_map = Arc::make_mut(&mut self.selections);
        for (key, entry) in mutable_selection_map.iter_mut() {
            match entry {
                NormalizedSelection::Field(field_selection) => {
                    if field_selection.field.name() == &TYPENAME_FIELD
                        && !is_interface_object
                        && typename_field_key.is_none()
                    {
                        typename_field_key = Some(key.clone());
                    } else if sibling_field_key.is_none() {
                        sibling_field_key = Some(key.clone());
                    }

                    let mutable_field_selection = Arc::make_mut(field_selection);
                    if let Some(field_selection_set) =
                        mutable_field_selection.selection_set.as_mut()
                    {
                        field_selection_set
                            .optimize_sibling_typenames(interface_types_with_interface_objects)?;
                    } else {
                        continue;
                    }
                }
                NormalizedSelection::InlineFragment(inline_fragment) => {
                    let mutable_inline_fragment = Arc::make_mut(inline_fragment);
                    mutable_inline_fragment
                        .selection_set
                        .optimize_sibling_typenames(interface_types_with_interface_objects)?;
                }
                NormalizedSelection::FragmentSpread(fragment_spread) => {
                    // at this point in time all fragment spreads should have been converted into inline fragments
                    return Err(FederationError::SingleFederationError(Internal {
                        message: format!(
                            "Error while optimizing sibling typename information, selection set contains {} named fragment",
                            fragment_spread.fragment_name
                        ),
                    }));
                }
            }
        }

        if let (Some(typename_key), Some(sibling_field_key)) =
            (typename_field_key, sibling_field_key)
        {
            if let (
                Some(NormalizedSelection::Field(typename_field)),
                Some(NormalizedSelection::Field(sibling_field)),
            ) = (
                mutable_selection_map.remove(&typename_key),
                mutable_selection_map.get_mut(&sibling_field_key),
            ) {
                let mutable_sibling_field = Arc::make_mut(sibling_field);
                mutable_sibling_field.sibling_typename = Some(typename_field.field.response_name());
            } else {
                unreachable!("typename and sibling fields must both exist at this point")
            }
        }
        Ok(())
    }
}

impl NormalizedFieldSelection {
    /// Copies field selection and assigns it a new unique selection ID.
    pub(crate) fn with_unique_id(&self) -> Self {
        let mut copy = self.clone();
        copy.field.selection_id = SelectionId::new();
        copy
    }

    /// Normalize this field selection (merging selections with the same keys), with the following
    /// additional transformations:
    /// - Expand fragment spreads into inline fragments.
    /// - Remove `__schema` or `__type` introspection fields, as these shouldn't be handled by query
    ///   planning.
    /// - Hoist fragment spreads/inline fragments into their parents if they have no directives and
    ///   their parent type matches.
    pub(crate) fn normalize_and_expand_fragments(
        field: &Field,
        parent_type_position: &CompositeTypeDefinitionPosition,
        fragments: &IndexMap<Name, Node<Fragment>>,
        schema: &ValidFederationSchema,
        inline_fragment_spreads: bool,
    ) -> Result<Option<NormalizedFieldSelection>, FederationError> {
        // Skip __schema/__type introspection fields as router takes care of those, and they do not
        // need to be query planned.
        if field.name == "__schema" || field.name == "__type" {
            return Ok(None);
        }
        let field_position = parent_type_position.field(field.name.clone())?;
        // We might be able to validate that the returned `FieldDefinition` matches that within
        // the given `field`, but on the off-chance there's a mutation somewhere in between
        // Operation creation and the creation of the ValidFederationSchema, it's safer to just
        // confirm it exists in this schema.
        field_position.get(schema.schema())?;
        let field_composite_type_result: Result<CompositeTypeDefinitionPosition, FederationError> =
            schema.get_type(field.selection_set.ty.clone())?.try_into();

        Ok(Some(NormalizedFieldSelection {
            field: NormalizedField {
                schema: schema.clone(),
                field_position,
                alias: field.alias.clone(),
                arguments: Arc::new(field.arguments.clone()),
                directives: Arc::new(field.directives.clone()),
                selection_id: SelectionId::new(),
            },
            selection_set: if field_composite_type_result.is_ok() {
                Some(NormalizedSelectionSet::normalize_and_expand_fragments(
                    &field.selection_set,
                    fragments,
                    schema,
                    inline_fragment_spreads,
                )?)
            } else {
                None
            },
            sibling_typename: None,
        }))
    }

    /// Merges the given normalized field selections into this one (this method assumes the keys
    /// already match).
    pub(crate) fn merge_into(
        &mut self,
        others: Vec<NormalizedFieldSelection>,
    ) -> Result<(), FederationError> {
        if !others.is_empty() {
            let self_field = &self.field;
            let mut selection_sets = vec![];
            for other in others {
                let other_field = &other.field;
                if other_field.schema != self_field.schema {
                    return Err(Internal {
                        message: "Cannot merge field selections from different schemas".to_owned(),
                    }
                    .into());
                }
                if other_field.field_position != self_field.field_position {
                    return Err(Internal {
                        message: format!(
                            "Cannot merge field selection for field \"{}\" into a field selection for field \"{}\"",
                            other_field.field_position,
                            self_field.field_position,
                        ),
                    }.into());
                }
                if self.selection_set.is_some() {
                    let Some(other_selection_set) = other.selection_set else {
                        return Err(Internal {
                            message: format!(
                                "Field \"{}\" has composite type but not a selection set",
                                other_field.field_position,
                            ),
                        }
                        .into());
                    };
                    selection_sets.push(other_selection_set);
                } else if other.selection_set.is_some() {
                    return Err(Internal {
                        message: format!(
                            "Field \"{}\" has non-composite type but also has a selection set",
                            other_field.field_position,
                        ),
                    }
                    .into());
                }
            }
            if let Some(self_selection_set) = &mut self.selection_set {
                self_selection_set.merge_into(selection_sets)?;
            }
        }
        Ok(())
    }
}

impl NormalizedFragmentSpreadSelection {
    /// Copies fragment spread selection and assigns it a new unique selection ID.
    pub(crate) fn with_unique_id(&self) -> Self {
        let mut copy = self.clone();
        copy.selection_id = SelectionId::new();
        copy
    }

    /// Normalize this fragment spread into a "normalized" spread representation with following
    /// modifications
    /// - Stores the schema (may be useful for directives).
    /// - Encloses list of directives in `Arc`s to facilitate cheaper cloning.
    /// - Stores unique selection ID (used for deferred fragments)
    pub(crate) fn normalize(
        fragment_spread: &FragmentSpread,
        schema: &ValidFederationSchema,
    ) -> NormalizedFragmentSpreadSelection {
        NormalizedFragmentSpreadSelection {
            schema: schema.clone(),
            fragment_name: fragment_spread.fragment_name.clone(),
            directives: Arc::new(fragment_spread.directives.clone()),
            selection_id: SelectionId::new(),
        }
    }

    /// Normalize this fragment spread (merging selections with the same keys), with the following
    /// additional transformations:
    /// - Expand fragment spreads into inline fragments.
    /// - Remove `__schema` or `__type` introspection fields, as these shouldn't be handled by query
    ///   planning.
    /// - Hoist fragment spreads/inline fragments into their parents if they have no directives and
    ///   their parent type matches.
    pub(crate) fn normalize_and_expand_fragments(
        fragment_spread: &FragmentSpread,
        parent_type_position: &CompositeTypeDefinitionPosition,
        fragments: &IndexMap<Name, Node<Fragment>>,
        schema: &ValidFederationSchema,
    ) -> Result<NormalizedInlineFragmentSelection, FederationError> {
        let Some(fragment) = fragments.get(&fragment_spread.fragment_name) else {
            return Err(Internal {
                message: format!(
                    "Fragment spread referenced non-existent fragment \"{}\"",
                    fragment_spread.fragment_name,
                ),
            }
            .into());
        };
        let type_condition_position: CompositeTypeDefinitionPosition = schema
            .get_type(fragment.type_condition().clone())?
            .try_into()?;

        // PORT_NOTE: The JS codebase combined the fragment spread's directives with the fragment
        // definition's directives. This was invalid GraphQL, so we're explicitly ignoring the
        // fragment definition's directives here (which isn't great, but there's not a simple
        // alternative at the moment).
        Ok(NormalizedInlineFragmentSelection {
            inline_fragment: NormalizedInlineFragment {
                schema: schema.clone(),
                parent_type_position: parent_type_position.clone(),
                type_condition_position: Some(type_condition_position),
                directives: Arc::new(fragment_spread.directives.clone()),
                selection_id: SelectionId::new(),
            },
            selection_set: NormalizedSelectionSet::normalize_and_expand_fragments(
                &fragment.selection_set,
                fragments,
                schema,
                true,
            )?,
        })
    }

    /// Merges the given normalized fragment spread selections into this one (this method assumes
    /// the keys already match).
    pub(crate) fn merge_into(
        &mut self,
        others: Vec<NormalizedFragmentSpreadSelection>,
    ) -> Result<(), FederationError> {
        if !others.is_empty() {
            for other in others {
                if other.schema != self.schema {
                    return Err(Internal {
                        message: "Cannot merge fragment spread from different schemas".to_owned(),
                    }
                    .into());
                }
                // Nothing to do since the fragment spread is already part of the selection set.
                // Fragment spreads are uniquely identified by fragment name and applied directives.
                // Since there is already an entry for the same fragment spread, there is no point
                // in attempting to merge its sub-selections, as the underlying entry should be
                // exactly the same as the currently processed one.
            }
        }
        Ok(())
    }
}

impl NormalizedInlineFragmentSelection {
    /// Copies inline fragment selection and assigns it a new unique selection ID.
    pub(crate) fn with_unique_id(&self) -> Self {
        let mut copy = self.clone();
        copy.inline_fragment.selection_id = SelectionId::new();
        copy
    }

    /// Normalize this inline fragment selection (merging selections with the same keys), with the
    /// following additional transformations:
    /// - Expand fragment spreads into inline fragments.
    /// - Remove `__schema` or `__type` introspection fields, as these shouldn't be handled by query
    ///   planning.
    /// - Hoist fragment spreads/inline fragments into their parents if they have no directives and
    ///   their parent type matches.
    pub(crate) fn normalize_and_expand_fragments(
        inline_fragment: &InlineFragment,
        parent_type_position: &CompositeTypeDefinitionPosition,
        fragments: &IndexMap<Name, Node<Fragment>>,
        schema: &ValidFederationSchema,
        inline_fragment_spreads: bool,
    ) -> Result<NormalizedInlineFragmentSelection, FederationError> {
        let type_condition_position: Option<CompositeTypeDefinitionPosition> =
            if let Some(type_condition) = &inline_fragment.type_condition {
                Some(schema.get_type(type_condition.clone())?.try_into()?)
            } else {
                None
            };
        Ok(NormalizedInlineFragmentSelection {
            inline_fragment: NormalizedInlineFragment {
                schema: schema.clone(),
                parent_type_position: parent_type_position.clone(),
                type_condition_position,
                directives: Arc::new(inline_fragment.directives.clone()),
                selection_id: SelectionId::new(),
            },
            selection_set: NormalizedSelectionSet::normalize_and_expand_fragments(
                &inline_fragment.selection_set,
                fragments,
                schema,
                inline_fragment_spreads,
            )?,
        })
    }

    /// Merges the given normalized inline fragment selections into this one (this method assumes
    /// the keys already match).
    pub(crate) fn merge_into(
        &mut self,
        others: Vec<NormalizedInlineFragmentSelection>,
    ) -> Result<(), FederationError> {
        if !others.is_empty() {
            let self_inline_fragment = &self.inline_fragment;
            let mut selection_sets = vec![];
            for other in others {
                let other_inline_fragment = &other.inline_fragment;
                if other_inline_fragment.schema != self_inline_fragment.schema {
                    return Err(Internal {
                        message: "Cannot merge inline fragment from different schemas".to_owned(),
                    }
                    .into());
                }
                if other_inline_fragment.parent_type_position
                    != self_inline_fragment.parent_type_position
                {
                    return Err(Internal {
                        message: format!(
                            "Cannot merge inline fragment of parent type \"{}\" into an inline fragment of parent type \"{}\"",
                            other_inline_fragment.parent_type_position,
                            self_inline_fragment.parent_type_position,
                        ),
                    }.into());
                }
                selection_sets.push(other.selection_set);
            }
            self.selection_set.merge_into(selection_sets)?;
        }
        Ok(())
    }
}

impl NormalizedFragment {
    fn normalize(
        fragment: &Fragment,
        schema: &ValidFederationSchema,
    ) -> Result<Self, FederationError> {
        Ok(Self {
            schema: schema.clone(),
            name: fragment.name.clone(),
            type_condition_position: schema
                .get_type(fragment.type_condition().clone())?
                .try_into()?,
            directives: Arc::new(fragment.directives.clone()),
            selection_set: NormalizedSelectionSet::normalize_and_expand_fragments(
                &fragment.selection_set,
                &IndexMap::new(),
                schema,
                false,
            )?,
        })
    }
}

impl TryFrom<&NormalizedSelectionSet> for SelectionSet {
    type Error = FederationError;

    fn try_from(val: &NormalizedSelectionSet) -> Result<Self, Self::Error> {
        let mut flattened = vec![];
        for normalized_selection in val.selections.values() {
            let selection = match normalized_selection {
                NormalizedSelection::Field(normalized_field_selection) => {
                    Selection::Field(Node::new(normalized_field_selection.deref().try_into()?))
                }
                NormalizedSelection::FragmentSpread(normalized_fragment_spread_selection) => {
                    Selection::FragmentSpread(Node::new(
                        normalized_fragment_spread_selection.deref().into(),
                    ))
                }
                NormalizedSelection::InlineFragment(normalized_inline_fragment_selection) => {
                    Selection::InlineFragment(Node::new(
                        normalized_inline_fragment_selection.deref().try_into()?,
                    ))
                }
            };
            flattened.push(selection);
        }
        Ok(Self {
            ty: val.type_position.type_name().clone(),
            selections: flattened,
        })
    }
}

impl TryFrom<&NormalizedFieldSelection> for Field {
    type Error = FederationError;

    fn try_from(val: &NormalizedFieldSelection) -> Result<Self, Self::Error> {
        let normalized_field = &val.field;
        let definition = normalized_field
            .field_position
            .get(normalized_field.schema.schema())?
            .node
            .to_owned();
        let selection_set = if let Some(selection_set) = &val.selection_set {
            selection_set.try_into()?
        } else {
            SelectionSet {
                ty: definition.ty.inner_named_type().clone(),
                selections: vec![],
            }
        };
        Ok(Self {
            definition,
            alias: normalized_field.alias.to_owned(),
            name: normalized_field.name().to_owned(),
            arguments: normalized_field.arguments.deref().to_owned(),
            directives: normalized_field.directives.deref().to_owned(),
            selection_set,
        })
    }
}

impl TryFrom<&NormalizedInlineFragmentSelection> for InlineFragment {
    type Error = FederationError;

    fn try_from(val: &NormalizedInlineFragmentSelection) -> Result<Self, Self::Error> {
        let normalized_inline_fragment = &val.inline_fragment;
        Ok(Self {
            type_condition: normalized_inline_fragment
                .type_condition_position
                .as_ref()
                .map(|pos| pos.type_name().clone()),
            directives: normalized_inline_fragment.directives.deref().to_owned(),
            selection_set: (&val.selection_set).try_into()?,
        })
    }
}

impl From<&NormalizedFragmentSpreadSelection> for FragmentSpread {
    fn from(val: &NormalizedFragmentSpreadSelection) -> Self {
        Self {
            fragment_name: val.fragment_name.to_owned(),
            directives: val.directives.deref().to_owned(),
        }
    }
}

impl From<&NormalizedSelection> for NormalizedSelectionKey {
    fn from(value: &NormalizedSelection) -> Self {
        match value {
            NormalizedSelection::Field(field_selection) => field_selection.deref().into(),
            NormalizedSelection::FragmentSpread(fragment_spread_selection) => {
                fragment_spread_selection.deref().into()
            }
            NormalizedSelection::InlineFragment(inline_fragment_selection) => {
                inline_fragment_selection.deref().into()
            }
        }
    }
}

impl From<&NormalizedFieldSelection> for NormalizedSelectionKey {
    fn from(field_selection: &NormalizedFieldSelection) -> Self {
        let deferred_id = if is_deferred_selection(&field_selection.field.directives) {
            Some(field_selection.field.selection_id.clone())
        } else {
            None
        };
        Self::Field {
            response_name: field_selection.field.response_name(),
            directives: Arc::new(directives_with_sorted_arguments(
                &field_selection.field.directives,
            )),
            deferred_id,
        }
    }
}

impl From<&NormalizedFragmentSpreadSelection> for NormalizedSelectionKey {
    fn from(fragment_spread_selection: &NormalizedFragmentSpreadSelection) -> Self {
        let deferred_id = if is_deferred_selection(&fragment_spread_selection.directives) {
            Some(fragment_spread_selection.selection_id.clone())
        } else {
            None
        };
        Self::FragmentSpread {
            name: fragment_spread_selection.fragment_name.clone(),
            directives: Arc::new(directives_with_sorted_arguments(
                &fragment_spread_selection.directives,
            )),
            deferred_id,
        }
    }
}

impl From<&NormalizedInlineFragmentSelection> for NormalizedSelectionKey {
    fn from(inline_fragment_selection: &NormalizedInlineFragmentSelection) -> Self {
        let deferred_id: Option<SelectionId> =
            if is_deferred_selection(&inline_fragment_selection.inline_fragment.directives) {
                Some(
                    inline_fragment_selection
                        .inline_fragment
                        .selection_id
                        .clone(),
                )
            } else {
                None
            };
        Self::InlineFragment {
            type_condition: inline_fragment_selection
                .inline_fragment
                .type_condition_position
                .as_ref()
                .map(|pos| pos.type_name().clone()),
            directives: Arc::new(directives_with_sorted_arguments(
                &inline_fragment_selection.inline_fragment.directives,
            )),
            deferred_id,
        }
    }
}

impl TryFrom<&NormalizedOperation> for Operation {
    type Error = FederationError;

    fn try_from(normalized_operation: &NormalizedOperation) -> Result<Self, Self::Error> {
        let operation_type = match normalized_operation.root_kind {
            SchemaRootDefinitionKind::Query => OperationType::Query,
            SchemaRootDefinitionKind::Mutation => OperationType::Mutation,
            SchemaRootDefinitionKind::Subscription => OperationType::Subscription,
        };
        Ok(Self {
            operation_type,
            name: normalized_operation.name.clone(),
            variables: normalized_operation.variables.deref().clone(),
            directives: normalized_operation.directives.deref().clone(),
            selection_set: (&normalized_operation.selection_set).try_into()?,
        })
    }
}

fn directives_with_sorted_arguments(directives: &DirectiveList) -> DirectiveList {
    let mut directives = directives.clone();
    for directive in &mut directives {
        directive
            .make_mut()
            .arguments
            .sort_by(|a1, a2| a1.name.cmp(&a2.name))
    }
    directives
}

fn is_deferred_selection(directives: &DirectiveList) -> bool {
    directives.has("defer")
}

/// Normalizes the selection set of the specified operation.
///
/// This method applies the following transformations:
/// - Merge selections with the same normalization "key".
/// - Expand fragment spreads into inline fragments.
/// - Remove `__schema` or `__type` introspection fields at all levels, as these shouldn't be
///   handled by query planning.
/// - Hoist fragment spreads/inline fragments into their parents if they have no directives and
///   their parent type matches.
pub(crate) fn normalize_operation(
    operation: &Operation,
    fragments: &IndexMap<Name, Node<Fragment>>,
    schema: &ValidFederationSchema,
    interface_types_with_interface_objects: &IndexSet<InterfaceTypeDefinitionPosition>,
) -> Result<NormalizedOperation, FederationError> {
    let mut normalized_selection_set = NormalizedSelectionSet::normalize_and_expand_fragments(
        &operation.selection_set,
        fragments,
        schema,
        true,
    )?;
    normalized_selection_set.optimize_sibling_typenames(interface_types_with_interface_objects)?;

    let normalized_fragments: IndexMap<Name, Node<NormalizedFragment>> = fragments
        .iter()
        .map(|(name, fragment)| {
            (
                name.clone(),
                Node::new(NormalizedFragment::normalize(fragment, schema).unwrap()),
            )
        })
        .collect();

    let schema_definition_root_kind = match operation.operation_type {
        OperationType::Query => SchemaRootDefinitionKind::Query,
        OperationType::Mutation => SchemaRootDefinitionKind::Mutation,
        OperationType::Subscription => SchemaRootDefinitionKind::Subscription,
    };
    let normalized_operation = NormalizedOperation {
        schema: schema.clone(),
        root_kind: schema_definition_root_kind,
        name: operation.name.clone(),
        variables: Arc::new(operation.variables.clone()),
        directives: Arc::new(operation.directives.clone()),
        selection_set: normalized_selection_set,
        fragments: Arc::new(normalized_fragments),
    };
    Ok(normalized_operation)
}

#[cfg(test)]
mod tests {
    use crate::query_plan::operation::normalize_operation;
    use crate::schema::position::InterfaceTypeDefinitionPosition;
    use crate::schema::ValidFederationSchema;
    use apollo_compiler::executable::{Fragment, Name, Operation};
    use apollo_compiler::{name, ExecutableDocument, Node};
    use indexmap::{IndexMap, IndexSet};

    fn parse_schema_and_operation(
        schema_and_operation: &str,
    ) -> (ValidFederationSchema, ExecutableDocument) {
        let (schema, executable_document) =
            apollo_compiler::parse_mixed_validate(schema_and_operation, "document.graphql")
                .unwrap();
        let executable_document = executable_document.into_inner();
        let schema = ValidFederationSchema::new(schema).unwrap();
        (schema, executable_document)
    }

    fn normalize_and_update_operation(
        operation: &mut Operation,
        fragments: &IndexMap<Name, Node<Fragment>>,
        schema: &ValidFederationSchema,
        interface_types_with_interface_objects: &IndexSet<InterfaceTypeDefinitionPosition>,
    ) {
        let normalized_operation = normalize_operation(
            operation,
            fragments,
            schema,
            interface_types_with_interface_objects,
        )
        .unwrap();

        // flatten normalized selection set back into a `SelectionSet`.
        operation.selection_set = (&normalized_operation.selection_set).try_into().unwrap();
    }

    #[test]
    fn expands_named_fragments() {
        let operation_with_named_fragment = r#"
query NamedFragmentQuery {
  foo {
    id
    ...Bar
  }
}

fragment Bar on Foo {
  bar
  baz
}

type Query {
  foo: Foo
}

type Foo {
  id: ID!
  bar: String!
  baz: Int
}
"#;
        let (schema, mut executable_document) =
            parse_schema_and_operation(operation_with_named_fragment);
        if let Some(operation) = executable_document
            .named_operations
            .get_mut("NamedFragmentQuery")
        {
            let operation = operation.make_mut();
            normalize_and_update_operation(
                operation,
                &executable_document.fragments,
                &schema,
                &IndexSet::new(),
            );

            let expected = r#"query NamedFragmentQuery {
  foo {
    id
    bar
    baz
  }
}"#;
            let actual = operation.to_string();
            assert_eq!(expected, actual);
        }
    }

    #[test]
    fn expands_and_deduplicates_fragments() {
        let operation_with_named_fragment = r#"
query NestedFragmentQuery {
  foo {
    ...FirstFragment
    ...SecondFragment
  }
}

fragment FirstFragment on Foo {
  id
  bar
  baz
}

fragment SecondFragment on Foo {
  id
  bar
}

type Query {
  foo: Foo
}

type Foo {
  id: ID!
  bar: String!
  baz: String
}
"#;
        let (schema, mut executable_document) =
            parse_schema_and_operation(operation_with_named_fragment);
        if let Some((_, operation)) = executable_document.named_operations.first_mut() {
            let operation = operation.make_mut();
            normalize_and_update_operation(
                operation,
                &executable_document.fragments,
                &schema,
                &IndexSet::new(),
            );

            let expected = r#"query NestedFragmentQuery {
  foo {
    id
    bar
    baz
  }
}"#;
            let actual = format!("{}", operation);
            assert_eq!(expected, actual);
        }
    }

    #[test]
    fn can_remove_introspection_selections() {
        let operation_with_introspection = r#"
query TestIntrospectionQuery {
  __schema {
    types {
      name
    }
  }
}

type Query {
  foo: String
}
"#;
        let (schema, mut executable_document) =
            parse_schema_and_operation(operation_with_introspection);
        if let Some(operation) = executable_document
            .named_operations
            .get_mut("TestIntrospectionQuery")
        {
            let normalized_operation = normalize_operation(
                operation,
                &executable_document.fragments,
                &schema,
                &IndexSet::new(),
            )
            .unwrap();

            assert!(normalized_operation.selection_set.selections.is_empty());
        }
    }

    #[test]
    fn merge_same_fields_without_directives() {
        let operation_string = r#"
query Test {
  t {
    v1
  }
  t {
    v2
 }
}

type Query {
  t: T
}

type T {
  v1: Int
  v2: String
}
"#;
        let (schema, mut executable_document) = parse_schema_and_operation(operation_string);
        if let Some((_, operation)) = executable_document.named_operations.first_mut() {
            let operation = operation.make_mut();
            normalize_and_update_operation(
                operation,
                &executable_document.fragments,
                &schema,
                &IndexSet::new(),
            );
            let expected = r#"query Test {
  t {
    v1
    v2
  }
}"#;
            let actual = format!("{}", operation);
            assert_eq!(expected, actual);
        } else {
            panic!("unable to parse document")
        }
    }

    #[test]
    fn merge_same_fields_with_same_directive() {
        let operation_with_directives = r#"
query Test($skipIf: Boolean!) {
  t @skip(if: $skipIf) {
    v1
  }
  t @skip(if: $skipIf) {
    v2
  }
}

type Query {
  t: T
}

type T {
  v1: Int
  v2: String
}
"#;
        let (schema, mut executable_document) =
            parse_schema_and_operation(operation_with_directives);
        if let Some((_, operation)) = executable_document.named_operations.first_mut() {
            let operation = operation.make_mut();
            normalize_and_update_operation(
                operation,
                &executable_document.fragments,
                &schema,
                &IndexSet::new(),
            );
            let expected = r#"query Test($skipIf: Boolean!) {
  t @skip(if: $skipIf) {
    v1
    v2
  }
}"#;
            let actual = format!("{}", operation);
            assert_eq!(expected, actual);
        } else {
            panic!("unable to parse document")
        }
    }

    #[test]
    fn merge_same_fields_with_same_directive_but_different_arg_order() {
        let operation_with_directives_different_arg_order = r#"
query Test($skipIf: Boolean!) {
  t @customSkip(if: $skipIf, label: "foo") {
    v1
  }
  t @customSkip(label: "foo", if: $skipIf) {
    v2
  }
}

directive @customSkip(if: Boolean!, label: String!) on FIELD | INLINE_FRAGMENT

type Query {
  t: T
}

type T {
  v1: Int
  v2: String
}
"#;
        let (schema, mut executable_document) =
            parse_schema_and_operation(operation_with_directives_different_arg_order);
        if let Some((_, operation)) = executable_document.named_operations.first_mut() {
            let operation = operation.make_mut();
            normalize_and_update_operation(
                operation,
                &executable_document.fragments,
                &schema,
                &IndexSet::new(),
            );
            let expected = r#"query Test($skipIf: Boolean!) {
  t @customSkip(if: $skipIf, label: "foo") {
    v1
    v2
  }
}"#;
            let actual = format!("{}", operation);
            assert_eq!(expected, actual);
        } else {
            panic!("unable to parse document")
        }
    }

    #[test]
    fn do_not_merge_when_only_one_field_specifies_directive() {
        let operation_one_field_with_directives = r#"
query Test($skipIf: Boolean!) {
  t {
    v1
  }
  t @skip(if: $skipIf) {
    v2
  }
}

type Query {
  t: T
}

type T {
  v1: Int
  v2: String
}
"#;
        let (schema, mut executable_document) =
            parse_schema_and_operation(operation_one_field_with_directives);
        if let Some((_, operation)) = executable_document.named_operations.first_mut() {
            let operation = operation.make_mut();
            normalize_and_update_operation(
                operation,
                &executable_document.fragments,
                &schema,
                &IndexSet::new(),
            );
            let expected = r#"query Test($skipIf: Boolean!) {
  t {
    v1
  }
  t @skip(if: $skipIf) {
    v2
  }
}"#;
            let actual = format!("{}", operation);
            assert_eq!(expected, actual);
        } else {
            panic!("unable to parse document")
        }
    }

    #[test]
    fn do_not_merge_when_fields_have_different_directives() {
        let operation_different_directives = r#"
query Test($skip1: Boolean!, $skip2: Boolean!) {
  t @skip(if: $skip1) {
    v1
  }
  t @skip(if: $skip2) {
    v2
  }
}

type Query {
  t: T
}

type T {
  v1: Int
  v2: String
}
"#;
        let (schema, mut executable_document) =
            parse_schema_and_operation(operation_different_directives);
        if let Some((_, operation)) = executable_document.named_operations.first_mut() {
            let operation = operation.make_mut();
            normalize_and_update_operation(
                operation,
                &executable_document.fragments,
                &schema,
                &IndexSet::new(),
            );
            let expected = r#"query Test($skip1: Boolean!, $skip2: Boolean!) {
  t @skip(if: $skip1) {
    v1
  }
  t @skip(if: $skip2) {
    v2
  }
}"#;
            let actual = format!("{}", operation);
            assert_eq!(expected, actual);
        } else {
            panic!("unable to parse document")
        }
    }

    // TODO enable when @defer is available in apollo-rs
    #[ignore]
    #[test]
    fn do_not_merge_fields_with_defer_directive() {
        let operation_defer_fields = r#"
query Test {
  t @defer {
    v1
  }
  t @defer {
    v2
  }
}

type Query {
  t: T
}

type T {
  v1: Int
  v2: String
}
"#;
        let (schema, mut executable_document) = parse_schema_and_operation(operation_defer_fields);
        if let Some((_, operation)) = executable_document.named_operations.first_mut() {
            let operation = operation.make_mut();
            normalize_and_update_operation(
                operation,
                &executable_document.fragments,
                &schema,
                &IndexSet::new(),
            );
            let expected = r#"query Test {
  t @defer {
    v1
  }
  t @defer {
    v2
  }
}"#;
            let actual = format!("{}", operation);
            assert_eq!(expected, actual);
        } else {
            panic!("unable to parse document")
        }
    }

    // TODO enable when @defer is available in apollo-rs
    #[ignore]
    #[test]
    fn merge_nested_field_selections() {
        let nested_operation = r#"
query Test {
  t {
    t1
    v @defer {
      v1
    }
  }
  t {
    t1
    t2
    v @defer {
      v2
    }
  }
}

type Query {
  t: T
}

type T {
  t1: Int
  t2: String
  v: V
}

type V {
  v1: Int
  v2: String
}
"#;
        let (schema, mut executable_document) = parse_schema_and_operation(nested_operation);
        if let Some((_, operation)) = executable_document.named_operations.first_mut() {
            let operation = operation.make_mut();
            normalize_and_update_operation(
                operation,
                &executable_document.fragments,
                &schema,
                &IndexSet::new(),
            );
            let expected = r#"query Test {
  t {
    t1
    v @defer {
      v1
    }
    t2
    v @defer {
      v2
    }
  }
}"#;
            let actual = format!("{}", operation);
            assert_eq!(expected, actual);
        } else {
            panic!("unable to parse document")
        }
    }

    //
    // inline fragments
    //

    #[test]
    fn merge_same_fragment_without_directives() {
        let operation_with_fragments = r#"
query Test {
  t {
    ... on T {
      v1
    }
    ... on T {
      v2
    }
  }
}

type Query {
  t: T
}

type T {
  v1: Int
  v2: String
}
"#;
        let (schema, mut executable_document) =
            parse_schema_and_operation(operation_with_fragments);
        if let Some((_, operation)) = executable_document.named_operations.first_mut() {
            let operation = operation.make_mut();
            normalize_and_update_operation(
                operation,
                &executable_document.fragments,
                &schema,
                &IndexSet::new(),
            );
            let expected = r#"query Test {
  t {
    v1
    v2
  }
}"#;
            let actual = format!("{}", operation);
            assert_eq!(expected, actual);
        } else {
            panic!("unable to parse document")
        }
    }

    #[test]
    fn merge_same_fragments_with_same_directives() {
        let operation_fragments_with_directives = r#"
query Test($skipIf: Boolean!) {
  t {
    ... on T @skip(if: $skipIf) {
      v1
    }
    ... on T @skip(if: $skipIf) {
      v2
    }
  }
}

type Query {
  t: T
}

type T {
  v1: Int
  v2: String
}
"#;
        let (schema, mut executable_document) =
            parse_schema_and_operation(operation_fragments_with_directives);
        if let Some((_, operation)) = executable_document.named_operations.first_mut() {
            let operation = operation.make_mut();
            normalize_and_update_operation(
                operation,
                &executable_document.fragments,
                &schema,
                &IndexSet::new(),
            );
            let expected = r#"query Test($skipIf: Boolean!) {
  t {
    ... on T @skip(if: $skipIf) {
      v1
      v2
    }
  }
}"#;
            let actual = format!("{}", operation);
            assert_eq!(expected, actual);
        } else {
            panic!("unable to parse document")
        }
    }

    #[test]
    fn merge_same_fragments_with_same_directive_but_different_arg_order() {
        let operation_fragments_with_directives_args_order = r#"
query Test($skipIf: Boolean!) {
  t {
    ... on T @customSkip(if: $skipIf, label: "foo") {
      v1
    }
    ... on T @customSkip(label: "foo", if: $skipIf) {
      v2
    }
  }
}

directive @customSkip(if: Boolean!, label: String!) on FIELD | INLINE_FRAGMENT

type Query {
  t: T
}

type T {
  v1: Int
  v2: String
}
"#;
        let (schema, mut executable_document) =
            parse_schema_and_operation(operation_fragments_with_directives_args_order);
        if let Some((_, operation)) = executable_document.named_operations.first_mut() {
            let operation = operation.make_mut();
            normalize_and_update_operation(
                operation,
                &executable_document.fragments,
                &schema,
                &IndexSet::new(),
            );
            let expected = r#"query Test($skipIf: Boolean!) {
  t {
    ... on T @customSkip(if: $skipIf, label: "foo") {
      v1
      v2
    }
  }
}"#;
            let actual = format!("{}", operation);
            assert_eq!(expected, actual);
        } else {
            panic!("unable to parse document")
        }
    }

    #[test]
    fn do_not_merge_when_only_one_fragment_specifies_directive() {
        let operation_one_fragment_with_directive = r#"
query Test($skipIf: Boolean!) {
  t {
    ... on T {
      v1
    }
    ... on T @skip(if: $skipIf) {
      v2
    }
  }
}

type Query {
  t: T
}

type T {
  v1: Int
  v2: String
}
"#;
        let (schema, mut executable_document) =
            parse_schema_and_operation(operation_one_fragment_with_directive);
        if let Some((_, operation)) = executable_document.named_operations.first_mut() {
            let operation = operation.make_mut();
            normalize_and_update_operation(
                operation,
                &executable_document.fragments,
                &schema,
                &IndexSet::new(),
            );
            let expected = r#"query Test($skipIf: Boolean!) {
  t {
    v1
    ... on T @skip(if: $skipIf) {
      v2
    }
  }
}"#;
            let actual = format!("{}", operation);
            assert_eq!(expected, actual);
        } else {
            panic!("unable to parse document")
        }
    }

    #[test]
    fn do_not_merge_when_fragments_have_different_directives() {
        let operation_fragments_with_different_directive = r#"
query Test($skip1: Boolean!, $skip2: Boolean!) {
  t {
    ... on T @skip(if: $skip1) {
      v1
    }
    ... on T @skip(if: $skip2) {
      v2
    }
  }
}

type Query {
  t: T
}

type T {
  v1: Int
  v2: String
}
"#;
        let (schema, mut executable_document) =
            parse_schema_and_operation(operation_fragments_with_different_directive);
        if let Some((_, operation)) = executable_document.named_operations.first_mut() {
            let operation = operation.make_mut();
            normalize_and_update_operation(
                operation,
                &executable_document.fragments,
                &schema,
                &IndexSet::new(),
            );
            let expected = r#"query Test($skip1: Boolean!, $skip2: Boolean!) {
  t {
    ... on T @skip(if: $skip1) {
      v1
    }
    ... on T @skip(if: $skip2) {
      v2
    }
  }
}"#;
            let actual = format!("{}", operation);
            assert_eq!(expected, actual);
        } else {
            panic!("unable to parse document")
        }
    }

    // TODO enable when @defer is available in apollo-rs
    #[ignore]
    #[test]
    fn do_not_merge_fragments_with_defer_directive() {
        let operation_fragments_with_defer = r#"
query Test {
  t {
    ... on T @defer {
      v1
    }
    ... on T @defer {
      v2
    }
  }
}

type Query {
  t: T
}

type T {
  v1: Int
  v2: String
}
"#;
        let (schema, mut executable_document) =
            parse_schema_and_operation(operation_fragments_with_defer);
        if let Some((_, operation)) = executable_document.named_operations.first_mut() {
            let operation = operation.make_mut();
            normalize_and_update_operation(
                operation,
                &executable_document.fragments,
                &schema,
                &IndexSet::new(),
            );
            let expected = r#"query Test {
  t {
    ... on T @defer {
      v1
    }
    ... on T @defer {
      v2
    }
  }
}"#;
            let actual = format!("{}", operation);
            assert_eq!(expected, actual);
        } else {
            panic!("unable to parse document")
        }
    }

    // TODO enable when @defer is available in apollo-rs
    #[ignore]
    #[test]
    fn merge_nested_fragments() {
        let operation_nested_fragments = r#"
query Test {
  t {
    ... on T {
      t1
    }
    ... on T {
      v @defer {
        v1
      }
    }
  }
  t {
    ... on T {
      t1
      t2
    }
    ... on T {
      v @defer {
        v2
      }
    }
  }
}

type Query {
  t: T
}

type T {
  t1: Int
  t2: String
  v: V
}

type V {
  v1: Int
  v2: String
}
"#;
        let (schema, mut executable_document) =
            parse_schema_and_operation(operation_nested_fragments);
        if let Some((_, operation)) = executable_document.named_operations.first_mut() {
            let operation = operation.make_mut();
            normalize_and_update_operation(
                operation,
                &executable_document.fragments,
                &schema,
                &IndexSet::new(),
            );
            let expected = r#"query Test {
  t {
    t1
    v @defer {
      v1
    }
    t2
    v @defer {
      v2
    }
  }
}"#;
            let actual = format!("{}", operation);
            assert_eq!(expected, actual);
        } else {
            panic!("unable to parse document")
        }
    }

    #[test]
    fn removes_sibling_typename() {
        let operation_with_typename = r#"
query TestQuery {
  foo {
    __typename
    v1
    v2
  }
}

type Query {
  foo: Foo
}

type Foo {
  v1: ID!
  v2: String
}
"#;
        let (schema, mut executable_document) = parse_schema_and_operation(operation_with_typename);
        if let Some(operation) = executable_document.named_operations.get_mut("TestQuery") {
            let operation = operation.make_mut();
            normalize_and_update_operation(
                operation,
                &executable_document.fragments,
                &schema,
                &IndexSet::new(),
            );
            let expected = r#"query TestQuery {
  foo {
    v1
    v2
  }
}"#;
            let actual = format!("{}", operation);
            assert_eq!(expected, actual);
        }
    }

    #[test]
    fn keeps_typename_if_no_other_selection() {
        let operation_with_single_typename = r#"
query TestQuery {
  foo {
    __typename
  }
}

type Query {
  foo: Foo
}

type Foo {
  v1: ID!
  v2: String
}
"#;
        let (schema, mut executable_document) =
            parse_schema_and_operation(operation_with_single_typename);
        if let Some(operation) = executable_document.named_operations.get_mut("TestQuery") {
            let operation = operation.make_mut();
            normalize_and_update_operation(
                operation,
                &executable_document.fragments,
                &schema,
                &IndexSet::new(),
            );
            let expected = r#"query TestQuery {
  foo {
    __typename
  }
}"#;
            let actual = format!("{}", operation);
            assert_eq!(expected, actual);
        }
    }

    #[test]
    fn keeps_typename_for_interface_object() {
        let operation_with_intf_object_typename = r#"
query TestQuery {
  foo {
    __typename
    v1
    v2
  }
}

directive @interfaceObject on OBJECT
directive @key(fields: FieldSet!, resolvable: Boolean = true) repeatable on OBJECT | INTERFACE

type Query {
  foo: Foo
}

type Foo @interfaceObject @key(fields: "id") {
  v1: ID!
  v2: String
}

scalar FieldSet
"#;
        let (schema, mut executable_document) =
            parse_schema_and_operation(operation_with_intf_object_typename);
        if let Some(operation) = executable_document.named_operations.get_mut("TestQuery") {
            let mut interface_objects: IndexSet<InterfaceTypeDefinitionPosition> = IndexSet::new();
            interface_objects.insert(InterfaceTypeDefinitionPosition {
                type_name: name!("Foo"),
            });

            let operation = operation.make_mut();
            normalize_and_update_operation(
                operation,
                &executable_document.fragments,
                &schema,
                &interface_objects,
            );
            let expected = r#"query TestQuery {
  foo {
    __typename
    v1
    v2
  }
}"#;
            let actual = format!("{}", operation);
            assert_eq!(expected, actual);
        }
    }
}
