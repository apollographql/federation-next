use crate::ast::AstNodeEnum;
use crate::core_spec::{CoreImport, FeatureUrl};
use crate::error::{
    AggregateGraphQLError, ErrorEnum, FederationError, GraphQLError, GraphQLErrors,
};
use crate::federation::{
    ChildFederationBlueprint, FederationBlueprint, UnattachedFederationBlueprint,
};
use crate::operations::{Field, FragmentElement, NamedFragmentDefinition};
use crate::private::{SealedMethod, SealedTrait};
use crate::utils::{CachedLinkedHashMap, CachedLinkedHashSet, InsertOnlyIndexMap, WithBorrow};
use crate::values::ValueEnum;
use enum_dispatch::enum_dispatch;
use std::cell::{Ref, RefCell, RefMut};
use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::rc::{Rc, Weak};

const VALIDATION_ERROR_CODE: &'static str = "GraphQLValidationFailed";
const DEFAULT_VALIDATION_ERROR_MESSAGE: &'static str = "The schema is not a valid GraphQL schema.";

// PORT_NOTE: The JS code allowed a message argument to override the default, but it was never used.
fn err_graphql_validation_failed(causes: Vec<GraphQLError>) -> AggregateGraphQLError {
    AggregateGraphQLError::new(
        VALIDATION_ERROR_CODE.to_owned(),
        DEFAULT_VALIDATION_ERROR_MESSAGE.to_owned(),
        causes,
        None,
    )
}

// enum_dispatch on an enum will generate automatic From implementations for each member, but only
// if the attribute gives at least one trait. I.e. an enum with "#[enum_dispatch]" won't generate
// From implementations, but an enum with "#[enum_dispatch(SomeArbitraryTrait)]" will. So we create
// this dummy trait here that serves purely as a placeholder for when actual traits get added to
// an enum_dispatch.
//
// We keep a separate dummy trait per file. This is because if we enum_dispatch to a trait that's in
// a different file, you'll need to add a "use" statement for that enum to this file. And not all
// enums have the visibility modifiers to make that "use" statement succeed. Another quirk of
// enum_dispatch is that even if you have a separate trait per file with pub(self)/module-private
// visibility, you'll get error messages from enum_dispatch all the same, so we add a prefix as well
// based on this file's name.
#[enum_dispatch]
trait DefinitionsEmptyEnumDispatchPlaceholder {}

// In order for enum_dispatch to work, both the enums and traits must be in the same crate (from
// what I understand, this is a limit of macros). We would like to dispatch the Display trait on
// enums, so to get around the above limitation, we create our own version of the Display trait
// here. We still need to write short boilerplate impl for Display for enums, and FederationDisplay
// for enum members with Display, but it's better than multiple many-armed match statements.
#[enum_dispatch]
pub trait FederationDisplay {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result;
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    strum_macros::Display,
    strum_macros::EnumIter,
    strum_macros::IntoStaticStr,
)]
pub enum DirectiveLocationEnum {
    /* Operation Definitions */
    #[strum(to_string = "QUERY")]
    Query,
    #[strum(to_string = "MUTATION")]
    Mutation,
    #[strum(to_string = "SUBSCRIPTION")]
    Subscription,
    #[strum(to_string = "FIELD")]
    Field,
    #[strum(to_string = "FRAGMENT_DEFINITION")]
    FragmentDefinition,
    #[strum(to_string = "FRAGMENT_SPREAD")]
    FragmentSpread,
    #[strum(to_string = "INLINE_FRAGMENT")]
    InlineFragment,
    #[strum(to_string = "VARIABLE_DEFINITION")]
    VariableDefinition,
    /* Type System Definitions */
    #[strum(to_string = "SCHEMA")]
    Schema,
    #[strum(to_string = "SCALAR")]
    Scalar,
    #[strum(to_string = "OBJECT")]
    Object,
    #[strum(to_string = "FIELD_DEFINITION")]
    FieldDefinition,
    #[strum(to_string = "ARGUMENT_DEFINITION")]
    ArgumentDefinition,
    #[strum(to_string = "INTERFACE")]
    Interface,
    #[strum(to_string = "UNION")]
    Union,
    #[strum(to_string = "ENUM")]
    Enum,
    #[strum(to_string = "ENUM_VALUE")]
    EnumValue,
    #[strum(to_string = "INPUT_OBJECT")]
    InputObject,
    #[strum(to_string = "INPUT_FIELD_DEFINITION")]
    InputFieldDefinition,
}

#[derive(Debug, strum_macros::Display, strum_macros::EnumIter, strum_macros::IntoStaticStr)]
pub(crate) enum SchemaRootKindEnum {
    #[strum(to_string = "query")]
    Query,
    #[strum(to_string = "mutation")]
    Mutation,
    #[strum(to_string = "subscription")]
    Subscription,
}

#[derive(
    Debug, PartialEq, Eq, Hash, strum_macros::Display, strum_macros::EnumIter, strum_macros::IntoStaticStr,
)]
pub enum NamedTypeKindEnum {
    SchemaDefinition,
    ScalarType,
    ObjectType,
    InterfaceType,
    UnionType,
    EnumType,
    InputObjectType,
    ListType,
    NonNullType,
    FieldDefinition,
    InputFieldDefinition,
    ArgumentDefinition,
    EnumValue,
    DirectiveDefinition,
}

// export type Type = NamedType | WrapperType;
// export type NamedType = ScalarType | ObjectType | InterfaceType | UnionType | EnumType | InputObjectType;
// export type AbstractType = InterfaceType | UnionType;

#[enum_dispatch(
    HasKind,
    HasParentSchema,
    SetParentWeakSchema,
    IsAttached,
    HasSourceAst,
    HasName,
    IsBuiltIn,
    FederationDisplay
)]
#[derive(Debug, Clone)]
pub enum NamedTypeEnum {
    ScalarType(ScalarType),
    ObjectType(ObjectType),
    InterfaceType(InterfaceType),
    UnionType(UnionType),
    EnumType(EnumType),
    InputObjectType(InputObjectType),
}

impl Display for NamedTypeEnum {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        FederationDisplay::fmt(self, f)
    }
}

impl SealedTrait for NamedTypeEnum {}

#[enum_dispatch(DefinitionsEmptyEnumDispatchPlaceholder)]
#[derive(Debug)]
pub enum UnattachedNamedTypeEnum {
    UnattachedScalarType(UnattachedScalarType),
    UnattachedObjectType(UnattachedObjectType),
    UnattachedInterfaceType(UnattachedInterfaceType),
    UnattachedUnionType(UnattachedUnionType),
    UnattachedEnumType(UnattachedEnumType),
    UnattachedInputObjectType(UnattachedInputObjectType),
}

impl UnattachedNamedTypeEnum {
    fn to_child(self) -> ChildNamedTypeEnum {
        match self {
            UnattachedNamedTypeEnum::UnattachedScalarType(unattached) => {
                unattached.to_child().into()
            }
            UnattachedNamedTypeEnum::UnattachedObjectType(unattached) => {
                unattached.to_child().into()
            }
            UnattachedNamedTypeEnum::UnattachedInterfaceType(unattached) => {
                unattached.to_child().into()
            }
            UnattachedNamedTypeEnum::UnattachedUnionType(unattached) => {
                unattached.to_child().into()
            }
            UnattachedNamedTypeEnum::UnattachedEnumType(unattached) => unattached.to_child().into(),
            UnattachedNamedTypeEnum::UnattachedInputObjectType(unattached) => {
                unattached.to_child().into()
            }
        }
    }
}

#[enum_dispatch(DefinitionsEmptyEnumDispatchPlaceholder)]
#[derive(Debug, Clone)]
enum ChildNamedTypeEnum {
    ChildScalarType(ChildScalarType),
    ChildObjectType(ChildObjectType),
    ChildInterfaceType(ChildInterfaceType),
    ChildUnionType(ChildUnionType),
    ChildEnumType(ChildEnumType),
    ChildInputObjectType(ChildInputObjectType),
}

impl ChildNamedTypeEnum {
    fn downgrade(&self) -> NamedTypeEnum {
        match self {
            ChildNamedTypeEnum::ChildScalarType(child) => child.downgrade().into(),
            ChildNamedTypeEnum::ChildObjectType(child) => child.downgrade().into(),
            ChildNamedTypeEnum::ChildInterfaceType(child) => child.downgrade().into(),
            ChildNamedTypeEnum::ChildUnionType(child) => child.downgrade().into(),
            ChildNamedTypeEnum::ChildEnumType(child) => child.downgrade().into(),
            ChildNamedTypeEnum::ChildInputObjectType(child) => child.downgrade().into(),
        }
    }
}

#[enum_dispatch(HasWeakSchema)]
#[derive(Debug, Clone)]
pub enum CompositeTypeEnum {
    ObjectType(ObjectType),
    InterfaceType(InterfaceType),
    UnionType(UnionType),
}

impl SealedTrait for CompositeTypeEnum {}

// Note that while unions have the field "__typename", we don't consider it a field-based type here.
#[enum_dispatch(HasWeakSchema)]
#[derive(Debug, Clone)]
pub enum FieldBasedTypeEnum {
    ObjectType(ObjectType),
    InterfaceType(InterfaceType),
}

impl SealedTrait for FieldBasedTypeEnum {}

#[enum_dispatch(HasWeakSchema)]
#[derive(Debug, Clone)]
pub enum ArgumentParentElementEnum {
    FieldDefinition(FieldDefinition),
    DirectiveDefinition(DirectiveDefinition),
}

impl SealedTrait for ArgumentParentElementEnum {}

#[enum_dispatch(HasWeakSchema)]
#[derive(Debug, Clone)]
pub enum DirectiveParentElementEnum {
    SchemaDefinition(SchemaDefinition),
    ScalarType(ScalarType),
    ObjectType(ObjectType),
    InterfaceType(InterfaceType),
    UnionType(UnionType),
    EnumType(EnumType),
    InputObjectType(InputObjectType),
    FieldDefinition(FieldDefinition),
    InputFieldDefinition(InputFieldDefinition),
    ArgumentDefinition(ArgumentDefinition),
    EnumValue(EnumValue),
    VariableDefinition(VariableDefinition),
    Field(Field),
    FragmentElement(FragmentElement),
    NamedFragmentDefinition(NamedFragmentDefinition),
}

impl SealedTrait for DirectiveParentElementEnum {}

#[enum_dispatch(SchemaElementHasParent, IsBuiltIn)]
#[derive(Debug, Clone)]
enum SchemaElementEnum {
    SchemaDefinition(SchemaDefinition),
    ScalarType(ScalarType),
    ObjectType(ObjectType),
    InterfaceType(InterfaceType),
    UnionType(UnionType),
    EnumType(EnumType),
    InputObjectType(InputObjectType),
    FieldDefinition(FieldDefinition),
    InputFieldDefinition(InputFieldDefinition),
    ArgumentDefinition(ArgumentDefinition),
    EnumValue(EnumValue),
    DirectiveDefinition(DirectiveDefinition),
}

impl SchemaElementEnum {
    fn to_directive_parent_element(self) -> Option<DirectiveParentElementEnum> {
        match self {
            SchemaElementEnum::SchemaDefinition(element) => Some(element.into()),
            SchemaElementEnum::ScalarType(element) => Some(element.into()),
            SchemaElementEnum::ObjectType(element) => Some(element.into()),
            SchemaElementEnum::InterfaceType(element) => Some(element.into()),
            SchemaElementEnum::UnionType(element) => Some(element.into()),
            SchemaElementEnum::EnumType(element) => Some(element.into()),
            SchemaElementEnum::InputObjectType(element) => Some(element.into()),
            SchemaElementEnum::FieldDefinition(element) => Some(element.into()),
            SchemaElementEnum::InputFieldDefinition(element) => Some(element.into()),
            SchemaElementEnum::ArgumentDefinition(element) => Some(element.into()),
            SchemaElementEnum::EnumValue(element) => Some(element.into()),
            SchemaElementEnum::DirectiveDefinition(_) => None,
        }
    }
}

impl SealedTrait for SchemaElementEnum {}

#[enum_dispatch(DefinitionsEmptyEnumDispatchPlaceholder)]
#[derive(Debug, Clone)]
enum SchemaElementParentEnum {
    Schema(Schema),
    ObjectType(ObjectType),
    InterfaceType(InterfaceType),
    UnionType(UnionType),
    EnumType(EnumType),
    InputObjectType(InputObjectType),
    FieldDefinition(FieldDefinition),
    DirectiveDefinition(DirectiveDefinition),
}

impl SchemaElementParentEnum {
    fn to_schema_element(self) -> Option<SchemaElementEnum> {
        match self {
            SchemaElementParentEnum::Schema(_) => None,
            SchemaElementParentEnum::ObjectType(parent) => Some(parent.into()),
            SchemaElementParentEnum::InterfaceType(parent) => Some(parent.into()),
            SchemaElementParentEnum::UnionType(parent) => Some(parent.into()),
            SchemaElementParentEnum::EnumType(parent) => Some(parent.into()),
            SchemaElementParentEnum::InputObjectType(parent) => Some(parent.into()),
            SchemaElementParentEnum::FieldDefinition(parent) => Some(parent.into()),
            SchemaElementParentEnum::DirectiveDefinition(parent) => Some(parent.into()),
        }
    }
}

impl From<CompositeTypeEnum> for SchemaElementParentEnum {
    fn from(value: CompositeTypeEnum) -> SchemaElementParentEnum {
        match value {
            CompositeTypeEnum::ObjectType(parent) => parent.into(),
            CompositeTypeEnum::InterfaceType(parent) => parent.into(),
            CompositeTypeEnum::UnionType(parent) => parent.into(),
        }
    }
}

impl From<ArgumentParentElementEnum> for SchemaElementParentEnum {
    fn from(value: ArgumentParentElementEnum) -> SchemaElementParentEnum {
        match value {
            ArgumentParentElementEnum::FieldDefinition(parent) => parent.into(),
            ArgumentParentElementEnum::DirectiveDefinition(parent) => parent.into(),
        }
    }
}

#[enum_dispatch(IsAttached, HasWeakSchema, FederationDisplay)]
#[derive(Debug, Clone)]
enum SchemaElementCheckUpdateEnum {
    RootType(RootType),
    InterfaceImplementation(InterfaceImplementation),
    UnionMember(UnionMember),
    FieldDefinition(FieldDefinition),
    InputFieldDefinition(InputFieldDefinition),
    ArgumentDefinition(ArgumentDefinition),
    EnumValue(EnumValue),
    // While a directive definition can't be a child, a directive can be, and we often pass a
    // directive definition in when applying a directive.
    DirectiveDefinition(DirectiveDefinition),
}

impl Display for SchemaElementCheckUpdateEnum {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        FederationDisplay::fmt(self, f)
    }
}

impl SealedTrait for SchemaElementCheckUpdateEnum {}

#[enum_dispatch(IsAttached, HasWeakSchema)]
#[derive(Debug, Clone)]
pub enum NamedOutputTypeEnum {
    ScalarType(ScalarType),
    ObjectType(ObjectType),
    InterfaceType(InterfaceType),
    UnionType(UnionType),
    EnumType(EnumType),
}

impl SealedTrait for NamedOutputTypeEnum {}

#[enum_dispatch(IsAttached, HasWeakSchema)]
#[derive(Debug, Clone)]
pub enum NamedInputTypeEnum {
    ScalarType(ScalarType),
    EnumType(EnumType),
    InputObjectType(InputObjectType),
}

impl SealedTrait for NamedInputTypeEnum {}

#[enum_dispatch(DefinitionsEmptyEnumDispatchPlaceholder)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum ScalarTypeReferencer {
    FieldDefinition(FieldDefinition),
    InputFieldDefinition(InputFieldDefinition),
    ArgumentDefinition(ArgumentDefinition),
}

#[enum_dispatch(DefinitionsEmptyEnumDispatchPlaceholder)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum ObjectTypeReferencer {
    SchemaDefinition(SchemaDefinition),
    UnionType(UnionType),
    FieldDefinition(FieldDefinition),
}

#[enum_dispatch(DefinitionsEmptyEnumDispatchPlaceholder)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum InterfaceTypeReferencer {
    ObjectType(ObjectType),
    InterfaceType(InterfaceType),
    FieldDefinition(FieldDefinition),
}

#[enum_dispatch(DefinitionsEmptyEnumDispatchPlaceholder)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum UnionTypeReferencer {
    FieldDefinition(FieldDefinition),
}

#[enum_dispatch(DefinitionsEmptyEnumDispatchPlaceholder)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum EnumTypeReferencer {
    FieldDefinition(FieldDefinition),
    InputFieldDefinition(InputFieldDefinition),
    ArgumentDefinition(ArgumentDefinition),
}

#[enum_dispatch(DefinitionsEmptyEnumDispatchPlaceholder)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum InputObjectTypeReferencer {
    InputFieldDefinition(InputFieldDefinition),
    ArgumentDefinition(ArgumentDefinition),
}

#[enum_dispatch(DefinitionsEmptyEnumDispatchPlaceholder)]
#[derive(Debug, Clone)]
pub enum ExtendableElementEnum {
    SchemaDefinition(SchemaDefinition),
    ScalarType(ScalarType),
    ObjectType(ObjectType),
    InterfaceType(InterfaceType),
    UnionType(UnionType),
    EnumType(EnumType),
    InputObjectType(InputObjectType),
}

#[enum_dispatch(HasName)]
#[derive(Debug, Clone)]
pub enum FeatureElementEnum {
    ScalarType(ScalarType),
    ObjectType(ObjectType),
    InterfaceType(InterfaceType),
    UnionType(UnionType),
    EnumType(EnumType),
    InputObjectType(InputObjectType),
    DirectiveDefinition(DirectiveDefinition),
}

impl SealedTrait for FeatureElementEnum {}

// PORT_NOTE: In JS code, you can create arbitrary unions of types because it's typescript, and you
// can call fields/methods as long as they exist on each type. The equivalent of unions in Rust is
// enums. However, they don't automatically dispatch to union member fields. We can use the
// enum_dispatch macro to help, but it works via traits on the members, and those traits can't have
// associated types or consts in them. The traits are tedious to implement for each possible member
// they can apply to, but the benefit is you don't have to reimplement them across multiple enums.
//
// The traits we use for the purpose of enum_dispatch are listed below. They're generally small to
// (1) allow traits to apply to different sets of elements, and to (2) allow different visibility
// modifiers (methods in a trait all have the same visibility of the trait). These traits are
// notably different from the other traits below those, which are more like abstract classes and
// exist to share code with their implementers (accordingly, they're mostly at module-private
// visibility).
//
// In the process of porting, some JS abstract classes had a few methods be removed to exist within
// enum_dispatch traits. These were generally cases where their implementations were directly on
// data in structs (simple getters/setters). This is necessary because traits cannot have fields
// like abstract classes can, so there's no benefit from trying to share getter/setter logic, and
// they effectively must be pushed to the implementation. The other common case was purely abstract
// methods with no default implementation. Sometimes this left JS classes empty, like in the case
// with "NamedSchemaElement", where all its methods were shifted to enum_dispatch traits so it
// effectively disappeared.
//
// For other methods with substantial logic, you might think they don't need enum_dispatch traits,
// but the common problem there is that the methods in these abstract-class-like traits have
// different visibility. To get the right visibility, it's usually easier to keep those traits at
// module-private visibility, and delegate to them from properly visible, smaller traits. This also
// allows a unified trait for a method when that method gets implemented for different structs via
// different means (e.g. directly on each struct, or through different traits).
#[enum_dispatch]
pub trait HasKind: SealedTrait {
    fn kind(&self) -> NamedTypeKindEnum;
}

#[enum_dispatch]
pub trait HasParent: SealedTrait {
    type TParent;

    fn parent(&self) -> Self::TParent;
}

#[enum_dispatch]
trait SetParent: SealedTrait {
    type TParent;

    fn set_parent(&self, parent: Self::TParent);
}

#[enum_dispatch]
pub trait HasParentSchema: SealedTrait {
    fn parent_schema(&self) -> Schema;
}

#[enum_dispatch]
trait SetParentWeakSchema: SealedTrait {
    fn set_parent_weak_schema(&self, parent: WeakSchema);
}

#[enum_dispatch]
pub trait IsAttached: SealedTrait {
    fn is_attached(&self) -> bool;
}

#[enum_dispatch]
pub trait HasSchema: SealedTrait {
    fn schema(&self) -> Schema;
}

#[enum_dispatch]
trait HasWeakSchema: SealedTrait {
    fn weak_schema(&self) -> WeakSchema;
    fn weak_schema_if_attached(&self) -> Option<WeakSchema>;
}

#[enum_dispatch]
pub trait HasSourceAst: SealedTrait {
    fn source_ast(&self) -> Option<AstNodeEnum>;
}

#[enum_dispatch]
pub(crate) trait SetSourceAst: SealedTrait {
    fn set_source_ast(&self, source_ast: Option<AstNodeEnum>);
}

#[enum_dispatch]
pub trait HasDescription: SealedTrait {
    fn description(&self) -> Option<Rc<str>>;
    fn set_description(&self, description: Option<Rc<str>>);
}

#[enum_dispatch]
trait SchemaElementHasParent: SealedTrait {
    fn schema_element_parent(&self) -> SchemaElementParentEnum;
}

#[enum_dispatch]
pub(crate) trait IsBuiltIn: SealedTrait {
    fn is_built_in(&self) -> bool;
}

#[enum_dispatch]
pub(crate) trait AddUnappliedDirectives: SealedTrait {
    fn add_unapplied_directive(&self, directive: UnappliedDirective);
    fn process_unapplied_directives(&self) -> Result<(), FederationError>;
}

#[enum_dispatch]
pub trait HasAppliedDirectives: SealedTrait {
    fn applied_directives(&self) -> Vec<Directive>;
    fn applied_directives_of(&self, definition: &DirectiveDefinition) -> Vec<Directive>;
    fn applied_directives_of_name(&self, name: &str) -> Vec<Directive>;
    fn has_applied_directive(&self, definition: &DirectiveDefinition) -> bool;
    fn has_applied_directive_name(&self, name: &str) -> bool;
}

#[enum_dispatch]
pub trait AddAppliedDirectives: SealedTrait {
    fn apply_directive(
        &self,
        definition: DirectiveDefinition,
        args: Option<Rc<InsertOnlyIndexMap<ValueEnum>>>,
        as_first_directive: Option<bool>,
    ) -> Directive;
    fn apply_directive_name(
        &self,
        name: Rc<str>,
        args: Option<Rc<InsertOnlyIndexMap<ValueEnum>>>,
        as_first_directive: Option<bool>,
    ) -> Result<Directive, FederationError>;
}

#[enum_dispatch]
pub(crate) trait PreserveEmptyDefinition {
    fn preserve_empty_definition(&self) -> bool;
    fn set_preserve_empty_definition(&self, preserve_empty_definition: bool);
}

#[enum_dispatch]
pub trait HasExtensions: SealedTrait {
    fn extensions(&self) -> Vec<Extension>;
    fn has_extension(&self, extension: &Extension) -> bool;
    fn has_extension_elements(&self) -> bool;
    fn has_non_extension_elements(&self) -> bool;
}

