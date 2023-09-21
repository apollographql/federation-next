use std::iter;
use std::path::Path;

#[allow(unused)]
use database::{SupergraphDatabase, SupergraphRootDatabase};

use apollo_compiler::ast::{
    Argument, Directive, EnumValueDefinition, FieldDefinition, NamedType, Value,
};
use apollo_compiler::schema::{
    Component, ComponentOrigin, EnumType, ExtendedType, InputObjectType, InputValueDefinition,
    InterfaceType, Name, ObjectType, ScalarType, UnionType,
};
use apollo_compiler::{ast, FileId, InputDatabase, Node, NodeStr, ReprDatabase, Schema, Source};
use apollo_subgraph::Subgraph;
use indexmap::map::Entry::{Occupied, Vacant};
use indexmap::IndexMap;

mod database;

type MergeError = &'static str;

// TODO: Same remark as in other crates: we need to define this more cleanly, and probably need
// some "federation errors" crate.
#[derive(Debug)]
pub struct SupergraphError {
    pub msg: String,
}

pub struct Supergraph {
    pub db: SupergraphRootDatabase,
}

impl Supergraph {
    pub fn new(schema_str: &str) -> Self {
        let mut db = SupergraphRootDatabase::default();
        db.set_recursion_limit(None);
        db.set_token_limit(None);
        db.set_type_system_hir_input(None);
        db.set_source_files(vec![]);

        // TODO: should be added theoretically.
        //self.add_implicit_types();

        let file_id = FileId::new();
        let mut sources = db.source_files();
        sources.push(file_id);
        let path: &Path = "supergraph".as_ref();
        db.set_input(file_id, Source::schema(path.to_owned(), schema_str));
        db.set_source_files(sources);

        // TODO: like for subgraphs, it would nice if `Supergraph` was always representing
        // a valid supergraph (which is simpler than for subgraph, but still at least means
        // that it's valid graphQL in the first place, and that it has the `join` spec).

        Self { db }
    }

    pub fn compose(subgraphs: Vec<&Subgraph>) -> Result<Self, MergeError> {
        let mut supergraph = Schema::new();
        // TODO handle @compose

        // add core features
        // TODO verify federation versions across subgraphs
        add_core_feature_link(&mut supergraph);
        add_core_feature_join(&mut supergraph, &subgraphs);

        // create stubs
        for subgraph in &subgraphs {
            merge_schema(&mut supergraph, &subgraph);

            for (key, value) in &subgraph.db.schema().types {
                if value.is_built_in() || !is_mergeable_type(key) {
                    // skip built-ins and federation specific types
                    continue;
                }

                match value {
                    ExtendedType::Enum(value) => merge_enum_type(
                        &mut supergraph.types,
                        subgraph.name.to_uppercase().clone(),
                        key.clone(),
                        value,
                    ),
                    ExtendedType::InputObject(value) => merge_input_object_type(
                        &mut supergraph.types,
                        subgraph.name.to_uppercase().clone(),
                        key.clone(),
                        value,
                    ),
                    ExtendedType::Interface(value) => merge_interface_type(
                        &mut supergraph.types,
                        subgraph.name.to_uppercase().clone(),
                        key.clone(),
                        value,
                    ),
                    ExtendedType::Object(value) => merge_object_type(
                        &mut supergraph.types,
                        subgraph.name.to_uppercase().clone(),
                        key.clone(),
                        value,
                    ),
                    ExtendedType::Union(value) => merge_union_type(
                        &mut supergraph.types,
                        subgraph.name.to_uppercase().clone(),
                        key.clone(),
                        value,
                    ),
                    ExtendedType::Scalar(_value) => {
                        // DO NOTHING
                    }
                }
            }
        }
        // println!("{}", supergraph);
        Ok(Self::new(supergraph.to_string().as_str()))
    }

    pub fn print_sdl(&self) -> String {
        let mut schema = self.db.schema();
        let schema = schema.make_mut();
        schema.types.sort_by(|k1, v1, k2, v2| {
            let type_order = print_type_order(v1).cmp(&print_type_order(v2));
            if type_order.is_eq() {
                k1.cmp(k2)
            } else {
                type_order
            }
        });
        // schema.types.values().into_iter().for_each(|t| match t {
        //     ExtendedType::Enum(e) => {
        //         let mut e = e.make_mut();
        //         e.directives.sort_by(|d1, d2| d1.name.cmp(&d2.name));
        //         e.values.sort_keys();
        //     }
        //     ExtendedType::InputObject(io) => {
        //         let mut io = io.make_mut();
        //         io.directives.sort_by(|d1, d2| d1.name.cmp(&d2.name));
        //         io.fields.sort_keys();
        //         io.fields.values().into_iter().for_each(|f| {
        //             let mut f = f.make_mut();
        //             f.directives.sort_by(|d1, d2| d1.name.cmp(&d2.name));
        //         });
        //     }
        //     ExtendedType::Interface(i) => {
        //         let mut i = i.make_mut();
        //         i.implements_interfaces.sort_keys();
        //         i.directives.sort_by(|d1, d2| d1.name.cmp(&d2.name));
        //         i.fields.sort_keys();
        //         i.fields.values().into_iter().for_each(|f| {
        //             let mut f = f.make_mut();
        //             f.directives.sort_by(|d1, d2| d1.name.cmp(&d2.name));
        //             f.arguments.sort_by(|a1, a2| a1.name.cmp(&a2.name));
        //             f.arguments.iter().for_each(|a| {
        //                 let mut a = a.make_mut();
        //                 a.directives.sort_by(|d1, d2| d1.name.cmp(&d2.name))
        //             });
        //         });
        //     }
        //     ExtendedType::Object(o) => {
        //         let mut o = o.make_mut();
        //         o.implements_interfaces.sort_keys();
        //         o.directives.sort_by(|d1, d2| d1.name.cmp(&d2.name));
        //         o.fields.sort_keys();
        //         o.fields.values().into_iter().for_each(|f| {
        //             let mut f = f.make_mut();
        //             f.directives.sort_by(|d1, d2| d1.name.cmp(&d2.name));
        //             f.arguments.sort_by(|a1, a2| a1.name.cmp(&a2.name));
        //             f.arguments.iter().for_each(|a| {
        //                 let mut a = a.make_mut();
        //                 a.directives.sort_by(|d1, d2| d1.name.cmp(&d2.name))
        //             });
        //         });
        //     }
        //     ExtendedType::Union(u) => {
        //         let mut u = u.make_mut();
        //         u.directives.sort_by(|d1, d2| d1.name.cmp(&d2.name));
        //         u.members.sort_keys();
        //     }
        //     _ => {}
        // });
        schema.directive_definitions.sort_keys();
        // schema
        //     .directive_definitions
        //     .values()
        //     .into_iter()
        //     .for_each(|d| {
        //         let d = d.make_mut();
        //         d.arguments.sort_by(|a, b| a.name.cmp(&b.name));
        //         d.locations.sort_by(|a, b| a.name().cmp(&b.name()));
        //     });
        schema.to_string()
    }
}

