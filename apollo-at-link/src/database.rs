use std::{collections::HashMap, path::Path, sync::Arc};

use apollo_compiler::{
    database::{db::Upcast, AstStorage, HirStorage, InputStorage},
    hir::{Directive, DirectiveLocation, Type, Value},
    FileId, HirDatabase, InputDatabase, Source,
};

use crate::{
    link::{Link, LinkError, LinksMetadata, DEFAULT_LINK_NAME},
    spec::{Identity, Url},
};

#[salsa::query_group(AtLinkStorage)]
pub trait AtLinkDatabase: HirDatabase {
    fn links_metadata(&self) -> Arc<LinksMetadata>;
}

pub fn links_metadata(db: &dyn AtLinkDatabase) -> Arc<LinksMetadata> {
    Arc::new(bootstrap(db).unwrap_or_default().unwrap_or_default())
}

fn bootstrap(db: &dyn AtLinkDatabase) -> Result<Option<LinksMetadata>, LinkError> {
    let schema_def = db.schema();
    let mut bootstrap_directives = schema_def
        .directives()
        .filter(|d| parse_link_if_bootstrap_directive(db, d));
    let bootstrap_directive = bootstrap_directives.next();
    if bootstrap_directive.is_none() {
        return Ok(None);
    }
    // We have _a_ bootstrap directives, but 2 is more than we bargained for.
    if bootstrap_directives.next().is_some() {
        return Err(LinkError::BootstrapError(format!(
            "the @link specification itself (\"{}\") is applied multiple times",
            Identity::link_identity()
        )));
    }
    // At this point, we know this schema uses "our" @link. So we now "just" want to validate
    // all of the @link usages (starting with the bootstrapping one) and extract their metadata.
    let link_name_in_schema = bootstrap_directive.unwrap().name();
    let mut links = Vec::new();
    let mut by_identity = HashMap::new();
    let mut by_name_in_schema = HashMap::new();
    let mut types_by_imported_name = HashMap::new();
    let mut directives_by_imported_name = HashMap::new();
    let link_applications = schema_def
        .directives()
        .filter(|d| d.name() == link_name_in_schema);
    for application in link_applications {
        let link = Arc::new(Link::from_directive_application(application)?);
        links.push(Arc::clone(&link));
        if by_identity
            .insert(link.url.identity.clone(), Arc::clone(&link))
            .is_some()
        {
            // TODO: we may want to lessen that limitation at some point. Including the same feature for 2 different major versions should be ok.
            return Err(LinkError::BootstrapError(format!(
                "duplicate @link inclusion of specification \"{}\"",
                link.url.identity
            )));
        }
        let name_in_schema = link.spec_name_in_schema().clone();
        if let Some(other) = by_name_in_schema.insert(name_in_schema, Arc::clone(&link)) {
            return Err(LinkError::BootstrapError(format!(
                "name conflict: {} and {} are imported under the same name (consider using the `@link(as:)` argument to disambiguate)",
                other.url, link.url,
            )));
        }
    }

    // We do a 2nd pass to collect and validate all the imports (it's a separate path so we
    // know all the names of the spec linked in the schema).
    for link in &links {
        for import in &link.imports {
            let imported_name = import.imported_name();
            let element_map = if import.is_directive {
                // the name of each spec (in the schema) acts as an implicit import for a
                // directive of the same name. So one cannot import a direcitive with the
                // same name than a linked spec.
                if let Some(other) = by_name_in_schema.get(imported_name) {
                    Err(LinkError::BootstrapError(format!(
                        "import for '{}' of {} conflicts with spec {}",
                        import.imported_display_name(),
                        link.url,
                        other.url
                    )))?
                }
                &mut directives_by_imported_name
            } else {
                &mut types_by_imported_name
            };
            if let Some((other_link, _)) = element_map.insert(
                imported_name.clone(),
                (Arc::clone(link), Arc::clone(import)),
            ) {
                Err(LinkError::BootstrapError(format!(
                    "name conflict: both {} and {} import {}",
                    link.url,
                    other_link.url,
                    import.imported_display_name()
                )))?
            }
        }
    }

    Ok(Some(LinksMetadata {
        links,
        by_identity,
        by_name_in_schema,
        types_by_imported_name,
        directives_by_imported_name,
    }))
}

// TODO: this should maybe be provided by apollo-compiler directly?
pub(crate) fn directive_arg_value<'a>(
    directive: &'a Directive,
    arg_name: &'static str,
) -> Option<&'a Value> {
    directive
        .arguments()
        .iter()
        .find(|arg| arg.name() == arg_name)
        .map(|arg| arg.value())
}

