use apollo_compiler::executable::Name;
use apollo_compiler::NodeStr;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct DeferDirectiveArguments {
    label: Option<NodeStr>,
    if_: Option<BooleanOrVariable>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct OperationConditional {
    pub(crate) kind: OperationConditionalKind,
    pub(crate) value: BooleanOrVariable,
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
pub(crate) enum OperationConditionalKind {
    #[strum(to_string = "include")]
    Include,
    #[strum(to_string = "skip")]
    Skip,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum BooleanOrVariable {
    Boolean(bool),
    Variable(Name),
}