fn merge_schema(supergraph_schema: &mut Schema, subgraph: &Subgraph) {
    let subgraph_schema = subgraph.db.schema();
    merge_options(
        &mut supergraph_schema.description,
        &subgraph_schema.description,
    );
    // if let Some(description) = &subgraph_schema.description {
    //     if let Some(supergraph_description) = &supergraph_schema.description {
    //         if !supergraph_description.eq(description) {
    //             // TODO add hint warning
    //             supergraph_schema.description = Some(description.clone());
    //         }
    //     } else {
    //         supergraph_schema.description = Some(description.clone());
    //     }
    // }

    if subgraph_schema.query_type.is_some() {
        supergraph_schema.query_type = subgraph_schema.query_type.clone();
    }
    if subgraph_schema.mutation_type.is_some() {
        supergraph_schema.mutation_type = subgraph_schema.mutation_type.clone();
    }
    if subgraph_schema.subscription_type.is_some() {
        supergraph_schema.subscription_type = subgraph_schema.subscription_type.clone();
    }
}

fn merge_options<T: Eq + Clone>(merged: &mut Option<T>, new: &Option<T>) -> Result<(), MergeError> {
    match (&mut *merged, new) {
        (_, None) => {}
        (None, Some(_)) => *merged = new.clone(),
        (Some(a), Some(b)) => {
            if a != b {
                return Err("conflicting optional values");
            }
        }
    }
    Ok(())
}

fn print_type_order(extended_type: &ExtendedType) -> i8 {
    match extended_type {
        ExtendedType::Enum(_) => 1,
        ExtendedType::Interface(_) => 2,
        ExtendedType::Union(_) => 3,
        ExtendedType::Object(_) => 4,
        ExtendedType::InputObject(_) => 5,
        ExtendedType::Scalar(_) => 6,
    }
}

// TODO handle federation specific types - skip if any of the link/fed spec
// TODO this info should be coming from other module
const FEDERATION_TYPES: [&str; 4] = ["_Any", "_Entity", "_Service", "@key"];
fn is_mergeable_type(type_name: &str) -> bool {
    if type_name.starts_with("federation__") || type_name.starts_with("link__") {
        return false;
    }
    !FEDERATION_TYPES.contains(&type_name)
}

fn merge_enum_type(
    types: &mut IndexMap<NamedType, ExtendedType>,
    subgraph_name: String,
    enum_name: NamedType,
    enum_type: &Node<EnumType>,
) {
    let existing_type = types
        .entry(enum_name)
        .or_insert(create_enum_type_stub(enum_type));
    if let ExtendedType::Enum(e) = existing_type {
        let join_type_directives =
            join_type_applied_directive(&subgraph_name, iter::empty(), false);
        e.make_mut().directives.extend(join_type_directives);

        merge_options(&mut e.make_mut().description, &enum_type.description);

        // TODO we need to merge those fields LAST so we know whether enum is used as input/output/both as different merge rules will apply
        // below logic only works for output enums
        for (enum_value_name, enum_value) in enum_type.values.iter() {
            let ev = e
                .make_mut()
                .values
                .entry(enum_value_name.clone())
                .or_insert(Component::new(EnumValueDefinition {
                    value: enum_value.value.clone(),
                    description: None,
                    directives: vec![],
                }));
            merge_options(&mut ev.make_mut().description, &enum_value.description);
            ev.make_mut().directives.push(Node::new(Directive {
                name: Name::new("join__enumValue"),
                arguments: vec![
                    (Node::new(Argument {
                        name: Name::new("graph"),
                        value: Node::new(Value::Enum(Name::new(&subgraph_name))),
                    })),
                ],
            }));
        }
    } else {
        // TODO - conflict
    }
}

fn merge_input_object_type(
    types: &mut IndexMap<ast::NamedType, ExtendedType>,
    subgraph_name: String,
    input_object_name: ast::NamedType,
    input_object: &Node<InputObjectType>,
) {
    let existing_type = types
        .entry(input_object_name)
        .or_insert(create_input_object_type(input_object));
    if let ExtendedType::InputObject(obj) = existing_type {
        let join_type_directives =
            join_type_applied_directive(&subgraph_name, iter::empty(), false);
        let mutable_object = obj.make_mut();
        mutable_object.directives.extend(join_type_directives);

        for (field_name, field) in input_object.fields.iter() {
            let existing_field = mutable_object.fields.entry(field_name.clone());
            match existing_field {
                Vacant(i) => {
                    // TODO warning - mismatch on input fields
                }
                Occupied(i) => {
                    // merge_options(&i.get_mut().description, &field.description);
                    // TODO check description
                    // TODO check type
                    // TODO check default value
                    // TODO process directives
                }
            }
        }
    } else {
        // TODO conflict on type
    }
}

fn merge_interface_type(
    types: &mut IndexMap<ast::NamedType, ExtendedType>,
    subgraph_name: String,
    interface_name: ast::NamedType,
    interface: &Node<InterfaceType>,
) {
    let existing_type = types
        .entry(interface_name.clone())
        .or_insert(create_interface_type(interface));
    if let ExtendedType::Interface(intf) = existing_type {
        let key_directives = interface.directives_by_name("key");
        let join_type_directives =
            join_type_applied_directive(&subgraph_name, key_directives, false);
        let mutable_intf = intf.make_mut();
        mutable_intf.directives.extend(join_type_directives);

        for (field_name, field) in interface.fields.iter() {
            let existing_field = mutable_intf.fields.entry(field_name.clone());
            match existing_field {
                Vacant(i) => {
                    // TODO warning mismatch missing fields
                    i.insert(Component::new(FieldDefinition {
                        name: field.name.clone(),
                        description: field.description.clone(),
                        arguments: vec![],
                        ty: field.ty.clone(),
                        directives: vec![],
                    }));
                }
                Occupied(i) => {
                    // TODO check description
                    // TODO check type
                    // TODO check default value
                    // TODO process directives
                }
            }
        }
    } else {
        // TODO conflict on type
    }
}