#[enum_dispatch]
pub(crate) trait AddExtensions: SealedTrait {
    fn new_extension(&self) -> Extension;
    fn add_extension(&self, extension: UnattachedExtension) -> Extension;
}

// We want to be able to rename some elements, but we prefer offering that through a `rename` method
// rather than exposing a name setter, as this feels more explicit (but that's arguably debatable).
// We also currently only offer renames on types (because that's the only one we currently need),
// though we could expand that.
#[enum_dispatch]
pub trait HasName: SealedTrait {
    fn name(&self) -> Rc<str>;
}

#[enum_dispatch]
pub trait HasCoordinate: SealedTrait {
    fn coordinate(&self) -> String;
}

#[enum_dispatch]
pub(crate) trait HasReferencers: SealedTrait {
    type TReferencer;

    fn referencers(&self) -> Rc<[Self::TReferencer]>;
    fn is_referenced(&self) -> bool;
}

#[enum_dispatch]
pub(crate) trait Rename: SealedTrait {
    fn rename(&self, new_name: Rc<str>);
}

#[enum_dispatch]
pub trait Remove: SealedTrait {
    type TReferencer;

    fn remove(&self) -> Vec<Self::TReferencer>;
}

// PORT_NOTE: Known as "DirectiveTargetElement" in the JS code. In the Rust code, this was renamed
// to "DirectiveParentOperationElement", since it was only ever extended by operation elements (the
// directive logic for schema elements lived in "SchemaElement" in the JS code, and is slightly
// different). We purposely omit the type parameter "T" from this trait, as Rust lets us use Self
// instead. The "schema()" method has been moved to the "HasSchema" trait.
pub(crate) trait DirectiveParentOperationElement: SealedTrait {
    // TODO: Switched to CachedLinkedHashSet, and used cached_values() where appropriate
    fn _protected_applied_directives_borrow<T, F: FnOnce(Ref<Vec<ChildDirective>>) -> T>(
        &self,
        f: F,
        _: SealedMethod,
    ) -> T;
    fn _protected_applied_directives_borrow_mut<T, F: FnOnce(RefMut<Vec<ChildDirective>>) -> T>(
        &self,
        f: F,
        _: SealedMethod,
    ) -> T;

    fn _protected_new<T, F: FnOnce(Vec<ChildDirective>) -> T>(
        self_weak: DirectiveParentElementEnum,
        directives: Vec<UnattachedDirective>,
        f: F,
        _: SealedMethod,
    ) -> T {
        let child_directives: Vec<ChildDirective> = directives
            .into_iter()
            .map(|d| {
                let child_directive = d.to_child();
                child_directive.downgrade().set_parent(self_weak.clone());
                child_directive
            })
            .collect();
        f(child_directives)
    }

    fn applied_directives(&self) -> Vec<Directive> {
        self._protected_applied_directives_borrow(
            |applied_directives_ref| {
                applied_directives_ref
                    .iter()
                    .map(|d| d.downgrade())
                    .collect()
            },
            SealedMethod,
        )
    }

    // PORT_NOTE: In the JS code, this took either a name or definition. We've split them into two
    // separate methods here instead of creating an enum solely for that.
    fn applied_directives_of(&self, definition: &DirectiveDefinition) -> Vec<Directive> {
        self.applied_directives_of_name(&definition.name())
    }

    fn applied_directives_of_name(&self, name: &str) -> Vec<Directive> {
        self._protected_applied_directives_borrow(
            |applied_directives_ref| {
                applied_directives_ref
                    .iter()
                    .filter_map(|d| {
                        let d = d.downgrade();
                        if d.name().deref() == name {
                            Some(d)
                        } else {
                            None
                        }
                    })
                    .collect()
            },
            SealedMethod,
        )
    }

    // PORT_NOTE: In the JS code, this took either a name or definition. We've split them into two
    // separate methods here instead of creating an enum solely for that.
    fn has_applied_directives(&self, definition: &DirectiveDefinition) -> bool {
        self.has_applied_directives_name(&definition.name())
    }

    fn has_applied_directives_name(&self, name: &str) -> bool {
        self._protected_applied_directives_borrow(
            |applied_directives_ref| {
                applied_directives_ref
                    .iter()
                    .any(|d| d.downgrade().name().deref() == name)
            },
            SealedMethod,
        )
    }
}

// This trait exists mostly to avoid code duplication between SchemaElement and Directive (which is
// not considered a SchemaElement since it can't have applied directives or a description).
//
// PORT_NOTE: In the JS code, this abstract class contained code for getting/setting the source AST.
// This was a thin wrapper around the field in the abstract class, but since fields can't live in
// traits, we instead make a trait called "HasSourceAst" and push the code into the implementations.
trait Element: Clone + Display + SealedTrait {
    type TParent: Clone + HasWeakSchema;

    fn _protected_parent_borrow<T, F: FnOnce(Ref<Option<Self::TParent>>) -> T>(&self, f: F) -> T;
    fn _protected_parent_borrow_mut<T, F: FnOnce(RefMut<Option<Self::TParent>>) -> T>(
        &self,
        f: F,
    ) -> T;

    fn _protected_weak_schema(&self) -> WeakSchema {
        self._protected_weak_schema_if_attached()
            .expect("requested schema does not exist. Probably because the element is unattached")
    }

    // PORT_NOTE: Known as "schema()" in the JS code, but we'd rather use this method name in the
    // "HasSchema" trait, as that trait gets implemented by operation elements as well. We instead
    // rename this to "_protected_schema()", and call this within "HasSchema".
    fn _protected_schema(&self) -> Schema {
        self._protected_weak_schema().upgrade()
    }

    // This function exists because sometimes we can have an element that will be attached soon even
    // though the current state is unattached (mainly for callbacks). Sometimes these intermediate
    // states need to get the schema if it exists, but it may not. All external clients should use
    // schema().
    //
    // PORT_NOTE: Known as "schema_internal()" in the JS code, to distinguish it from "schema()".
    // Since we prefix Rust method names with private/protected (Rust doesn't have these concepts),
    // it no longer conflicts with "schema()" and the "internal" becomes redundant, so we remove it.
    // We add "if_attached" to distinguish that it returns an option, and "weak" to indicate it's
    // a weak reference to a schema.
    fn _protected_weak_schema_if_attached(&self) -> Option<WeakSchema> {
        let parent_ref = self._protected_parent_borrow(|parent_ref| parent_ref.clone());
        parent_ref.and_then(|parent_ref| parent_ref.weak_schema_if_attached())
    }

    fn _protected_parent(&self) -> Self::TParent {
        self._protected_parent_borrow(|parent_ref| {
            parent_ref
                .as_ref()
                .expect("trying to access non-existent parent")
                .clone()
        })
    }

    fn _protected_is_attached(&self) -> bool {
        self._protected_parent_borrow(|parent_ref| parent_ref.is_some())
    }

    // This method does not try to upgrade the parent's weak pointer, so it's safe to call in
    // Rc::new_cyclic() within constructors.
    //
    // PORT_NOTE: This method was marked private in "setParent()" in the JS code, but the codebase
    // was calling it outside the abstract class through "Element.prototype['setParent']". The real
    // reason to mark it "private" was to keep it out of the public API, and accordingly it's
    // marked module-private in its enum_dispatch trait.
    fn _protected_set_parent(&self, parent: Self::TParent) {
        self._protected_parent_borrow_mut(|mut parent_refmut| {
            if parent_refmut.is_none() {
                *parent_refmut = Some(parent)
            } else {
                panic!("Cannot set parent of an already attached element")
            }
        });
        self._protected_on_attached();
    }

    fn _protected_on_attached(&self) {
        // Nothing by default, but can be overridden.
    }

    // PORT_NOTE: Known as "checkUpdate()" in the JS code, but this method gets overridden, and it
    // turns out Rust doesn't really allow traits to override supertraits (only impls can override
    // traits, in the sense the default implementation in the trait doesn't get used). So we name
    // it differently to avoid having callers manually qualify the method call.
    fn _protected_check_update_attached(&self) {
        // Allowing the addition of an element to a detached element would get hairy. Because that
        // would mean that when you do attach an element, you have to recurse within that element to
        // all child elements to check whether they are attached or not and to which schema. And if
        // they aren't attached, attaching them as side-effect could be surprising (think that
        // adding a single field to a schema could bring a whole hierarchy of types and directives
        // for instance). If they are attached, it only works if it's to the same schema, but you
        // have to check. Overall, it's simpler to force attaching elements before you add other
        // elements to them.
        assert!(
            self._protected_is_attached(),
            "Cannot modify detached element {}",
            self
        );
    }
}

#[derive(Debug)]
struct ExtensionData {
    self_weak: Extension,
    extended_element: Option<ExtendableElementEnum>,
    source_ast: Option<AstNodeEnum>,
}

#[derive(Debug, Clone)]
pub struct Extension {
    data: Weak<RefCell<ExtensionData>>,
}

impl Extension {
    // This exists solely because ChildExtension is stored in sets, and we need to check whether
    // an Extension is in the set.
    fn upgrade(&self) -> ChildExtension {
        ChildExtension {
            data: self
                .data
                .upgrade()
                .expect("Element has been removed or owning schema has been dropped."),
        }
    }
}

impl PartialEq for Extension {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.data, &other.data)
    }
}

impl Eq for Extension {}

impl Hash for Extension {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Weak::as_ptr(&self.data).hash(state)
    }
}

// We purposely do not derive Clone to prevent Rc references from leaking.
#[derive(Debug)]
pub struct UnattachedExtension {
    data: Rc<RefCell<ExtensionData>>,
}

impl UnattachedExtension {
    fn to_child(self) -> ChildExtension {
        ChildExtension { data: self.data }
    }
}

#[derive(Debug, Clone)]
struct ChildExtension {
    data: Rc<RefCell<ExtensionData>>,
}

impl ChildExtension {
    fn downgrade(&self) -> Extension {
        Extension {
            data: Rc::downgrade(&self.data),
        }
    }
}

impl PartialEq for ChildExtension {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.data, &other.data)
    }
}

impl Eq for ChildExtension {}

impl Hash for ChildExtension {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Rc::as_ptr(&self.data).hash(state)
    }
}

impl Extension {
    pub fn new() -> UnattachedExtension {
        let self_rc = Rc::new_cyclic(|self_weak| {
            RefCell::new(ExtensionData {
                self_weak: Extension {
                    data: self_weak.clone(),
                },
                extended_element: None,
                source_ast: None,
            })
        });
        UnattachedExtension { data: self_rc }
    }

    fn _private_upgrade(&self) -> Rc<RefCell<ExtensionData>> {
        self.data
            .upgrade()
            .expect("Element has been removed or owning schema has been dropped.")
    }

    pub fn extended_element(&self) -> Option<ExtendableElementEnum> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.extended_element.clone())
    }

    // PORT_NOTE: This was private in the JS code, but was getting called in other classes via
    // Extension.prototype['setExtendedElement'], so module-private (the default) is fine here.
    fn set_extended_element(&self, extendable_element: ExtendableElementEnum) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            assert!(
                self_refmut.extended_element.is_none(),
                "Cannot attached already attached extension"
            );
            self_refmut.extended_element = Some(extendable_element)
        });
    }
}

impl HasSourceAst for Extension {
    fn source_ast(&self) -> Option<AstNodeEnum> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.source_ast.clone())
    }
}

impl SetSourceAst for Extension {
    fn set_source_ast(&self, source_ast: Option<AstNodeEnum>) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.source_ast = source_ast;
        })
    }
}

impl SealedTrait for Extension {}

// PORT_NOTE: In the JS code, you could store either a string or a DirectiveDefinition, but it turns
// out we were never invoking the DirectiveDefinition form, so we don't bother representing it here.
#[derive(Debug, Clone)]
pub(crate) struct UnappliedDirective {
    name: Rc<str>,
    args: Rc<InsertOnlyIndexMap<ValueEnum>>,
    extension: Option<Extension>,
    source_ast: AstNodeEnum,
}

impl UnappliedDirective {
    pub(crate) fn new(
        name: Rc<str>,
        args: Rc<InsertOnlyIndexMap<ValueEnum>>,
        extension: Option<Extension>,
        source_ast: AstNodeEnum,
    ) -> UnappliedDirective {
        UnappliedDirective {
            name,
            args,
            extension,
            source_ast,
        }
    }
}

// PORT_NOTE: We purposely omit the type parameter "TOwnType" from this trait, as Rust lets us use
// Self instead.
trait SchemaElement: Element + Into<SchemaElementEnum> + SealedTrait {
    fn _protected_applied_directives_borrow<
        T,
        F: FnOnce(Ref<CachedLinkedHashSet<ChildDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T;
    fn _protected_applied_directives_borrow_mut<
        T,
        F: FnOnce(RefMut<CachedLinkedHashSet<ChildDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T;
    fn _protected_unapplied_directives_borrow<T, F: FnOnce(Ref<Vec<UnappliedDirective>>) -> T>(
        &self,
        f: F,
    ) -> T;
    fn _protected_unapplied_directives_borrow_mut<
        T,
        F: FnOnce(RefMut<Vec<UnappliedDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T;

    fn _protected_add_unapplied_directive(&self, directive: UnappliedDirective) {
        self._protected_unapplied_directives_borrow_mut(|mut unapplied_directives_refmut| {
            unapplied_directives_refmut.push(directive);
        });
    }

    fn _protected_process_unapplied_directives(&self) -> Result<(), FederationError> {
        let unapplied_directives =
            self._protected_unapplied_directives_borrow(|unapplied_directives_ref| {
                unapplied_directives_ref.clone()
            });
        for UnappliedDirective {
            name,
            args,
            extension,
            source_ast,
        } in unapplied_directives
        {
            let directive = self._protected_apply_directive_name(name, Some(args), None)?;
            directive.set_of_extension(extension);
            directive.set_source_ast(Some(source_ast));
        }
        self._protected_unapplied_directives_borrow_mut(|mut unapplied_directives_refmut| {
            unapplied_directives_refmut.clear();
        });
        Ok(())
    }

    fn _protected_applied_directives(&self) -> Vec<Directive> {
        self._protected_applied_directives_borrow(|applied_directives_ref| {
            applied_directives_ref
                .cached_values()
                .iter()
                .map(|d| d.downgrade())
                .collect()
        })
    }

    // PORT_NOTE: In the JS code, this took either a name or definition. We've split them into two
    // separate methods here instead of creating an enum solely for that.
    fn _protected_applied_directives_of(&self, definition: &DirectiveDefinition) -> Vec<Directive> {
        self._protected_applied_directives_of_name(&definition.name())
    }

    fn _protected_applied_directives_of_name(&self, name: &str) -> Vec<Directive> {
        self._protected_applied_directives_borrow(|applied_directives_ref| {
            applied_directives_ref
                .cached_values()
                .iter()
                .filter_map(|d| {
                    let d = d.downgrade();
                    if d.name().deref() == name {
                        Some(d)
                    } else {
                        None
                    }
                })
                .collect()
        })
    }

    // PORT_NOTE: In the JS code, this took either a name or definition. We've split them into two
    // separate methods here instead of creating an enum for that.
    fn _protected_has_applied_directive(&self, definition: &DirectiveDefinition) -> bool {
        self._protected_has_applied_directive_name(&definition.name())
    }

    fn _protected_has_applied_directive_name(&self, name: &str) -> bool {
        self._protected_applied_directives_borrow(|applied_directives_ref| {
            applied_directives_ref
                .cached_values()
                .iter()
                .any(|d| d.downgrade().name().deref() == name)
        })
    }

    fn _protected_apply_directive(
        &self,
        definition: DirectiveDefinition,
        args: Option<Rc<InsertOnlyIndexMap<ValueEnum>>>,
        as_first_directive: Option<bool>,
    ) -> Directive {
        self._protected_check_update(Some(definition.clone().into()));
        let child_directive = Directive::new(
            definition.name(),
            args.unwrap_or_else(|| Rc::new(InsertOnlyIndexMap::new())),
        )
        .to_child();
        self._private_apply_child_directive(
            child_directive,
            definition,
            as_first_directive.unwrap_or(false),
        )
    }

    fn _protected_apply_directive_name(
        &self,
        name: Rc<str>,
        args: Option<Rc<InsertOnlyIndexMap<ValueEnum>>>,
        as_first_directive: Option<bool>,
    ) -> Result<Directive, FederationError> {
        self._protected_check_update(None);
        let child_directive = Directive::new(
            name.clone(),
            args.unwrap_or_else(|| Rc::new(InsertOnlyIndexMap::new())),
        )
        .to_child();
        let directive = child_directive.downgrade();
        let schema = self._protected_schema();
        let definition_result = Ok(schema.directive(&name)).and_then(|d| {
            if d.is_some() {
                Ok(d)
            } else {
                schema
                    .blueprint()
                    .on_missing_directive_definition(&schema, &directive)
            }
        });
        let definition = match definition_result {
            Ok(Some(definition)) => definition,
            Ok(None) => {
                return Err(schema
                    .blueprint()
                    .on_graphql_validation_error(
                        &schema,
                        ErrorEnum::InvalidGraphQL
                            .definition()
                            .err(format!("Unknown directive \"@{}\".", &name), None),
                    )
                    .into())
            }
            Err(error) => return Err(err_graphql_validation_failed(error.to_causes()).into()),
        };
        Ok(self._private_apply_child_directive(
            child_directive,
            definition,
            as_first_directive.unwrap_or(false),
        ))
    }

    fn _private_apply_child_directive(
        &self,
        child_directive: ChildDirective,
        definition: DirectiveDefinition,
        as_first_directive: bool,
    ) -> Directive {
        let directive = child_directive.downgrade();
        let self_schema_element: SchemaElementEnum = self.clone().into();
        directive.set_parent(
            self_schema_element
                .to_directive_parent_element()
                .unwrap_or_else(|| panic!("Cannot apply directive to {}", self)),
        );
        // TODO: The JS code here made a note about wanting to type-check the arguments in the
        // directive application against the definition eventually.
        self._protected_applied_directives_borrow_mut(|mut applied_directives_refmut| {
            if as_first_directive {
                applied_directives_refmut.replace_front(child_directive);
            } else {
                applied_directives_refmut.replace(child_directive);
            }
        });
        definition.add_referencer(directive.clone());
        self._protected_on_modification();
        directive
    }

    fn _protected_on_modification(&self) {
        if let Some(schema) = self._protected_weak_schema_if_attached() {
            schema.upgrade().on_modification()
        }
    }

    fn _protected_is_built_in(&self) -> bool {
        false
    }

    fn _protected_check_update(&self, child: Option<SchemaElementCheckUpdateEnum>) {
        self._protected_check_update_attached();
        if !self._protected_schema().can_modify_built_in() {
            // Ensure this element (the modified one), is not a built-in, or part of one.
            let mut ancestor: Option<SchemaElementEnum> = Some(self.clone().into());
            while let Some(ref ancestor_ref) = ancestor {
                assert!(
                    !ancestor_ref.is_built_in(),
                    "Cannot modify built-in (or part of built-in) {}",
                    self
                );
                ancestor = ancestor_ref.schema_element_parent().to_schema_element();
            }
        }
        if let Some(child) = child {
            if child.is_attached() {
                assert_eq!(
                    self._protected_weak_schema(),
                    child.weak_schema(),
                    "Cannot add element {} to {} as it is attached to another schema",
                    child,
                    self
                )
            }
        }
    }
}

trait ExtendableElement: SchemaElement + PreserveEmptyDefinition + Into<ExtendableElementEnum> + SealedTrait {
    fn _protected_extensions_borrow<T, F: FnOnce(Ref<CachedLinkedHashSet<ChildExtension>>) -> T>(
        &self,
        f: F,
    ) -> T;
    fn _protected_extensions_borrow_mut<
        T,
        F: FnOnce(RefMut<CachedLinkedHashSet<ChildExtension>>) -> T,
    >(
        &self,
        f: F,
    ) -> T;
    fn _protected_has_non_extension_inner_elements(&self) -> bool;

    fn _protected_extensions(&self) -> Vec<Extension> {
        self._protected_extensions_borrow(|extensions_ref| {
            extensions_ref
                .cached_values()
                .iter()
                .map(|e| e.downgrade())
                .collect()
        })
    }

    fn _protected_has_extension(&self, extension: &Extension) -> bool {
        self._protected_extensions_borrow(|extensions_ref| {
            extensions_ref.contains(&extension.upgrade())
        })
    }

    fn _protected_new_extension(&self) -> Extension {
        self._protected_add_extension(Extension::new())
    }

    fn _protected_add_extension(&self, extension: UnattachedExtension) -> Extension {
        self._protected_check_update(None);
        let child_extension = extension.to_child();
        let extension = child_extension.downgrade();
        // PORT_NOTE: The JS code allowed the added extension to potentially be attached. The Rust
        // types should forbid it, although if we leak methods somehow it might be possible, so we
        // still check.
        assert!(
            extension.extended_element().is_none(),
            "Cannot add extension to element {}: it is already added to another element",
            self
        );
        self._protected_extensions_borrow_mut(|mut extensions_refmut| {
            extensions_refmut.replace(child_extension);
        });
        extension.set_extended_element(self.clone().into());
        self._protected_on_modification();
        extension
    }

    fn _protected_has_extension_elements(&self) -> bool {
        self._protected_extensions_borrow(|extensions_ref| !extensions_ref.is_empty())
    }

    fn _protected_has_non_extension_elements(&self) -> bool {
        self.preserve_empty_definition() ||
            self._protected_applied_directives().iter().any(|d| d.of_extension().is_none()) ||
            self._protected_has_non_extension_inner_elements()
    }
}

// PORT_NOTE: Known as "BaseNamedType" in the JS code. This was ostensibly shorthand for "the base
// class for named types", but most of the JS classes were base classes and didn't start with the
// prefix "base". My guess is we needed to distinguish it from the JS union type "NamedType", which
// got renamed in Rust to have the "Enum" suffix.
trait NamedType: ExtendableElement + HasName + SealedTrait {
    type TReferencer: Eq + Hash + Clone;

    fn _protected_referencers_borrow<
        T,
        F: FnOnce(Ref<CachedLinkedHashSet<Self::TReferencer>>) -> T,
    >(
        &self,
        f: F,
    ) -> T;
    fn _protected_referencers_borrow_mut<
        T,
        F: FnOnce(RefMut<CachedLinkedHashSet<Self::TReferencer>>) -> T,
    >(
        &self,
        f: F,
    ) -> T;
    fn _protected_name_borrow_mut<
        T,
        F: FnOnce(RefMut<Rc<str>>) -> T,
    >(
        &self,
        f: F,
    ) -> T;

    // PORT_NOTE: This method was marked private in "addReferencer()" in the JS code, but the
    // codebase was calling it outside the abstract class through
    // "BaseNamedType.prototype['addReferencer']". The real reason to mark it "private" was
    // to keep it out of the public API, and accordingly it's marked module-private here.
    fn add_referencer(&self, referencer: Self::TReferencer) {
        self._protected_referencers_borrow_mut(|mut referencers_refmut| {
            referencers_refmut.replace(referencer);
        });
    }

    // PORT_NOTE: This method was marked private in "removeReferencer()" in the JS code, but the
    // codebase was calling it outside the abstract class through
    // "BaseNamedType.prototype['removeReferencer']". The real reason to mark it "private" was
    // to keep it out of the public API, and accordingly it's marked module-private here.
    fn remove_referencer(&self, referencer: &Self::TReferencer) {
        self._protected_referencers_borrow_mut(|mut referencers_refmut| {
            referencers_refmut.remove(referencer);
        });
    }

    fn _protected_rename(&self, new_name: Rc<str>) {
        // Mostly called to ensure we don't rename built-in types. It does mean we can't rename
        // detached types, and while this shouldn't be dangerous, but it's probably not a big deal
        // (the API is designed in such a way that you probably should avoid reusing detached
        // elements).
        self._protected_check_update(None);
        let old_name = self.name();
        self._protected_name_borrow_mut(|mut name_refmut| { *name_refmut = new_name.clone() });
        self._protected_schema().rename_type_internal(&old_name, new_name);
        self._protected_on_modification();
    }

    fn _protected_referencers(&self) -> Rc<[Self::TReferencer]> {
        self._protected_referencers_borrow(|referencers_ref| {
            referencers_ref.cached_values()
        })
    }

    fn _protected_is_referenced(&self) -> bool {
        self._protected_referencers_borrow(|referencers_ref| {
            !referencers_ref.is_empty()
        })
    }
}

trait NamedSchemaElementWithType: SchemaElement + SealedTrait {
    type TType;
}

// PORT_NOTE: Known as "BaseExtensionMember" in the JS code. This was ostensibly shorthand for "the
// base class for extension members", but most of the JS classes were base classes and didn't start
// with the prefix "base" (the exception was "BaseNamedType", which we've also renamed). Also note
// that "BaseExtensionMember" was only extended by a few elements in the JS code, but that's
// because JS classes can only extend at most one class, so we fixed that in the Rust code.
trait ExtensionMember: Element + SealedTrait {
    type TExtended;
}

#[enum_dispatch]
pub(crate) trait SchemaBlueprint: SealedTrait {
    fn on_missing_directive_definition(
        &self,
        schema: &Schema,
        directive: &Directive,
    ) -> Result<Option<DirectiveDefinition>, GraphQLErrors> {
        // No-op by default, but used for federation.
        Ok(None)
    }
    fn on_invalidation(&self, schema: &Schema) {
        // No-op by default, but used for federation.
    }

    /// Allows to intercept some apollo-rs error messages when we can provide additional guidance
    /// to users.
    fn on_graphql_validation_error(&self, schema: &Schema, error: GraphQLError) -> GraphQLError {
        // TODO: This function is very particular to the capabilities and message string formatting
        // of graphql-js, and we'll accordingly have to shift the logic here. For now though, we
        // just make this a no-op.
        error
    }
}

#[derive(Debug, Clone)]
pub struct DefaultBlueprint {
    data: Weak<RefCell<DefaultBlueprintData>>,
}

// We purposely do not derive Clone to prevent Rc references from leaking.
#[derive(Debug)]
pub struct UnattachedDefaultBlueprint {
    data: Rc<RefCell<DefaultBlueprintData>>,
}

impl UnattachedDefaultBlueprint {
    fn to_child(self) -> ChildDefaultBlueprint {
        ChildDefaultBlueprint { data: self.data }
    }
}

#[derive(Debug, Clone)]
struct ChildDefaultBlueprint {
    data: Rc<RefCell<DefaultBlueprintData>>,
}

impl ChildDefaultBlueprint {
    fn downgrade(&self) -> DefaultBlueprint {
        DefaultBlueprint {
            data: Rc::downgrade(&self.data),
        }
    }
}

#[derive(Debug)]
struct DefaultBlueprintData;

impl DefaultBlueprint {
    pub fn new() -> UnattachedDefaultBlueprint {
        UnattachedDefaultBlueprint {
            data: Rc::new(RefCell::new(DefaultBlueprintData)),
        }
    }
}

impl SchemaBlueprint for DefaultBlueprint {}

impl SealedTrait for DefaultBlueprint {}

#[enum_dispatch(SchemaBlueprint)]
#[derive(Debug, Clone)]
pub enum SchemaBlueprintEnum {
    DefaultBlueprint(DefaultBlueprint),
    FederationBlueprint(FederationBlueprint),
}

impl SealedTrait for SchemaBlueprintEnum {}

#[enum_dispatch(DefinitionsEmptyEnumDispatchPlaceholder)]
#[derive(Debug)]
pub enum UnattachedSchemaBlueprintEnum {
    DefaultBlueprint(UnattachedDefaultBlueprint),
    FederationBlueprint(UnattachedFederationBlueprint),
}

impl UnattachedSchemaBlueprintEnum {
    fn to_child(self) -> ChildSchemaBlueprintEnum {
        match self {
            UnattachedSchemaBlueprintEnum::DefaultBlueprint(blueprint) => {
                blueprint.to_child().into()
            }
            UnattachedSchemaBlueprintEnum::FederationBlueprint(blueprint) => {
                blueprint.to_child().into()
            }
        }
    }
}

#[enum_dispatch(DefinitionsEmptyEnumDispatchPlaceholder)]
#[derive(Debug, Clone)]
enum ChildSchemaBlueprintEnum {
    DefaultBlueprint(ChildDefaultBlueprint),
    FederationBlueprint(ChildFederationBlueprint),
}

impl ChildSchemaBlueprintEnum {
    fn downgrade(&self) -> SchemaBlueprintEnum {
        match self {
            ChildSchemaBlueprintEnum::DefaultBlueprint(blueprint) => blueprint.downgrade().into(),
            ChildSchemaBlueprintEnum::FederationBlueprint(blueprint) => {
                blueprint.downgrade().into()
            }
        }
    }
}

// PORT_NOTE: The JS code has "Directive", but this was only ever used for getting the source AST
// for error messages (and at times it wasn't even there, it depended on how the Directive was
// constructed). So we just pass in the source AST directly here if it's available.
#[derive(Debug)]
pub(crate) struct CoreFeature {
    url: FeatureUrl,
    name_in_schema: String,
    imports: Vec<CoreImport>,
    purpose: Option<String>,
    directive_source_ast: Option<apollo_parser::ast::Directive>,
}

impl CoreFeature {
    pub(crate) fn new(
        url: FeatureUrl,
        name_in_schema: String,
        imports: Vec<CoreImport>,
        purpose: Option<String>,
        directive_source_ast: Option<apollo_parser::ast::Directive>,
    ) -> CoreFeature {
        CoreFeature {
            url,
            name_in_schema,
            imports,
            purpose,
            directive_source_ast,
        }
    }

    pub fn is_feature_definition(&self, element: FeatureElementEnum) -> bool {
        let element_name_rc = element.name();
        let element_name = element_name_rc.deref();
        let is_directive_definition = matches!(element, FeatureElementEnum::DirectiveDefinition(_));
        let import_name = if is_directive_definition {
            "@".to_owned() + element_name
        } else {
            element_name.to_owned()
        };
        let prefix = self.name_in_schema.clone() + "__";
        element_name.starts_with(&prefix)
            || (is_directive_definition && element_name == self.name_in_schema)
            || self
                .imports
                .iter()
                .any(|i| import_name == i.as_().unwrap_or(i.name()))
    }

    pub fn directive_name_in_schema(&self, name: &str) -> String {
        let import_name = "@".to_owned() + name;
        let element_import = self.imports.iter().find(|i| i.name() == import_name);
        if let Some(element_import) = element_import {
            element_import
                .as_()
                .map(|as_| {
                    let mut iter = as_.chars();
                    iter.next();
                    iter.as_str()
                })
                .unwrap_or(name)
                .to_owned()
        } else {
            if name == self.url.name() {
                self.name_in_schema.clone()
            } else {
                self.name_in_schema.clone() + "__" + name
            }
        }
    }

    pub fn type_name_in_schema(&self, name: &str) -> String {
        let element_import = self.imports.iter().find(|i| i.name() == name);
        if let Some(element_import) = element_import {
            element_import.as_().unwrap_or(name).to_owned()
        } else {
            self.name_in_schema.clone() + "__" + name
        }
    }
}

#[derive(Debug)]
pub(crate) struct CoreFeatures {}

#[derive(Debug, Clone)]
pub struct Schema {
    data: Rc<RefCell<SchemaData>>,
}

impl Schema {
    fn downgrade(&self) -> WeakSchema {
        WeakSchema {
            data: Rc::downgrade(&self.data),
        }
    }
}

impl PartialEq for Schema {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.data, &other.data)
    }
}

impl Eq for Schema {}

impl Hash for Schema {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Rc::as_ptr(&self.data).hash(state)
    }
}

