use crate::error::FederationError;
use crate::link::federation_spec_definition::FederationSpecDefinition;
use crate::link::spec::Version;
use crate::link::spec_definition::SpecDefinition;
use crate::query_plan::operation::{NormalizedSelection, NormalizedSelectionSet};
use crate::schema::field_set::{
    add_interface_field_implementations, collect_target_fields_from_field_set,
};
use crate::schema::position::{
    CompositeTypeDefinitionPosition, FieldDefinitionPosition,
    ObjectOrInterfaceFieldDefinitionPosition, ObjectOrInterfaceTypeDefinitionPosition,
};
use crate::schema::FederationSchema;
use apollo_compiler::schema::DirectiveDefinition;
use apollo_compiler::validation::Valid;
use apollo_compiler::{Node, Schema};
use indexmap::IndexSet;
use std::sync::Arc;

// PORT_NOTE: The JS codebase called this `FederationMetadata`, but this naming didn't make it
// apparent that this was just subgraph schema metadata, so we've renamed it accordingly.
#[derive(Debug, Clone)]
pub(crate) struct SubgraphMetadata {
    schema: Arc<Valid<FederationSchema>>,
    federation_spec_definition: &'static FederationSpecDefinition,
    is_fed2: bool,
    external_metadata: ExternalMetadata,
}

impl SubgraphMetadata {
    pub(super) fn new(
        schema: Arc<Valid<FederationSchema>>,
        valid_schema: &Valid<Schema>,
        federation_spec_definition: &'static FederationSpecDefinition,
    ) -> Result<Self, FederationError> {
        // let federation_spec_definition = get_federation_spec_definition_from_subgraph(&schema)?;
        let is_fed2 = federation_spec_definition
            .version()
            .satisfies(&Version { major: 2, minor: 0 });
        let external_metadata = ExternalMetadata::new(
            schema.clone(),
            valid_schema,
            federation_spec_definition,
            is_fed2,
        )?;
        Ok(Self {
            schema,
            federation_spec_definition,
            is_fed2,
            external_metadata,
        })
    }

    pub(crate) fn federation_spec_definition(&self) -> &'static FederationSpecDefinition {
        self.federation_spec_definition
    }

    pub(crate) fn is_fed2(&self) -> bool {
        self.is_fed2
    }

    pub(crate) fn external_metadata(&self) -> &ExternalMetadata {
        &self.external_metadata
    }
}

// PORT_NOTE: The JS codebase called this `ExternalTester`, but this naming didn't make it
// apparent that this was just @external-related subgraph metadata, so we've renamed it accordingly.
// Also note the field "externalFieldsOnType" was renamed to "fields_on_external_types", as it's
// more accurate.
#[derive(Debug, Clone)]
pub(crate) struct ExternalMetadata {
    schema: Arc<Valid<FederationSchema>>,
    federation_spec_definition: &'static FederationSpecDefinition,
    is_fed2: bool,
    external_directive_definition: Node<DirectiveDefinition>,
    fake_external_fields: IndexSet<FieldDefinitionPosition>,
    provided_fields: IndexSet<FieldDefinitionPosition>,
    fields_on_external_types: IndexSet<FieldDefinitionPosition>,
}

impl ExternalMetadata {
    fn new(
        schema: Arc<Valid<FederationSchema>>,
        valid_schema: &Valid<Schema>,
        federation_spec_definition: &'static FederationSpecDefinition,
        is_fed2: bool,
    ) -> Result<Self, FederationError> {
        let external_directive_definition = federation_spec_definition
            .external_directive_definition(&schema)?
            .clone();
        let mut external_metadata = Self {
            schema,
            federation_spec_definition,
            is_fed2,
            external_directive_definition,
            fake_external_fields: IndexSet::new(),
            provided_fields: IndexSet::new(),
            fields_on_external_types: IndexSet::new(),
        };
        external_metadata.collect_fake_externals(valid_schema)?;
        external_metadata.collect_provided_fields(valid_schema)?;
        external_metadata.collect_fields_on_external_types()?;
        Ok(external_metadata)
    }

    fn collect_fake_externals(
        &mut self,
        valid_schema: &Valid<Schema>,
    ) -> Result<(), FederationError> {
        let extends_directive_definition = self
            .federation_spec_definition
            .extends_directive_definition(&self.schema)?;
        let key_directive_definition = self
            .federation_spec_definition
            .key_directive_definition(&self.schema)?;
        let key_directive_referencers = self
            .schema
            .referencers
            .get_directive(&key_directive_definition.name)?;
        let mut key_type_positions: Vec<ObjectOrInterfaceTypeDefinitionPosition> = vec![];
        for object_type_position in &key_directive_referencers.object_types {
            key_type_positions.push(object_type_position.clone().into());
        }
        for interface_type_position in &key_directive_referencers.interface_types {
            key_type_positions.push(interface_type_position.clone().into());
        }
        for type_position in key_type_positions {
            let directives = match &type_position {
                ObjectOrInterfaceTypeDefinitionPosition::Object(pos) => {
                    &pos.get(self.schema.schema())?.directives
                }
                ObjectOrInterfaceTypeDefinitionPosition::Interface(pos) => {
                    &pos.get(self.schema.schema())?.directives
                }
            };
            let has_extends_directive = directives.has(&extends_directive_definition.name);
            for key_directive_application in directives.get_all(&key_directive_definition.name) {
                // PORT_NOTE: The JS codebase treats the "extend" GraphQL keyword as applying to
                // only the extension it's on, while it treats the "@extends" directive as applying
                // to all definitions/extensions in the subgraph. We accordingly do the same.
                if has_extends_directive
                    || key_directive_application.origin.extension_id().is_some()
                {
                    let key_directive_arguments = self
                        .federation_spec_definition
                        .key_directive_arguments(key_directive_application)?;
                    self.fake_external_fields
                        .extend(collect_target_fields_from_field_set(
                            valid_schema,
                            type_position.type_name().clone(),
                            key_directive_arguments.fields,
                        )?);
                }
            }
        }
        Ok(())
    }

