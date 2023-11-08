use crate::error::FederationError;
use crate::link::LinksMetadata;
use crate::schema::position::{CompositeTypeDefinitionPosition, ObjectTypeDefinitionPosition};
use apollo_compiler::Schema;
use indexmap::IndexSet;
use referencer::Referencers;

pub(crate) mod position;
pub(crate) mod referencer;

pub struct FederationSchema {
    schema: Schema,
    metadata: Option<LinksMetadata>,
    referencers: Referencers,
}

impl FederationSchema {
    pub(crate) fn schema(&self) -> &Schema {
        &self.schema
    }

    pub(crate) fn metadata(&self) -> &Option<LinksMetadata> {
        &self.metadata
    }

    pub(crate) fn referencers(&self) -> &Referencers {
        &self.referencers
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
}