// This is public because it appears in a sealed method of a public trait, but there's otherwise no
// reason for this to be public (and its fields and methods are accordingly not public).
#[derive(Debug, Clone)]
pub(crate) struct WeakSchema {
    data: Weak<RefCell<SchemaData>>,
}

impl WeakSchema {
    fn upgrade(&self) -> Schema {
        Schema {
            data: self.data.upgrade().expect("Schema has been dropped."),
        }
    }
}

impl PartialEq for WeakSchema {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.data, &other.data)
    }
}

impl Eq for WeakSchema {}

impl Hash for WeakSchema {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Weak::as_ptr(&self.data).hash(state)
    }
}

#[derive(Debug)]
struct SchemaData {
    self_weak: WeakSchema,
    blueprint: ChildSchemaBlueprintEnum,
    schema_definition: ChildSchemaDefinition,
    built_in_types: CachedLinkedHashMap<ChildNamedTypeEnum>,
    types: CachedLinkedHashMap<ChildNamedTypeEnum>,
    built_in_directives: CachedLinkedHashMap<DirectiveDefinition>,
    directives: CachedLinkedHashMap<DirectiveDefinition>,
    core_features: Option<CoreFeatures>,
    is_constructed: bool,
    is_validated: bool,
    api_schema: Option<Schema>,
}

impl Schema {
    // PORT_NOTE: In the JS code, there was a second argument for config, but it only had one key
    // in it named "cacheAST", and it was never actually enabled in constructor calls.
    pub fn new(blueprint: Option<UnattachedSchemaBlueprintEnum>) -> Schema {
        let self_rc = Rc::new_cyclic(|self_weak| {
            let weak_schema = WeakSchema {
                data: self_weak.clone(),
            };
            let child_schema_definition = SchemaDefinition::new().to_child();
            child_schema_definition
                .downgrade()
                .set_parent(weak_schema.clone());
            RefCell::new(SchemaData {
                self_weak: weak_schema,
                blueprint: blueprint
                    .unwrap_or_else(|| DefaultBlueprint::new().into())
                    .to_child(),
                schema_definition: child_schema_definition,
                built_in_types: CachedLinkedHashMap::new(),
                types: CachedLinkedHashMap::new(),
                built_in_directives: CachedLinkedHashMap::new(),
                directives: CachedLinkedHashMap::new(),
                core_features: None,
                is_constructed: false,
                is_validated: false,
                api_schema: None,
            })
        });
        // TODO: Rest of the constructor
        Schema { data: self_rc }
    }

    pub(crate) fn blueprint(&self) -> SchemaBlueprintEnum {
        self.data
            .with_borrow(|self_ref| self_ref.blueprint.downgrade())
    }

    // PORT_NOTE: This was private in the JS code, but was getting called in other classes via
    // Schema.prototype['canModifyBuiltIn'], so module-private (the default) is fine here.
    fn can_modify_built_in(&self) -> bool {
        self.data
            .with_borrow(|self_ref| !self_ref.is_constructed.clone())
    }

    // PORT_NOTE: This was private in the JS code, but was getting called in other classes via
    // Schema.prototype['renameTypeInternal'], so module-private (the default) is fine here.
    fn rename_type_internal(&self, old_name: &str, new_name: Rc<str>) {
        self.data.with_borrow_mut(|mut self_refmut| {
            let type_ = self_refmut.types.remove(old_name).unwrap_or_else(|| {
                panic!("Type {} does not exist in this schema", old_name)
            });
            self_refmut.types.replace(new_name, type_);
        });
    }

    // PORT_NOTE: This was private in the JS code, but was getting called in other classes via
    // Schema.prototype['onModification'], so module-private (the default) is fine here.
    fn on_modification(&self) {
        let is_constructed = self
            .data
            .with_borrow(|self_ref| self_ref.is_constructed.clone());
        if is_constructed {
            self.invalidate();
            self.data.with_borrow_mut(|mut self_refmut| {
                self_refmut.api_schema = None;
            });
        }
    }

    // PORT_NOTE: The JS code uses "type", but it's a reserved keyword in Rust, so we change it to
    // "type_" here.
    /// The type of the provide name in this schema if one is defined or if it is the name of a
    /// built-in.
    pub fn type_(&self, name: &str) -> Option<NamedTypeEnum> {
        self.data.with_borrow(|self_ref| {
            self_ref
                .types
                .get(name)
                .or_else(|| self_ref.built_in_types.get(name))
                .map(|child| child.downgrade())
        })
    }

    pub fn add_type(&self, type_: UnattachedNamedTypeEnum) -> NamedTypeEnum {
        let child_type = type_.to_child();
        let type_ = child_type.downgrade();
        let type_name = type_.name();
        let existing = self
            .data
            .with_borrow(|self_ref| self_ref.types.get(&type_name).map(|t| t.downgrade()));
        if let Some(existing) = existing {
            // Like for directive, we let user shadow built-in types, but the definition must be valid.
            assert!(
                existing.is_built_in(),
                "Type {} already exists in this schema",
                type_
            )
        }
        // PORT_NOTE: The JS code allowed the added type to potentially be attached. The Rust types
        // should forbid it, although if we leak methods somehow it might be possible, so we still
        // check.
        assert!(
            !type_.is_attached(),
            "Type {} was unexpectedly already attached",
            type_
        );
        if type_.is_built_in() {
            let is_constructed = self.data.with_borrow(|self_ref| self_ref.is_constructed);
            assert!(!is_constructed, "Cannot add built-in {} to this schema (built-ins can only be added at schema construction time)", type_);
            self.data.with_borrow_mut(|mut self_refmut| {
                self_refmut
                    .built_in_types
                    .replace(Rc::from(type_name), child_type);
            })
        } else {
            self.data.with_borrow_mut(|mut self_refmut| {
                self_refmut.types.replace(Rc::from(type_name), child_type);
            })
        }
        type_.set_parent_weak_schema(self.downgrade());
        // If a type is the default name of a root, it "becomes" that root automatically, unless
        // some other root has already been set.
        //
        // PORT_NOTE: We inlined the JS function "checkDefaultSchemaRoot()" here since it was only
        // used here.

        // TODO: Finish this once SchemaDefinition is more fleshed out.
        type_
    }

    pub fn directive(&self, name: &str) -> Option<DirectiveDefinition> {
        self.data
            .with_borrow(|self_ref| self_ref.directives.get(name).map(|d| d.clone()))
            .or_else(|| self.built_in_directive(name))
    }

    pub fn built_in_directive(&self, name: &str) -> Option<DirectiveDefinition> {
        self.data
            .with_borrow(|self_ref| self_ref.built_in_directives.get(name).map(|d| d.clone()))
    }

    pub fn invalidate(&self) {
        let (is_validated, blueprint) = self
            .data
            .with_borrow(|self_ref| (self_ref.is_validated.clone(), self_ref.blueprint.clone()));
        if is_validated {
            blueprint.downgrade().on_invalidation(self);
        }
        self.data.with_borrow_mut(|mut self_refmut| {
            self_refmut.is_validated = false;
        });
    }
}

impl HasWeakSchema for WeakSchema {
    fn weak_schema(&self) -> WeakSchema {
        self.clone()
    }

    fn weak_schema_if_attached(&self) -> Option<WeakSchema> {
        Some(self.clone())
    }
}

impl SealedTrait for WeakSchema {}

#[derive(Debug, Clone)]
pub struct RootType {
    data: Weak<RefCell<RootTypeData>>,
}

impl PartialEq for RootType {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.data, &other.data)
    }
}

impl Eq for RootType {}

impl Hash for RootType {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Weak::as_ptr(&self.data).hash(state)
    }
}

// We purposely do not derive Clone to prevent Rc references from leaking.
#[derive(Debug)]
pub struct UnattachedRootType {
    data: Rc<RefCell<RootTypeData>>,
}

#[derive(Debug, Clone)]
struct ChildRootType {
    data: Rc<RefCell<RootTypeData>>,
}

#[derive(Debug)]
struct RootTypeData {
    // This Option should always be present post-construction.
    self_weak: Option<RootType>,
    parent: Option<SchemaDefinition>,
    source_ast: Option<AstNodeEnum>,
}

impl RootType {
    fn _private_upgrade(&self) -> Rc<RefCell<RootTypeData>> {
        self.data
            .upgrade()
            .expect("Element has been removed or owning schema has been dropped.")
    }
}

impl ExtensionMember for RootType {
    type TExtended = ();
}

impl Element for RootType {
    type TParent = SchemaDefinition;

    fn _protected_parent_borrow<T, F: FnOnce(Ref<Option<Self::TParent>>) -> T>(
        &self,
        f: F,
    ) -> T {
        self._private_upgrade()
            .with_borrow(|self_ref| f(Ref::map(self_ref, |e| &e.parent)))
    }

    fn _protected_parent_borrow_mut<T, F: FnOnce(RefMut<Option<Self::TParent>>) -> T>(
        &self,
        f: F,
    ) -> T {
        self._private_upgrade()
            .with_borrow_mut(|self_refmut| f(RefMut::map(self_refmut, |e| &mut e.parent)))
    }
}

impl HasParent for RootType {
    type TParent = SchemaDefinition;

    fn parent(&self) -> Self::TParent {
        self._protected_parent()
    }
}

impl SetParent for RootType {
    type TParent = SchemaDefinition;

    fn set_parent(&self, parent: Self::TParent) {
        self._protected_set_parent(parent)
    }
}

impl IsAttached for RootType {
    fn is_attached(&self) -> bool {
        self._protected_is_attached()
    }
}

impl HasSchema for RootType {
    fn schema(&self) -> Schema {
        self._protected_schema()
    }
}

impl HasWeakSchema for RootType {
    fn weak_schema(&self) -> WeakSchema {
        self._protected_weak_schema()
    }

    fn weak_schema_if_attached(&self) -> Option<WeakSchema> {
        self._protected_weak_schema_if_attached()
    }
}

impl HasSourceAst for RootType {
    fn source_ast(&self) -> Option<AstNodeEnum> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.source_ast.clone())
    }
}

impl SetSourceAst for RootType {
    fn set_source_ast(&self, source_ast: Option<AstNodeEnum>) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.source_ast = source_ast;
        })
    }
}

impl SealedTrait for RootType {}

impl Display for RootType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl FederationDisplay for RootType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

#[derive(Debug, Clone)]
pub struct SchemaDefinition {
    data: Weak<RefCell<SchemaDefinitionData>>,
}

impl PartialEq for SchemaDefinition {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.data, &other.data)
    }
}

impl Eq for SchemaDefinition {}

impl Hash for SchemaDefinition {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Weak::as_ptr(&self.data).hash(state)
    }
}

// We purposely do not derive Clone to prevent Rc references from leaking.
#[derive(Debug)]
pub struct UnattachedSchemaDefinition {
    data: Rc<RefCell<SchemaDefinitionData>>,
}

impl UnattachedSchemaDefinition {
    fn to_child(self) -> ChildSchemaDefinition {
        ChildSchemaDefinition { data: self.data }
    }
}

#[derive(Debug, Clone)]
struct ChildSchemaDefinition {
    data: Rc<RefCell<SchemaDefinitionData>>,
}

impl ChildSchemaDefinition {
    fn downgrade(&self) -> SchemaDefinition {
        SchemaDefinition {
            data: Rc::downgrade(&self.data),
        }
    }
}

#[derive(Debug)]
struct SchemaDefinitionData {
    self_weak: SchemaDefinition,
    parent: Option<WeakSchema>,
    source_ast: Option<AstNodeEnum>,
    description: Option<Rc<str>>,
}

impl SchemaDefinition {
    fn new() -> UnattachedSchemaDefinition {
        let self_rc = Rc::new_cyclic(|self_weak| {
            RefCell::new(SchemaDefinitionData {
                self_weak: SchemaDefinition {
                    data: self_weak.clone(),
                },
                parent: None,
                source_ast: None,
                description: None,
            })
        });
        UnattachedSchemaDefinition { data: self_rc }
    }
}

impl SchemaDefinition {
    fn _private_upgrade(&self) -> Rc<RefCell<SchemaDefinitionData>> {
        self.data
            .upgrade()
            .expect("Element has been removed or owning schema has been dropped.")
    }
}

impl ExtendableElement for SchemaDefinition {
    fn _protected_extensions_borrow<T, F: FnOnce(Ref<CachedLinkedHashSet<ChildExtension>>) -> T>(&self, f: F) -> T {
        todo!()
    }

    fn _protected_extensions_borrow_mut<T, F: FnOnce(RefMut<CachedLinkedHashSet<ChildExtension>>) -> T>(&self, f: F) -> T {
        todo!()
    }

    fn _protected_has_non_extension_inner_elements(&self) -> bool {
        todo!()
    }
}

impl SchemaElement for SchemaDefinition {
    fn _protected_applied_directives_borrow<
        T,
        F: FnOnce(Ref<CachedLinkedHashSet<ChildDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_applied_directives_borrow_mut<
        T,
        F: FnOnce(RefMut<CachedLinkedHashSet<ChildDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_unapplied_directives_borrow<T, F: FnOnce(Ref<Vec<UnappliedDirective>>) -> T>(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_unapplied_directives_borrow_mut<
        T,
        F: FnOnce(RefMut<Vec<UnappliedDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }
}

impl Element for SchemaDefinition {
    type TParent = WeakSchema;

    fn _protected_parent_borrow<T, F: FnOnce(Ref<Option<Self::TParent>>) -> T>(&self, f: F) -> T {
        self._private_upgrade()
            .with_borrow(|self_ref| f(Ref::map(self_ref, |e| &e.parent)))
    }

    fn _protected_parent_borrow_mut<T, F: FnOnce(RefMut<Option<Self::TParent>>) -> T>(
        &self,
        f: F,
    ) -> T {
        self._private_upgrade()
            .with_borrow_mut(|self_refmut| f(RefMut::map(self_refmut, |e| &mut e.parent)))
    }
}

impl HasKind for SchemaDefinition {
    fn kind(&self) -> NamedTypeKindEnum {
        NamedTypeKindEnum::SchemaDefinition
    }
}

impl HasParent for SchemaDefinition {
    type TParent = Schema;

    fn parent(&self) -> Self::TParent {
        self._protected_parent().upgrade()
    }
}

impl SetParent for SchemaDefinition {
    type TParent = WeakSchema;

    fn set_parent(&self, parent: Self::TParent) {
        self._protected_set_parent(parent)
    }
}

impl HasParentSchema for SchemaDefinition {
    fn parent_schema(&self) -> Schema {
        self.parent()
    }
}

impl SetParentWeakSchema for SchemaDefinition {
    fn set_parent_weak_schema(&self, parent: WeakSchema) {
        self.set_parent(parent)
    }
}

impl IsAttached for SchemaDefinition {
    fn is_attached(&self) -> bool {
        self._protected_is_attached()
    }
}

impl HasSchema for SchemaDefinition {
    fn schema(&self) -> Schema {
        self._protected_schema()
    }
}

impl HasWeakSchema for SchemaDefinition {
    fn weak_schema(&self) -> WeakSchema {
        self._protected_weak_schema()
    }

    fn weak_schema_if_attached(&self) -> Option<WeakSchema> {
        self._protected_weak_schema_if_attached()
    }
}

impl HasSourceAst for SchemaDefinition {
    fn source_ast(&self) -> Option<AstNodeEnum> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.source_ast.clone())
    }
}

impl SetSourceAst for SchemaDefinition {
    fn set_source_ast(&self, source_ast: Option<AstNodeEnum>) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.source_ast = source_ast;
        })
    }
}