    fn collect_provided_fields(
        &mut self,
        valid_schema: &Valid<Schema>,
    ) -> Result<(), FederationError> {
        let provides_directive_definition = self
            .federation_spec_definition
            .provides_directive_definition(&self.schema)?;
        let provides_directive_referencers = self
            .schema
            .referencers
            .get_directive(&provides_directive_definition.name)?;
        let mut provides_field_positions: Vec<ObjectOrInterfaceFieldDefinitionPosition> = vec![];
        for object_field_position in &provides_directive_referencers.object_fields {
            provides_field_positions.push(object_field_position.clone().into());
        }
        for interface_field_position in &provides_directive_referencers.interface_fields {
            provides_field_positions.push(interface_field_position.clone().into());
        }
        for field_position in provides_field_positions {
            let field = field_position.get(self.schema.schema())?;
            let field_type_position: CompositeTypeDefinitionPosition = self
                .schema
                .get_type(field.ty.inner_named_type().clone())?
                .try_into()?;
            for provides_directive_application in field
                .directives
                .get_all(&provides_directive_definition.name)
            {
                let provides_directive_arguments = self
                    .federation_spec_definition
                    .provides_directive_arguments(provides_directive_application)?;
                self.provided_fields
                    .extend(add_interface_field_implementations(
                        collect_target_fields_from_field_set(
                            valid_schema,
                            field_type_position.type_name().clone(),
                            provides_directive_arguments.fields,
                        )?,
                        &self.schema,
                    )?);
            }
        }
        Ok(())
    }

    fn collect_fields_on_external_types(&mut self) -> Result<(), FederationError> {
        // We do not collect @external on types for Fed 1 schemas since those will be discarded by
        // the schema upgrader. The schema upgrader, through calls to `is_external()`, relies on the
        // populated `fields_on_external_types` set to inform when @shareable should be
        // automatically added. In the Fed 1 case, if the set is populated then @shareable won't be
        // added in places where it should be.
        if !self.is_fed2 {
            return Ok(());
        }

        let external_directive_referencers = self
            .schema
            .referencers
            .get_directive(&self.external_directive_definition.name)?;
        for object_type_position in &external_directive_referencers.object_types {
            let object_type = object_type_position.get(self.schema.schema())?;
            // PORT_NOTE: The JS codebase does not differentiate fields at a definition/extension
            // level here, and we accordingly do the same. I.e., if a type is marked @external for
            // one definition/extension in a subgraph, then it is considered to be marked @external
            // for all definitions/extensions in that subgraph.
            for field_name in object_type.fields.keys() {
                self.fields_on_external_types
                    .insert(object_type_position.field(field_name.clone()).into());
            }
        }
        Ok(())
    }

    pub(crate) fn is_external(
        &self,
        field_definition_position: &FieldDefinitionPosition,
    ) -> Result<bool, FederationError> {
        let field = field_definition_position.get(self.schema.schema())?;
        Ok((field
            .directives
            .has(&self.external_directive_definition.name)
            || self
                .fields_on_external_types
                .contains(field_definition_position))
            && !self.is_fake_external(field_definition_position))
    }

    pub(crate) fn is_fake_external(
        &self,
        field_definition_position: &FieldDefinitionPosition,
    ) -> bool {
        self.fake_external_fields
            .contains(field_definition_position)
    }

    pub(crate) fn selects_any_external_field(
        &self,
        selection_set: &NormalizedSelectionSet,
    ) -> Result<bool, FederationError> {
        for selection in selection_set.selections.values() {
            if let NormalizedSelection::Field(field_selection) = selection {
                if self.is_external(&field_selection.field.data().field_position)? {
                    return Ok(true);
                }
            }
            if let Some(selection_set) = selection.selection_set()? {
                if self.selects_any_external_field(selection_set)? {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    pub(crate) fn is_partially_external(
        &self,
        field_definition_position: &FieldDefinitionPosition,
    ) -> Result<bool, FederationError> {
        Ok(self.is_external(field_definition_position)?
            && self.provided_fields.contains(field_definition_position))
    }

    pub(crate) fn is_fully_external(
        &self,
        field_definition_position: &FieldDefinitionPosition,
    ) -> Result<bool, FederationError> {
        Ok(self.is_external(field_definition_position)?
            && !self.provided_fields.contains(field_definition_position))
    }
}