fn merge_object_type(
    types: &mut IndexMap<NamedType, ExtendedType>,
    subgraph_name: String,
    object_name: NamedType,
    object: &Node<ObjectType>,
) {
    let is_interface_object = object.directives_by_name("interfaceObject").count() > 0;
    let existing_type = types
        .entry(object_name.clone())
        .or_insert(create_object_type_stub(
            &object_name,
            is_interface_object,
            object.description.clone(),
        ));
    if let ExtendedType::Object(obj) = existing_type {
        let mut key_directives = object.directives_by_name("key").peekable();
        let is_join_field = key_directives.peek().is_some() || object_name.eq("Query");
        let join_type_directives =
            join_type_applied_directive(&subgraph_name, key_directives, false);
        let mutable_object = obj.make_mut();
        mutable_object.directives.extend(join_type_directives);
        merge_options(&mut mutable_object.description, &object.description);
        object
            .implements_interfaces
            .iter()
            .for_each(|(intf_name, intf)| {
                let implement_interface_entry = mutable_object
                    .implements_interfaces
                    .entry(intf_name.clone());
                if let Vacant(i) = implement_interface_entry {
                    // TODO warning mismatch on interface implementations
                    i.insert(intf.clone());
                }
                let join_implements_directive = join_type_implements(&subgraph_name, intf_name);
                mutable_object.directives.push(join_implements_directive);
            });

        for (field_name, field) in object.fields.iter() {
            let existing_field = mutable_object.fields.entry(field_name.clone());
            let supergraph_field = match existing_field {
                Occupied(f) => {
                    // check description
                    // check type
                    // check args
                    f.into_mut()
                }
                Vacant(f) => f.insert(Component::new(FieldDefinition {
                    name: field.name.clone(),
                    description: field.description.clone(),
                    arguments: vec![],
                    directives: vec![],
                    ty: field.ty.clone(),
                })),
            };
            merge_options(
                &mut supergraph_field.make_mut().description,
                &field.description,
            );
            // let mut existing_args = supergraph_field.arguments.iter();
            // for arg in field.arguments.iter() {
            //     let existing_arg = &existing_args.find(|a| a.name.eq(&arg.name));
            // }

            if is_join_field {
                let is_key_field = false;
                if !is_key_field {
                    supergraph_field
                        .make_mut()
                        .directives
                        .push(Node::new(Directive {
                            name: Name::new("join__field"),
                            arguments: vec![
                                (Node::new(Argument {
                                    name: Name::new("graph"),
                                    value: Node::new(Value::Enum(Name::new(&subgraph_name))),
                                })),
                            ],
                        }));
                }
            }
        }
    } else if let ExtendedType::Interface(intf) = existing_type {
        // TODO support interface object
        let key_directives = object.directives_by_name("key");
        let join_type_directives =
            join_type_applied_directive(&subgraph_name, key_directives, true);
        intf.make_mut().directives.extend(join_type_directives);
    };
    // TODO merge fields
}

fn merge_union_type(
    types: &mut IndexMap<ast::NamedType, ExtendedType>,
    subgraph_name: String,
    union_name: ast::NamedType,
    union: &Node<UnionType>,
) {
    let existing_type = types
        .entry(union_name.clone())
        .or_insert(create_union_type_stub(
            &union_name,
            union.description.clone(),
        ));
    if let ExtendedType::Union(u) = existing_type {
        let join_type_directives =
            join_type_applied_directive(&subgraph_name, iter::empty(), false);
        u.make_mut().directives.extend(join_type_directives);

        for (union_member, _) in union.members.iter() {
            u.make_mut()
                .members
                .entry(union_member.clone())
                .or_insert(ComponentOrigin::Definition);
            u.make_mut().directives.push(Component::new(Directive {
                name: Name::new("join__unionMember"),
                arguments: vec![
                    Node::new(Argument {
                        name: Name::new("graph"),
                        value: Node::new(Value::Enum(Name::new(&subgraph_name))),
                    }),
                    Node::new(Argument {
                        name: Name::new("member"),
                        value: Node::new(Value::String(Name::new(&union_member))),
                    }),
                ],
            }));
        }
    }
}

fn create_enum_type_stub(enum_type: &Node<EnumType>) -> ExtendedType {
    ExtendedType::Enum(Node::new(EnumType {
        name: enum_type.name.clone(),
        description: enum_type.description.clone(),
        directives: vec![],
        values: IndexMap::new(),
    }))
}

fn create_input_object_type(input_object: &Node<InputObjectType>) -> ExtendedType {
    let mut new_input_object = InputObjectType {
        name: input_object.name.clone(),
        description: input_object.description.clone(),
        directives: vec![],
        fields: IndexMap::new(),
    };

    for (field_name, input_field) in input_object.fields.iter() {
        new_input_object.fields.insert(
            field_name.clone(),
            Component::new(InputValueDefinition {
                name: input_field.name.clone(),
                description: input_field.description.clone(),
                directives: vec![],
                ty: input_field.ty.clone(),
                default_value: input_field.default_value.clone(),
            }),
        );
    }

    ExtendedType::InputObject(Node::new(new_input_object))
}

fn create_interface_type(interface: &Node<InterfaceType>) -> ExtendedType {
    let mut new_interface = InterfaceType {
        name: interface.name.clone(),
        description: interface.description.clone(),
        directives: vec![],
        fields: IndexMap::new(),
        implements_interfaces: IndexMap::new(),
    };
    for (field_name, field) in interface.fields.iter() {
        let args: Vec<Node<InputValueDefinition>> = field
            .arguments
            .iter()
            .map(|a| {
                Node::new(InputValueDefinition {
                    name: a.name.clone(),
                    description: a.description.clone(),
                    directives: vec![],
                    ty: a.ty.clone(),
                    default_value: a.default_value.clone(),
                })
            })
            .collect();
        let new_field = Component::new(FieldDefinition {
            name: field.name.clone(),
            description: field.description.clone(),
            directives: vec![],
            arguments: args,
            ty: field.ty.clone(),
        });

        new_interface.fields.insert(field_name.clone(), new_field);
    }

    ExtendedType::Interface(Node::new(new_interface))
}

fn create_object_type_stub(
    name: &ast::NamedType,
    is_interface_object: bool,
    description: Option<NodeStr>,
) -> ExtendedType {
    if is_interface_object {
        // create_interface_type(name, description)
        panic!("foo")
    } else {
        ExtendedType::Object(Node::new(ObjectType {
            name: name.clone(),
            description,
            directives: vec![],
            fields: IndexMap::new(),
            implements_interfaces: IndexMap::new(),
        }))
    }
}

fn create_union_type_stub(name: &ast::NamedType, description: Option<NodeStr>) -> ExtendedType {
    ExtendedType::Union(Node::new(UnionType {
        name: name.clone(),
        description,
        directives: vec![],
        members: IndexMap::new(),
    }))
}