pub(crate) fn directive_string_arg_value<'a>(
    directive: &'a Directive,
    arg_name: &'static str,
) -> Option<&'a String> {
    match directive_arg_value(directive, arg_name) {
        Some(Value::String { value, loc: _ }) => Some(value),
        _ => None,
    }
}

// Note: currently only recognizing @link, not @core. Doesn't feel worth bothering with @core at
// this point, but the latter uses the "feature" arg instead of "url".
fn parse_link_if_bootstrap_directive(db: &dyn AtLinkDatabase, directive: &Directive) -> bool {
    if let Some(definition) = db.find_directive_definition_by_name(directive.name().to_string()) {
        let locations = definition.directive_locations();
        let is_correct_def = definition.repeatable()
            && locations.len() == 1
            && locations[0] == DirectiveLocation::Schema;
        let is_correct_def = is_correct_def
            && definition
                .arguments()
                .input_values()
                .iter()
                .find(|arg| arg.name() == "as")
                .filter(|arg| {
                    let ty = arg.ty();
                    ty.is_named() && ty.name() == "String"
                })
                .is_some();
        let is_correct_def = is_correct_def
            && definition
                .arguments()
                .input_values()
                .iter()
                .find(|arg| arg.name() == "url")
                .filter(|arg| {
                    // The "true" type of `url` in the @link spec is actually `String` (nullable), and this
                    // for future-proofing reasons (the idea was that we may introduce later other
                    // ways to identify specs that are not urls). But we allow the definition to
                    // have a non-nullable type both for convenience and because some early
                    // federation previews actually generated that.
                    if let Type::NonNull { ty, loc: _ } = arg.ty() {
                        return ty.is_named() && ty.name() == "String";
                    }
                    let ty = arg.ty();
                    ty.is_named() && ty.name() == "String"
                })
                .is_some();
        if !is_correct_def {
            return false;
        }
        if let Some(url) = directive_string_arg_value(directive, "url") {
            let url = url.parse::<Url>();
            let expected_name: &str = directive_string_arg_value(directive, "as")
                .map(|v| v.as_ref())
                .unwrap_or(DEFAULT_LINK_NAME);
            return url.map_or(false, |url| {
                url.identity == Identity::link_identity() && directive.name() == expected_name
            });
        }
    }
    false
}

#[salsa::database(InputStorage, AstStorage, HirStorage, AtLinkStorage)]
#[derive(Default)]
pub struct AtLinkedRootDatabase {
    pub storage: salsa::Storage<AtLinkedRootDatabase>,
}

impl salsa::Database for AtLinkedRootDatabase {}

impl salsa::ParallelDatabase for AtLinkedRootDatabase {
    fn snapshot(&self) -> salsa::Snapshot<AtLinkedRootDatabase> {
        salsa::Snapshot::new(AtLinkedRootDatabase {
            storage: self.storage.snapshot(),
        })
    }
}

impl Upcast<dyn HirDatabase> for AtLinkedRootDatabase {
    fn upcast(&self) -> &(dyn HirDatabase + 'static) {
        self
    }
}

pub struct AtLinkedCompiler {
    pub db: AtLinkedRootDatabase,
}

#[allow(clippy::new_without_default)]
impl AtLinkedCompiler {
    pub fn new() -> Self {
        let mut db = AtLinkedRootDatabase::default();
        // TODO(@goto-bus-stop) can we make salsa fill in these defaults for us…?
        db.set_recursion_limit(None);
        db.set_token_limit(None);
        db.set_type_system_hir_input(None);
        db.set_source_files(vec![]);

        Self { db }
    }

    fn add_input(&mut self, source: Source) -> FileId {
        let file_id = FileId::new();
        let mut sources = self.db.source_files();
        sources.push(file_id);
        self.db.set_input(file_id, source);
        self.db.set_source_files(sources);

        file_id
    }

    pub fn add_type_system(&mut self, input: &str, path: impl AsRef<Path>) -> FileId {
        if self.db.type_system_hir_input().is_some() {
            panic!(
                "Having both string inputs and pre-computed inputs \
                 for type system definitions is not supported"
            )
        }
        let filename = path.as_ref().to_owned();
        // TODO: should be added theoretically; but ideally we can "inherit" all this from
        // ApolloCompiler more easily.
        //self.add_implicit_types();
        self.add_input(Source::schema(filename, input))
    }