impl HasDescription for SchemaDefinition {
    fn description(&self) -> Option<Rc<str>> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.description.clone())
    }

    fn set_description(&self, description: Option<Rc<str>>) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.description = description;
        })
    }
}

impl SchemaElementHasParent for SchemaDefinition {
    fn schema_element_parent(&self) -> SchemaElementParentEnum {
        self.parent().into()
    }
}

impl IsBuiltIn for SchemaDefinition {
    fn is_built_in(&self) -> bool {
        self._protected_is_built_in()
    }
}

impl AddUnappliedDirectives for SchemaDefinition {
    fn add_unapplied_directive(&self, directive: UnappliedDirective) {
        self._protected_add_unapplied_directive(directive)
    }

    fn process_unapplied_directives(&self) -> Result<(), FederationError> {
        self._protected_process_unapplied_directives()
    }
}

impl HasAppliedDirectives for SchemaDefinition {
    fn applied_directives(&self) -> Vec<Directive> {
        self._protected_applied_directives()
    }

    fn applied_directives_of(&self, definition: &DirectiveDefinition) -> Vec<Directive> {
        self._protected_applied_directives_of(definition)
    }

    fn applied_directives_of_name(&self, name: &str) -> Vec<Directive> {
        self._protected_applied_directives_of_name(name)
    }

    fn has_applied_directive(&self, definition: &DirectiveDefinition) -> bool {
        self._protected_has_applied_directive(definition)
    }

    fn has_applied_directive_name(&self, name: &str) -> bool {
        self._protected_has_applied_directive_name(name)
    }
}

impl AddAppliedDirectives for SchemaDefinition {
    fn apply_directive(&self, definition: DirectiveDefinition, args: Option<Rc<InsertOnlyIndexMap<ValueEnum>>>, as_first_directive: Option<bool>) -> Directive {
        self._protected_apply_directive(definition, args, as_first_directive)
    }

    fn apply_directive_name(&self, name: Rc<str>, args: Option<Rc<InsertOnlyIndexMap<ValueEnum>>>, as_first_directive: Option<bool>) -> Result<Directive, FederationError> {
        self._protected_apply_directive_name(name, args, as_first_directive)
    }
}

impl PreserveEmptyDefinition for SchemaDefinition {
    fn preserve_empty_definition(&self) -> bool {
        todo!()
    }

    fn set_preserve_empty_definition(&self, preserve_empty_definition: bool) {
        todo!()
    }
}

impl HasExtensions for SchemaDefinition {
    fn extensions(&self) -> Vec<Extension> {
        self._protected_extensions()
    }

    fn has_extension(&self, extension: &Extension) -> bool {
        self._protected_has_extension(extension)
    }

    fn has_extension_elements(&self) -> bool {
        self._protected_has_extension_elements()
    }

    fn has_non_extension_elements(&self) -> bool {
        self._protected_has_non_extension_elements()
    }
}

impl AddExtensions for SchemaDefinition {
    fn new_extension(&self) -> Extension {
        self._protected_new_extension()
    }

    fn add_extension(&self, extension: UnattachedExtension) -> Extension {
        self._protected_add_extension(extension)
    }
}

impl SealedTrait for SchemaDefinition {}

impl Display for SchemaDefinition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl FederationDisplay for SchemaDefinition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

#[derive(Debug, Clone)]
pub struct ScalarType {
    data: Weak<RefCell<ScalarTypeData>>,
}

impl PartialEq for ScalarType {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.data, &other.data)
    }
}

impl Eq for ScalarType {}

impl Hash for ScalarType {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Weak::as_ptr(&self.data).hash(state)
    }
}

// We purposely do not derive Clone to prevent Rc references from leaking.
#[derive(Debug)]
pub struct UnattachedScalarType {
    data: Rc<RefCell<ScalarTypeData>>,
}

impl UnattachedScalarType {
    fn to_child(self) -> ChildScalarType {
        ChildScalarType { data: self.data }
    }
}

#[derive(Debug, Clone)]
struct ChildScalarType {
    data: Rc<RefCell<ScalarTypeData>>,
}

impl ChildScalarType {
    fn downgrade(&self) -> ScalarType {
        ScalarType {
            data: Rc::downgrade(&self.data),
        }
    }
}

#[derive(Debug)]
struct ScalarTypeData {
    // This Option should always be present post-construction.
    self_weak: Option<ScalarType>,
    parent: Option<WeakSchema>,
    source_ast: Option<AstNodeEnum>,
    description: Option<Rc<str>>,
}

impl ScalarType {
    fn _private_upgrade(&self) -> Rc<RefCell<ScalarTypeData>> {
        self.data
            .upgrade()
            .expect("Element has been removed or owning schema has been dropped.")
    }
}

impl NamedType for ScalarType {
    type TReferencer = ScalarTypeReferencer;

    fn _protected_referencers_borrow<
        T,
        F: FnOnce(Ref<CachedLinkedHashSet<Self::TReferencer>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_referencers_borrow_mut<
        T,
        F: FnOnce(RefMut<CachedLinkedHashSet<Self::TReferencer>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_name_borrow_mut<T, F: FnOnce(RefMut<Rc<str>>) -> T>(&self, f: F) -> T {
        todo!()
    }
}

impl ExtendableElement for ScalarType {
    fn _protected_extensions_borrow<T, F: FnOnce(Ref<CachedLinkedHashSet<ChildExtension>>) -> T>(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_extensions_borrow_mut<
        T,
        F: FnOnce(RefMut<CachedLinkedHashSet<ChildExtension>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_has_non_extension_inner_elements(&self) -> bool {
        todo!()
    }
}

impl SchemaElement for ScalarType {
    fn _protected_applied_directives_borrow<
        T,
        F: FnOnce(Ref<CachedLinkedHashSet<ChildDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_applied_directives_borrow_mut<
        T,
        F: FnOnce(RefMut<CachedLinkedHashSet<ChildDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_unapplied_directives_borrow<T, F: FnOnce(Ref<Vec<UnappliedDirective>>) -> T>(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_unapplied_directives_borrow_mut<
        T,
        F: FnOnce(RefMut<Vec<UnappliedDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }
}

impl Element for ScalarType {
    type TParent = WeakSchema;

    fn _protected_parent_borrow<T, F: FnOnce(Ref<Option<Self::TParent>>) -> T>(&self, f: F) -> T {
        self._private_upgrade()
            .with_borrow(|self_ref| f(Ref::map(self_ref, |e| &e.parent)))
    }

    fn _protected_parent_borrow_mut<T, F: FnOnce(RefMut<Option<Self::TParent>>) -> T>(
        &self,
        f: F,
    ) -> T {
        self._private_upgrade()
            .with_borrow_mut(|self_refmut| f(RefMut::map(self_refmut, |e| &mut e.parent)))
    }
}

impl HasKind for ScalarType {
    fn kind(&self) -> NamedTypeKindEnum {
        NamedTypeKindEnum::ScalarType
    }
}

impl HasParent for ScalarType {
    type TParent = Schema;

    fn parent(&self) -> Self::TParent {
        self._protected_parent().upgrade()
    }
}

impl SetParent for ScalarType {
    type TParent = WeakSchema;

    fn set_parent(&self, parent: Self::TParent) {
        self._protected_set_parent(parent)
    }
}

impl HasParentSchema for ScalarType {
    fn parent_schema(&self) -> Schema {
        self.parent()
    }
}

impl SetParentWeakSchema for ScalarType {
    fn set_parent_weak_schema(&self, parent: WeakSchema) {
        self.set_parent(parent)
    }
}

impl IsAttached for ScalarType {
    fn is_attached(&self) -> bool {
        self._protected_is_attached()
    }
}

impl HasSchema for ScalarType {
    fn schema(&self) -> Schema {
        self._protected_schema()
    }
}

impl HasWeakSchema for ScalarType {
    fn weak_schema(&self) -> WeakSchema {
        self._protected_weak_schema()
    }

    fn weak_schema_if_attached(&self) -> Option<WeakSchema> {
        self._protected_weak_schema_if_attached()
    }
}

impl HasSourceAst for ScalarType {
    fn source_ast(&self) -> Option<AstNodeEnum> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.source_ast.clone())
    }
}

impl SetSourceAst for ScalarType {
    fn set_source_ast(&self, source_ast: Option<AstNodeEnum>) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.source_ast = source_ast;
        })
    }
}

impl HasDescription for ScalarType {
    fn description(&self) -> Option<Rc<str>> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.description.clone())
    }

    fn set_description(&self, description: Option<Rc<str>>) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.description = description;
        })
    }
}

impl SchemaElementHasParent for ScalarType {
    fn schema_element_parent(&self) -> SchemaElementParentEnum {
        self.parent().into()
    }
}

impl IsBuiltIn for ScalarType {
    fn is_built_in(&self) -> bool {
        todo!()
    }
}

impl AddUnappliedDirectives for ScalarType {
    fn add_unapplied_directive(&self, directive: UnappliedDirective) {
        self._protected_add_unapplied_directive(directive)
    }

    fn process_unapplied_directives(&self) -> Result<(), FederationError> {
        self._protected_process_unapplied_directives()
    }
}

impl HasAppliedDirectives for ScalarType {
    fn applied_directives(&self) -> Vec<Directive> {
        self._protected_applied_directives()
    }

    fn applied_directives_of(&self, definition: &DirectiveDefinition) -> Vec<Directive> {
        self._protected_applied_directives_of(definition)
    }

    fn applied_directives_of_name(&self, name: &str) -> Vec<Directive> {
        self._protected_applied_directives_of_name(name)
    }

    fn has_applied_directive(&self, definition: &DirectiveDefinition) -> bool {
        self._protected_has_applied_directive(definition)
    }

    fn has_applied_directive_name(&self, name: &str) -> bool {
        self._protected_has_applied_directive_name(name)
    }
}

impl AddAppliedDirectives for ScalarType {
    fn apply_directive(&self, definition: DirectiveDefinition, args: Option<Rc<InsertOnlyIndexMap<ValueEnum>>>, as_first_directive: Option<bool>) -> Directive {
        self._protected_apply_directive(definition, args, as_first_directive)
    }

    fn apply_directive_name(&self, name: Rc<str>, args: Option<Rc<InsertOnlyIndexMap<ValueEnum>>>, as_first_directive: Option<bool>) -> Result<Directive, FederationError> {
        self._protected_apply_directive_name(name, args, as_first_directive)
    }
}

impl PreserveEmptyDefinition for ScalarType {
    fn preserve_empty_definition(&self) -> bool {
        todo!()
    }

    fn set_preserve_empty_definition(&self, preserve_empty_definition: bool) {
        todo!()
    }
}

impl HasExtensions for ScalarType {
    fn extensions(&self) -> Vec<Extension> {
        self._protected_extensions()
    }

    fn has_extension(&self, extension: &Extension) -> bool {
        self._protected_has_extension(extension)
    }

    fn has_extension_elements(&self) -> bool {
        self._protected_has_extension_elements()
    }

    fn has_non_extension_elements(&self) -> bool {
        self._protected_has_non_extension_elements()
    }
}

impl AddExtensions for ScalarType {
    fn new_extension(&self) -> Extension {
        self._protected_new_extension()
    }

    fn add_extension(&self, extension: UnattachedExtension) -> Extension {
        self._protected_add_extension(extension)
    }
}

impl HasName for ScalarType {
    fn name(&self) -> Rc<str> {
        todo!()
    }
}

impl HasCoordinate for ScalarType {
    fn coordinate(&self) -> String {
        todo!()
    }
}

impl HasReferencers for ScalarType {
    type TReferencer = ScalarTypeReferencer;

    fn referencers(&self) -> Rc<[Self::TReferencer]> {
        self._protected_referencers()
    }

    fn is_referenced(&self) -> bool {
        self._protected_is_referenced()
    }
}

impl Rename for ScalarType {
    fn rename(&self, new_name: Rc<str>) {
        self._protected_rename(new_name)
    }
}

impl SealedTrait for ScalarType {}

impl Display for ScalarType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl FederationDisplay for ScalarType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

#[derive(Debug, Clone)]
pub struct InterfaceImplementation {
    data: Weak<RefCell<InterfaceImplementationData>>,
}

impl PartialEq for InterfaceImplementation {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.data, &other.data)
    }
}

impl Eq for InterfaceImplementation {}

impl Hash for InterfaceImplementation {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Weak::as_ptr(&self.data).hash(state)
    }
}

// We purposely do not derive Clone to prevent Rc references from leaking.
#[derive(Debug)]
pub struct UnattachedInterfaceImplementation {
    data: Rc<RefCell<InterfaceImplementationData>>,
}

#[derive(Debug, Clone)]
struct ChildInterfaceImplementation {
    data: Rc<RefCell<InterfaceImplementationData>>,
}

#[derive(Debug)]
struct InterfaceImplementationData {
    // This Option should always be present post-construction.
    self_weak: Option<InterfaceImplementation>,
    parent: Option<FieldBasedTypeEnum>,
    source_ast: Option<AstNodeEnum>,
}

impl InterfaceImplementation {
    fn _private_upgrade(&self) -> Rc<RefCell<InterfaceImplementationData>> {
        self.data
            .upgrade()
            .expect("Element has been removed or owning schema has been dropped.")
    }
}

impl ExtensionMember for InterfaceImplementation {
    type TExtended = ();
}

impl Element for InterfaceImplementation {
    type TParent = FieldBasedTypeEnum;

    fn _protected_parent_borrow<T, F: FnOnce(Ref<Option<Self::TParent>>) -> T>(
        &self,
        f: F,
    ) -> T {
        self._private_upgrade()
            .with_borrow(|self_ref| f(Ref::map(self_ref, |e| &e.parent)))
    }

    fn _protected_parent_borrow_mut<T, F: FnOnce(RefMut<Option<Self::TParent>>) -> T>(
        &self,
        f: F,
    ) -> T {
        self._private_upgrade()
            .with_borrow_mut(|self_refmut| f(RefMut::map(self_refmut, |e| &mut e.parent)))
    }
}

impl HasParent for InterfaceImplementation {
    type TParent = FieldBasedTypeEnum;

    fn parent(&self) -> Self::TParent {
        self._protected_parent()
    }
}

impl SetParent for InterfaceImplementation {
    type TParent = FieldBasedTypeEnum;

    fn set_parent(&self, parent: Self::TParent) {
        self._protected_set_parent(parent)
    }
}

impl IsAttached for InterfaceImplementation {
    fn is_attached(&self) -> bool {
        self._protected_is_attached()
    }
}

impl HasSchema for InterfaceImplementation {
    fn schema(&self) -> Schema {
        self._protected_schema()
    }
}

impl HasWeakSchema for InterfaceImplementation {
    fn weak_schema(&self) -> WeakSchema {
        self._protected_weak_schema()
    }

    fn weak_schema_if_attached(&self) -> Option<WeakSchema> {
        self._protected_weak_schema_if_attached()
    }
}

impl HasSourceAst for InterfaceImplementation {
    fn source_ast(&self) -> Option<AstNodeEnum> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.source_ast.clone())
    }
}

impl SetSourceAst for InterfaceImplementation {
    fn set_source_ast(&self, source_ast: Option<AstNodeEnum>) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.source_ast = source_ast;
        })
    }
}

impl SealedTrait for InterfaceImplementation {}

impl Display for InterfaceImplementation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl FederationDisplay for InterfaceImplementation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

// PORT_NOTE: We purposely omit the type parameter "T" from this trait, as Rust lets us use Self
// instead.
trait FieldBasedType: NamedType + SealedTrait {}

#[derive(Debug, Clone)]
pub struct ObjectType {
    data: Weak<RefCell<ObjectTypeData>>,
}

impl PartialEq for ObjectType {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.data, &other.data)
    }
}

impl Eq for ObjectType {}

impl Hash for ObjectType {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Weak::as_ptr(&self.data).hash(state)
    }
}

// We purposely do not derive Clone to prevent Rc references from leaking.
#[derive(Debug)]
pub struct UnattachedObjectType {
    data: Rc<RefCell<ObjectTypeData>>,
}

impl UnattachedObjectType {
    fn to_child(self) -> ChildObjectType {
        ChildObjectType { data: self.data }
    }
}

#[derive(Debug, Clone)]
struct ChildObjectType {
    data: Rc<RefCell<ObjectTypeData>>,
}

impl ChildObjectType {
    fn downgrade(&self) -> ObjectType {
        ObjectType {
            data: Rc::downgrade(&self.data),
        }
    }
}

#[derive(Debug)]
struct ObjectTypeData {
    // This Option should always be present post-construction.
    self_weak: Option<ObjectType>,
    parent: Option<WeakSchema>,
    source_ast: Option<AstNodeEnum>,
    description: Option<Rc<str>>,
}

impl ObjectType {
    fn _private_upgrade(&self) -> Rc<RefCell<ObjectTypeData>> {
        self.data
            .upgrade()
            .expect("Element has been removed or owning schema has been dropped.")
    }
}

impl FieldBasedType for ObjectType {}

impl NamedType for ObjectType {
    type TReferencer = ObjectTypeReferencer;

    fn _protected_referencers_borrow<
        T,
        F: FnOnce(Ref<CachedLinkedHashSet<Self::TReferencer>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_referencers_borrow_mut<
        T,
        F: FnOnce(RefMut<CachedLinkedHashSet<Self::TReferencer>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_name_borrow_mut<T, F: FnOnce(RefMut<Rc<str>>) -> T>(&self, f: F) -> T {
        todo!()
    }
}

impl ExtendableElement for ObjectType {
    fn _protected_extensions_borrow<T, F: FnOnce(Ref<CachedLinkedHashSet<ChildExtension>>) -> T>(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_extensions_borrow_mut<
        T,
        F: FnOnce(RefMut<CachedLinkedHashSet<ChildExtension>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_has_non_extension_inner_elements(&self) -> bool {
        todo!()
    }
}

impl SchemaElement for ObjectType {
    fn _protected_applied_directives_borrow<
        T,
        F: FnOnce(Ref<CachedLinkedHashSet<ChildDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_applied_directives_borrow_mut<
        T,
        F: FnOnce(RefMut<CachedLinkedHashSet<ChildDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_unapplied_directives_borrow<T, F: FnOnce(Ref<Vec<UnappliedDirective>>) -> T>(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_unapplied_directives_borrow_mut<
        T,
        F: FnOnce(RefMut<Vec<UnappliedDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }
}

impl Element for ObjectType {
    type TParent = WeakSchema;

    fn _protected_parent_borrow<T, F: FnOnce(Ref<Option<Self::TParent>>) -> T>(&self, f: F) -> T {
        self._private_upgrade()
            .with_borrow(|self_ref| f(Ref::map(self_ref, |e| &e.parent)))
    }

    fn _protected_parent_borrow_mut<T, F: FnOnce(RefMut<Option<Self::TParent>>) -> T>(
        &self,
        f: F,
    ) -> T {
        self._private_upgrade()
            .with_borrow_mut(|self_refmut| f(RefMut::map(self_refmut, |e| &mut e.parent)))
    }
}

impl HasKind for ObjectType {
    fn kind(&self) -> NamedTypeKindEnum {
        NamedTypeKindEnum::ObjectType
    }
}

impl HasParent for ObjectType {
    type TParent = Schema;

    fn parent(&self) -> Self::TParent {
        self._protected_parent().upgrade()
    }
}

impl SetParent for ObjectType {
    type TParent = WeakSchema;

    fn set_parent(&self, parent: Self::TParent) {
        self._protected_set_parent(parent)
    }
}

impl HasParentSchema for ObjectType {
    fn parent_schema(&self) -> Schema {
        self.parent()
    }
}

impl SetParentWeakSchema for ObjectType {
    fn set_parent_weak_schema(&self, parent: WeakSchema) {
        self.set_parent(parent)
    }
}

impl IsAttached for ObjectType {
    fn is_attached(&self) -> bool {
        self._protected_is_attached()
    }
}

impl HasSchema for ObjectType {
    fn schema(&self) -> Schema {
        self._protected_schema()
    }
}

impl HasWeakSchema for ObjectType {
    fn weak_schema(&self) -> WeakSchema {
        self._protected_weak_schema()
    }

    fn weak_schema_if_attached(&self) -> Option<WeakSchema> {
        self._protected_weak_schema_if_attached()
    }
}

impl HasSourceAst for ObjectType {
    fn source_ast(&self) -> Option<AstNodeEnum> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.source_ast.clone())
    }
}

impl SetSourceAst for ObjectType {
    fn set_source_ast(&self, source_ast: Option<AstNodeEnum>) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.source_ast = source_ast;
        })
    }
}

impl HasDescription for ObjectType {
    fn description(&self) -> Option<Rc<str>> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.description.clone())
    }

    fn set_description(&self, description: Option<Rc<str>>) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.description = description;
        })
    }
}

impl SchemaElementHasParent for ObjectType {
    fn schema_element_parent(&self) -> SchemaElementParentEnum {
        self.parent().into()
    }
}

impl IsBuiltIn for ObjectType {
    fn is_built_in(&self) -> bool {
        todo!()
    }
}

impl AddUnappliedDirectives for ObjectType {
    fn add_unapplied_directive(&self, directive: UnappliedDirective) {
        self._protected_add_unapplied_directive(directive)
    }

    fn process_unapplied_directives(&self) -> Result<(), FederationError> {
        self._protected_process_unapplied_directives()
    }
}

impl HasAppliedDirectives for ObjectType {
    fn applied_directives(&self) -> Vec<Directive> {
        self._protected_applied_directives()
    }

    fn applied_directives_of(&self, definition: &DirectiveDefinition) -> Vec<Directive> {
        self._protected_applied_directives_of(definition)
    }

    fn applied_directives_of_name(&self, name: &str) -> Vec<Directive> {
        self._protected_applied_directives_of_name(name)
    }

    fn has_applied_directive(&self, definition: &DirectiveDefinition) -> bool {
        self._protected_has_applied_directive(definition)
    }

    fn has_applied_directive_name(&self, name: &str) -> bool {
        self._protected_has_applied_directive_name(name)
    }
}

impl AddAppliedDirectives for ObjectType {
    fn apply_directive(&self, definition: DirectiveDefinition, args: Option<Rc<InsertOnlyIndexMap<ValueEnum>>>, as_first_directive: Option<bool>) -> Directive {
        self._protected_apply_directive(definition, args, as_first_directive)
    }