fn join_type_applied_directive<'a>(
    subgraph_name: &str,
    key_directives: impl Iterator<Item = &'a Component<Directive>> + Sized,
    is_interface_object: bool,
) -> Vec<Component<Directive>> {
    let mut join_type_directive = Directive {
        name: Name::new("join__type"),
        arguments: vec![Node::new(Argument {
            name: Name::new("graph"),
            value: Node::new(Value::Enum(Name::new(subgraph_name))),
        })],
    };
    if is_interface_object {
        join_type_directive.arguments.push(Node::new(Argument {
            name: Name::new("isInterfaceObject"),
            value: Node::new(Value::Boolean(is_interface_object)),
        }));
    }

    let mut result = vec![];
    for key_directive in key_directives {
        let mut join_type_directive_with_key = join_type_directive.clone();
        let field_set = directive_string_arg_value(key_directive, "fields").unwrap();
        join_type_directive_with_key
            .arguments
            .push(Node::new(Argument {
                name: Name::new("key"),
                value: Node::new(Value::String(NodeStr::new(field_set.as_str()))),
            }));

        let resolvable = directive_bool_arg_value(key_directive, "resolvable").unwrap_or(&true);
        if !resolvable {
            join_type_directive_with_key
                .arguments
                .push(Node::new(Argument {
                    name: Name::new("resolvable"),
                    value: Node::new(Value::Boolean(false)),
                }));
        }
        result.push(join_type_directive_with_key)
    }
    if result.is_empty() {
        result.push(join_type_directive)
    }
    result
        .into_iter()
        .map(|d| Component::new(d))
        .collect::<Vec<Component<Directive>>>()
}

fn join_type_implements(subgraph_name: &str, intf_name: &str) -> Component<Directive> {
    Component::new(Directive {
        name: Name::new("join__implements"),
        arguments: vec![
            Node::new(Argument {
                name: Name::new("graph"),
                value: Node::new(Value::String(NodeStr::new(subgraph_name))),
            }),
            Node::new(Argument {
                name: Name::new("interface"),
                value: Node::new(Value::String(NodeStr::new(intf_name))),
            }),
        ],
    })
}

fn directive_arg_value<'a>(directive: &'a Directive, arg_name: &'static str) -> Option<&'a Value> {
    directive
        .arguments
        .iter()
        .find(|arg| arg.name == arg_name)
        .map(|arg| arg.value.as_ref())
}

fn directive_string_arg_value<'a>(
    directive: &'a Directive,
    arg_name: &'static str,
) -> Option<&'a NodeStr> {
    match directive_arg_value(directive, arg_name) {
        Some(Value::String(value)) => Some(value),
        _ => None,
    }
}

fn directive_bool_arg_value<'a>(
    directive: &'a Directive,
    arg_name: &'static str,
) -> Option<&'a bool> {
    match directive_arg_value(directive, arg_name) {
        Some(Value::Boolean(value)) => Some(value),
        _ => None,
    }
}

// TODO link spec
fn add_core_feature_link(supergraph: &mut Schema) {
    // @link(url: "https://specs.apollo.dev/link/v1.0")
    supergraph.directives.push(Component::new(Directive {
        name: Name::new("link"),
        arguments: vec![Node::new(Argument {
            name: Name::new("url"),
            value: Node::new(Value::String(NodeStr::new(
                "https://specs.apollo.dev/link/v1.0",
            ))),
        })],
    }));

    let link_purpose_enum = link_purpose_enum_type();
    supergraph.types.insert(
        link_purpose_enum.name.clone(),
        ExtendedType::Enum(Node::new(link_purpose_enum)),
    );

    // scalar Import
    let link_import_scalar = ExtendedType::Scalar(Node::new(ScalarType {
        name: Name::new("link__Import"),
        directives: Vec::new(),
        description: None,
    }));
    supergraph
        .types
        .insert(link_import_scalar.name().clone(), link_import_scalar);

    let link_directive_definition = link_directive_definition();
    supergraph.directive_definitions.insert(
        ast::NamedType::new("link"),
        Node::new(link_directive_definition),
    );
}

/// directive @link(url: String, as: String, import: [Import], for: link__Purpose) repeatable on SCHEMA
fn link_directive_definition() -> ast::DirectiveDefinition {
    ast::DirectiveDefinition {
        name: Name::new("link"),
        description: None,
        arguments: vec![
            Node::new(ast::InputValueDefinition {
                name: Name::new("url"),
                description: None,
                directives: vec![],
                ty: ast::Type::Named(NodeStr::new("String")),
                default_value: None,
            }),
            Node::new(ast::InputValueDefinition {
                name: Name::new("as"),
                description: None,
                directives: vec![],
                ty: ast::Type::Named(NodeStr::new("String")),
                default_value: None,
            }),
            Node::new(ast::InputValueDefinition {
                name: Name::new("for"),
                description: None,
                directives: vec![],
                ty: ast::Type::Named(NodeStr::new("link__Purpose")),
                default_value: None,
            }),
            Node::new(ast::InputValueDefinition {
                name: Name::new("import"),
                description: None,
                directives: vec![],
                ty: ast::Type::List(Box::new(ast::Type::Named(NodeStr::new("link__Import")))),
                default_value: None,
            }),
        ],
        locations: vec![ast::DirectiveLocation::Schema],
        repeatable: true,
    }
}

/// enum link__Purpose {
///   """
///   \`SECURITY\` features provide metadata necessary to securely resolve fields.
///   """
///   SECURITY
///
///   """
///   \`EXECUTION\` features provide metadata necessary for operation execution.
///   """
///   EXECUTION
/// }
fn link_purpose_enum_type() -> EnumType {
    let mut link_purpose_enum = EnumType {
        name: Name::new("link__Purpose"),
        description: None,
        directives: Vec::new(),
        values: IndexMap::new(),
    };
    let link_purpose_security_value = ast::EnumValueDefinition {
        description: Some(NodeStr::new(
            r"SECURITY features provide metadata necessary to securely resolve fields.",
        )),
        directives: Vec::new(),
        value: Name::new("SECURITY"),
    };
    let link_purpose_execution_value = ast::EnumValueDefinition {
        description: Some(NodeStr::new(
            r"EXECUTION features provide metadata necessary for operation execution.",
        )),
        directives: Vec::new(),
        value: Name::new("EXECUTION"),
    };
    link_purpose_enum.values.insert(
        link_purpose_security_value.value.clone(),
        Component::new(link_purpose_security_value),
    );
    link_purpose_enum.values.insert(
        link_purpose_execution_value.value.clone(),
        Component::new(link_purpose_execution_value),
    );
    link_purpose_enum
}

