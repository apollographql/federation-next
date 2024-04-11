use crate::error::{FederationError, SingleFederationError};
use crate::link::federation_spec_definition::FEDERATION_ENTITY_TYPE_NAME_IN_SPEC;
use crate::link::LinksMetadata;
use crate::schema::position::{
    CompositeTypeDefinitionPosition, DirectiveDefinitionPosition, EnumTypeDefinitionPosition,
    InputObjectTypeDefinitionPosition, InterfaceTypeDefinitionPosition,
    ObjectTypeDefinitionPosition, ScalarTypeDefinitionPosition, TypeDefinitionPosition,
    UnionTypeDefinitionPosition,
};
use apollo_compiler::schema::{ExtendedType, Name};
use apollo_compiler::validation::Valid;
use apollo_compiler::{NodeStr, Schema};
use indexmap::IndexSet;
use referencer::Referencers;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::Arc;

pub(crate) mod definitions;
pub(crate) mod position;
pub(crate) mod referencer;

/// A GraphQL schema with federation data.
#[derive(Debug)]
pub struct FederationSchema {
    schema: Schema,
    metadata: Option<LinksMetadata>,
    referencers: Referencers,
}

impl FederationSchema {
    pub(crate) fn schema(&self) -> &Schema {
        &self.schema
    }

    pub(crate) fn schema_mut(&mut self) -> &mut Schema {
        &mut self.schema
    }

    /// Discard the Federation metadata and return the apollo-compiler schema.
    pub fn into_inner(self) -> Schema {
        self.schema
    }

    pub(crate) fn metadata(&self) -> Option<&LinksMetadata> {
        self.metadata.as_ref()
    }

    pub(crate) fn referencers(&self) -> &Referencers {
        &self.referencers
    }

