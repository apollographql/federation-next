use crate::definitions::{ChildDirective, DirectiveParentOperationElement, HasSchema, Schema};
use crate::private::{SealedMethod, SealedTrait};
use std::cell::{Ref, RefCell, RefMut};
use std::rc::{Rc, Weak};

// PORT_NOTE: Known as "AbstractOperationElement" in the JS code. This was ostensibly shorthand for
// "the abstract class for operation elements", but most of the JS classes were abstract classes and
// didn't start with the prefix "abstract". My guess is we needed to distinguish it from the JS
// union type "OperationElement", which got renamed in Rust to have the "Enum" suffix. We purposely
// omit the type parameter "T" from this trait, as Rust lets us use Self instead.
pub(crate) trait OperationElement: DirectiveParentOperationElement + SealedTrait {}

#[derive(Debug, Clone)]
pub struct Field {
    data: Weak<RefCell<FieldData>>,
}

// We purposely do not derive Clone to prevent Rc references from leaking.
#[derive(Debug)]
pub struct UnattachedField {
    data: Rc<RefCell<FieldData>>,
}

#[derive(Debug, Clone)]
struct ChildField {
    data: Rc<RefCell<FieldData>>,
}

// PORT_NOTE: We purposely omit the type parameter "TArgs" from this trait, as it was usually "any"
// or "{[key: string]: any}" in JS code.
#[derive(Debug)]
struct FieldData {}

impl OperationElement for Field {}

impl DirectiveParentOperationElement for Field {
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

impl HasSchema for Field {
    fn schema(&self) -> Schema {
        todo!()
    }
}

impl SealedTrait for Field {}

#[derive(Debug, Clone)]
pub struct FragmentElement {
    data: Weak<RefCell<FragmentElementData>>,
}

// We purposely do not derive Clone to prevent Rc references from leaking.
#[derive(Debug)]
pub struct UnattachedFragmentElement {
    data: Rc<RefCell<FragmentElementData>>,
}

#[derive(Debug, Clone)]
struct ChildFragmentElement {
    data: Rc<RefCell<FragmentElementData>>,
}

#[derive(Debug)]
struct FragmentElementData {}

impl OperationElement for FragmentElement {}

impl DirectiveParentOperationElement for FragmentElement {
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

impl HasSchema for FragmentElement {
    fn schema(&self) -> Schema {
        todo!()
    }
}

impl SealedTrait for FragmentElement {}

pub(crate) struct Operation {}

#[derive(Debug, Clone)]
pub struct NamedFragmentDefinition {
    data: Weak<RefCell<NamedFragmentDefinitionData>>,
}


// We purposely do not derive Clone to prevent Rc references from leaking.
#[derive(Debug)]
pub struct UnattachedNamedFragmentDefinition {
    data: Rc<RefCell<NamedFragmentDefinitionData>>,
}

#[derive(Debug, Clone)]
struct ChildNamedFragmentDefinition {
    data: Rc<RefCell<NamedFragmentDefinitionData>>,
}

#[derive(Debug)]
struct NamedFragmentDefinitionData {}

impl DirectiveParentOperationElement for NamedFragmentDefinition {
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

impl HasSchema for NamedFragmentDefinition {
    fn schema(&self) -> Schema {
        todo!()
    }
}

impl SealedTrait for NamedFragmentDefinition {}
