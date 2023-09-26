use std::cell::RefCell;
use std::rc::Rc;
use apollo_parser::ast::Document;
use crate::definitions::{Schema, SchemaBlueprintEnum};
use crate::error::{AggregateGraphQLError, GraphQLError};

pub(crate) struct BuildSchemaOptions {
    blueprint: Option<SchemaBlueprintEnum>,
    validate: Option<bool>,
}

pub(crate) fn build_schema_from_ast(
    document: Document,
    options: Option<BuildSchemaOptions>,
) -> Result<Rc<RefCell<Schema>>, AggregateGraphQLError> {
    let mut errors: Vec<GraphQLError> = Vec::new();
    // let schema = Schema::new();
    todo!()
}