    /// Returns all the types in the schema, minus builtins.
    pub(crate) fn get_types(&self) -> impl Iterator<Item = TypeDefinitionPosition> + '_ {
        self.schema
            .types
            .iter()
            .filter(|(_, ty)| !ty.is_built_in())
            .map(|(type_name, type_)| {
                let type_name = type_name.clone();
                match type_ {
                    ExtendedType::Scalar(_) => ScalarTypeDefinitionPosition { type_name }.into(),
                    ExtendedType::Object(_) => ObjectTypeDefinitionPosition { type_name }.into(),
                    ExtendedType::Interface(_) => {
                        InterfaceTypeDefinitionPosition { type_name }.into()
                    }
                    ExtendedType::Union(_) => UnionTypeDefinitionPosition { type_name }.into(),
                    ExtendedType::Enum(_) => EnumTypeDefinitionPosition { type_name }.into(),
                    ExtendedType::InputObject(_) => {
                        InputObjectTypeDefinitionPosition { type_name }.into()
                    }
                }
            })
    }

    pub(crate) fn get_directive_definitions(
        &self,
    ) -> impl Iterator<Item = DirectiveDefinitionPosition> + '_ {
        self.schema
            .directive_definitions
            .keys()
            .map(|name| DirectiveDefinitionPosition {
                directive_name: name.clone(),
            })
    }

    pub(crate) fn get_type(
        &self,
        type_name: Name,
    ) -> Result<TypeDefinitionPosition, FederationError> {
        let type_ =
            self.schema
                .types
                .get(&type_name)
                .ok_or_else(|| SingleFederationError::Internal {
                    message: format!("Schema has no type \"{}\"", type_name),
                })?;
        Ok(match type_ {
            ExtendedType::Scalar(_) => ScalarTypeDefinitionPosition { type_name }.into(),
            ExtendedType::Object(_) => ObjectTypeDefinitionPosition { type_name }.into(),
            ExtendedType::Interface(_) => InterfaceTypeDefinitionPosition { type_name }.into(),
            ExtendedType::Union(_) => UnionTypeDefinitionPosition { type_name }.into(),
            ExtendedType::Enum(_) => EnumTypeDefinitionPosition { type_name }.into(),
            ExtendedType::InputObject(_) => InputObjectTypeDefinitionPosition { type_name }.into(),
        })
    }

    pub(crate) fn try_get_type(&self, type_name: Name) -> Option<TypeDefinitionPosition> {
        self.get_type(type_name).ok()
    }

    pub(crate) fn possible_runtime_types(
        &self,
        composite_type_definition_position: CompositeTypeDefinitionPosition,
    ) -> Result<IndexSet<ObjectTypeDefinitionPosition>, FederationError> {
        Ok(match composite_type_definition_position {
            CompositeTypeDefinitionPosition::Object(pos) => IndexSet::from([pos]),
            CompositeTypeDefinitionPosition::Interface(pos) => self
                .referencers()
                .get_interface_type(&pos.type_name)?
                .object_types
                .clone(),
            CompositeTypeDefinitionPosition::Union(pos) => pos
                .get(self.schema())?
                .members
                .iter()
                .map(|t| ObjectTypeDefinitionPosition {
                    type_name: t.name.clone(),
                })
                .collect::<IndexSet<_>>(),
        })
    }

    pub(crate) fn validate(self) -> Result<ValidFederationSchema, FederationError> {
        self.validate_or_return_self().map_err(|e| e.1)
    }

    /// Similar to `Self::validate` but returns `self` as part of the error should it be needed by
    /// the caller
    pub(crate) fn validate_or_return_self(
        mut self,
    ) -> Result<ValidFederationSchema, (Self, FederationError)> {
        let schema = match self.schema.validate() {
            Ok(schema) => schema.into_inner(),
            Err(e) => {
                self.schema = e.partial;
                return Err((self, e.errors.into()));
            }
        };
        Ok(ValidFederationSchema(Arc::new(Valid::assume_valid(
            FederationSchema {
                schema,
                metadata: self.metadata,
                referencers: self.referencers,
            },
        ))))
    }

    pub(crate) fn assume_valid(self) -> ValidFederationSchema {
        ValidFederationSchema(Arc::new(Valid::assume_valid(self)))
    }

    pub(crate) fn get_directive_definition(
        &self,
        name: &Name,
    ) -> Option<DirectiveDefinitionPosition> {
        self.schema
            .directive_definitions
            .contains_key(name)
            .then(|| DirectiveDefinitionPosition {
                directive_name: name.clone(),
            })
    }

    /// Note that a subgraph may have no "entities" and so no `_Entity` type.
    pub(crate) fn entity_type(
        &self,
    ) -> Result<Option<UnionTypeDefinitionPosition>, FederationError> {
        // Note that the _Entity type is special in that:
        // 1. Spec renaming doesn't take place for it (there's no prefixing or importing needed),
        //    in order to maintain backwards compatibility with Fed 1.
        // 2. Its presence is optional; if absent, it means the subgraph has no resolvable keys.
        match self.schema.types.get(&FEDERATION_ENTITY_TYPE_NAME_IN_SPEC) {
            Some(ExtendedType::Union(_)) => Ok(Some(UnionTypeDefinitionPosition {
                type_name: FEDERATION_ENTITY_TYPE_NAME_IN_SPEC,
            })),
            Some(_) => Err(FederationError::internal(format!(
                "Unexpectedly found non-union for federation spec's `{}` type definition",
                FEDERATION_ENTITY_TYPE_NAME_IN_SPEC
            ))),
            None => Ok(None),
        }
    }
}

/// A GraphQL schema with federation data that is known to be valid, and cheap to clone.
#[derive(Debug, Clone)]
pub struct ValidFederationSchema(pub(crate) Arc<Valid<FederationSchema>>);

impl ValidFederationSchema {
    pub fn new(schema: Valid<Schema>) -> Result<ValidFederationSchema, FederationError> {
        let schema = FederationSchema::new(schema.into_inner())?;
        Ok(ValidFederationSchema(Arc::new(Valid::assume_valid(schema))))
    }

    /// Access the GraphQL schema.
    pub fn schema(&self) -> &Valid<Schema> {
        Valid::assume_valid_ref(&self.schema)
    }

    pub(crate) fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    pub(crate) fn federation_type_name_in_schema(
        &self,
        name: &str,
    ) -> Result<Name, FederationError> {
        // Currently, the types used to define the federation operations, that is _Any, _Entity and _Service,
        // are not considered part of the federation spec, and are instead hardcoded to the names above.
        // The reason being that there is no way to maintain backward compatbility with fed2 if we were to add
        // those to the federation spec without requiring users to add those types to their @link `import`,
        // and that wouldn't be a good user experience (because most users don't really know what those types
        // are/do). And so we special case it.
        if name.starts_with("_") {
            return NodeStr::new(name)
                .try_into()
                .map_err(|_| FederationError::internal("invalid name".to_string()));
        }

        todo!()
    }
}

impl Deref for ValidFederationSchema {
    type Target = FederationSchema;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Eq for ValidFederationSchema {}

impl PartialEq for ValidFederationSchema {
    fn eq(&self, other: &ValidFederationSchema) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Hash for ValidFederationSchema {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}
