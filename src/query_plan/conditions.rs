use crate::error::FederationError;
use apollo_compiler::executable::DirectiveList;
use apollo_compiler::executable::Name;
use apollo_compiler::executable::Value;
use indexmap::map::Entry;
use indexmap::IndexMap;
use std::sync::Arc;

/// This struct is meant for tracking whether a selection set in a `FetchDependencyGraphNode` needs
/// to be queried, based on the `@skip`/`@include` applications on the selections within.
/// Accordingly, there is much logic around merging and short-circuiting; `OperationConditional` is
/// the more appropriate struct when trying to record the original structure/intent of those
/// `@skip`/`@include` applications.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Conditions {
    Variables(VariableConditions),
    Boolean(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Condition {
    Variable(VariableCondition),
    Boolean(bool),
}

/// A list of variable conditions, represented as a map from variable names to whether that variable
/// is negated in the condition. We maintain the invariant that there's at least one condition (i.e.
/// the map is non-empty), and that there's at most one condition per variable name.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VariableConditions(Arc<IndexMap<Name, bool>>);

impl VariableConditions {
    /// Construct VariableConditions from a non-empty map of variable names.
    ///
    /// In release builds, this does not check if the map is empty.
    fn new_unchecked(map: IndexMap<Name, bool>) -> Self {
        debug_assert!(!map.is_empty());
        Self(Arc::new(map))
    }

    pub fn insert(&mut self, name: Name, negated: bool) {
        Arc::make_mut(&mut self.0).insert(name, negated);
    }

    /// Returns true if there are no conditions.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns a variable condition by name.
    pub fn get(&self, name: &str) -> Option<VariableCondition> {
        self.0.get(name).map(|&negated| {
            // The name string existing in the map implies that it exists as a
            // `Name` instance, and thus that it's a valid name.
            let variable = Name::new_unchecked(name.into());
            VariableCondition { variable, negated }
        })
    }

    /// Returns whether a variable condition is negated, or None if there is no condition for the variable name.
    pub fn is_negated(&self, name: &str) -> Option<bool> {
        self.0.get(name).copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Name, bool)> {
        self.0.iter().map(|(name, &negated)| (name, negated))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VariableCondition {
    variable: Name,
    negated: bool,
}

impl Conditions {
    /// Create conditions from a map of variable conditions. If empty, instead returns a
    /// condition that always evaluates to true.
    fn from_variables(map: IndexMap<Name, bool>) -> Self {
        if map.is_empty() {
            Self::Boolean(true)
        } else {
            Self::Variables(VariableConditions::new_unchecked(map))
        }
    }

    pub(crate) fn from_directives(directives: &DirectiveList) -> Result<Self, FederationError> {
        let mut variables = IndexMap::new();
        for directive in directives {
            let negated = match directive.name.as_str() {
                "include" => false,
                "skip" => true,
                _ => continue,
            };
            let value = directive.argument_by_name("if").ok_or_else(|| {
                FederationError::internal(format!(
                    "missing if argument on @{}",
                    if negated { "skip" } else { "include" },
                ))
            })?;
            match &**value {
                Value::Boolean(false) if !negated => return Ok(Self::Boolean(false)),
                Value::Boolean(true) if negated => return Ok(Self::Boolean(false)),
                Value::Boolean(_) => {}
                Value::Variable(name) => match variables.entry(name.clone()) {
                    Entry::Occupied(entry) => {
                        let previous_negated = *entry.get();
                        if previous_negated != negated {
                            return Ok(Self::Boolean(false));
                        }
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(negated);
                    }
                },
                _ => {
                    return Err(FederationError::internal(format!(
                        "expected boolean or variable `if` argument, got {value}",
                    )))
                }
            }
        }
        Ok(Self::from_variables(variables))
    }

    pub(crate) fn update_with(&self, new_conditions: &Self) -> Self {
        match (new_conditions, self) {
            (Conditions::Boolean(_), _) | (_, Conditions::Boolean(_)) => new_conditions.clone(),
            (Conditions::Variables(new_conditions), Conditions::Variables(handled_conditions)) => {
                let mut filtered = IndexMap::new();
                for (cond_name, &cond_negated) in new_conditions.0.iter() {
                    match handled_conditions.is_negated(cond_name) {
                        Some(handled_cond) if cond_negated != handled_cond => {
                            // If we've already handled that exact condition, we can skip it.
                            // But if we've already handled the _negation_ of this condition, then this mean the overall conditions
                            // are unreachable and we can just return `false` directly.
                            return Conditions::Boolean(false);
                        }
                        Some(_) => {}
                        None => {
                            filtered.insert(cond_name.clone(), cond_negated);
                        }
                    }
                }
                Self::from_variables(filtered)
            }
        }
    }

    pub(crate) fn merge(self, other: Self) -> Self {
        match (self, other) {
            // Absorbing element
            (Conditions::Boolean(false), _) | (_, Conditions::Boolean(false)) => {
                Conditions::Boolean(false)
            }

            // Neutral element
            (Conditions::Boolean(true), x) | (x, Conditions::Boolean(true)) => x,

            (Conditions::Variables(mut self_vars), Conditions::Variables(other_vars)) => {
                let vars = Arc::make_mut(&mut self_vars.0);
                for (name, other_negated) in other_vars.0.iter() {
                    match vars.entry(name.clone()) {
                        Entry::Occupied(entry) => {
                            let self_negated = entry.get();
                            if self_negated != other_negated {
                                return Conditions::Boolean(false);
                            }
                        }
                        Entry::Vacant(entry) => {
                            entry.insert(*other_negated);
                        }
                    }
                }
                Conditions::Variables(self_vars)
            }
        }
    }
}
