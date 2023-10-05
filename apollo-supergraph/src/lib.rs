use apollo_compiler::ast::Directives;
use apollo_compiler::ast::{
    Argument, Directive, DirectiveDefinition, DirectiveLocation, EnumValueDefinition,
    FieldDefinition, NamedType, Type, Value,
};
use apollo_compiler::schema::{
    Component, EnumType, ExtendedType, InputObjectType, InputValueDefinition, InterfaceType, Name,
    ObjectType, ScalarType, UnionType,
};
use apollo_compiler::{Node, NodeStr, Schema};
use apollo_subgraph::Subgraph;
use indexmap::map::Entry::{Occupied, Vacant};
use indexmap::map::Iter;
use indexmap::{IndexMap, IndexSet};
use std::collections::HashSet;
use std::iter;

pub mod database;

type MergeError = &'static str;

// TODO: Same remark as in other crates: we need to define this more cleanly, and probably need
// some "federation errors" crate.
#[derive(Debug)]
pub struct SupergraphError {
    pub msg: String,
}

pub struct Supergraph {
    pub schema: Schema,
}

impl Supergraph {
    pub fn new(schema_str: &str) -> Self {
        let schema = Schema::parse(schema_str, "schema.graphql");

        // TODO: like for subgraphs, it would nice if `Supergraph` was always representing
        // a valid supergraph (which is simpler than for subgraph, but still at least means
        // that it's valid graphQL in the first place, and that it has the `join` spec).

        Self { schema }
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
            let subgraph_name = subgraph.name.to_uppercase().clone();
            merge_schema(&mut supergraph, subgraph);
            // TODO merge directives

            for (key, value) in &subgraph.schema.types {
                if value.is_built_in() || !is_mergeable_type(key) {
                    // skip built-ins and federation specific types
                    continue;
                }

                match value {
                    ExtendedType::Enum(value) => {
                        merge_enum_type(&mut supergraph.types, &subgraph_name, key.clone(), value)
                    }
                    ExtendedType::InputObject(value) => merge_input_object_type(
                        &mut supergraph.types,
                        &subgraph_name,
                        key.clone(),
                        value,
                    ),
                    ExtendedType::Interface(value) => merge_interface_type(
                        &mut supergraph.types,
                        &subgraph_name,
                        key.clone(),
                        value,
                    ),
                    ExtendedType::Object(value) => {
                        merge_object_type(&mut supergraph.types, &subgraph_name, key.clone(), value)
                    }
                    ExtendedType::Union(value) => {
                        merge_union_type(&mut supergraph.types, &subgraph_name, key.clone(), value)
                    }
                    ExtendedType::Scalar(_value) => {
                        // DO NOTHING
                    }
                }
            }

            // merge executable directives
            for (_, directive) in subgraph.schema.directive_definitions.iter() {
                if is_executable_directive(directive) {
                    merge_directive(&mut supergraph.directive_definitions, directive);
                }
            }

            // explicitly add @join__type info to Query type in order to handle __entities queries
            let subgraph_query_type_exists = subgraph
                .schema
                .query_root_operation()
                .is_some_and(|query_type| subgraph.schema.types.contains_key(query_type));
            if !subgraph_query_type_exists {
                if supergraph.query_root_operation().is_none() {
                    supergraph
                        .schema_definition
                        .get_or_insert_with(Default::default)
                        .make_mut()
                        .query = Some("Query".into());
                }
                let join_type_directives =
                    join_type_applied_directive(&subgraph_name, iter::empty(), false);
                if let Some(query_type_name) = supergraph.query_root_operation() {
                    let query_type = supergraph
                        .types
                        .entry(query_type_name.clone())
                        .or_insert_with(|| {
                            ExtendedType::Object(Node::new(ObjectType {
                                description: None,
                                directives: Default::default(),
                                fields: IndexMap::new(),
                                implements_interfaces: IndexSet::new(),
                            }))
                        });
                    if let ExtendedType::Object(query) = query_type {
                        query.make_mut().directives.extend(join_type_directives);
                    }
                }
            }
        }
        // println!("{}", supergraph);
        Ok(Self::new(supergraph.to_string().as_str()))
    }

    pub fn print_sdl(&self) -> String {
        let mut schema = self.schema.clone();
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

const EXECUTABLE_DIRECTIVE_LOCATIONS: [DirectiveLocation; 8] = [
    DirectiveLocation::Query,
    DirectiveLocation::Mutation,
    DirectiveLocation::Subscription,
    DirectiveLocation::Field,
    DirectiveLocation::FragmentDefinition,
    DirectiveLocation::FragmentSpread,
    DirectiveLocation::InlineFragment,
    DirectiveLocation::VariableDefinition,
];
fn is_executable_directive(directive: &Node<DirectiveDefinition>) -> bool {
    directive
        .locations
        .iter()
        .any(|loc| EXECUTABLE_DIRECTIVE_LOCATIONS.contains(loc))
}

fn merge_schema(supergraph_schema: &mut Schema, subgraph: &Subgraph) {
    let (supergraph_def, subgraph_def) = match (
        &mut supergraph_schema.schema_definition,
        &subgraph.schema.schema_definition,
    ) {
        (_, None) => return,
        (None, Some(_)) => {
            supergraph_schema.schema_definition = subgraph.schema.schema_definition.clone();
            return;
        }
        (Some(supergraph_def), Some(subgraph_def)) => (supergraph_def.make_mut(), subgraph_def),
    };
    merge_descriptions(&mut supergraph_def.description, &subgraph_def.description);

    if subgraph_def.query.is_some() {
        supergraph_def.query = subgraph_def.query.clone();
        // TODO mismatch on query types
    }
    if subgraph_def.mutation.is_some() {
        supergraph_def.mutation = subgraph_def.mutation.clone();
        // TODO mismatch on mutation types
    }
    if subgraph_def.subscription.is_some() {
        supergraph_def.subscription = subgraph_def.subscription.clone();
        // TODO mismatch on subscription types
    }
}

fn merge_descriptions<T: Eq + Clone>(
    merged: &mut Option<T>,
    new: &Option<T>,
) -> Result<(), MergeError> {
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
    subgraph_name: &str,
    enum_name: NamedType,
    enum_type: &Node<EnumType>,
) {
    let existing_type = types.entry(enum_name).or_insert(copy_enum_type(enum_type));
    if let ExtendedType::Enum(e) = existing_type {
        let join_type_directives =
            join_type_applied_directive(&subgraph_name, iter::empty(), false);
        e.make_mut().directives.extend(join_type_directives);

        merge_descriptions(&mut e.make_mut().description, &enum_type.description);

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
                    directives: Default::default(),
                }));
            merge_descriptions(&mut ev.make_mut().description, &enum_value.description);
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
    types: &mut IndexMap<NamedType, ExtendedType>,
    subgraph_name: &str,
    input_object_name: NamedType,
    input_object: &Node<InputObjectType>,
) {
    let existing_type = types
        .entry(input_object_name)
        .or_insert(copy_input_object_type(input_object));
    if let ExtendedType::InputObject(obj) = existing_type {
        let join_type_directives = join_type_applied_directive(subgraph_name, iter::empty(), false);
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
    types: &mut IndexMap<NamedType, ExtendedType>,
    subgraph_name: &str,
    interface_name: NamedType,
    interface: &Node<InterfaceType>,
) {
    let existing_type = types
        .entry(interface_name.clone())
        .or_insert(copy_interface_type(interface));
    if let ExtendedType::Interface(intf) = existing_type {
        let key_directives = interface.directives.get_all("key");
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
                        directives: Default::default(),
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
    subgraph_name: &str,
    object_name: NamedType,
    object: &Node<ObjectType>,
) {
    let is_interface_object = object.directives.has("interfaceObject");
    let existing_type = types
        .entry(object_name.clone())
        .or_insert(copy_object_type_stub(&object, is_interface_object));
    if let ExtendedType::Object(obj) = existing_type {
        let key_fields: HashSet<&str> = parse_keys(object.directives.get_all("key"));
        let is_join_field = !key_fields.is_empty() || object_name.eq("Query");
        let key_directives = object.directives.get_all("key");
        let join_type_directives =
            join_type_applied_directive(subgraph_name, key_directives, false);
        let mutable_object = obj.make_mut();
        mutable_object.directives.extend(join_type_directives);
        merge_descriptions(&mut mutable_object.description, &object.description);
        object.implements_interfaces.iter().for_each(|intf_name| {
            // IndexSet::insert deduplicates
            mutable_object
                .implements_interfaces
                .insert(intf_name.clone());
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
                    directives: Default::default(),
                    ty: field.ty.clone(),
                })),
            };
            merge_descriptions(
                &mut supergraph_field.make_mut().description,
                &field.description,
            );
            let mut existing_args = supergraph_field.arguments.iter();
            for arg in field.arguments.iter() {
                if let Some(existing_arg) = &existing_args.find(|a| a.name.eq(&arg.name)) {
                } else {
                    // TODO mismatch no args
                }
            }

            if is_join_field {
                let is_key_field = key_fields.contains(field_name.as_str());
                if !is_key_field {
                    let requires_directive_option =
                        Option::and_then(field.directives.get_all("requires").next(), |p| {
                            let requires_fields = directive_string_arg_value(p, "fields").unwrap();
                            Some(requires_fields.as_str())
                        });
                    let provides_directive_option =
                        Option::and_then(field.directives.get_all("provides").next(), |p| {
                            let provides_fields = directive_string_arg_value(p, "fields").unwrap();
                            Some(provides_fields.as_str())
                        });
                    let external_field = field.directives.get_all("external").next().is_some();
                    let join_field_directive = join_field_applied_directive(
                        subgraph_name,
                        requires_directive_option,
                        provides_directive_option,
                        external_field,
                    );

                    supergraph_field
                        .make_mut()
                        .directives
                        .push(Node::new(join_field_directive));
                }
            }
        }
    } else if let ExtendedType::Interface(intf) = existing_type {
        // TODO support interface object
        let key_directives = object.directives.get_all("key");
        let join_type_directives = join_type_applied_directive(subgraph_name, key_directives, true);
        intf.make_mut().directives.extend(join_type_directives);
    };
    // TODO merge fields
}