    pub fn update_type_system(&mut self, file_id: FileId, input: &str) {
        let schema = self.db.input(file_id);
        self.db
            .set_input(file_id, Source::schema(schema.filename().to_owned(), input))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        link::{Import, Purpose},
        spec::{Version, APOLLO_SPEC_DOMAIN},
    };

    use super::*;

    #[test]
    fn computes_link_metadata() {
        let schema = r#"
          extend schema
            @link(url: "https://specs.apollo.dev/link/v1.0", import: ["Import"])
            @link(url: "https://specs.apollo.dev/federation/v2.3", import: ["@key", { name: "@tag", as: "@myTag" }])
            @link(url: "https://custom.com/someSpec/v0.2", as: "mySpec")
            @link(url: "https://megacorp.com/auth/v1.0", for: SECURITY)

          type Query {
            x: Int
          }

          enum link__Purpose {
            SECURITY
            EXECUTION
          }

          scalar Import

          directive @link(url: String, as: String, import: [Import], for: link__Purpose) repeatable on SCHEMA
        "#;

        let mut compiler = AtLinkedCompiler::new();
        compiler.add_type_system(schema, "testSchema");

        let meta = compiler.db.links_metadata();
        let names_in_schema = meta
            .all_links()
            .iter()
            .map(|l| l.spec_name_in_schema())
            .collect::<Vec<&String>>();
        assert_eq!(names_in_schema.len(), 4);
        assert_eq!(names_in_schema[0], "link");
        assert_eq!(names_in_schema[1], "federation");
        assert_eq!(names_in_schema[2], "mySpec");
        assert_eq!(names_in_schema[3], "auth");

        let link_spec = meta.for_identity(&Identity::link_identity()).unwrap();
        assert_eq!(
            link_spec.imports.get(0).unwrap().as_ref(),
            &Import {
                element: "Import".to_string(),
                is_directive: false,
                alias: None
            }
        );

        let fed_spec = meta
            .for_identity(&Identity {
                domain: APOLLO_SPEC_DOMAIN.to_string(),
                name: "federation".to_string(),
            })
            .unwrap();
        assert_eq!(fed_spec.url.version, Version { major: 2, minor: 3 });
        assert_eq!(fed_spec.purpose, None);

        let imports = &fed_spec.imports;
        assert_eq!(imports.len(), 2);
        assert_eq!(
            imports.get(0).unwrap().as_ref(),
            &Import {
                element: "key".to_string(),
                is_directive: true,
                alias: None
            }
        );
        assert_eq!(
            imports.get(1).unwrap().as_ref(),
            &Import {
                element: "tag".to_string(),
                is_directive: true,
                alias: Some("myTag".to_string())
            }
        );

        let auth_spec = meta
            .for_identity(&Identity {
                domain: "https://megacorp.com".to_string(),
                name: "auth".to_string(),
            })
            .unwrap();
        assert_eq!(auth_spec.purpose, Some(Purpose::SECURITY));

        let import_source = meta.source_link_of_type("Import").unwrap();
        assert_eq!(import_source.link.url.identity.name, "link");
        assert!(!import_source.import.as_ref().unwrap().is_directive);
        assert_eq!(import_source.import.as_ref().unwrap().alias, None);

        // Purpose is not imported, so it should only be accessible in fql form
        assert!(meta.source_link_of_type("Purpose").is_none());

        let purpose_source = meta.source_link_of_type("link__Purpose").unwrap();
        assert_eq!(purpose_source.link.url.identity.name, "link");
        assert_eq!(purpose_source.import, None);

        let key_source = meta.source_link_of_directive("key").unwrap();
        assert_eq!(key_source.link.url.identity.name, "federation");
        assert!(key_source.import.as_ref().unwrap().is_directive);
        assert_eq!(key_source.import.as_ref().unwrap().alias, None);

        // tag is imported under an alias, so "tag" itself should not match
        assert!(meta.source_link_of_directive("tag").is_none());

        let tag_source = meta.source_link_of_directive("myTag").unwrap();
        assert_eq!(tag_source.link.url.identity.name, "federation");
        assert_eq!(tag_source.import.as_ref().unwrap().element, "tag");
        assert!(tag_source.import.as_ref().unwrap().is_directive);
        assert_eq!(
            tag_source.import.as_ref().unwrap().alias,
            Some("myTag".to_string())
        );
    }
}