// TODO join spec
fn add_core_feature_join(supergraph: &mut Schema, subgraphs: &Vec<&Subgraph>) {
    // @link(url: "https://specs.apollo.dev/join/v0.3", for: EXECUTION)
    supergraph.directives.push(Component::new(Directive {
        name: Name::new("link"),
        arguments: vec![
            Node::new(Argument {
                name: Name::new("url"),
                value: Node::new(Value::String(NodeStr::new(
                    "https://specs.apollo.dev/join/v0.3",
                ))),
            }),
            Node::new(Argument {
                name: Name::new("for"),
                value: Node::new(Value::Enum(NodeStr::new("EXECUTION"))),
            }),
        ],
    }));

    // scalar FieldSet
    let join_field_set_scalar = ExtendedType::Scalar(Node::new(ScalarType {
        name: Name::new("join__FieldSet"),
        directives: Vec::new(),
        description: None,
    }));
    supergraph
        .types
        .insert(join_field_set_scalar.name().clone(), join_field_set_scalar);

    let join_graph_directive_definition = join_graph_directive_definition();
    supergraph.directive_definitions.insert(
        join_graph_directive_definition.name.clone(),
        Node::new(join_graph_directive_definition),
    );

    let join_type_directive_definition = join_type_directive_definition();
    supergraph.directive_definitions.insert(
        join_type_directive_definition.name.clone(),
        Node::new(join_type_directive_definition),
    );

    let join_field_directive_definition = join_field_directive_definition();
    supergraph.directive_definitions.insert(
        join_field_directive_definition.name.clone(),
        Node::new(join_field_directive_definition),
    );

    let join_implements_directive_definition = join_implements_directive_definition();
    supergraph.directive_definitions.insert(
        join_implements_directive_definition.name.clone(),
        Node::new(join_implements_directive_definition),
    );

    let join_union_member_directive_definition = join_union_member_directive_definition();
    supergraph.directive_definitions.insert(
        join_union_member_directive_definition.name.clone(),
        Node::new(join_union_member_directive_definition),
    );

    let join_enum_value_directive_definition = join_enum_value_directive_definition();
    supergraph.directive_definitions.insert(
        join_enum_value_directive_definition.name.clone(),
        Node::new(join_enum_value_directive_definition),
    );

    let join_graph_enum_type = join_graph_enum_type(subgraphs);
    supergraph.types.insert(
        join_graph_enum_type.name.clone(),
        ExtendedType::Enum(Node::new(join_graph_enum_type)),
    );
}

/// directive @enumValue(graph: join__Graph!) repeatable on ENUM_VALUE
fn join_enum_value_directive_definition() -> ast::DirectiveDefinition {
    ast::DirectiveDefinition {
        name: Name::new("join__enumValue"),
        description: None,
        arguments: vec![Node::new(ast::InputValueDefinition {
            name: Name::new("graph"),
            description: None,
            directives: vec![],
            ty: ast::Type::NonNullNamed(NodeStr::new("join__Graph")),
            default_value: None,
        })],
        locations: vec![ast::DirectiveLocation::EnumValue],
        repeatable: true,
    }
}

/// directive @field(
///   graph: Graph,
///   requires: FieldSet,
///   provides: FieldSet,
///   type: String,
///   external: Boolean,
///   override: String,
///   usedOverridden: Boolean
/// ) repeatable on FIELD_DEFINITION | INPUT_FIELD_DEFINITION
fn join_field_directive_definition() -> ast::DirectiveDefinition {
    ast::DirectiveDefinition {
        name: Name::new("join__field"),
        description: None,
        arguments: vec![
            Node::new(ast::InputValueDefinition {
                name: Name::new("graph"),
                description: None,
                directives: vec![],
                ty: ast::Type::Named(NodeStr::new("join__Graph")),
                default_value: None,
            }),
            Node::new(ast::InputValueDefinition {
                name: Name::new("requires"),
                description: None,
                directives: vec![],
                ty: ast::Type::Named(NodeStr::new("join__FieldSet")),
                default_value: None,
            }),
            Node::new(ast::InputValueDefinition {
                name: Name::new("provides"),
                description: None,
                directives: vec![],
                ty: ast::Type::Named(NodeStr::new("join__FieldSet")),
                default_value: None,
            }),
            Node::new(ast::InputValueDefinition {
                name: Name::new("type"),
                description: None,
                directives: vec![],
                ty: ast::Type::Named(NodeStr::new("String")),
                default_value: None,
            }),
            Node::new(ast::InputValueDefinition {
                name: Name::new("external"),
                description: None,
                directives: vec![],
                ty: ast::Type::Named(NodeStr::new("Boolean")),
                default_value: None,
            }),
            Node::new(ast::InputValueDefinition {
                name: Name::new("override"),
                description: None,
                directives: vec![],
                ty: ast::Type::Named(NodeStr::new("String")),
                default_value: None,
            }),
            Node::new(ast::InputValueDefinition {
                name: Name::new("usedOverridden"),
                description: None,
                directives: vec![],
                ty: ast::Type::Named(NodeStr::new("Boolean")),
                default_value: None,
            }),
        ],
        locations: vec![
            ast::DirectiveLocation::FieldDefinition,
            ast::DirectiveLocation::InputFieldDefinition,
        ],
        repeatable: true,
    }
}

/// directive @graph(name: String!, url: String!) on ENUM_VALUE
fn join_graph_directive_definition() -> ast::DirectiveDefinition {
    ast::DirectiveDefinition {
        name: Name::new("join__graph"),
        description: None,
        arguments: vec![
            Node::new(ast::InputValueDefinition {
                name: Name::new("name"),
                description: None,
                directives: vec![],
                ty: ast::Type::NonNullNamed(NodeStr::new("String")),
                default_value: None,
            }),
            Node::new(ast::InputValueDefinition {
                name: Name::new("url"),
                description: None,
                directives: vec![],
                ty: ast::Type::NonNullNamed(NodeStr::new("String")),
                default_value: None,
            }),
        ],
        locations: vec![ast::DirectiveLocation::EnumValue],
        repeatable: false,
    }
}

/// directive @implements(
///   graph: Graph!,
///   interface: String!
/// ) on OBJECT | INTERFACE
fn join_implements_directive_definition() -> ast::DirectiveDefinition {
    ast::DirectiveDefinition {
        name: Name::new("join__implements"),
        description: None,
        arguments: vec![
            Node::new(ast::InputValueDefinition {
                name: Name::new("graph"),
                description: None,
                directives: vec![],
                ty: ast::Type::NonNullNamed(NodeStr::new("join__Graph")),
                default_value: None,
            }),
            Node::new(ast::InputValueDefinition {
                name: Name::new("interface"),
                description: None,
                directives: vec![],
                ty: ast::Type::NonNullNamed(NodeStr::new("String")),
                default_value: None,
            }),
        ],
        locations: vec![
            ast::DirectiveLocation::Interface,
            ast::DirectiveLocation::Object,
        ],
        repeatable: true,
    }
}

