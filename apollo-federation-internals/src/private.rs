// Traits that list this trait as a supertrait cannot be implemented outside of this crate. This
// does not prevent other crates from using such traits in error bounds or calling their methods.
pub trait SealedTrait {}

// Trait methods that list this type as an argument cannot directly be called outside of this crate.
// This does not prevent other crates from calling other functions/methods that call such methods.
// If such methods have no implementation/are required, then the method's entire trait cannot be
// implemented outside of this crate. If all such methods on a trait have default implementations/
// are optional (and the trait does not have a sealed supertrait), then the trait may be implemented
// but such methods may not be overridden. Note this is a Zero Sized Type (ZST), so the runtime cost
// is negligible.
pub struct SealedMethod;