    fn apply_directive_name(&self, name: Rc<str>, args: Option<Rc<InsertOnlyIndexMap<ValueEnum>>>, as_first_directive: Option<bool>) -> Result<Directive, FederationError> {
        self._protected_apply_directive_name(name, args, as_first_directive)
    }
}

impl PreserveEmptyDefinition for ObjectType {
    fn preserve_empty_definition(&self) -> bool {
        todo!()
    }

    fn set_preserve_empty_definition(&self, preserve_empty_definition: bool) {
        todo!()
    }
}

impl HasExtensions for ObjectType {
    fn extensions(&self) -> Vec<Extension> {
        self._protected_extensions()
    }

    fn has_extension(&self, extension: &Extension) -> bool {
        self._protected_has_extension(extension)
    }

    fn has_extension_elements(&self) -> bool {
        self._protected_has_extension_elements()
    }

    fn has_non_extension_elements(&self) -> bool {
        self._protected_has_non_extension_elements()
    }
}

impl AddExtensions for ObjectType {
    fn new_extension(&self) -> Extension {
        self._protected_new_extension()
    }

    fn add_extension(&self, extension: UnattachedExtension) -> Extension {
        self._protected_add_extension(extension)
    }
}

impl HasName for ObjectType {
    fn name(&self) -> Rc<str> {
        todo!()
    }
}

impl HasCoordinate for ObjectType {
    fn coordinate(&self) -> String {
        todo!()
    }
}

impl HasReferencers for ObjectType {
    type TReferencer = ObjectTypeReferencer;

    fn referencers(&self) -> Rc<[Self::TReferencer]> {
        self._protected_referencers()
    }

    fn is_referenced(&self) -> bool {
        self._protected_is_referenced()
    }
}

impl Rename for ObjectType {
    fn rename(&self, new_name: Rc<str>) {
        self._protected_rename(new_name)
    }
}

impl SealedTrait for ObjectType {}

impl Display for ObjectType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl FederationDisplay for ObjectType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

#[derive(Debug, Clone)]
pub struct InterfaceType {
    data: Weak<RefCell<InterfaceTypeData>>,
}

impl PartialEq for InterfaceType {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.data, &other.data)
    }
}

impl Eq for InterfaceType {}

impl Hash for InterfaceType {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Weak::as_ptr(&self.data).hash(state)
    }
}

// We purposely do not derive Clone to prevent Rc references from leaking.
#[derive(Debug)]
pub struct UnattachedInterfaceType {
    data: Rc<RefCell<InterfaceTypeData>>,
}

impl UnattachedInterfaceType {
    fn to_child(self) -> ChildInterfaceType {
        ChildInterfaceType { data: self.data }
    }
}

#[derive(Debug, Clone)]
struct ChildInterfaceType {
    data: Rc<RefCell<InterfaceTypeData>>,
}

impl ChildInterfaceType {
    fn downgrade(&self) -> InterfaceType {
        InterfaceType {
            data: Rc::downgrade(&self.data),
        }
    }
}

#[derive(Debug)]
struct InterfaceTypeData {
    // This Option should always be present post-construction.
    self_weak: Option<InterfaceType>,
    parent: Option<WeakSchema>,
    source_ast: Option<AstNodeEnum>,
    description: Option<Rc<str>>,
}

impl InterfaceType {
    fn _private_upgrade(&self) -> Rc<RefCell<InterfaceTypeData>> {
        self.data
            .upgrade()
            .expect("Element has been removed or owning schema has been dropped.")
    }
}

impl FieldBasedType for InterfaceType {}

impl NamedType for InterfaceType {
    type TReferencer = InterfaceTypeReferencer;

    fn _protected_referencers_borrow<
        T,
        F: FnOnce(Ref<CachedLinkedHashSet<Self::TReferencer>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_referencers_borrow_mut<
        T,
        F: FnOnce(RefMut<CachedLinkedHashSet<Self::TReferencer>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_name_borrow_mut<T, F: FnOnce(RefMut<Rc<str>>) -> T>(&self, f: F) -> T {
        todo!()
    }
}

impl ExtendableElement for InterfaceType {
    fn _protected_extensions_borrow<T, F: FnOnce(Ref<CachedLinkedHashSet<ChildExtension>>) -> T>(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_extensions_borrow_mut<
        T,
        F: FnOnce(RefMut<CachedLinkedHashSet<ChildExtension>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_has_non_extension_inner_elements(&self) -> bool {
        todo!()
    }
}

impl SchemaElement for InterfaceType {
    fn _protected_applied_directives_borrow<
        T,
        F: FnOnce(Ref<CachedLinkedHashSet<ChildDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_applied_directives_borrow_mut<
        T,
        F: FnOnce(RefMut<CachedLinkedHashSet<ChildDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_unapplied_directives_borrow<T, F: FnOnce(Ref<Vec<UnappliedDirective>>) -> T>(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_unapplied_directives_borrow_mut<
        T,
        F: FnOnce(RefMut<Vec<UnappliedDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }
}

impl Element for InterfaceType {
    type TParent = WeakSchema;

    fn _protected_parent_borrow<T, F: FnOnce(Ref<Option<Self::TParent>>) -> T>(&self, f: F) -> T {
        self._private_upgrade()
            .with_borrow(|self_ref| f(Ref::map(self_ref, |e| &e.parent)))
    }

    fn _protected_parent_borrow_mut<T, F: FnOnce(RefMut<Option<Self::TParent>>) -> T>(
        &self,
        f: F,
    ) -> T {
        self._private_upgrade()
            .with_borrow_mut(|self_refmut| f(RefMut::map(self_refmut, |e| &mut e.parent)))
    }
}

impl HasKind for InterfaceType {
    fn kind(&self) -> NamedTypeKindEnum {
        NamedTypeKindEnum::InterfaceType
    }
}

impl HasParent for InterfaceType {
    type TParent = Schema;

    fn parent(&self) -> Self::TParent {
        self._protected_parent().upgrade()
    }
}

impl SetParent for InterfaceType {
    type TParent = WeakSchema;

    fn set_parent(&self, parent: Self::TParent) {
        self._protected_set_parent(parent)
    }
}

impl HasParentSchema for InterfaceType {
    fn parent_schema(&self) -> Schema {
        self.parent()
    }
}

impl SetParentWeakSchema for InterfaceType {
    fn set_parent_weak_schema(&self, parent: WeakSchema) {
        self.set_parent(parent)
    }
}

impl IsAttached for InterfaceType {
    fn is_attached(&self) -> bool {
        self._protected_is_attached()
    }
}

impl HasSchema for InterfaceType {
    fn schema(&self) -> Schema {
        self._protected_schema()
    }
}

impl HasWeakSchema for InterfaceType {
    fn weak_schema(&self) -> WeakSchema {
        self._protected_weak_schema()
    }

    fn weak_schema_if_attached(&self) -> Option<WeakSchema> {
        self._protected_weak_schema_if_attached()
    }
}

impl HasSourceAst for InterfaceType {
    fn source_ast(&self) -> Option<AstNodeEnum> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.source_ast.clone())
    }
}

impl SetSourceAst for InterfaceType {
    fn set_source_ast(&self, source_ast: Option<AstNodeEnum>) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.source_ast = source_ast;
        })
    }
}

impl HasDescription for InterfaceType {
    fn description(&self) -> Option<Rc<str>> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.description.clone())
    }

    fn set_description(&self, description: Option<Rc<str>>) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.description = description;
        })
    }
}

impl SchemaElementHasParent for InterfaceType {
    fn schema_element_parent(&self) -> SchemaElementParentEnum {
        self.parent().into()
    }
}

impl IsBuiltIn for InterfaceType {
    fn is_built_in(&self) -> bool {
        todo!()
    }
}

impl AddUnappliedDirectives for InterfaceType {
    fn add_unapplied_directive(&self, directive: UnappliedDirective) {
        self._protected_add_unapplied_directive(directive)
    }

    fn process_unapplied_directives(&self) -> Result<(), FederationError> {
        self._protected_process_unapplied_directives()
    }
}

impl HasAppliedDirectives for InterfaceType {
    fn applied_directives(&self) -> Vec<Directive> {
        self._protected_applied_directives()
    }

    fn applied_directives_of(&self, definition: &DirectiveDefinition) -> Vec<Directive> {
        self._protected_applied_directives_of(definition)
    }

    fn applied_directives_of_name(&self, name: &str) -> Vec<Directive> {
        self._protected_applied_directives_of_name(name)
    }

    fn has_applied_directive(&self, definition: &DirectiveDefinition) -> bool {
        self._protected_has_applied_directive(definition)
    }

    fn has_applied_directive_name(&self, name: &str) -> bool {
        self._protected_has_applied_directive_name(name)
    }
}

impl AddAppliedDirectives for InterfaceType {
    fn apply_directive(&self, definition: DirectiveDefinition, args: Option<Rc<InsertOnlyIndexMap<ValueEnum>>>, as_first_directive: Option<bool>) -> Directive {
        self._protected_apply_directive(definition, args, as_first_directive)
    }

    fn apply_directive_name(&self, name: Rc<str>, args: Option<Rc<InsertOnlyIndexMap<ValueEnum>>>, as_first_directive: Option<bool>) -> Result<Directive, FederationError> {
        self._protected_apply_directive_name(name, args, as_first_directive)
    }
}

impl PreserveEmptyDefinition for InterfaceType {
    fn preserve_empty_definition(&self) -> bool {
        todo!()
    }

    fn set_preserve_empty_definition(&self, preserve_empty_definition: bool) {
        todo!()
    }
}

impl HasExtensions for InterfaceType {
    fn extensions(&self) -> Vec<Extension> {
        self._protected_extensions()
    }

    fn has_extension(&self, extension: &Extension) -> bool {
        self._protected_has_extension(extension)
    }

    fn has_extension_elements(&self) -> bool {
        self._protected_has_extension_elements()
    }

    fn has_non_extension_elements(&self) -> bool {
        self._protected_has_non_extension_elements()
    }
}

impl AddExtensions for InterfaceType {
    fn new_extension(&self) -> Extension {
        self._protected_new_extension()
    }

    fn add_extension(&self, extension: UnattachedExtension) -> Extension {
        self._protected_add_extension(extension)
    }
}

impl HasName for InterfaceType {
    fn name(&self) -> Rc<str> {
        todo!()
    }
}

impl HasCoordinate for InterfaceType {
    fn coordinate(&self) -> String {
        todo!()
    }
}

impl HasReferencers for InterfaceType {
    type TReferencer = InterfaceTypeReferencer;

    fn referencers(&self) -> Rc<[Self::TReferencer]> {
        self._protected_referencers()
    }

    fn is_referenced(&self) -> bool {
        self._protected_is_referenced()
    }
}

impl Rename for InterfaceType {
    fn rename(&self, new_name: Rc<str>) {
        self._protected_rename(new_name)
    }
}

impl SealedTrait for InterfaceType {}

impl Display for InterfaceType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl FederationDisplay for InterfaceType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

#[derive(Debug, Clone)]
pub struct UnionMember {
    data: Weak<RefCell<UnionMemberData>>,
}

impl PartialEq for UnionMember {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.data, &other.data)
    }
}

impl Eq for UnionMember {}

impl Hash for UnionMember {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Weak::as_ptr(&self.data).hash(state)
    }
}

// We purposely do not derive Clone to prevent Rc references from leaking.
#[derive(Debug)]
pub struct UnattachedUnionMember {
    data: Rc<RefCell<UnionMemberData>>,
}

#[derive(Debug, Clone)]
struct ChildUnionMember {
    data: Rc<RefCell<UnionMemberData>>,
}

#[derive(Debug)]
struct UnionMemberData {
    // This Option should always be present post-construction.
    self_weak: Option<UnionMember>,
    parent: Option<UnionType>,
    source_ast: Option<AstNodeEnum>,
}

impl UnionMember {
    fn _private_upgrade(&self) -> Rc<RefCell<UnionMemberData>> {
        self.data
            .upgrade()
            .expect("Element has been removed or owning schema has been dropped.")
    }
}

impl ExtensionMember for UnionMember {
    type TExtended = ();
}

impl Element for UnionMember {
    type TParent = UnionType;

    fn _protected_parent_borrow<T, F: FnOnce(Ref<Option<Self::TParent>>) -> T>(&self, f: F) -> T {
        self._private_upgrade()
            .with_borrow(|self_ref| f(Ref::map(self_ref, |e| &e.parent)))
    }

    fn _protected_parent_borrow_mut<T, F: FnOnce(RefMut<Option<Self::TParent>>) -> T>(
        &self,
        f: F,
    ) -> T {
        self._private_upgrade()
            .with_borrow_mut(|self_refmut| f(RefMut::map(self_refmut, |e| &mut e.parent)))
    }
}

impl HasParent for UnionMember {
    type TParent = UnionType;

    fn parent(&self) -> Self::TParent {
        self._protected_parent()
    }
}

impl SetParent for UnionMember {
    type TParent = UnionType;

    fn set_parent(&self, parent: Self::TParent) {
        self._protected_set_parent(parent)
    }
}

impl IsAttached for UnionMember {
    fn is_attached(&self) -> bool {
        self._protected_is_attached()
    }
}

impl HasSchema for UnionMember {
    fn schema(&self) -> Schema {
        self._protected_schema()
    }
}

impl HasWeakSchema for UnionMember {
    fn weak_schema(&self) -> WeakSchema {
        self._protected_weak_schema()
    }

    fn weak_schema_if_attached(&self) -> Option<WeakSchema> {
        self._protected_weak_schema_if_attached()
    }
}

impl HasSourceAst for UnionMember {
    fn source_ast(&self) -> Option<AstNodeEnum> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.source_ast.clone())
    }
}

impl SetSourceAst for UnionMember {
    fn set_source_ast(&self, source_ast: Option<AstNodeEnum>) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.source_ast = source_ast;
        })
    }
}

impl SealedTrait for UnionMember {}

impl Display for UnionMember {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl FederationDisplay for UnionMember {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

#[derive(Debug, Clone)]
pub struct UnionType {
    data: Weak<RefCell<UnionTypeData>>,
}

impl PartialEq for UnionType {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.data, &other.data)
    }
}

impl Eq for UnionType {}

impl Hash for UnionType {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Weak::as_ptr(&self.data).hash(state)
    }
}

// We purposely do not derive Clone to prevent Rc references from leaking.
#[derive(Debug)]
pub struct UnattachedUnionType {
    data: Rc<RefCell<UnionTypeData>>,
}

impl UnattachedUnionType {
    fn to_child(self) -> ChildUnionType {
        ChildUnionType { data: self.data }
    }
}

#[derive(Debug, Clone)]
struct ChildUnionType {
    data: Rc<RefCell<UnionTypeData>>,
}

impl ChildUnionType {
    fn downgrade(&self) -> UnionType {
        UnionType {
            data: Rc::downgrade(&self.data),
        }
    }
}

#[derive(Debug)]
struct UnionTypeData {
    // This Option should always be present post-construction.
    self_weak: Option<UnionType>,
    parent: Option<WeakSchema>,
    source_ast: Option<AstNodeEnum>,
    description: Option<Rc<str>>,
}

impl UnionType {
    fn _private_upgrade(&self) -> Rc<RefCell<UnionTypeData>> {
        self.data
            .upgrade()
            .expect("Element has been removed or owning schema has been dropped.")
    }
}

impl NamedType for UnionType {
    type TReferencer = UnionTypeReferencer;

    fn _protected_referencers_borrow<
        T,
        F: FnOnce(Ref<CachedLinkedHashSet<Self::TReferencer>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_referencers_borrow_mut<
        T,
        F: FnOnce(RefMut<CachedLinkedHashSet<Self::TReferencer>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_name_borrow_mut<T, F: FnOnce(RefMut<Rc<str>>) -> T>(&self, f: F) -> T {
        todo!()
    }
}

impl ExtendableElement for UnionType {
    fn _protected_extensions_borrow<T, F: FnOnce(Ref<CachedLinkedHashSet<ChildExtension>>) -> T>(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_extensions_borrow_mut<
        T,
        F: FnOnce(RefMut<CachedLinkedHashSet<ChildExtension>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_has_non_extension_inner_elements(&self) -> bool {
        todo!()
    }
}

impl SchemaElement for UnionType {
    fn _protected_applied_directives_borrow<
        T,
        F: FnOnce(Ref<CachedLinkedHashSet<ChildDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_applied_directives_borrow_mut<
        T,
        F: FnOnce(RefMut<CachedLinkedHashSet<ChildDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_unapplied_directives_borrow<T, F: FnOnce(Ref<Vec<UnappliedDirective>>) -> T>(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_unapplied_directives_borrow_mut<
        T,
        F: FnOnce(RefMut<Vec<UnappliedDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }
}

impl Element for UnionType {
    type TParent = WeakSchema;

    fn _protected_parent_borrow<T, F: FnOnce(Ref<Option<Self::TParent>>) -> T>(&self, f: F) -> T {
        self._private_upgrade()
            .with_borrow(|self_ref| f(Ref::map(self_ref, |e| &e.parent)))
    }

    fn _protected_parent_borrow_mut<T, F: FnOnce(RefMut<Option<Self::TParent>>) -> T>(
        &self,
        f: F,
    ) -> T {
        self._private_upgrade()
            .with_borrow_mut(|self_refmut| f(RefMut::map(self_refmut, |e| &mut e.parent)))
    }
}

impl HasKind for UnionType {
    fn kind(&self) -> NamedTypeKindEnum {
        NamedTypeKindEnum::UnionType
    }
}

impl HasParent for UnionType {
    type TParent = Schema;

    fn parent(&self) -> Self::TParent {
        self._protected_parent().upgrade()
    }
}

impl SetParent for UnionType {
    type TParent = WeakSchema;

    fn set_parent(&self, parent: Self::TParent) {
        self._protected_set_parent(parent)
    }
}

impl HasParentSchema for UnionType {
    fn parent_schema(&self) -> Schema {
        self.parent()
    }
}

impl SetParentWeakSchema for UnionType {
    fn set_parent_weak_schema(&self, parent: WeakSchema) {
        self.set_parent(parent)
    }
}

impl IsAttached for UnionType {
    fn is_attached(&self) -> bool {
        self._protected_is_attached()
    }
}

impl HasSchema for UnionType {
    fn schema(&self) -> Schema {
        self._protected_schema()
    }
}

impl HasWeakSchema for UnionType {
    fn weak_schema(&self) -> WeakSchema {
        self._protected_weak_schema()
    }

    fn weak_schema_if_attached(&self) -> Option<WeakSchema> {
        self._protected_weak_schema_if_attached()
    }
}

impl HasSourceAst for UnionType {
    fn source_ast(&self) -> Option<AstNodeEnum> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.source_ast.clone())
    }
}

impl SetSourceAst for UnionType {
    fn set_source_ast(&self, source_ast: Option<AstNodeEnum>) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.source_ast = source_ast;
        })
    }
}

impl HasDescription for UnionType {
    fn description(&self) -> Option<Rc<str>> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.description.clone())
    }

    fn set_description(&self, description: Option<Rc<str>>) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.description = description;
        })
    }
}

impl SchemaElementHasParent for UnionType {
    fn schema_element_parent(&self) -> SchemaElementParentEnum {
        self.parent().into()
    }
}

impl IsBuiltIn for UnionType {
    fn is_built_in(&self) -> bool {
        todo!()
    }
}

impl AddUnappliedDirectives for UnionType {
    fn add_unapplied_directive(&self, directive: UnappliedDirective) {
        self._protected_add_unapplied_directive(directive)
    }

    fn process_unapplied_directives(&self) -> Result<(), FederationError> {
        self._protected_process_unapplied_directives()
    }
}

impl HasAppliedDirectives for UnionType {
    fn applied_directives(&self) -> Vec<Directive> {
        self._protected_applied_directives()
    }

    fn applied_directives_of(&self, definition: &DirectiveDefinition) -> Vec<Directive> {
        self._protected_applied_directives_of(definition)
    }

    fn applied_directives_of_name(&self, name: &str) -> Vec<Directive> {
        self._protected_applied_directives_of_name(name)
    }

    fn has_applied_directive(&self, definition: &DirectiveDefinition) -> bool {
        self._protected_has_applied_directive(definition)
    }

    fn has_applied_directive_name(&self, name: &str) -> bool {
        self._protected_has_applied_directive_name(name)
    }
}

impl AddAppliedDirectives for UnionType {
    fn apply_directive(&self, definition: DirectiveDefinition, args: Option<Rc<InsertOnlyIndexMap<ValueEnum>>>, as_first_directive: Option<bool>) -> Directive {
        self._protected_apply_directive(definition, args, as_first_directive)
    }

    fn apply_directive_name(&self, name: Rc<str>, args: Option<Rc<InsertOnlyIndexMap<ValueEnum>>>, as_first_directive: Option<bool>) -> Result<Directive, FederationError> {
        self._protected_apply_directive_name(name, args, as_first_directive)
    }
}

impl PreserveEmptyDefinition for UnionType {
    fn preserve_empty_definition(&self) -> bool {
        todo!()
    }

    fn set_preserve_empty_definition(&self, preserve_empty_definition: bool) {
        todo!()
    }
}

impl HasExtensions for UnionType {
    fn extensions(&self) -> Vec<Extension> {
        self._protected_extensions()
    }

    fn has_extension(&self, extension: &Extension) -> bool {
        self._protected_has_extension(extension)
    }

    fn has_extension_elements(&self) -> bool {
        self._protected_has_extension_elements()
    }

    fn has_non_extension_elements(&self) -> bool {
        self._protected_has_non_extension_elements()
    }
}

impl AddExtensions for UnionType {
    fn new_extension(&self) -> Extension {
        self._protected_new_extension()
    }

    fn add_extension(&self, extension: UnattachedExtension) -> Extension {
        self._protected_add_extension(extension)
    }
}

impl HasName for UnionType {
    fn name(&self) -> Rc<str> {
        todo!()
    }
}

impl HasCoordinate for UnionType {
    fn coordinate(&self) -> String {
        todo!()
    }
}

impl HasReferencers for UnionType {
    type TReferencer = UnionTypeReferencer;

    fn referencers(&self) -> Rc<[Self::TReferencer]> {
        self._protected_referencers()
    }

    fn is_referenced(&self) -> bool {
        self._protected_is_referenced()
    }
}

impl Rename for UnionType {
    fn rename(&self, new_name: Rc<str>) {
        self._protected_rename(new_name)
    }
}

impl SealedTrait for UnionType {}

impl Display for UnionType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl FederationDisplay for UnionType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

#[derive(Debug, Clone)]
pub struct EnumType {
    data: Weak<RefCell<EnumTypeData>>,
}

impl PartialEq for EnumType {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.data, &other.data)
    }
}

impl Eq for EnumType {}

impl Hash for EnumType {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Weak::as_ptr(&self.data).hash(state)
    }
}

// We purposely do not derive Clone to prevent Rc references from leaking.
#[derive(Debug)]
pub struct UnattachedEnumType {
    data: Rc<RefCell<EnumTypeData>>,
}

impl UnattachedEnumType {
    fn to_child(self) -> ChildEnumType {
        ChildEnumType { data: self.data }
    }
}

#[derive(Debug, Clone)]
struct ChildEnumType {
    data: Rc<RefCell<EnumTypeData>>,
}