/// directive @type(
///   graph: Graph!,
///   key: FieldSet,
///   extension: Boolean! = false,
///   resolvable: Boolean = true,
///   isInterfaceObject: Boolean = false
/// ) repeatable on OBJECT | INTERFACE | UNION | ENUM | INPUT_OBJECT | SCALAR
fn join_type_directive_definition() -> ast::DirectiveDefinition {
    ast::DirectiveDefinition {
        name: Name::new("join__type"),
        description: None,
        arguments: vec![
            Node::new(ast::InputValueDefinition {
                name: Name::new("graph"),
                description: None,
                directives: vec![],
                ty: ast::Type::NonNullNamed(NodeStr::new("join__Graph")),
                default_value: None,
            }),
            Node::new(ast::InputValueDefinition {
                name: Name::new("key"),
                description: None,
                directives: vec![],
                ty: ast::Type::Named(NodeStr::new("join__FieldSet")),
                default_value: None,
            }),
            Node::new(ast::InputValueDefinition {
                name: Name::new("extension"),
                description: None,
                directives: vec![],
                ty: ast::Type::NonNullNamed(NodeStr::new("Boolean")),
                default_value: Some(Node::new(ast::Value::Boolean(false))),
            }),
            Node::new(ast::InputValueDefinition {
                name: Name::new("resolvable"),
                description: None,
                directives: vec![],
                ty: ast::Type::NonNullNamed(NodeStr::new("Boolean")),
                default_value: Some(Node::new(ast::Value::Boolean(true))),
            }),
            Node::new(ast::InputValueDefinition {
                name: Name::new("isInterfaceObject"),
                description: None,
                directives: vec![],
                ty: ast::Type::NonNullNamed(NodeStr::new("Boolean")),
                default_value: Some(Node::new(ast::Value::Boolean(false))),
            }),
        ],
        locations: vec![
            ast::DirectiveLocation::Enum,
            ast::DirectiveLocation::InputObject,
            ast::DirectiveLocation::Interface,
            ast::DirectiveLocation::Object,
            ast::DirectiveLocation::Scalar,
            ast::DirectiveLocation::Union,
        ],
        repeatable: true,
    }
}

/// directive @unionMember(graph: join__Graph!, member: String!) repeatable on UNION
fn join_union_member_directive_definition() -> ast::DirectiveDefinition {
    ast::DirectiveDefinition {
        name: Name::new("join__unionMember"),
        description: None,
        arguments: vec![
            Node::new(ast::InputValueDefinition {
                name: Name::new("graph"),
                description: None,
                directives: vec![],
                ty: ast::Type::NonNullNamed(NodeStr::new("join__Graph")),
                default_value: None,
            }),
            Node::new(ast::InputValueDefinition {
                name: Name::new("member"),
                description: None,
                directives: vec![],
                ty: ast::Type::NonNullNamed(NodeStr::new("String")),
                default_value: None,
            }),
        ],
        locations: vec![ast::DirectiveLocation::Union],
        repeatable: true,
    }
}