fn merge_union_type(
    types: &mut IndexMap<NamedType, ExtendedType>,
    subgraph_name: &str,
    union_name: NamedType,
    union: &Node<UnionType>,
) {
    let existing_type = types
        .entry(union_name.clone())
        .or_insert(copy_union_type(&union_name, union.description.clone()));
    if let ExtendedType::Union(u) = existing_type {
        let join_type_directives = join_type_applied_directive(subgraph_name, iter::empty(), false);
        u.make_mut().directives.extend(join_type_directives);

        for union_member in union.members.iter() {
            // IndexSet::insert deduplicates
            u.make_mut().members.insert(union_member.clone());
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

fn copy_enum_type(enum_type: &Node<EnumType>) -> ExtendedType {
    ExtendedType::Enum(Node::new(EnumType {
        description: enum_type.description.clone(),
        directives: Default::default(),
        values: IndexMap::new(),
    }))
}

fn copy_input_object_type(input_object: &Node<InputObjectType>) -> ExtendedType {
    let mut new_input_object = InputObjectType {
        description: input_object.description.clone(),
        directives: Default::default(),
        fields: IndexMap::new(),
    };

    for (field_name, input_field) in input_object.fields.iter() {
        new_input_object.fields.insert(
            field_name.clone(),
            Component::new(InputValueDefinition {
                name: input_field.name.clone(),
                description: input_field.description.clone(),
                directives: Default::default(),
                ty: input_field.ty.clone(),
                default_value: input_field.default_value.clone(),
            }),
        );
    }

    ExtendedType::InputObject(Node::new(new_input_object))
}

fn copy_interface_type(interface: &Node<InterfaceType>) -> ExtendedType {
    let new_interface = InterfaceType {
        description: interface.description.clone(),
        directives: Default::default(),
        fields: copy_fields(interface.fields.iter()),
        implements_interfaces: interface.implements_interfaces.clone(),
    };
    ExtendedType::Interface(Node::new(new_interface))
}

fn copy_object_type_stub(object: &Node<ObjectType>, is_interface_object: bool) -> ExtendedType {
    if is_interface_object {
        let new_interface = InterfaceType {
            description: object.description.clone(),
            directives: Default::default(),
            fields: copy_fields(object.fields.iter()),
            implements_interfaces: object.implements_interfaces.clone(),
        };
        ExtendedType::Interface(Node::new(new_interface))
    } else {
        let new_object = ObjectType {
            description: object.description.clone(),
            directives: Default::default(),
            fields: copy_fields(object.fields.iter()),
            implements_interfaces: object.implements_interfaces.clone(),
        };
        ExtendedType::Object(Node::new(new_object))
    }
}

fn copy_fields(
    fields_to_copy: Iter<Name, Component<FieldDefinition>>,
) -> IndexMap<Name, Component<FieldDefinition>> {
    let mut new_fields: IndexMap<Name, Component<FieldDefinition>> = IndexMap::new();
    for (field_name, field) in fields_to_copy {
        let args: Vec<Node<InputValueDefinition>> = field
            .arguments
            .iter()
            .map(|a| {
                Node::new(InputValueDefinition {
                    name: a.name.clone(),
                    description: a.description.clone(),
                    directives: Default::default(),
                    ty: a.ty.clone(),
                    default_value: a.default_value.clone(),
                })
            })
            .collect();
        let new_field = Component::new(FieldDefinition {
            name: field.name.clone(),
            description: field.description.clone(),
            directives: Default::default(),
            arguments: args,
            ty: field.ty.clone(),
        });

        new_fields.insert(field_name.clone(), new_field);
    }
    new_fields
}

fn copy_union_type(name: &NamedType, description: Option<NodeStr>) -> ExtendedType {
    ExtendedType::Union(Node::new(UnionType {
        description,
        directives: Default::default(),
        members: IndexSet::new(),
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
        .map(Component::new)
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
    supergraph
        .schema_definition
        .get_or_insert_with(Default::default)
        .make_mut()
        .directives
        .push(Component::new(Directive {
            name: Name::new("link"),
            arguments: vec![Node::new(Argument {
                name: Name::new("url"),
                value: Node::new(Value::String(NodeStr::new(
                    "https://specs.apollo.dev/link/v1.0",
                ))),
            })],
        }));

    let (name, link_purpose_enum) = link_purpose_enum_type();
    supergraph.types.insert(name, link_purpose_enum.into());

    // scalar Import
    let link_import_scalar = ExtendedType::Scalar(Node::new(ScalarType {
        directives: Default::default(),
        description: None,
    }));
    supergraph
        .types
        .insert("link__Import".into(), link_import_scalar);

    let link_directive_definition = link_directive_definition();
    supergraph
        .directive_definitions
        .insert(NamedType::new("link"), Node::new(link_directive_definition));
}

/// directive @link(url: String, as: String, import: [Import], for: link__Purpose) repeatable on SCHEMA
fn link_directive_definition() -> DirectiveDefinition {
    DirectiveDefinition {
        name: Name::new("link"),
        description: None,
        arguments: vec![
            Node::new(InputValueDefinition {
                name: Name::new("url"),
                description: None,
                directives: Default::default(),
                ty: Type::new_named("String").into(),
                default_value: None,
            }),
            Node::new(InputValueDefinition {
                name: Name::new("as"),
                description: None,
                directives: Default::default(),
                ty: Type::new_named("String").into(),
                default_value: None,
            }),
            Node::new(InputValueDefinition {
                name: Name::new("for"),
                description: None,
                directives: Default::default(),
                ty: Type::new_named("link__Purpose").into(),
                default_value: None,
            }),
            Node::new(InputValueDefinition {
                name: Name::new("import"),
                description: None,
                directives: Default::default(),
                ty: Type::new_named("link__Import").list().into(),
                default_value: None,
            }),
        ],
        locations: vec![DirectiveLocation::Schema],
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
fn link_purpose_enum_type() -> (Name, EnumType) {
    let mut link_purpose_enum = EnumType {
        description: None,
        directives: Default::default(),
        values: IndexMap::new(),
    };
    let link_purpose_security_value = EnumValueDefinition {
        description: Some(NodeStr::new(
            r"SECURITY features provide metadata necessary to securely resolve fields.",
        )),
        directives: Default::default(),
        value: Name::new("SECURITY"),
    };
    let link_purpose_execution_value = EnumValueDefinition {
        description: Some(NodeStr::new(
            r"EXECUTION features provide metadata necessary for operation execution.",
        )),
        directives: Default::default(),
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
    (Name::new("link__Purpose"), link_purpose_enum)
}

// TODO join spec
fn add_core_feature_join(supergraph: &mut Schema, subgraphs: &Vec<&Subgraph>) {
    // @link(url: "https://specs.apollo.dev/join/v0.3", for: EXECUTION)
    supergraph
        .schema_definition
        .get_or_insert_with(Default::default)
        .make_mut()
        .directives
        .push(Component::new(Directive {
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
        directives: Default::default(),
        description: None,
    }));
    supergraph
        .types
        .insert("join__FieldSet".into(), join_field_set_scalar);

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

    let (name, join_graph_enum_type) = join_graph_enum_type(subgraphs);
    supergraph.types.insert(name, join_graph_enum_type.into());
}

/// directive @enumValue(graph: join__Graph!) repeatable on ENUM_VALUE
fn join_enum_value_directive_definition() -> DirectiveDefinition {
    DirectiveDefinition {
        name: Name::new("join__enumValue"),
        description: None,
        arguments: vec![Node::new(InputValueDefinition {
            name: Name::new("graph"),
            description: None,
            directives: Default::default(),
            ty: Type::new_named("join__Graph").non_null().into(),
            default_value: None,
        })],
        locations: vec![DirectiveLocation::EnumValue],
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
fn join_field_directive_definition() -> DirectiveDefinition {
    DirectiveDefinition {
        name: Name::new("join__field"),
        description: None,
        arguments: vec![
            Node::new(InputValueDefinition {
                name: Name::new("graph"),
                description: None,
                directives: Default::default(),
                ty: Type::new_named("join__Graph").into(),
                default_value: None,
            }),
            Node::new(InputValueDefinition {
                name: Name::new("requires"),
                description: None,
                directives: Default::default(),
                ty: Type::new_named("join__FieldSet").into(),
                default_value: None,
            }),
            Node::new(InputValueDefinition {
                name: Name::new("provides"),
                description: None,
                directives: Default::default(),
                ty: Type::new_named("join__FieldSet").into(),
                default_value: None,
            }),
            Node::new(InputValueDefinition {
                name: Name::new("type"),
                description: None,
                directives: Default::default(),
                ty: Type::new_named("String").into(),
                default_value: None,
            }),
            Node::new(InputValueDefinition {
                name: Name::new("external"),
                description: None,
                directives: Default::default(),
                ty: Type::new_named("Boolean").into(),
                default_value: None,
            }),
            Node::new(InputValueDefinition {
                name: Name::new("override"),
                description: None,
                directives: Default::default(),
                ty: Type::new_named("String").into(),
                default_value: None,
            }),
            Node::new(InputValueDefinition {
                name: Name::new("usedOverridden"),
                description: None,
                directives: Default::default(),
                ty: Type::new_named("Boolean").into(),
                default_value: None,
            }),
        ],
        locations: vec![
            DirectiveLocation::FieldDefinition,
            DirectiveLocation::InputFieldDefinition,
        ],
        repeatable: true,
    }
}

fn join_field_applied_directive(
    subgraph_name: &str,
    requires: Option<&str>,
    provides: Option<&str>,
    external: bool,
) -> Directive {
    let mut join_field_directive = Directive {
        name: Name::new("join__field"),
        arguments: vec![Node::new(Argument {
            name: Name::new("graph"),
            value: Node::new(Value::Enum(Name::new(subgraph_name))),
        })],
    };
    if let Some(required_fields) = requires {
        join_field_directive.arguments.push(Node::new(Argument {
            name: Name::new("requires"),
            value: Node::new(Value::String(Name::new(required_fields))),
        }));
    }
    if let Some(provided_fields) = provides {
        join_field_directive.arguments.push(Node::new(Argument {
            name: Name::new("provides"),
            value: Node::new(Value::String(Name::new(provided_fields))),
        }));
    }
    if external {
        join_field_directive.arguments.push(Node::new(Argument {
            name: Name::new("external"),
            value: Node::new(Value::Boolean(external)),
        }));
    }
    join_field_directive
}

/// directive @graph(name: String!, url: String!) on ENUM_VALUE
fn join_graph_directive_definition() -> DirectiveDefinition {
    DirectiveDefinition {
        name: Name::new("join__graph"),
        description: None,
        arguments: vec![
            Node::new(InputValueDefinition {
                name: Name::new("name"),
                description: None,
                directives: Default::default(),
                ty: Type::new_named("String").non_null().into(),
                default_value: None,
            }),
            Node::new(InputValueDefinition {
                name: Name::new("url"),
                description: None,
                directives: Default::default(),
                ty: Type::new_named("String").non_null().into(),
                default_value: None,
            }),
        ],
        locations: vec![DirectiveLocation::EnumValue],
        repeatable: false,
    }
}

/// directive @implements(
///   graph: Graph!,
///   interface: String!
/// ) on OBJECT | INTERFACE
fn join_implements_directive_definition() -> DirectiveDefinition {
    DirectiveDefinition {
        name: Name::new("join__implements"),
        description: None,
        arguments: vec![
            Node::new(InputValueDefinition {
                name: Name::new("graph"),
                description: None,
                directives: Default::default(),
                ty: Type::new_named("join__Graph").non_null().into(),
                default_value: None,
            }),
            Node::new(InputValueDefinition {
                name: Name::new("interface"),
                description: None,
                directives: Default::default(),
                ty: Type::new_named("String").non_null().into(),
                default_value: None,
            }),
        ],
        locations: vec![DirectiveLocation::Interface, DirectiveLocation::Object],
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
fn join_type_directive_definition() -> DirectiveDefinition {
    DirectiveDefinition {
        name: Name::new("join__type"),
        description: None,
        arguments: vec![
            Node::new(InputValueDefinition {
                name: Name::new("graph"),
                description: None,
                directives: Default::default(),
                ty: Type::new_named("join__Graph").non_null().into(),
                default_value: None,
            }),
            Node::new(InputValueDefinition {
                name: Name::new("key"),
                description: None,
                directives: Default::default(),
                ty: Type::new_named("join__FieldSet").into(),
                default_value: None,
            }),
            Node::new(InputValueDefinition {
                name: Name::new("extension"),
                description: None,
                directives: Default::default(),
                ty: Type::new_named("Boolean").non_null().into(),
                default_value: Some(Node::new(Value::Boolean(false))),
            }),
            Node::new(InputValueDefinition {
                name: Name::new("resolvable"),
                description: None,
                directives: Default::default(),
                ty: Type::new_named("Boolean").non_null().into(),
                default_value: Some(Node::new(Value::Boolean(true))),
            }),
            Node::new(InputValueDefinition {
                name: Name::new("isInterfaceObject"),
                description: None,
                directives: Default::default(),
                ty: Type::new_named("Boolean").non_null().into(),
                default_value: Some(Node::new(Value::Boolean(false))),
            }),
        ],
        locations: vec![
            DirectiveLocation::Enum,
            DirectiveLocation::InputObject,
            DirectiveLocation::Interface,
            DirectiveLocation::Object,
            DirectiveLocation::Scalar,
            DirectiveLocation::Union,
        ],
        repeatable: true,
    }
}

/// directive @unionMember(graph: join__Graph!, member: String!) repeatable on UNION
fn join_union_member_directive_definition() -> DirectiveDefinition {
    DirectiveDefinition {
        name: Name::new("join__unionMember"),
        description: None,
        arguments: vec![
            Node::new(InputValueDefinition {
                name: Name::new("graph"),
                description: None,
                directives: Default::default(),
                ty: Type::new_named("join__Graph").non_null().into(),
                default_value: None,
            }),
            Node::new(InputValueDefinition {
                name: Name::new("member"),
                description: None,
                directives: Default::default(),
                ty: Type::new_named("String").non_null().into(),
                default_value: None,
            }),
        ],
        locations: vec![DirectiveLocation::Union],
        repeatable: true,
    }
}

/// enum Graph
fn join_graph_enum_type(subgraphs: &Vec<&Subgraph>) -> (Name, EnumType) {
    let mut join_graph_enum_type = EnumType {
        description: None,
        directives: Default::default(),
        values: IndexMap::new(),
    };
    for s in subgraphs {
        let join_graph_applied_directive = Directive {
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
        let graph = EnumValueDefinition {
            description: None,
            directives: Directives(vec![Node::new(join_graph_applied_directive)]),
            value: Name::new(s.name.to_uppercase().as_str()),
        };
        join_graph_enum_type
            .values
            .insert(graph.value.clone(), Component::new(graph));
    }
    (Name::new("join__Graph"), join_graph_enum_type)
}

fn parse_keys<'a>(
    directives: impl Iterator<Item = &'a Component<Directive>> + Sized,
) -> HashSet<&'a str> {
    HashSet::from_iter(
        directives
            .flat_map(|k| {
                let field_set = directive_string_arg_value(k, "fields").unwrap();
                field_set.split_whitespace()
            })
            .collect::<Vec<&str>>(),
    )
}

fn merge_directive(
    supergraph_directives: &mut IndexMap<Name, Node<DirectiveDefinition>>,
    directive: &Node<DirectiveDefinition>,
) {
    if !supergraph_directives.contains_key(&directive.name.clone()) {
        supergraph_directives.insert(directive.name.clone(), directive.clone());
    }
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
        let _subgraphs = database::extract_subgraphs(&supergraph)
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

"The foo directive description"
directive @foo(url: String) on FIELD

directive @join__enumValue(graph: join__Graph!) repeatable on ENUM_VALUE

directive @join__field(graph: join__Graph, requires: join__FieldSet, provides: join__FieldSet, type: String, external: Boolean, override: String, usedOverridden: Boolean) repeatable on FIELD_DEFINITION | INPUT_FIELD_DEFINITION

directive @join__graph(name: String!, url: String!) on ENUM_VALUE

directive @join__implements(graph: join__Graph!, interface: String!) repeatable on INTERFACE | OBJECT

directive @join__type(graph: join__Graph!, key: join__FieldSet, extension: Boolean! = false, resolvable: Boolean! = true, isInterfaceObject: Boolean! = false) repeatable on ENUM | INPUT_OBJECT | INTERFACE | OBJECT | SCALAR | UNION

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

"Available queries\nNot much yet"
type Query @join__type(graph: SUBGRAPH1) @join__type(graph: SUBGRAPH2) {
  "Returns tea"
  t(
    "An argument that is very important"
    x: String!,
  ): String @join__field(graph: SUBGRAPH1)
}

scalar join__FieldSet

scalar link__Import
"#;
        let supergraph = Supergraph::compose(vec![&s1, &s2]).unwrap();
        // TODO currently printer does not respect multi line comments
        // TODO printer also adds extra comma after arguments
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
                extend schema @link(url: "https://specs.apollo.dev/federation/v2.5", import: [ "@key", "@provides", "@external" ])
                
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
                extend schema @link(url: "https://specs.apollo.dev/federation/v2.5", import: [ "@key", "@shareable" ])
            
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
  SUBGRAPHA @join__graph(name: "SubgraphA", url: "https://subgraphA")
  SUBGRAPHB @join__graph(name: "SubgraphB", url: "https://subgraphB")
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