impl ChildEnumType {
    fn downgrade(&self) -> EnumType {
        EnumType {
            data: Rc::downgrade(&self.data),
        }
    }
}

#[derive(Debug)]
struct EnumTypeData {
    // This Option should always be present post-construction.
    self_weak: Option<EnumType>,
    parent: Option<WeakSchema>,
    source_ast: Option<AstNodeEnum>,
    description: Option<Rc<str>>,
}

impl EnumType {
    fn _private_upgrade(&self) -> Rc<RefCell<EnumTypeData>> {
        self.data
            .upgrade()
            .expect("Element has been removed or owning schema has been dropped.")
    }
}

impl NamedType for EnumType {
    type TReferencer = EnumTypeReferencer;

    fn _protected_referencers_borrow<
        T,
        F: FnOnce(Ref<CachedLinkedHashSet<Self::TReferencer>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_referencers_borrow_mut<
        T,
        F: FnOnce(RefMut<CachedLinkedHashSet<Self::TReferencer>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_name_borrow_mut<T, F: FnOnce(RefMut<Rc<str>>) -> T>(&self, f: F) -> T {
        todo!()
    }
}

impl ExtendableElement for EnumType {
    fn _protected_extensions_borrow<T, F: FnOnce(Ref<CachedLinkedHashSet<ChildExtension>>) -> T>(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_extensions_borrow_mut<
        T,
        F: FnOnce(RefMut<CachedLinkedHashSet<ChildExtension>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_has_non_extension_inner_elements(&self) -> bool {
        todo!()
    }
}

impl SchemaElement for EnumType {
    fn _protected_applied_directives_borrow<
        T,
        F: FnOnce(Ref<CachedLinkedHashSet<ChildDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_applied_directives_borrow_mut<
        T,
        F: FnOnce(RefMut<CachedLinkedHashSet<ChildDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_unapplied_directives_borrow<T, F: FnOnce(Ref<Vec<UnappliedDirective>>) -> T>(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_unapplied_directives_borrow_mut<
        T,
        F: FnOnce(RefMut<Vec<UnappliedDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }
}

impl Element for EnumType {
    type TParent = WeakSchema;

    fn _protected_parent_borrow<T, F: FnOnce(Ref<Option<Self::TParent>>) -> T>(&self, f: F) -> T {
        self._private_upgrade()
            .with_borrow(|self_ref| f(Ref::map(self_ref, |e| &e.parent)))
    }

    fn _protected_parent_borrow_mut<T, F: FnOnce(RefMut<Option<Self::TParent>>) -> T>(
        &self,
        f: F,
    ) -> T {
        self._private_upgrade()
            .with_borrow_mut(|self_refmut| f(RefMut::map(self_refmut, |e| &mut e.parent)))
    }
}

impl HasKind for EnumType {
    fn kind(&self) -> NamedTypeKindEnum {
        NamedTypeKindEnum::EnumType
    }
}

impl HasParent for EnumType {
    type TParent = Schema;

    fn parent(&self) -> Self::TParent {
        self._protected_parent().upgrade()
    }
}

impl SetParent for EnumType {
    type TParent = WeakSchema;

    fn set_parent(&self, parent: Self::TParent) {
        self._protected_set_parent(parent)
    }
}

impl HasParentSchema for EnumType {
    fn parent_schema(&self) -> Schema {
        self.parent()
    }
}

impl SetParentWeakSchema for EnumType {
    fn set_parent_weak_schema(&self, parent: WeakSchema) {
        self.set_parent(parent)
    }
}

impl IsAttached for EnumType {
    fn is_attached(&self) -> bool {
        self._protected_is_attached()
    }
}

impl HasSchema for EnumType {
    fn schema(&self) -> Schema {
        self._protected_schema()
    }
}

impl HasWeakSchema for EnumType {
    fn weak_schema(&self) -> WeakSchema {
        self._protected_weak_schema()
    }

    fn weak_schema_if_attached(&self) -> Option<WeakSchema> {
        self._protected_weak_schema_if_attached()
    }
}

impl HasSourceAst for EnumType {
    fn source_ast(&self) -> Option<AstNodeEnum> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.source_ast.clone())
    }
}

impl SetSourceAst for EnumType {
    fn set_source_ast(&self, source_ast: Option<AstNodeEnum>) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.source_ast = source_ast;
        })
    }
}

impl HasDescription for EnumType {
    fn description(&self) -> Option<Rc<str>> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.description.clone())
    }

    fn set_description(&self, description: Option<Rc<str>>) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.description = description;
        })
    }
}

impl SchemaElementHasParent for EnumType {
    fn schema_element_parent(&self) -> SchemaElementParentEnum {
        self.parent().into()
    }
}

impl IsBuiltIn for EnumType {
    fn is_built_in(&self) -> bool {
        todo!()
    }
}

impl AddUnappliedDirectives for EnumType {
    fn add_unapplied_directive(&self, directive: UnappliedDirective) {
        self._protected_add_unapplied_directive(directive)
    }

    fn process_unapplied_directives(&self) -> Result<(), FederationError> {
        self._protected_process_unapplied_directives()
    }
}

impl HasAppliedDirectives for EnumType {
    fn applied_directives(&self) -> Vec<Directive> {
        self._protected_applied_directives()
    }

    fn applied_directives_of(&self, definition: &DirectiveDefinition) -> Vec<Directive> {
        self._protected_applied_directives_of(definition)
    }

    fn applied_directives_of_name(&self, name: &str) -> Vec<Directive> {
        self._protected_applied_directives_of_name(name)
    }

    fn has_applied_directive(&self, definition: &DirectiveDefinition) -> bool {
        self._protected_has_applied_directive(definition)
    }

    fn has_applied_directive_name(&self, name: &str) -> bool {
        self._protected_has_applied_directive_name(name)
    }
}

impl AddAppliedDirectives for EnumType {
    fn apply_directive(&self, definition: DirectiveDefinition, args: Option<Rc<InsertOnlyIndexMap<ValueEnum>>>, as_first_directive: Option<bool>) -> Directive {
        self._protected_apply_directive(definition, args, as_first_directive)
    }

    fn apply_directive_name(&self, name: Rc<str>, args: Option<Rc<InsertOnlyIndexMap<ValueEnum>>>, as_first_directive: Option<bool>) -> Result<Directive, FederationError> {
        self._protected_apply_directive_name(name, args, as_first_directive)
    }
}

impl PreserveEmptyDefinition for EnumType {
    fn preserve_empty_definition(&self) -> bool {
        todo!()
    }

    fn set_preserve_empty_definition(&self, preserve_empty_definition: bool) {
        todo!()
    }
}

impl HasExtensions for EnumType {
    fn extensions(&self) -> Vec<Extension> {
        self._protected_extensions()
    }

    fn has_extension(&self, extension: &Extension) -> bool {
        self._protected_has_extension(extension)
    }

    fn has_extension_elements(&self) -> bool {
        self._protected_has_extension_elements()
    }

    fn has_non_extension_elements(&self) -> bool {
        self._protected_has_non_extension_elements()
    }
}

impl AddExtensions for EnumType {
    fn new_extension(&self) -> Extension {
        self._protected_new_extension()
    }

    fn add_extension(&self, extension: UnattachedExtension) -> Extension {
        self._protected_add_extension(extension)
    }
}

impl HasName for EnumType {
    fn name(&self) -> Rc<str> {
        todo!()
    }
}

impl HasCoordinate for EnumType {
    fn coordinate(&self) -> String {
        todo!()
    }
}

impl HasReferencers for EnumType {
    type TReferencer = EnumTypeReferencer;

    fn referencers(&self) -> Rc<[Self::TReferencer]> {
        self._protected_referencers()
    }

    fn is_referenced(&self) -> bool {
        self._protected_is_referenced()
    }
}

impl Rename for EnumType {
    fn rename(&self, new_name: Rc<str>) {
        self._protected_rename(new_name)
    }
}

impl SealedTrait for EnumType {}

impl Display for EnumType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl FederationDisplay for EnumType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

#[derive(Debug, Clone)]
pub struct InputObjectType {
    data: Weak<RefCell<InputObjectTypeData>>,
}

impl PartialEq for InputObjectType {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.data, &other.data)
    }
}

impl Eq for InputObjectType {}

impl Hash for InputObjectType {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Weak::as_ptr(&self.data).hash(state)
    }
}

// We purposely do not derive Clone to prevent Rc references from leaking.
#[derive(Debug)]
pub struct UnattachedInputObjectType {
    data: Rc<RefCell<InputObjectTypeData>>,
}

impl UnattachedInputObjectType {
    fn to_child(self) -> ChildInputObjectType {
        ChildInputObjectType { data: self.data }
    }
}

#[derive(Debug, Clone)]
struct ChildInputObjectType {
    data: Rc<RefCell<InputObjectTypeData>>,
}

impl ChildInputObjectType {
    fn downgrade(&self) -> InputObjectType {
        InputObjectType {
            data: Rc::downgrade(&self.data),
        }
    }
}

#[derive(Debug)]
struct InputObjectTypeData {
    // This Option should always be present post-construction.
    self_weak: Option<InputObjectType>,
    parent: Option<WeakSchema>,
    source_ast: Option<AstNodeEnum>,
    description: Option<Rc<str>>,
}

impl InputObjectType {
    fn _private_upgrade(&self) -> Rc<RefCell<InputObjectTypeData>> {
        self.data
            .upgrade()
            .expect("Element has been removed or owning schema has been dropped.")
    }
}

impl NamedType for InputObjectType {
    type TReferencer = InputObjectTypeReferencer;

    fn _protected_referencers_borrow<
        T,
        F: FnOnce(Ref<CachedLinkedHashSet<Self::TReferencer>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_referencers_borrow_mut<
        T,
        F: FnOnce(RefMut<CachedLinkedHashSet<Self::TReferencer>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_name_borrow_mut<T, F: FnOnce(RefMut<Rc<str>>) -> T>(&self, f: F) -> T {
        todo!()
    }
}

impl ExtendableElement for InputObjectType {
    fn _protected_extensions_borrow<T, F: FnOnce(Ref<CachedLinkedHashSet<ChildExtension>>) -> T>(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_extensions_borrow_mut<
        T,
        F: FnOnce(RefMut<CachedLinkedHashSet<ChildExtension>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_has_non_extension_inner_elements(&self) -> bool {
        todo!()
    }
}

impl SchemaElement for InputObjectType {
    fn _protected_applied_directives_borrow<
        T,
        F: FnOnce(Ref<CachedLinkedHashSet<ChildDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_applied_directives_borrow_mut<
        T,
        F: FnOnce(RefMut<CachedLinkedHashSet<ChildDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_unapplied_directives_borrow<T, F: FnOnce(Ref<Vec<UnappliedDirective>>) -> T>(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_unapplied_directives_borrow_mut<
        T,
        F: FnOnce(RefMut<Vec<UnappliedDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }
}

impl Element for InputObjectType {
    type TParent = WeakSchema;

    fn _protected_parent_borrow<T, F: FnOnce(Ref<Option<Self::TParent>>) -> T>(&self, f: F) -> T {
        self._private_upgrade()
            .with_borrow(|self_ref| f(Ref::map(self_ref, |e| &e.parent)))
    }

    fn _protected_parent_borrow_mut<T, F: FnOnce(RefMut<Option<Self::TParent>>) -> T>(
        &self,
        f: F,
    ) -> T {
        self._private_upgrade()
            .with_borrow_mut(|self_refmut| f(RefMut::map(self_refmut, |e| &mut e.parent)))
    }
}

impl HasKind for InputObjectType {
    fn kind(&self) -> NamedTypeKindEnum {
        NamedTypeKindEnum::InputObjectType
    }
}

impl HasParent for InputObjectType {
    type TParent = Schema;

    fn parent(&self) -> Self::TParent {
        self._protected_parent().upgrade()
    }
}

impl SetParent for InputObjectType {
    type TParent = WeakSchema;

    fn set_parent(&self, parent: Self::TParent) {
        self._protected_set_parent(parent)
    }
}

impl HasParentSchema for InputObjectType {
    fn parent_schema(&self) -> Schema {
        self.parent()
    }
}

impl SetParentWeakSchema for InputObjectType {
    fn set_parent_weak_schema(&self, parent: WeakSchema) {
        self.set_parent(parent)
    }
}

impl IsAttached for InputObjectType {
    fn is_attached(&self) -> bool {
        self._protected_is_attached()
    }
}

impl HasSchema for InputObjectType {
    fn schema(&self) -> Schema {
        self._protected_schema()
    }
}

impl HasWeakSchema for InputObjectType {
    fn weak_schema(&self) -> WeakSchema {
        self._protected_weak_schema()
    }

    fn weak_schema_if_attached(&self) -> Option<WeakSchema> {
        self._protected_weak_schema_if_attached()
    }
}

impl HasSourceAst for InputObjectType {
    fn source_ast(&self) -> Option<AstNodeEnum> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.source_ast.clone())
    }
}

impl SetSourceAst for InputObjectType {
    fn set_source_ast(&self, source_ast: Option<AstNodeEnum>) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.source_ast = source_ast;
        })
    }
}

impl HasDescription for InputObjectType {
    fn description(&self) -> Option<Rc<str>> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.description.clone())
    }

    fn set_description(&self, description: Option<Rc<str>>) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.description = description;
        })
    }
}

impl SchemaElementHasParent for InputObjectType {
    fn schema_element_parent(&self) -> SchemaElementParentEnum {
        self.parent().into()
    }
}

impl IsBuiltIn for InputObjectType {
    fn is_built_in(&self) -> bool {
        todo!()
    }
}

impl AddUnappliedDirectives for InputObjectType {
    fn add_unapplied_directive(&self, directive: UnappliedDirective) {
        self._protected_add_unapplied_directive(directive)
    }

    fn process_unapplied_directives(&self) -> Result<(), FederationError> {
        self._protected_process_unapplied_directives()
    }
}

impl HasAppliedDirectives for InputObjectType {
    fn applied_directives(&self) -> Vec<Directive> {
        self._protected_applied_directives()
    }

    fn applied_directives_of(&self, definition: &DirectiveDefinition) -> Vec<Directive> {
        self._protected_applied_directives_of(definition)
    }

    fn applied_directives_of_name(&self, name: &str) -> Vec<Directive> {
        self._protected_applied_directives_of_name(name)
    }

    fn has_applied_directive(&self, definition: &DirectiveDefinition) -> bool {
        self._protected_has_applied_directive(definition)
    }

    fn has_applied_directive_name(&self, name: &str) -> bool {
        self._protected_has_applied_directive_name(name)
    }
}

impl AddAppliedDirectives for InputObjectType {
    fn apply_directive(&self, definition: DirectiveDefinition, args: Option<Rc<InsertOnlyIndexMap<ValueEnum>>>, as_first_directive: Option<bool>) -> Directive {
        self._protected_apply_directive(definition, args, as_first_directive)
    }

    fn apply_directive_name(&self, name: Rc<str>, args: Option<Rc<InsertOnlyIndexMap<ValueEnum>>>, as_first_directive: Option<bool>) -> Result<Directive, FederationError> {
        self._protected_apply_directive_name(name, args, as_first_directive)
    }
}

impl PreserveEmptyDefinition for InputObjectType {
    fn preserve_empty_definition(&self) -> bool {
        todo!()
    }

    fn set_preserve_empty_definition(&self, preserve_empty_definition: bool) {
        todo!()
    }
}

impl HasExtensions for InputObjectType {
    fn extensions(&self) -> Vec<Extension> {
        self._protected_extensions()
    }

    fn has_extension(&self, extension: &Extension) -> bool {
        self._protected_has_extension(extension)
    }

    fn has_extension_elements(&self) -> bool {
        self._protected_has_extension_elements()
    }

    fn has_non_extension_elements(&self) -> bool {
        self._protected_has_non_extension_elements()
    }
}

impl AddExtensions for InputObjectType {
    fn new_extension(&self) -> Extension {
        self._protected_new_extension()
    }

    fn add_extension(&self, extension: UnattachedExtension) -> Extension {
        self._protected_add_extension(extension)
    }
}

impl HasName for InputObjectType {
    fn name(&self) -> Rc<str> {
        todo!()
    }
}

impl HasCoordinate for InputObjectType {
    fn coordinate(&self) -> String {
        todo!()
    }
}

impl HasReferencers for InputObjectType {
    type TReferencer = InputObjectTypeReferencer;

    fn referencers(&self) -> Rc<[Self::TReferencer]> {
        self._protected_referencers()
    }

    fn is_referenced(&self) -> bool {
        self._protected_is_referenced()
    }
}

impl Rename for InputObjectType {
    fn rename(&self, new_name: Rc<str>) {
        self._protected_rename(new_name)
    }
}

impl SealedTrait for InputObjectType {}

impl Display for InputObjectType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl FederationDisplay for InputObjectType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

// PORT_NOTE: The JS code contained the types "BaseWrapperType", "ListType", and "NonNullType", but
// in the Rust code this has been replaced with enums "OutputTypeEnum", "InputTypeEnum", and
// "TypeEnum" which have variants for "ListType" and "NonNullType". The methods aren't implemented
// directly, but callers will instead have to use base_type() to get an instance of
// "NamedOutputTypeEnum", "InputTypeEnum", or "TypeEnum", which in turn use enum_dispatch.
#[derive(Debug, Clone)]
pub enum OutputTypeEnum {
    ScalarType(ScalarType),
    ObjectType(ObjectType),
    InterfaceType(InterfaceType),
    UnionType(UnionType),
    EnumType(EnumType),
    ListType(Box<OutputTypeEnum>),
    NonNullType(Box<OutputTypeEnum>),
}

impl OutputTypeEnum {
    pub fn base_type(&self) -> NamedOutputTypeEnum {
        match self {
            OutputTypeEnum::ScalarType(base) => base.clone().into(),
            OutputTypeEnum::ObjectType(base) => base.clone().into(),
            OutputTypeEnum::InterfaceType(base) => base.clone().into(),
            OutputTypeEnum::UnionType(base) => base.clone().into(),
            OutputTypeEnum::EnumType(base) => base.clone().into(),
            OutputTypeEnum::ListType(base) => base.base_type(),
            OutputTypeEnum::NonNullType(base) => base.base_type(),
        }
    }
}

impl From<ScalarType> for OutputTypeEnum {
    fn from(value: ScalarType) -> Self {
        OutputTypeEnum::ScalarType(value)
    }
}

impl From<ObjectType> for OutputTypeEnum {
    fn from(value: ObjectType) -> Self {
        OutputTypeEnum::ObjectType(value)
    }
}

impl From<InterfaceType> for OutputTypeEnum {
    fn from(value: InterfaceType) -> Self {
        OutputTypeEnum::InterfaceType(value)
    }
}

impl From<UnionType> for OutputTypeEnum {
    fn from(value: UnionType) -> Self {
        OutputTypeEnum::UnionType(value)
    }
}

impl From<EnumType> for OutputTypeEnum {
    fn from(value: EnumType) -> Self {
        OutputTypeEnum::EnumType(value)
    }
}

#[derive(Debug, Clone)]
pub enum InputTypeEnum {
    ScalarType(ScalarType),
    EnumType(EnumType),
    InputObjectType(InputObjectType),
    ListType(Box<InputTypeEnum>),
    NonNullType(Box<InputTypeEnum>),
}

impl InputTypeEnum {
    pub fn base_type(&self) -> NamedInputTypeEnum {
        match self {
            InputTypeEnum::ScalarType(base) => base.clone().into(),
            InputTypeEnum::EnumType(base) => base.clone().into(),
            InputTypeEnum::InputObjectType(base) => base.clone().into(),
            InputTypeEnum::ListType(base) => base.base_type(),
            InputTypeEnum::NonNullType(base) => base.base_type(),
        }
    }
}

impl From<ScalarType> for InputTypeEnum {
    fn from(value: ScalarType) -> Self {
        InputTypeEnum::ScalarType(value)
    }
}

impl From<EnumType> for InputTypeEnum {
    fn from(value: EnumType) -> Self {
        InputTypeEnum::EnumType(value)
    }
}

impl From<InputObjectType> for InputTypeEnum {
    fn from(value: InputObjectType) -> Self {
        InputTypeEnum::InputObjectType(value)
    }
}

#[derive(Debug, Clone)]
pub enum TypeEnum {
    ScalarType(ScalarType),
    ObjectType(ObjectType),
    InterfaceType(InterfaceType),
    UnionType(UnionType),
    EnumType(EnumType),
    InputObjectType(InputObjectType),
    ListType(Box<TypeEnum>),
    NonNullType(Box<TypeEnum>),
}

impl TypeEnum {
    pub fn base_type(&self) -> NamedTypeEnum {
        match self {
            TypeEnum::ScalarType(base) => base.clone().into(),
            TypeEnum::ObjectType(base) => base.clone().into(),
            TypeEnum::InterfaceType(base) => base.clone().into(),
            TypeEnum::UnionType(base) => base.clone().into(),
            TypeEnum::EnumType(base) => base.clone().into(),
            TypeEnum::InputObjectType(base) => base.clone().into(),
            TypeEnum::ListType(base) => base.base_type(),
            TypeEnum::NonNullType(base) => base.base_type(),
        }
    }
}

impl From<OutputTypeEnum> for TypeEnum {
    fn from(value: OutputTypeEnum) -> Self {
        match value {
            OutputTypeEnum::ScalarType(base) => {
                base.into()
            }
            OutputTypeEnum::ObjectType(base) => {
                base.into()
            }
            OutputTypeEnum::InterfaceType(base) => {
                base.into()
            }
            OutputTypeEnum::UnionType(base) => {
                base.into()
            }
            OutputTypeEnum::EnumType(base) => {
                base.into()
            }
            OutputTypeEnum::ListType(base) => {
                TypeEnum::ListType(base.deref().into())
            }
            OutputTypeEnum::NonNullType(base) => {
                TypeEnum::NonNullType(base.into())
            }
        }
    }
}

impl From<ScalarType> for TypeEnum {
    fn from(value: ScalarType) -> Self {
        TypeEnum::ScalarType(value)
    }
}

impl From<ObjectType> for TypeEnum {
    fn from(value: ObjectType) -> Self {
        TypeEnum::ObjectType(value)
    }
}

impl From<InterfaceType> for TypeEnum {
    fn from(value: InterfaceType) -> Self {
        TypeEnum::InterfaceType(value)
    }
}

impl From<UnionType> for TypeEnum {
    fn from(value: UnionType) -> Self {
        TypeEnum::UnionType(value)
    }
}

impl From<EnumType> for TypeEnum {
    fn from(value: EnumType) -> Self {
        TypeEnum::EnumType(value)
    }
}

impl From<InputObjectType> for TypeEnum {
    fn from(value: InputObjectType) -> Self {
        TypeEnum::InputObjectType(value)
    }
}

#[derive(Debug, Clone)]
pub struct FieldDefinition {
    data: Weak<RefCell<FieldDefinitionData>>,
}

impl PartialEq for FieldDefinition {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.data, &other.data)
    }
}

impl Eq for FieldDefinition {}

impl Hash for FieldDefinition {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Weak::as_ptr(&self.data).hash(state)
    }
}

// We purposely do not derive Clone to prevent Rc references from leaking.
#[derive(Debug)]
pub struct UnattachedFieldDefinition {
    data: Rc<RefCell<FieldDefinitionData>>,
}