/// enum Graph
fn join_graph_enum_type(subgraphs: &Vec<&Subgraph>) -> EnumType {
    let mut join_graph_enum_type = EnumType {
        name: Name::new("join__Graph"),
        description: None,
        directives: Vec::new(),
        values: IndexMap::new(),
    };
    for s in subgraphs {
        let join_graph_applied_directive = ast::Directive {
            name: Name::new("join__graph"),
            arguments: vec![
                (Node::new(Argument {
                    name: Name::new("name"),
                    value: Node::new(Value::String(NodeStr::new(s.name.as_str()))),
                })),
                (Node::new(Argument {
                    name: Name::new("url"),
                    value: Node::new(Value::String(NodeStr::new(s.url.as_str()))),
                })),
            ],
        };
        let graph = ast::EnumValueDefinition {
            description: None,
            directives: vec![Node::new(join_graph_applied_directive)],
            value: Name::new(s.name.to_uppercase().as_str()),
        };
        join_graph_enum_type
            .values
            .insert(graph.value.clone(), Component::new(graph));
    }
    join_graph_enum_type
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_extract_subgraph() {
        // TODO: not actually implemented; just here to give a sense of the API.
        let schema = r#"
          schema
            @link(url: "https://specs.apollo.dev/link/v1.0")
            @link(url: "https://specs.apollo.dev/join/v0.3", for: EXECUTION)
          {
            query: Query
          }

          directive @join__enumValue(graph: join__Graph!) repeatable on ENUM_VALUE

          directive @join__field(graph: join__Graph, requires: join__FieldSet, provides: join__FieldSet, type: String, external: Boolean, override: String, usedOverridden: Boolean) repeatable on FIELD_DEFINITION | INPUT_FIELD_DEFINITION

          directive @join__graph(name: String!, url: String!) on ENUM_VALUE

          directive @join__implements(graph: join__Graph!, interface: String!) repeatable on OBJECT | INTERFACE

          directive @join__type(graph: join__Graph!, key: join__FieldSet, extension: Boolean! = false, resolvable: Boolean! = true, isInterfaceObject: Boolean! = false) repeatable on OBJECT | INTERFACE | UNION | ENUM | INPUT_OBJECT | SCALAR

          directive @join__unionMember(graph: join__Graph!, member: String!) repeatable on UNION

          directive @link(url: String, as: String, for: link__Purpose, import: [link__Import]) repeatable on SCHEMA

          enum E
            @join__type(graph: SUBGRAPH2)
          {
            V1 @join__enumValue(graph: SUBGRAPH2)
            V2 @join__enumValue(graph: SUBGRAPH2)
          }

          scalar join__FieldSet

          enum join__Graph {
            SUBGRAPH1 @join__graph(name: "Subgraph1", url: "https://Subgraph1")
            SUBGRAPH2 @join__graph(name: "Subgraph2", url: "https://Subgraph2")
          }

          scalar link__Import

          enum link__Purpose {
            """
            \`SECURITY\` features provide metadata necessary to securely resolve fields.
            """
            SECURITY

            """
            \`EXECUTION\` features provide metadata necessary for operation execution.
            """
            EXECUTION
          }

          type Query
            @join__type(graph: SUBGRAPH1)
            @join__type(graph: SUBGRAPH2)
          {
            t: T @join__field(graph: SUBGRAPH1)
          }

          type S
            @join__type(graph: SUBGRAPH1)
          {
            x: Int
          }

          type T
            @join__type(graph: SUBGRAPH1, key: "k")
            @join__type(graph: SUBGRAPH2, key: "k")
          {
            k: ID
            a: Int @join__field(graph: SUBGRAPH2)
            b: String @join__field(graph: SUBGRAPH2)
          }

          union U
            @join__type(graph: SUBGRAPH1)
            @join__unionMember(graph: SUBGRAPH1, member: "S")
            @join__unionMember(graph: SUBGRAPH1, member: "T")
           = S | T
        "#;

        let supergraph = Supergraph::new(schema);
        let _subgraphs = supergraph
            .db
            .extract_subgraphs()
            .expect("Should have been able to extract subgraphs");
        // TODO: actual assertions on the subgraph once it's actually implemented.
    }

    #[test]
    fn can_compose_supergraph() {
        let s1 = Subgraph::parse_and_expand(
            "Subgraph1",
            "https://subgraph1",
            r#"
                type Query {
                  t: T
                }
        
                type T @key(fields: "k") {
                  k: ID
                }
        
                type S {
                  x: Int
                }
        
                union U = S | T
            "#,
        )
        .unwrap();
        let s2 = Subgraph::parse_and_expand(
            "Subgraph2",
            "https://subgraph2",
            r#"
                type T @key(fields: "k") {
                  k: ID
                  a: Int
                  b: String
                }
                
                enum E {
                  V1
                  V2
                }
            "#,
        )
        .unwrap();

        let supergraph = Supergraph::compose(vec![&s1, &s2]).unwrap();
        let expected_supergraph_sdl = r#"schema @link(url: "https://specs.apollo.dev/link/v1.0") @link(url: "https://specs.apollo.dev/join/v0.3", for: EXECUTION) {
  query: Query
}

directive @join__enumValue(graph: join__Graph!) repeatable on ENUM_VALUE

directive @join__field(graph: join__Graph, requires: join__FieldSet, provides: join__FieldSet, type: String, external: Boolean, override: String, usedOverridden: Boolean) repeatable on FIELD_DEFINITION | INPUT_FIELD_DEFINITION

directive @join__graph(name: String!, url: String!) on ENUM_VALUE

directive @join__implements(graph: join__Graph!, interface: String!) repeatable on INTERFACE | OBJECT

directive @join__type(graph: join__Graph!, key: join__FieldSet, extension: Boolean! = false, resolvable: Boolean! = true, isInterfaceObject: Boolean! = false) repeatable on ENUM | INPUT_OBJECT | INTERFACE | OBJECT | SCALAR | UNION

directive @join__unionMember(graph: join__Graph!, member: String!) repeatable on UNION

directive @link(url: String, as: String, for: link__Purpose, import: [link__Import]) repeatable on SCHEMA

enum E @join__type(graph: SUBGRAPH2) {
  V1 @join__enumValue(graph: SUBGRAPH2)
  V2 @join__enumValue(graph: SUBGRAPH2)
}

enum join__Graph {
  SUBGRAPH1 @join__graph(name: "Subgraph1", url: "https://subgraph1")
  SUBGRAPH2 @join__graph(name: "Subgraph2", url: "https://subgraph2")
}

enum link__Purpose {
  "SECURITY features provide metadata necessary to securely resolve fields."
  SECURITY
  "EXECUTION features provide metadata necessary for operation execution."
  EXECUTION
}

union U @join__type(graph: SUBGRAPH1) @join__unionMember(graph: SUBGRAPH1, member: "S") @join__unionMember(graph: SUBGRAPH1, member: "T") = S | T

type Query @join__type(graph: SUBGRAPH1) @join__type(graph: SUBGRAPH2) {
  t: T @join__field(graph: SUBGRAPH1)
}

type S @join__type(graph: SUBGRAPH1) {
  x: Int
}

type T @join__type(graph: SUBGRAPH1, key: "k") @join__type(graph: SUBGRAPH2, key: "k") {
  k: ID
  a: Int @join__field(graph: SUBGRAPH2)
  b: String @join__field(graph: SUBGRAPH2)
}

scalar join__FieldSet

scalar link__Import
"#;
        assert_eq!(supergraph.print_sdl(), expected_supergraph_sdl);
    }

    #[test]
    fn can_compose_with_descriptions() {
        let s1 = Subgraph::parse_and_expand(
            "Subgraph1",
            "https://subgraph1",
            r#"
                "The foo directive description"
                directive @foo(url: String) on FIELD
    
                "A cool schema"
                schema {
                  query: Query
                }
    
                """
                Available queries
                Not much yet
                """
                type Query {
                  "Returns tea"
                  t(
                    "An argument that is very important"
                    x: String!
                  ): String
                }
            "#,
        )
        .unwrap();

        let s2 = Subgraph::parse_and_expand(
            "Subgraph2",
            "https://subgraph2",
            r#"
                "The foo directive description"
                directive @foo(url: String) on FIELD
    
                "An enum"
                enum E {
                  "The A value"
                  A
                  "The B value"
                  B
                }
            "#,
        )
        .unwrap();

        let expected_supergraph_sdl = r#""A cool schema"
schema @link(url: "https://specs.apollo.dev/link/v1.0") @link(url: "https://specs.apollo.dev/join/v0.3", for: EXECUTION) {
  query: Query
}

"""The foo directive description"""
directive @foo(url: String) on FIELD

directive @join__enumValue(graph: join__Graph!) repeatable on ENUM_VALUE

directive @join__field(graph: join__Graph, requires: join__FieldSet, provides: join__FieldSet, type: String, external: Boolean, override: String, usedOverridden: Boolean) repeatable on FIELD_DEFINITION | INPUT_FIELD_DEFINITION

directive @join__graph(name: String!, url: String!) on ENUM_VALUE

directive @join__implements(graph: join__Graph!, interface: String!) repeatable on OBJECT | INTERFACE

directive @join__type(graph: join__Graph!, key: join__FieldSet, extension: Boolean! = false, resolvable: Boolean! = true, isInterfaceObject: Boolean! = false) repeatable on OBJECT | INTERFACE | UNION | ENUM | INPUT_OBJECT | SCALAR

directive @join__unionMember(graph: join__Graph!, member: String!) repeatable on UNION

directive @link(url: String, as: String, for: link__Purpose, import: [link__Import]) repeatable on SCHEMA

"An enum"
enum E @join__type(graph: SUBGRAPH2) {
  "The A value"
  A @join__enumValue(graph: SUBGRAPH2)
  "The B value"
  B @join__enumValue(graph: SUBGRAPH2)
}

enum join__Graph {
  SUBGRAPH1 @join__graph(name: "Subgraph1", url: "https://subgraph1")
  SUBGRAPH2 @join__graph(name: "Subgraph2", url: "https://subgraph2")
}

enum link__Purpose {
  "SECURITY features provide metadata necessary to securely resolve fields."
  SECURITY
  "EXECUTION features provide metadata necessary for operation execution."
  EXECUTION
}

"""
Available queries
Not much yet
"""
type Query @join__type(graph: SUBGRAPH1) @join__type(graph: SUBGRAPH2) {
  """Returns tea"""
  t(
    """An argument that is very important"""
    x: String!
  ): String @join__field(graph: SUBGRAPH1)
}

scalar join__FieldSet

scalar link__Import
"#;
        let supergraph = Supergraph::compose(vec![&s1, &s2]).unwrap();
        assert_eq!(supergraph.print_sdl(), expected_supergraph_sdl);
    }

    #[test]
    fn can_compose_types_from_different_subgraphs() {
        let s1 = Subgraph::parse_and_expand(
            "SubgraphA",
            "https://subgraphA",
            r#"
                type Query {
                    products: [Product!]
                }

                type Product {
                    sku: String!
                    name: String!
                }
            "#,
        )
        .unwrap();

        let s2 = Subgraph::parse_and_expand(
            "SubgraphB",
            "https://subgraphB",
            r#"
                type User {
                    name: String
                    email: String!
                }
            "#,
        )
        .unwrap();

        let expected_supergraph_sdl = r#"schema @link(url: "https://specs.apollo.dev/link/v1.0") @link(url: "https://specs.apollo.dev/join/v0.3", for: EXECUTION) {
  query: Query
}

