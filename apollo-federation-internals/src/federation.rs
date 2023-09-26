use std::cell::RefCell;
use std::rc::{Rc, Weak};
use crate::definitions::SchemaBlueprint;
use crate::private::SealedTrait;

#[derive(Debug, Clone)]
pub struct FederationBlueprint {
    data: Weak<RefCell<FederationBlueprintData>>,
}

// We purposely do not derive Clone to prevent Rc references from leaking.
#[derive(Debug)]
pub struct UnattachedFederationBlueprint {
    data: Rc<RefCell<FederationBlueprintData>>,
}

impl UnattachedFederationBlueprint {
    pub(crate) fn to_child(self) -> ChildFederationBlueprint {
        ChildFederationBlueprint { data: self.data }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ChildFederationBlueprint {
    data: Rc<RefCell<FederationBlueprintData>>,
}

impl ChildFederationBlueprint {
    pub(crate) fn downgrade(&self) -> FederationBlueprint {
        FederationBlueprint {
            data: Rc::downgrade(&self.data),
        }
    }
}

#[derive(Debug)]
struct FederationBlueprintData;

impl FederationBlueprint {
    pub fn new() -> UnattachedFederationBlueprint {
        UnattachedFederationBlueprint {
            data: Rc::new(RefCell::new(FederationBlueprintData))
        }
    }
}

impl SchemaBlueprint for FederationBlueprint {}

impl SealedTrait for FederationBlueprint {}