#[derive(Debug, Clone)]
struct ChildFieldDefinition {
    data: Rc<RefCell<FieldDefinitionData>>,
}

#[derive(Debug)]
struct FieldDefinitionData {
    // This Option should always be present post-construction.
    self_weak: Option<FieldDefinition>,
    parent: Option<CompositeTypeEnum>,
    source_ast: Option<AstNodeEnum>,
    description: Option<Rc<str>>,
}

impl FieldDefinition {
    fn _private_upgrade(&self) -> Rc<RefCell<FieldDefinitionData>> {
        self.data
            .upgrade()
            .expect("Element has been removed or owning schema has been dropped.")
    }
}

impl NamedSchemaElementWithType for FieldDefinition {
    type TType = ();
}

impl SchemaElement for FieldDefinition {
    fn _protected_applied_directives_borrow<
        T,
        F: FnOnce(Ref<CachedLinkedHashSet<ChildDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_applied_directives_borrow_mut<
        T,
        F: FnOnce(RefMut<CachedLinkedHashSet<ChildDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_unapplied_directives_borrow<T, F: FnOnce(Ref<Vec<UnappliedDirective>>) -> T>(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_unapplied_directives_borrow_mut<
        T,
        F: FnOnce(RefMut<Vec<UnappliedDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }
}

impl ExtensionMember for FieldDefinition {
    type TExtended = ();
}

impl Element for FieldDefinition {
    type TParent = CompositeTypeEnum;

    fn _protected_parent_borrow<T, F: FnOnce(Ref<Option<Self::TParent>>) -> T>(
        &self,
        f: F,
    ) -> T {
        self._private_upgrade()
            .with_borrow(|self_ref| f(Ref::map(self_ref, |e| &e.parent)))
    }

    fn _protected_parent_borrow_mut<T, F: FnOnce(RefMut<Option<Self::TParent>>) -> T>(
        &self,
        f: F,
    ) -> T {
        self._private_upgrade()
            .with_borrow_mut(|self_refmut| f(RefMut::map(self_refmut, |e| &mut e.parent)))
    }
}

impl HasKind for FieldDefinition {
    fn kind(&self) -> NamedTypeKindEnum {
        NamedTypeKindEnum::FieldDefinition
    }
}

impl HasParent for FieldDefinition {
    type TParent = CompositeTypeEnum;

    fn parent(&self) -> Self::TParent {
        self._protected_parent()
    }
}

impl SetParent for FieldDefinition {
    type TParent = CompositeTypeEnum;

    fn set_parent(&self, parent: Self::TParent) {
        self._protected_set_parent(parent)
    }
}

impl IsAttached for FieldDefinition {
    fn is_attached(&self) -> bool {
        self._protected_is_attached()
    }
}

impl HasSchema for FieldDefinition {
    fn schema(&self) -> Schema {
        self._protected_schema()
    }
}

impl HasWeakSchema for FieldDefinition {
    fn weak_schema(&self) -> WeakSchema {
        self._protected_weak_schema()
    }

    fn weak_schema_if_attached(&self) -> Option<WeakSchema> {
        self._protected_weak_schema_if_attached()
    }
}

impl HasSourceAst for FieldDefinition {
    fn source_ast(&self) -> Option<AstNodeEnum> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.source_ast.clone())
    }
}

impl SetSourceAst for FieldDefinition {
    fn set_source_ast(&self, source_ast: Option<AstNodeEnum>) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.source_ast = source_ast;
        })
    }
}

impl HasDescription for FieldDefinition {
    fn description(&self) -> Option<Rc<str>> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.description.clone())
    }

    fn set_description(&self, description: Option<Rc<str>>) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.description = description;
        })
    }
}

impl SchemaElementHasParent for FieldDefinition {
    fn schema_element_parent(&self) -> SchemaElementParentEnum {
        self.parent().into()
    }
}

impl IsBuiltIn for FieldDefinition {
    fn is_built_in(&self) -> bool {
        self._protected_is_built_in()
    }
}

impl AddUnappliedDirectives for FieldDefinition {
    fn add_unapplied_directive(&self, directive: UnappliedDirective) {
        self._protected_add_unapplied_directive(directive)
    }

    fn process_unapplied_directives(&self) -> Result<(), FederationError> {
        self._protected_process_unapplied_directives()
    }
}

impl HasAppliedDirectives for FieldDefinition {
    fn applied_directives(&self) -> Vec<Directive> {
        self._protected_applied_directives()
    }

    fn applied_directives_of(&self, definition: &DirectiveDefinition) -> Vec<Directive> {
        self._protected_applied_directives_of(definition)
    }

    fn applied_directives_of_name(&self, name: &str) -> Vec<Directive> {
        self._protected_applied_directives_of_name(name)
    }

    fn has_applied_directive(&self, definition: &DirectiveDefinition) -> bool {
        self._protected_has_applied_directive(definition)
    }

    fn has_applied_directive_name(&self, name: &str) -> bool {
        self._protected_has_applied_directive_name(name)
    }
}

impl AddAppliedDirectives for FieldDefinition {
    fn apply_directive(&self, definition: DirectiveDefinition, args: Option<Rc<InsertOnlyIndexMap<ValueEnum>>>, as_first_directive: Option<bool>) -> Directive {
        self._protected_apply_directive(definition, args, as_first_directive)
    }

    fn apply_directive_name(&self, name: Rc<str>, args: Option<Rc<InsertOnlyIndexMap<ValueEnum>>>, as_first_directive: Option<bool>) -> Result<Directive, FederationError> {
        self._protected_apply_directive_name(name, args, as_first_directive)
    }
}

impl HasName for FieldDefinition {
    fn name(&self) -> Rc<str> {
        todo!()
    }
}

impl HasCoordinate for FieldDefinition {
    fn coordinate(&self) -> String {
        todo!()
    }
}

impl SealedTrait for FieldDefinition {}

impl Display for FieldDefinition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl FederationDisplay for FieldDefinition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

#[derive(Debug, Clone)]
pub struct InputFieldDefinition {
    data: Weak<RefCell<InputFieldDefinitionData>>,
}

impl PartialEq for InputFieldDefinition {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.data, &other.data)
    }
}

impl Eq for InputFieldDefinition {}

impl Hash for InputFieldDefinition {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Weak::as_ptr(&self.data).hash(state)
    }
}

// We purposely do not derive Clone to prevent Rc references from leaking.
#[derive(Debug)]
pub struct UnattachedInputFieldDefinition {
    data: Rc<RefCell<InputFieldDefinitionData>>,
}

#[derive(Debug, Clone)]
struct ChildInputFieldDefinition {
    data: Rc<RefCell<InputFieldDefinitionData>>,
}

#[derive(Debug)]
struct InputFieldDefinitionData {
    // This Option should always be present post-construction.
    self_weak: Option<InputFieldDefinition>,
    parent: Option<InputObjectType>,
    source_ast: Option<AstNodeEnum>,
    description: Option<Rc<str>>,
}

impl InputFieldDefinition {
    fn _private_upgrade(&self) -> Rc<RefCell<InputFieldDefinitionData>> {
        self.data
            .upgrade()
            .expect("Element has been removed or owning schema has been dropped.")
    }
}

impl NamedSchemaElementWithType for InputFieldDefinition {
    type TType = ();
}

impl SchemaElement for InputFieldDefinition {
    fn _protected_applied_directives_borrow<
        T,
        F: FnOnce(Ref<CachedLinkedHashSet<ChildDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_applied_directives_borrow_mut<
        T,
        F: FnOnce(RefMut<CachedLinkedHashSet<ChildDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_unapplied_directives_borrow<T, F: FnOnce(Ref<Vec<UnappliedDirective>>) -> T>(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_unapplied_directives_borrow_mut<
        T,
        F: FnOnce(RefMut<Vec<UnappliedDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }
}

impl ExtensionMember for InputFieldDefinition {
    type TExtended = ();
}

impl Element for InputFieldDefinition {
    type TParent = InputObjectType;

    fn _protected_parent_borrow<T, F: FnOnce(Ref<Option<Self::TParent>>) -> T>(&self, f: F) -> T {
        self._private_upgrade()
            .with_borrow(|self_ref| f(Ref::map(self_ref, |e| &e.parent)))
    }

    fn _protected_parent_borrow_mut<T, F: FnOnce(RefMut<Option<Self::TParent>>) -> T>(
        &self,
        f: F,
    ) -> T {
        self._private_upgrade()
            .with_borrow_mut(|self_refmut| f(RefMut::map(self_refmut, |e| &mut e.parent)))
    }
}

impl HasKind for InputFieldDefinition {
    fn kind(&self) -> NamedTypeKindEnum {
        NamedTypeKindEnum::InputFieldDefinition
    }
}

impl HasParent for InputFieldDefinition {
    type TParent = InputObjectType;

    fn parent(&self) -> Self::TParent {
        self._protected_parent()
    }
}

impl SetParent for InputFieldDefinition {
    type TParent = InputObjectType;

    fn set_parent(&self, parent: Self::TParent) {
        self._protected_set_parent(parent)
    }
}

impl IsAttached for InputFieldDefinition {
    fn is_attached(&self) -> bool {
        self._protected_is_attached()
    }
}

impl HasSchema for InputFieldDefinition {
    fn schema(&self) -> Schema {
        self._protected_schema()
    }
}

impl HasWeakSchema for InputFieldDefinition {
    fn weak_schema(&self) -> WeakSchema {
        self._protected_weak_schema()
    }

    fn weak_schema_if_attached(&self) -> Option<WeakSchema> {
        self._protected_weak_schema_if_attached()
    }
}

impl HasSourceAst for InputFieldDefinition {
    fn source_ast(&self) -> Option<AstNodeEnum> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.source_ast.clone())
    }
}

impl HasDescription for InputFieldDefinition {
    fn description(&self) -> Option<Rc<str>> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.description.clone())
    }

    fn set_description(&self, description: Option<Rc<str>>) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.description = description;
        })
    }
}

impl SetSourceAst for InputFieldDefinition {
    fn set_source_ast(&self, source_ast: Option<AstNodeEnum>) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.source_ast = source_ast;
        })
    }
}

impl SchemaElementHasParent for InputFieldDefinition {
    fn schema_element_parent(&self) -> SchemaElementParentEnum {
        self.parent().into()
    }
}

impl IsBuiltIn for InputFieldDefinition {
    fn is_built_in(&self) -> bool {
        self._protected_is_built_in()
    }
}

impl AddUnappliedDirectives for InputFieldDefinition {
    fn add_unapplied_directive(&self, directive: UnappliedDirective) {
        self._protected_add_unapplied_directive(directive)
    }

    fn process_unapplied_directives(&self) -> Result<(), FederationError> {
        self._protected_process_unapplied_directives()
    }
}

impl HasAppliedDirectives for InputFieldDefinition {
    fn applied_directives(&self) -> Vec<Directive> {
        self._protected_applied_directives()
    }

    fn applied_directives_of(&self, definition: &DirectiveDefinition) -> Vec<Directive> {
        self._protected_applied_directives_of(definition)
    }

    fn applied_directives_of_name(&self, name: &str) -> Vec<Directive> {
        self._protected_applied_directives_of_name(name)
    }

    fn has_applied_directive(&self, definition: &DirectiveDefinition) -> bool {
        self._protected_has_applied_directive(definition)
    }

    fn has_applied_directive_name(&self, name: &str) -> bool {
        self._protected_has_applied_directive_name(name)
    }
}

impl AddAppliedDirectives for InputFieldDefinition {
    fn apply_directive(&self, definition: DirectiveDefinition, args: Option<Rc<InsertOnlyIndexMap<ValueEnum>>>, as_first_directive: Option<bool>) -> Directive {
        self._protected_apply_directive(definition, args, as_first_directive)
    }

    fn apply_directive_name(&self, name: Rc<str>, args: Option<Rc<InsertOnlyIndexMap<ValueEnum>>>, as_first_directive: Option<bool>) -> Result<Directive, FederationError> {
        self._protected_apply_directive_name(name, args, as_first_directive)
    }
}

impl HasName for InputFieldDefinition {
    fn name(&self) -> Rc<str> {
        todo!()
    }
}

impl HasCoordinate for InputFieldDefinition {
    fn coordinate(&self) -> String {
        todo!()
    }
}

impl SealedTrait for InputFieldDefinition {}

impl Display for InputFieldDefinition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl FederationDisplay for InputFieldDefinition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

#[derive(Debug, Clone)]
pub struct ArgumentDefinition {
    data: Weak<RefCell<ArgumentDefinitionData>>,
}

impl PartialEq for ArgumentDefinition {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.data, &other.data)
    }
}

impl Eq for ArgumentDefinition {}

impl Hash for ArgumentDefinition {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Weak::as_ptr(&self.data).hash(state)
    }
}

// We purposely do not derive Clone to prevent Rc references from leaking.
#[derive(Debug)]
pub struct UnattachedArgumentDefinition {
    data: Rc<RefCell<ArgumentDefinitionData>>,
}

impl UnattachedArgumentDefinition {
    fn to_child(self) -> ChildArgumentDefinition {
        ChildArgumentDefinition { data: self.data }
    }
}

#[derive(Debug, Clone)]
struct ChildArgumentDefinition {
    data: Rc<RefCell<ArgumentDefinitionData>>,
}

impl ChildArgumentDefinition {
    fn downgrade(&self) -> ArgumentDefinition {
        ArgumentDefinition {
            data: Rc::downgrade(&self.data),
        }
    }
}

#[derive(Debug)]
struct ArgumentDefinitionData {
    self_weak: ArgumentDefinition,
    parent: Option<ArgumentParentElementEnum>,
    source_ast: Option<AstNodeEnum>,
    description: Option<Rc<str>>,
    applied_directives: CachedLinkedHashSet<ChildDirective>,
    unapplied_directives: Vec<UnappliedDirective>,
    name: Rc<str>,
}

impl ArgumentDefinition {
    pub fn new(name: Rc<str>) -> UnattachedArgumentDefinition {
        let self_rc = Rc::new_cyclic(|self_weak| {
            RefCell::new(ArgumentDefinitionData {
                self_weak: ArgumentDefinition {
                    data: self_weak.clone(),
                },
                parent: None,
                source_ast: None,
                description: None,
                applied_directives: CachedLinkedHashSet::new(),
                unapplied_directives: Vec::new(),
                name,
            })
        });
        UnattachedArgumentDefinition { data: self_rc }
    }

    fn _private_upgrade(&self) -> Rc<RefCell<ArgumentDefinitionData>> {
        self.data
            .upgrade()
            .expect("Element has been removed or owning schema has been dropped.")
    }
}

impl NamedSchemaElementWithType for ArgumentDefinition {
    type TType = ();
}

impl SchemaElement for ArgumentDefinition {
    fn _protected_applied_directives_borrow<
        T,
        F: FnOnce(Ref<CachedLinkedHashSet<ChildDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_applied_directives_borrow_mut<
        T,
        F: FnOnce(RefMut<CachedLinkedHashSet<ChildDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_unapplied_directives_borrow<T, F: FnOnce(Ref<Vec<UnappliedDirective>>) -> T>(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_unapplied_directives_borrow_mut<
        T,
        F: FnOnce(RefMut<Vec<UnappliedDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }
}

impl Element for ArgumentDefinition {
    type TParent = ArgumentParentElementEnum;

    fn _protected_parent_borrow<T, F: FnOnce(Ref<Option<Self::TParent>>) -> T>(
        &self,
        f: F,
    ) -> T {
        self._private_upgrade()
            .with_borrow(|self_ref| f(Ref::map(self_ref, |e| &e.parent)))
    }

    fn _protected_parent_borrow_mut<
        T,
        F: FnOnce(RefMut<Option<Self::TParent>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        self._private_upgrade()
            .with_borrow_mut(|self_refmut| f(RefMut::map(self_refmut, |e| &mut e.parent)))
    }
}

impl HasKind for ArgumentDefinition {
    fn kind(&self) -> NamedTypeKindEnum {
        NamedTypeKindEnum::ArgumentDefinition
    }
}

impl HasParent for ArgumentDefinition {
    type TParent = ArgumentParentElementEnum;

    fn parent(&self) -> Self::TParent {
        self._protected_parent()
    }
}

impl SetParent for ArgumentDefinition {
    type TParent = ArgumentParentElementEnum;

    fn set_parent(&self, parent: Self::TParent) {
        self._protected_set_parent(parent)
    }
}

impl IsAttached for ArgumentDefinition {
    fn is_attached(&self) -> bool {
        self._protected_is_attached()
    }
}

impl HasSchema for ArgumentDefinition {
    fn schema(&self) -> Schema {
        self._protected_schema()
    }
}

impl HasWeakSchema for ArgumentDefinition {
    fn weak_schema(&self) -> WeakSchema {
        self._protected_weak_schema()
    }

    fn weak_schema_if_attached(&self) -> Option<WeakSchema> {
        self._protected_weak_schema_if_attached()
    }
}

impl HasSourceAst for ArgumentDefinition {
    fn source_ast(&self) -> Option<AstNodeEnum> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.source_ast.clone())
    }
}

impl SetSourceAst for ArgumentDefinition {
    fn set_source_ast(&self, source_ast: Option<AstNodeEnum>) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.source_ast = source_ast;
        })
    }
}

impl HasDescription for ArgumentDefinition {
    fn description(&self) -> Option<Rc<str>> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.description.clone())
    }

    fn set_description(&self, description: Option<Rc<str>>) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.description = description;
        })
    }
}

impl SchemaElementHasParent for ArgumentDefinition {
    fn schema_element_parent(&self) -> SchemaElementParentEnum {
        self.parent().into()
    }
}

impl IsBuiltIn for ArgumentDefinition {
    fn is_built_in(&self) -> bool {
        self._protected_is_built_in()
    }
}

impl AddUnappliedDirectives for ArgumentDefinition {
    fn add_unapplied_directive(&self, directive: UnappliedDirective) {
        self._protected_add_unapplied_directive(directive)
    }

    fn process_unapplied_directives(&self) -> Result<(), FederationError> {
        self._protected_process_unapplied_directives()
    }
}

impl HasAppliedDirectives for ArgumentDefinition {
    fn applied_directives(&self) -> Vec<Directive> {
        self._protected_applied_directives()
    }

    fn applied_directives_of(&self, definition: &DirectiveDefinition) -> Vec<Directive> {
        self._protected_applied_directives_of(definition)
    }

    fn applied_directives_of_name(&self, name: &str) -> Vec<Directive> {
        self._protected_applied_directives_of_name(name)
    }

    fn has_applied_directive(&self, definition: &DirectiveDefinition) -> bool {
        self._protected_has_applied_directive(definition)
    }

    fn has_applied_directive_name(&self, name: &str) -> bool {
        self._protected_has_applied_directive_name(name)
    }
}

impl AddAppliedDirectives for ArgumentDefinition {
    fn apply_directive(&self, definition: DirectiveDefinition, args: Option<Rc<InsertOnlyIndexMap<ValueEnum>>>, as_first_directive: Option<bool>) -> Directive {
        self._protected_apply_directive(definition, args, as_first_directive)
    }

    fn apply_directive_name(&self, name: Rc<str>, args: Option<Rc<InsertOnlyIndexMap<ValueEnum>>>, as_first_directive: Option<bool>) -> Result<Directive, FederationError> {
        self._protected_apply_directive_name(name, args, as_first_directive)
    }
}

impl HasName for ArgumentDefinition {
    fn name(&self) -> Rc<str> {
        todo!()
    }
}

impl HasCoordinate for ArgumentDefinition {
    fn coordinate(&self) -> String {
        todo!()
    }
}

impl SealedTrait for ArgumentDefinition {}

impl Display for ArgumentDefinition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl FederationDisplay for ArgumentDefinition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

#[derive(Debug, Clone)]
pub struct EnumValue {
    data: Weak<RefCell<EnumValueData>>,
}

impl PartialEq for EnumValue {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.data, &other.data)
    }
}

impl Eq for EnumValue {}

impl Hash for EnumValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Weak::as_ptr(&self.data).hash(state)
    }
}

// We purposely do not derive Clone to prevent Rc references from leaking.
#[derive(Debug)]
pub struct UnattachedEnumValue {
    data: Rc<RefCell<EnumValueData>>,
}

#[derive(Debug, Clone)]
struct ChildEnumValue {
    data: Rc<RefCell<EnumValueData>>,
}

#[derive(Debug)]
struct EnumValueData {
    // This Option should always be present post-construction.
    self_weak: Option<EnumValue>,
    parent: Option<EnumType>,
    source_ast: Option<AstNodeEnum>,
    description: Option<Rc<str>>,
}

impl EnumValue {
    fn _private_upgrade(&self) -> Rc<RefCell<EnumValueData>> {
        self.data
            .upgrade()
            .expect("Element has been removed or owning schema has been dropped.")
    }
}

impl SchemaElement for EnumValue {
    fn _protected_applied_directives_borrow<
        T,
        F: FnOnce(Ref<CachedLinkedHashSet<ChildDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_applied_directives_borrow_mut<
        T,
        F: FnOnce(RefMut<CachedLinkedHashSet<ChildDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_unapplied_directives_borrow<T, F: FnOnce(Ref<Vec<UnappliedDirective>>) -> T>(
        &self,
        f: F,
    ) -> T {
        todo!()
    }

    fn _protected_unapplied_directives_borrow_mut<
        T,
        F: FnOnce(RefMut<Vec<UnappliedDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        todo!()
    }
}

impl ExtensionMember for EnumValue {
    type TExtended = ();
}

impl Element for EnumValue {
    type TParent = EnumType;

    fn _protected_parent_borrow<T, F: FnOnce(Ref<Option<Self::TParent>>) -> T>(&self, f: F) -> T {
        self._private_upgrade()
            .with_borrow(|self_ref| f(Ref::map(self_ref, |e| &e.parent)))
    }

    fn _protected_parent_borrow_mut<T, F: FnOnce(RefMut<Option<Self::TParent>>) -> T>(&self, f: F) -> T {
        self._private_upgrade()
            .with_borrow_mut(|self_refmut| f(RefMut::map(self_refmut, |e| &mut e.parent)))
    }
}

impl HasKind for EnumValue {
    fn kind(&self) -> NamedTypeKindEnum {
        NamedTypeKindEnum::EnumValue
    }
}

impl HasParent for EnumValue {
    type TParent = EnumType;

    fn parent(&self) -> Self::TParent {
        self._protected_parent()
    }
}

impl SetParent for EnumValue {
    type TParent = EnumType;

    fn set_parent(&self, parent: Self::TParent) {
        self._protected_set_parent(parent)
    }
}

impl IsAttached for EnumValue {
    fn is_attached(&self) -> bool {
        self._protected_is_attached()
    }
}

impl HasSchema for EnumValue {
    fn schema(&self) -> Schema {
        self._protected_schema()
    }
}

impl HasWeakSchema for EnumValue {
    fn weak_schema(&self) -> WeakSchema {
        self._protected_weak_schema()
    }

    fn weak_schema_if_attached(&self) -> Option<WeakSchema> {
        self._protected_weak_schema_if_attached()
    }
}

impl HasSourceAst for EnumValue {
    fn source_ast(&self) -> Option<AstNodeEnum> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.source_ast.clone())
    }
}