directive @join__enumValue(graph: join__Graph!) repeatable on ENUM_VALUE

directive @join__field(graph: join__Graph, requires: join__FieldSet, provides: join__FieldSet, type: String, external: Boolean, override: String, usedOverridden: Boolean) repeatable on FIELD_DEFINITION | INPUT_FIELD_DEFINITION

directive @join__graph(name: String!, url: String!) on ENUM_VALUE

directive @join__implements(graph: join__Graph!, interface: String!) repeatable on INTERFACE | OBJECT

directive @join__type(graph: join__Graph!, key: join__FieldSet, extension: Boolean! = false, resolvable: Boolean! = true, isInterfaceObject: Boolean! = false) repeatable on ENUM | INPUT_OBJECT | INTERFACE | OBJECT | SCALAR | UNION

directive @join__unionMember(graph: join__Graph!, member: String!) repeatable on UNION

directive @link(url: String, as: String, for: link__Purpose, import: [link__Import]) repeatable on SCHEMA

enum join__Graph {
  SUBGRAPHA @join__graph(name: "SubgraphA", url: "https://subgraphA")
  SUBGRAPHB @join__graph(name: "SubgraphB", url: "https://subgraphB")
}

enum link__Purpose {
  "SECURITY features provide metadata necessary to securely resolve fields."
  SECURITY
  "EXECUTION features provide metadata necessary for operation execution."
  EXECUTION
}

type Product @join__type(graph: SUBGRAPHA) {
  sku: String!
  name: String!
}

type Query @join__type(graph: SUBGRAPHA) @join__type(graph: SUBGRAPHB) {
  products: [Product!] @join__field(graph: SUBGRAPHA)
}

type User @join__type(graph: SUBGRAPHB) {
  name: String
  email: String!
}

scalar join__FieldSet

scalar link__Import
"#;
        let supergraph = Supergraph::compose(vec![&s1, &s2]).unwrap();
        assert_eq!(supergraph.print_sdl(), expected_supergraph_sdl);
    }

    #[test]
    fn compose_removes_federation_directives() {
        let s1 = Subgraph::parse_and_expand(
            "SubgraphA",
            "https://subgraphA",
            r#"
                type Query {
                  products: [Product!] @provides(fields: "name")
                }
        
                type Product @key(fields: "sku") {
                  sku: String!
                  name: String! @external
                }
            "#,
        )
        .unwrap();

        let s2 = Subgraph::parse_and_expand(
            "SubgraphB",
            "https://subgraphB",
            r#"
                type Product @key(fields: "sku") {
                  sku: String!
                  name: String! @shareable
                }
            "#,
        )
        .unwrap();

        let expected_supergraph_sdl = r#"schema @link(url: "https://specs.apollo.dev/link/v1.0") @link(url: "https://specs.apollo.dev/join/v0.3", for: EXECUTION) {
  query: Query
}

directive @join__enumValue(graph: join__Graph!) repeatable on ENUM_VALUE

directive @join__field(graph: join__Graph, requires: join__FieldSet, provides: join__FieldSet, type: String, external: Boolean, override: String, usedOverridden: Boolean) repeatable on FIELD_DEFINITION | INPUT_FIELD_DEFINITION

directive @join__graph(name: String!, url: String!) on ENUM_VALUE

directive @join__implements(graph: join__Graph!, interface: String!) repeatable on INTERFACE | OBJECT

directive @join__type(graph: join__Graph!, key: join__FieldSet, extension: Boolean! = false, resolvable: Boolean! = true, isInterfaceObject: Boolean! = false) repeatable on ENUM | INPUT_OBJECT | INTERFACE | OBJECT | SCALAR | UNION 

directive @join__unionMember(graph: join__Graph!, member: String!) repeatable on UNION

directive @link(url: String, as: String, for: link__Purpose, import: [link__Import]) repeatable on SCHEMA

enum join__Graph {
  SUBGRAPHA @join__graph(name: "subgraphA", url: "https://subgraphA")
  SUBGRAPHB @join__graph(name: "subgraphB", url: "https://subgraphB")
}

enum link__Purpose {
  "SECURITY features provide metadata necessary to securely resolve fields."
  SECURITY
  "EXECUTION features provide metadata necessary for operation execution."
  EXECUTION
}

type Product @join__type(graph: SUBGRAPHA, key: "sku") @join__type(graph: SUBGRAPHB, key: "sku") {
  sku: String!
  name: String! @join__field(graph: SUBGRAPHA, external: true) @join__field(graph: SUBGRAPHB)
}

type Query @join__type(graph: SUBGRAPHA) @join__type(graph: SUBGRAPHB) {
  products: [Product!] @join__field(graph: SUBGRAPHA, provides: "name")
}

scalar join__FieldSet

scalar link__Import
"#;

        let supergraph = Supergraph::compose(vec![&s1, &s2]).unwrap();
        assert_eq!(supergraph.print_sdl(), expected_supergraph_sdl);
    }
}
