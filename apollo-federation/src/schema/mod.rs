use crate::error::FederationError;
use crate::link::database::links_metadata;
use crate::link::LinksMetadata;
use apollo_compiler::Schema;
use referencer::{referencers, Referencers};

pub(crate) mod location;
pub(crate) mod referencer;
pub(crate) mod remove;

// A new type that exists purely to implement AsRef and AsMut on them, as we can't do it on
// Option<LinksMetadata> directly due to the orphan rule.
pub struct OptionLinksMetadata(Option<LinksMetadata>);

impl From<Option<LinksMetadata>> for OptionLinksMetadata {
    fn from(value: Option<LinksMetadata>) -> Self {
        OptionLinksMetadata(value)
    }
}

impl AsMut<OptionLinksMetadata> for OptionLinksMetadata {
    fn as_mut(&mut self) -> &mut OptionLinksMetadata {
        self
    }
}

impl AsRef<OptionLinksMetadata> for OptionLinksMetadata {
    fn as_ref(&self) -> &OptionLinksMetadata {
        self
    }
}

// Note that LinksMetadata is not automatically updated for all changes to the schema, and it's up
// to the caller to determine when re-computation is necessary (in the JS code, this metadata was
// embedded in the schema as "CoreFeatures").
pub struct FederationSchema {
    pub schema: Schema,
    metadata: OptionLinksMetadata,
}

impl FederationSchema {
    pub fn new(schema: Schema) -> Result<Self, FederationError> {
        let metadata = links_metadata(&schema)?;
        Ok(Self {
            schema,
            metadata: metadata.into(),
        })
    }

    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    pub fn metadata(&self) -> &Option<LinksMetadata> {
        &self.metadata.0
    }

    pub fn as_federation_schema_ref(&self) -> FederationSchemaRef<&OptionLinksMetadata> {
        FederationSchemaRef {
            schema: &self.schema,
            metadata: self.metadata.as_ref(),
        }
    }

    pub fn as_federation_schema_mut(&mut self) -> FederationSchemaMut<&mut OptionLinksMetadata> {
        FederationSchemaMut {
            schema: &mut self.schema,
            metadata: self.metadata.as_mut(),
        }
    }

    pub fn as_referencer_federation_schema_ref(
        &self,
    ) -> ReferencerFederationSchemaRef<&OptionLinksMetadata, Referencers> {
        let referencers = referencers(&self.schema);
        ReferencerFederationSchemaRef {
            schema: &self.schema,
            metadata: self.metadata.as_ref(),
            referencers,
        }
    }

    pub fn as_referencer_federation_schema_mut(
        &mut self,
    ) -> ReferencerFederationSchemaMut<&mut OptionLinksMetadata, Referencers> {
        let referencers = referencers(&self.schema);
        ReferencerFederationSchemaMut {
            schema: &mut self.schema,
            metadata: self.metadata.as_mut(),
            referencers,
        }
    }
}

// Note that LinksMetadata is not automatically updated for all changes to the schema, and it's up
// to the caller to determine when re-computation is necessary (in the JS code, this metadata was
// embedded in the schema as "CoreFeatures").
pub struct FederationSchemaMut<'schema, T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>>
{
    schema: &'schema mut Schema,
    metadata: T,
}

impl<'schema, T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>>
    FederationSchemaMut<'schema, T>
{
    pub fn new(
        schema: &'schema mut Schema,
    ) -> Result<FederationSchemaMut<'schema, OptionLinksMetadata>, FederationError> {
        let metadata = links_metadata(schema)?;
        Ok(FederationSchemaMut {
            schema,
            metadata: metadata.into(),
        })
    }

    pub fn schema(&self) -> &Schema {
        self.schema
    }

    pub fn metadata(&self) -> &Option<LinksMetadata> {
        &self.metadata.as_ref().0
    }

    pub fn as_federation_schema_ref(&self) -> FederationSchemaRef<&OptionLinksMetadata> {
        FederationSchemaRef {
            schema: self.schema,
            metadata: self.metadata.as_ref(),
        }
    }

    pub fn as_referencer_federation_schema_ref(
        &self,
    ) -> ReferencerFederationSchemaRef<&OptionLinksMetadata, Referencers> {
        let referencers = referencers(self.schema);
        ReferencerFederationSchemaRef {
            schema: self.schema,
            metadata: self.metadata.as_ref(),
            referencers,
        }
    }

    pub fn as_referencer_federation_schema_mut(
        &mut self,
    ) -> ReferencerFederationSchemaMut<&mut OptionLinksMetadata, Referencers> {
        let referencers = referencers(self.schema);
        ReferencerFederationSchemaMut {
            schema: self.schema,
            metadata: self.metadata.as_mut(),
            referencers,
        }
    }
}

pub struct FederationSchemaRef<'schema, T: AsRef<OptionLinksMetadata>> {
    pub schema: &'schema Schema,
    metadata: T,
}

impl<'schema, T: AsRef<OptionLinksMetadata>> FederationSchemaRef<'schema, T> {
    pub fn new(
        schema: &'schema Schema,
    ) -> Result<FederationSchemaRef<'schema, OptionLinksMetadata>, FederationError> {
        let metadata = links_metadata(schema)?;
        Ok(FederationSchemaRef {
            schema,
            metadata: metadata.into(),
        })
    }

    pub fn metadata(&self) -> &Option<LinksMetadata> {
        &self.metadata.as_ref().0
    }

    pub fn as_referencer_federation_schema_ref(
        &self,
    ) -> ReferencerFederationSchemaRef<&OptionLinksMetadata, Referencers> {
        let referencers = referencers(self.schema);
        ReferencerFederationSchemaRef {
            schema: self.schema,
            metadata: self.metadata.as_ref(),
            referencers,
        }
    }
}

pub struct ReferencerFederationSchemaMut<
    'schema,
    T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
    U: AsMut<Referencers> + AsRef<Referencers>,
> {
    schema: &'schema mut Schema,
    metadata: T,
    referencers: U,
}

impl<
        'schema,
        T: AsMut<OptionLinksMetadata> + AsRef<OptionLinksMetadata>,
        U: AsMut<Referencers> + AsRef<Referencers>,
    > ReferencerFederationSchemaMut<'schema, T, U>
{
    pub fn schema(&self) -> &Schema {
        self.schema
    }

    pub fn metadata(&self) -> &Option<LinksMetadata> {
        &self.metadata.as_ref().0
    }

    pub fn referencers(&self) -> &Referencers {
        self.referencers.as_ref()
    }

    pub fn as_referencer_federation_schema_ref(
        &self,
    ) -> ReferencerFederationSchemaRef<&OptionLinksMetadata, &Referencers> {
        ReferencerFederationSchemaRef {
            schema: self.schema,
            metadata: self.metadata.as_ref(),
            referencers: self.referencers.as_ref(),
        }
    }
}

pub struct ReferencerFederationSchemaRef<
    'schema,
    T: AsRef<OptionLinksMetadata>,
    U: AsRef<Referencers>,
> {
    pub schema: &'schema Schema,
    metadata: T,
    referencers: U,
}

impl<'schema, T: AsRef<OptionLinksMetadata>, U: AsRef<Referencers>>
    ReferencerFederationSchemaRef<'schema, T, U>
{
    pub fn metadata(&self) -> &Option<LinksMetadata> {
        &self.metadata.as_ref().0
    }

    pub fn referencers(&self) -> &Referencers {
        self.referencers.as_ref()
    }
}