impl SetSourceAst for EnumValue {
    fn set_source_ast(&self, source_ast: Option<AstNodeEnum>) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.source_ast = source_ast;
        })
    }
}

impl HasDescription for EnumValue {
    fn description(&self) -> Option<Rc<str>> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.description.clone())
    }

    fn set_description(&self, description: Option<Rc<str>>) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.description = description;
        })
    }
}

impl SchemaElementHasParent for EnumValue {
    fn schema_element_parent(&self) -> SchemaElementParentEnum {
        self.parent().into()
    }
}

impl IsBuiltIn for EnumValue {
    fn is_built_in(&self) -> bool {
        self._protected_is_built_in()
    }
}

impl AddUnappliedDirectives for EnumValue {
    fn add_unapplied_directive(&self, directive: UnappliedDirective) {
        self._protected_add_unapplied_directive(directive)
    }

    fn process_unapplied_directives(&self) -> Result<(), FederationError> {
        self._protected_process_unapplied_directives()
    }
}

impl HasAppliedDirectives for EnumValue {
    fn applied_directives(&self) -> Vec<Directive> {
        self._protected_applied_directives()
    }

    fn applied_directives_of(&self, definition: &DirectiveDefinition) -> Vec<Directive> {
        self._protected_applied_directives_of(definition)
    }

    fn applied_directives_of_name(&self, name: &str) -> Vec<Directive> {
        self._protected_applied_directives_of_name(name)
    }

    fn has_applied_directive(&self, definition: &DirectiveDefinition) -> bool {
        self._protected_has_applied_directive(definition)
    }

    fn has_applied_directive_name(&self, name: &str) -> bool {
        self._protected_has_applied_directive_name(name)
    }
}

impl AddAppliedDirectives for EnumValue {
    fn apply_directive(&self, definition: DirectiveDefinition, args: Option<Rc<InsertOnlyIndexMap<ValueEnum>>>, as_first_directive: Option<bool>) -> Directive {
        self._protected_apply_directive(definition, args, as_first_directive)
    }

    fn apply_directive_name(&self, name: Rc<str>, args: Option<Rc<InsertOnlyIndexMap<ValueEnum>>>, as_first_directive: Option<bool>) -> Result<Directive, FederationError> {
        self._protected_apply_directive_name(name, args, as_first_directive)
    }
}

impl HasName for EnumValue {
    fn name(&self) -> Rc<str> {
        todo!()
    }
}

impl HasCoordinate for EnumValue {
    fn coordinate(&self) -> String {
        todo!()
    }
}

impl SealedTrait for EnumValue {}

impl Display for EnumValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl FederationDisplay for EnumValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

#[derive(Debug, Clone)]
pub struct DirectiveDefinition {
    data: Weak<RefCell<DirectiveDefinitionData>>,
}

impl PartialEq for DirectiveDefinition {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.data, &other.data)
    }
}

impl Eq for DirectiveDefinition {}

impl Hash for DirectiveDefinition {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Weak::as_ptr(&self.data).hash(state)
    }
}

// We purposely do not derive Clone to prevent Rc references from leaking.
#[derive(Debug)]
pub struct UnattachedDirectiveDefinition {
    data: Rc<RefCell<DirectiveDefinitionData>>,
}

#[derive(Debug, Clone)]
struct ChildDirectiveDefinition {
    data: Rc<RefCell<DirectiveDefinitionData>>,
}

// PORT_NOTE: We purposely omit the type parameter "TApplicationArgs" from this trait, as it was
// usually "any" or "{[key: string]: any}" in JS code.
#[derive(Debug)]
struct DirectiveDefinitionData {
    self_weak: DirectiveDefinition,
    parent: Option<WeakSchema>,
    source_ast: Option<AstNodeEnum>,
    description: Option<Rc<str>>,
    applied_directives: CachedLinkedHashSet<ChildDirective>,
    unapplied_directives: Vec<UnappliedDirective>,
    name: Rc<str>,
    is_built_in: bool,
    args: CachedLinkedHashMap<ChildArgumentDefinition>,
    repeatable: bool,
    locations: CachedLinkedHashSet<DirectiveLocationEnum>,
    referencers: CachedLinkedHashSet<Directive>,
}

impl DirectiveDefinition {
    pub fn new(name: Rc<str>, is_built_in: Option<bool>) -> UnattachedDirectiveDefinition {
        let self_rc = Rc::new_cyclic(|self_weak| {
            RefCell::new(DirectiveDefinitionData {
                self_weak: DirectiveDefinition {
                    data: self_weak.clone(),
                },
                parent: None,
                source_ast: None,
                description: None,
                applied_directives: CachedLinkedHashSet::new(),
                unapplied_directives: Vec::new(),
                name,
                is_built_in: is_built_in.unwrap_or(false),
                args: CachedLinkedHashMap::new(),
                repeatable: false,
                locations: CachedLinkedHashSet::new(),
                referencers: CachedLinkedHashSet::new(),
            })
        });
        UnattachedDirectiveDefinition { data: self_rc }
    }

    fn _private_upgrade(&self) -> Rc<RefCell<DirectiveDefinitionData>> {
        self.data
            .upgrade()
            .expect("Element has been removed or owning schema has been dropped.")
    }

    pub fn repeatable(&self) -> bool {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.repeatable.clone())
    }

    pub fn set_repeatable(&self, repeatable: bool) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.repeatable = repeatable;
        })
        // BUG?: Should call _protected_on_modification() here.
    }

    pub fn arguments(&self) -> Vec<ArgumentDefinition> {
        self._private_upgrade().with_borrow(|self_ref| {
            self_ref
                .args
                .cached_values()
                .iter()
                .map(|def| def.downgrade())
                .collect()
        })
    }

    pub fn argument(&self, name: &str) -> Option<ArgumentDefinition> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.args.get(name).map(|def| def.downgrade()))
    }

    // PORT_NOTE: In the JS code, this took either a name or definition. There were no callers for
    // the definition version, so we've omitted it below in Rust.
    pub fn add_argument_name(
        &self,
        name: Rc<str>,
        type_: Option<InputTypeEnum>,
        default_value: Option<ValueEnum>,
    ) -> Result<ArgumentDefinition, FederationError> {
        self._protected_check_update(None);
        let child_arg = ArgumentDefinition::new(name.clone()).to_child();
        let arg = child_arg.downgrade();
        if self.argument(&name).is_some() {
            return Err(ErrorEnum::InvalidGraphQL
                .definition()
                .err(
                    format!("Argument {} already exists on field {}", name, self.name(),),
                    None,
                )
                .into());
        }
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.args.replace(name, child_arg);
        });
        arg.set_parent(self.clone().into());
        // TODO: Add type and default value to arg, if present.
        self._protected_on_modification();
        Ok(arg)
    }

    pub fn locations(&self) -> Rc<[DirectiveLocationEnum]> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.locations.cached_values())
    }

    pub fn add_locations(&self, locations: Vec<DirectiveLocationEnum>) {
        let mut is_modified = false;
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            for l in locations {
                is_modified = is_modified || self_refmut.locations.replace(l);
            }
        });
        if is_modified {
            self._protected_on_modification();
        }
    }

    pub fn remove_locations(&self, locations: &[DirectiveLocationEnum]) {
        let mut is_modified = false;
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            for l in locations {
                is_modified = is_modified || self_refmut.locations.remove(l);
            }
        });
        if is_modified {
            self._protected_on_modification();
        }
    }

    pub fn applications(&self) -> Rc<[Directive]> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.referencers.cached_values())
    }

    // PORT_NOTE: This method was marked private in "addReferencer()" in the JS code, but the
    // codebase was calling it outside the abstract class through
    // "DirectiveDefinition.prototype['addReferencer']". The real reason to mark it "private" was
    // to keep it out of the public API, and accordingly it's marked module-private here.
    fn add_referencer(&self, referencer: Directive) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.referencers.replace(referencer);
        });
    }

    // PORT_NOTE: This method was marked private in "removeReferencer()" in the JS code, but the
    // codebase was calling it outside the abstract class through
    // "DirectiveDefinition.prototype['removeReferencer']". The real reason to mark it "private" was
    // to keep it out of the public API, and accordingly it's marked module-private here.
    fn remove_referencer(&self, referencer: &Directive) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.referencers.remove(referencer);
        });
    }
}

impl SchemaElement for DirectiveDefinition {
    fn _protected_applied_directives_borrow<
        T,
        F: FnOnce(Ref<CachedLinkedHashSet<ChildDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        self._private_upgrade()
            .with_borrow(|self_ref| f(Ref::map(self_ref, |e| &e.applied_directives)))
    }

    fn _protected_applied_directives_borrow_mut<
        T,
        F: FnOnce(RefMut<CachedLinkedHashSet<ChildDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        self._private_upgrade().with_borrow_mut(|self_refmut| {
            f(RefMut::map(self_refmut, |e| &mut e.applied_directives))
        })
    }

    fn _protected_unapplied_directives_borrow<T, F: FnOnce(Ref<Vec<UnappliedDirective>>) -> T>(
        &self,
        f: F,
    ) -> T {
        self._private_upgrade()
            .with_borrow(|self_ref| f(Ref::map(self_ref, |e| &e.unapplied_directives)))
    }

    fn _protected_unapplied_directives_borrow_mut<
        T,
        F: FnOnce(RefMut<Vec<UnappliedDirective>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        self._private_upgrade().with_borrow_mut(|self_refmut| {
            f(RefMut::map(self_refmut, |e| &mut e.unapplied_directives))
        })
    }
}

impl Element for DirectiveDefinition {
    type TParent = WeakSchema;

    fn _protected_parent_borrow<T, F: FnOnce(Ref<Option<Self::TParent>>) -> T>(&self, f: F) -> T {
        self._private_upgrade()
            .with_borrow(|self_ref| f(Ref::map(self_ref, |e| &e.parent)))
    }

    fn _protected_parent_borrow_mut<T, F: FnOnce(RefMut<Option<Self::TParent>>) -> T>(
        &self,
        f: F,
    ) -> T {
        self._private_upgrade()
            .with_borrow_mut(|self_refmut| f(RefMut::map(self_refmut, |e| &mut e.parent)))
    }
}

impl HasKind for DirectiveDefinition {
    fn kind(&self) -> NamedTypeKindEnum {
        NamedTypeKindEnum::DirectiveDefinition
    }
}

impl HasParent for DirectiveDefinition {
    type TParent = Schema;

    fn parent(&self) -> Self::TParent {
        self._protected_parent().upgrade()
    }
}

impl SetParent for DirectiveDefinition {
    type TParent = WeakSchema;

    fn set_parent(&self, parent: Self::TParent) {
        self._protected_set_parent(parent)
    }
}

impl HasParentSchema for DirectiveDefinition {
    fn parent_schema(&self) -> Schema {
        self.parent()
    }
}

impl SetParentWeakSchema for DirectiveDefinition {
    fn set_parent_weak_schema(&self, parent: WeakSchema) {
        self.set_parent(parent)
    }
}

impl IsAttached for DirectiveDefinition {
    fn is_attached(&self) -> bool {
        self._protected_is_attached()
    }
}

impl HasSchema for DirectiveDefinition {
    fn schema(&self) -> Schema {
        self._protected_schema()
    }
}

impl HasWeakSchema for DirectiveDefinition {
    fn weak_schema(&self) -> WeakSchema {
        self._protected_weak_schema()
    }

    fn weak_schema_if_attached(&self) -> Option<WeakSchema> {
        self._protected_weak_schema_if_attached()
    }
}

impl HasSourceAst for DirectiveDefinition {
    fn source_ast(&self) -> Option<AstNodeEnum> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.source_ast.clone())
    }
}

impl SetSourceAst for DirectiveDefinition {
    fn set_source_ast(&self, source_ast: Option<AstNodeEnum>) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.source_ast = source_ast;
        })
    }
}

impl HasDescription for DirectiveDefinition {
    fn description(&self) -> Option<Rc<str>> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.description.clone())
    }

    fn set_description(&self, description: Option<Rc<str>>) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.description = description;
        })
    }
}

impl SchemaElementHasParent for DirectiveDefinition {
    fn schema_element_parent(&self) -> SchemaElementParentEnum {
        self.parent().into()
    }
}

impl HasName for DirectiveDefinition {
    fn name(&self) -> Rc<str> {
        self._private_upgrade()
            .with_borrow(|self_ref| Rc::clone(&self_ref.name))
    }
}

impl HasCoordinate for DirectiveDefinition {
    fn coordinate(&self) -> String {
        "@".to_owned() + &self.name()
    }
}

impl IsBuiltIn for DirectiveDefinition {
    fn is_built_in(&self) -> bool {
        todo!()
    }
}

impl AddUnappliedDirectives for DirectiveDefinition {
    fn add_unapplied_directive(&self, directive: UnappliedDirective) {
        self._protected_add_unapplied_directive(directive)
    }

    fn process_unapplied_directives(&self) -> Result<(), FederationError> {
        self._protected_process_unapplied_directives()
    }
}

impl HasAppliedDirectives for DirectiveDefinition {
    fn applied_directives(&self) -> Vec<Directive> {
        self._protected_applied_directives()
    }

    fn applied_directives_of(&self, definition: &DirectiveDefinition) -> Vec<Directive> {
        self._protected_applied_directives_of(definition)
    }

    fn applied_directives_of_name(&self, name: &str) -> Vec<Directive> {
        self._protected_applied_directives_of_name(name)
    }

    fn has_applied_directive(&self, definition: &DirectiveDefinition) -> bool {
        self._protected_has_applied_directive(definition)
    }

    fn has_applied_directive_name(&self, name: &str) -> bool {
        self._protected_has_applied_directive_name(name)
    }
}

impl AddAppliedDirectives for DirectiveDefinition {
    fn apply_directive(&self, definition: DirectiveDefinition, args: Option<Rc<InsertOnlyIndexMap<ValueEnum>>>, as_first_directive: Option<bool>) -> Directive {
        self._protected_apply_directive(definition, args, as_first_directive)
    }

    fn apply_directive_name(&self, name: Rc<str>, args: Option<Rc<InsertOnlyIndexMap<ValueEnum>>>, as_first_directive: Option<bool>) -> Result<Directive, FederationError> {
        self._protected_apply_directive_name(name, args, as_first_directive)
    }
}

impl SealedTrait for DirectiveDefinition {}

impl Display for DirectiveDefinition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "@{}", &self.name())
    }
}

impl FederationDisplay for DirectiveDefinition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

#[derive(Debug, Clone)]
pub struct Directive {
    data: Weak<RefCell<DirectiveData>>,
}

impl PartialEq for Directive {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.data, &other.data)
    }
}

impl Eq for Directive {}

impl Hash for Directive {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Weak::as_ptr(&self.data).hash(state)
    }
}

// We purposely do not derive Clone to prevent Rc references from leaking.
#[derive(Debug)]
pub struct UnattachedDirective {
    data: Rc<RefCell<DirectiveData>>,
}

impl UnattachedDirective {
    fn to_child(self) -> ChildDirective {
        ChildDirective { data: self.data }
    }
}

// This is public because it appears in a sealed method of a public trait, but there's otherwise no
// reason for this to be public (and its fields and methods are accordingly module-private).
#[derive(Debug, Clone)]
pub(crate) struct ChildDirective {
    data: Rc<RefCell<DirectiveData>>,
}

impl ChildDirective {
    fn downgrade(&self) -> Directive {
        Directive {
            data: Rc::downgrade(&self.data),
        }
    }
}

impl PartialEq for ChildDirective {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.data, &other.data)
    }
}

impl Eq for ChildDirective {}

impl Hash for ChildDirective {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Rc::as_ptr(&self.data).hash(state)
    }
}

// PORT_NOTE: We purposely omit the type parameter "TArgs" from this trait, as it was usually "any"
// or "{[key: string]: any}" in JS code.
#[derive(Debug)]
struct DirectiveData {
    self_weak: Directive,
    parent: Option<DirectiveParentElementEnum>,
    source_ast: Option<AstNodeEnum>,
    extension: Option<Extension>,
    name: Rc<str>,
    args: Rc<InsertOnlyIndexMap<ValueEnum>>,
}

impl Directive {
    pub fn new(name: Rc<str>, args: Rc<InsertOnlyIndexMap<ValueEnum>>) -> UnattachedDirective {
        let self_rc = Rc::new_cyclic(|self_weak| {
            RefCell::new(DirectiveData {
                self_weak: Directive {
                    data: self_weak.clone(),
                },
                parent: None,
                source_ast: None,
                extension: None,
                name,
                args,
            })
        });
        UnattachedDirective { data: self_rc }
    }

    fn _private_upgrade(&self) -> Rc<RefCell<DirectiveData>> {
        self.data
            .upgrade()
            .expect("Element has been removed or owning schema has been dropped.")
    }

    pub fn definition(&self) -> Option<DirectiveDefinition> {
        if self._protected_is_attached() {
            self.schema().directive(&self.name())
        } else {
            None
        }
    }

    // PORT_NOTE: In the JS code, this method took an argument named "includeDefaultValues" that was
    // almost always false (the true case was only ever called by merge logic). So we've split out
    // the case where it's true into "arguments_with_default_values()".
    pub fn arguments(&self) -> Rc<InsertOnlyIndexMap<ValueEnum>> {
        self._private_upgrade()
            .with_borrow(|self_ref| Rc::clone(&self_ref.args))
    }

    pub fn arguments_with_default_values(&self) -> Rc<InsertOnlyIndexMap<ValueEnum>> {
        todo!()
    }

    // PORT_NOTE: For some reason, the JS code called isAttachedToSchemaElement(), which was just a
    // plain wrapper around isAttached(). Not sure why, so removed the wrapping function.
    fn _private_on_modification(&self) {
        if self._protected_is_attached() {
            self.schema().on_modification()
        }
    }

    pub fn set_arguments(&self, args: Rc<InsertOnlyIndexMap<ValueEnum>>) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.args = args;
        });
        self._private_on_modification();
    }

    pub fn of_extension(&self) -> Option<Extension> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.extension.clone())
    }

    pub fn remove_of_extension(&self) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.extension = None;
        });
        // BUG?: Should call _private_on_modification() here.
    }

    pub fn set_of_extension(&self, extension: Option<Extension>) {
        self._protected_check_update_attached();
        if let Some(ref extension_ref) = extension {
            // TODO
        }
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.extension = extension;
        });
        self._private_on_modification();
    }
}

impl ExtensionMember for Directive {
    type TExtended = ();
}

impl Element for Directive {
    type TParent = DirectiveParentElementEnum;

    fn _protected_parent_borrow<T, F: FnOnce(Ref<Option<Self::TParent>>) -> T>(
        &self,
        f: F,
    ) -> T {
        self._private_upgrade()
            .with_borrow(|self_ref| f(Ref::map(self_ref, |e| &e.parent)))
    }

    fn _protected_parent_borrow_mut<
        T,
        F: FnOnce(RefMut<Option<Self::TParent>>) -> T,
    >(
        &self,
        f: F,
    ) -> T {
        self._private_upgrade()
            .with_borrow_mut(|self_refmut| f(RefMut::map(self_refmut, |e| &mut e.parent)))
    }
}

impl HasParent for Directive {
    type TParent = DirectiveParentElementEnum;

    fn parent(&self) -> Self::TParent {
        self._protected_parent()
    }
}

impl SetParent for Directive {
    type TParent = DirectiveParentElementEnum;

    fn set_parent(&self, parent: Self::TParent) {
        self._protected_set_parent(parent)
    }
}

impl IsAttached for Directive {
    fn is_attached(&self) -> bool {
        self._protected_is_attached()
    }
}

impl HasSchema for Directive {
    fn schema(&self) -> Schema {
        self._protected_schema()
    }
}

impl HasWeakSchema for Directive {
    fn weak_schema(&self) -> WeakSchema {
        self._protected_weak_schema()
    }

    fn weak_schema_if_attached(&self) -> Option<WeakSchema> {
        self._protected_weak_schema_if_attached()
    }
}

impl HasSourceAst for Directive {
    fn source_ast(&self) -> Option<AstNodeEnum> {
        self._private_upgrade()
            .with_borrow(|self_ref| self_ref.source_ast.clone())
    }
}

impl SetSourceAst for Directive {
    fn set_source_ast(&self, source_ast: Option<AstNodeEnum>) {
        self._private_upgrade().with_borrow_mut(|mut self_refmut| {
            self_refmut.source_ast = source_ast;
        })
    }
}

impl HasName for Directive {
    fn name(&self) -> Rc<str> {
        self._private_upgrade()
            .with_borrow(|self_ref| Rc::clone(&self_ref.name))
    }
}

impl HasCoordinate for Directive {
    fn coordinate(&self) -> String {
        todo!()
    }
}

impl SealedTrait for Directive {}

impl Display for Directive {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl FederationDisplay for Directive {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

pub(crate) struct Variable {}

#[derive(Debug, Clone)]
pub struct VariableDefinition {
    data: Weak<RefCell<VariableDefinitionData>>,
}

// We purposely do not derive Clone to prevent Rc references from leaking.
#[derive(Debug)]
pub struct UnattachedVariableDefinition {
    data: Rc<RefCell<VariableDefinitionData>>,
}

#[derive(Debug, Clone)]
struct ChildVariableDefinition {
    data: Rc<RefCell<VariableDefinitionData>>,
}

#[derive(Debug)]
struct VariableDefinitionData {}

impl DirectiveParentOperationElement for VariableDefinition {
    fn _protected_applied_directives_borrow<T, F: FnOnce(Ref<Vec<ChildDirective>>) -> T>(
        &self,
        f: F,
        _: SealedMethod,
    ) -> T {
        todo!()
    }

    fn _protected_applied_directives_borrow_mut<T, F: FnOnce(RefMut<Vec<ChildDirective>>) -> T>(
        &self,
        f: F,
        _: SealedMethod,
    ) -> T {
        todo!()
    }
}

impl HasSchema for VariableDefinition {
    fn schema(&self) -> Schema {
        todo!()
    }
}

impl HasWeakSchema for VariableDefinition {
    fn weak_schema(&self) -> WeakSchema {
        self.schema().downgrade()
    }

    fn weak_schema_if_attached(&self) -> Option<WeakSchema> {
        Some(self.weak_schema())
    }
}

impl SealedTrait for VariableDefinition {}

pub(crate) struct VariableDefinitions {}

impl HasWeakSchema for Field {
    fn weak_schema(&self) -> WeakSchema {
        self.schema().downgrade()
    }

    fn weak_schema_if_attached(&self) -> Option<WeakSchema> {
        Some(self.weak_schema())
    }
}

impl HasWeakSchema for FragmentElement {
    fn weak_schema(&self) -> WeakSchema {
        self.schema().downgrade()
    }

    fn weak_schema_if_attached(&self) -> Option<WeakSchema> {
        Some(self.weak_schema())
    }
}

impl HasWeakSchema for NamedFragmentDefinition {
    fn weak_schema(&self) -> WeakSchema {
        self.schema().downgrade()
    }

    fn weak_schema_if_attached(&self) -> Option<WeakSchema> {
        Some(self.weak_schema())
    }
}
