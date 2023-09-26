use std::rc::Rc;
use crate::utils::InsertOnlyIndexMap;

#[derive(Debug, Clone)]
pub enum ValueEnum {
    VariableValue(VariableValue),
    IntValue(IntValue),
    FloatValue(FloatValue),
    StringValue(StringValue),
    BooleanValue(BooleanValue),
    NullValue,
    EnumValue(EnumValue),
    ListValue(ListValue),
    ObjectValue(ObjectValue),
}

#[derive(Debug, Clone)]
pub struct VariableValue(Rc<str>);

#[derive(Debug, Clone)]
pub struct IntValue(i32);

#[derive(Debug, Clone)]
pub struct FloatValue(f64);

#[derive(Debug, Clone)]
pub struct StringValue(Rc<str>);

#[derive(Debug, Clone)]
pub struct BooleanValue(bool);

#[derive(Debug, Clone)]
pub struct NullValue;

#[derive(Debug, Clone)]
pub struct EnumValue(Rc<str>);

#[derive(Debug, Clone)]
pub struct ListValue(Rc<[ValueEnum]>);

#[derive(Debug, Clone)]
pub struct ObjectValue(Rc<InsertOnlyIndexMap<ValueEnum>>);
