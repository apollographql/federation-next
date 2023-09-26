use crate::definitions::{CoreFeature, HasKind, HasName, HasSourceAst, NamedTypeEnum, NamedTypeKindEnum, ScalarType, Schema, UnattachedNamedTypeEnum};
use crate::error::{ErrorEnum, GraphQLError, GraphQLErrorOptions};

pub(crate) struct TypeSpecification {
    name: String,
    // Fn(schema: Schema, feature: Option<CoreFeature>, as_built_in: Option<bool>) -> Vec<GraphQLError>
    check_or_add: Box<dyn 'static + Sync + Fn(Schema, Option<CoreFeature>, Option<bool>) -> Result<(), Vec<GraphQLError>>>
}

/*
export function createScalarTypeSpecification({ name }: { name: string }): TypeSpecification {
  return {
    name,
    checkOrAdd: (schema: Schema, feature?: CoreFeature, asBuiltIn?: boolean) => {
      const actualName = feature?.typeNameInSchema(name) ?? name;
      const existing = schema.type(actualName);
      if (existing) {
        return ensureSameTypeKind('ScalarType', existing);
      } else {
        schema.addType(new ScalarType(actualName, asBuiltIn));
        return [];
      }
    },
  }
}
 */

pub(crate) fn create_scalar_type_specification(name: String) -> TypeSpecification {
    TypeSpecification {
        name: name.clone(),
        check_or_add: Box::new(move |schema, feature, as_built_in| {
            let actual_name = feature.map(|f| f.type_name_in_schema(&name)).unwrap_or(name.clone());
            let existing = schema.type_(&actual_name);
            if let Some(existing) = existing {
                ensure_same_type_kind(NamedTypeKindEnum::ScalarType, existing)
            } else {
                // TODO: Once constructors are done, uncomment this.
                // schema.add_type(UnattachedNamedTypeEnum::UnattachedUnionType(ScalarType::new()));
                Ok(())
            }
        }),
    }
}

// PORT_NOTE: Known as "ensure_same_type_kind()" in the JS code. This function effectively converted
// the type kind to a String and did a comparison against an expected type kind, but it turns out
// the expected kind was known at compile time and could be converted to a Rust "if let", so we did
// that instead of some funky
fn ensure_same_type_kind(expected: NamedTypeKindEnum, actual: NamedTypeEnum) -> Result<(), Vec<GraphQLError>> {
    let kind = actual.kind();
    if expected == kind {
       Ok(())
    } else {
        let name = actual.name();
        Err(vec![ErrorEnum::TypeDefinitionInvalid.definition().err(
            format!(
                "Invalid definition for type {}: {} should be a ${} but is defined as a ${}",
                name,
                name,
                expected,
                kind,
            ),
            Some(GraphQLErrorOptions::new_single(
                actual.source_ast(),
                None
            ))
        )])
    }
}

/*
function ensureSameTypeKind(expected: NamedType['kind'], actual: NamedType): GraphQLError[] {
  return expected === actual.kind
    ? []
    : [
      ERRORS.TYPE_DEFINITION_INVALID.err(
        `Invalid definition for type ${actual.name}: ${actual.name} should be a ${expected} but is defined as a ${actual.kind}`,
        { nodes: actual.sourceAST },
      )
    ];
}
 */
