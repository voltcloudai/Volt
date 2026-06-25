use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Type {
    I32,
    I64,
    U32,
    U64,
    F32,
    F64,
    Bool,
    String,
    Void,
    Option(Box<Type>),
    Result(Box<Type>, Box<Type>),
    Struct(String),
    None,
    InferInt,
    InferFloat,
    Unknown,
}

impl Type {
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            Type::I32
                | Type::I64
                | Type::U32
                | Type::U64
                | Type::F32
                | Type::F64
                | Type::InferInt
                | Type::InferFloat
        )
    }

    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            Type::I32 | Type::I64 | Type::U32 | Type::U64 | Type::InferInt
        )
    }

    pub fn is_assignable_from(&self, actual: &Type) -> bool {
        if self == actual || matches!(self, Type::Unknown) || matches!(actual, Type::Unknown) {
            return true;
        }

        match (self, actual) {
            (Type::I32 | Type::I64 | Type::U32 | Type::U64, Type::InferInt) => true,
            (Type::F32 | Type::F64, Type::InferInt | Type::InferFloat) => true,
            (Type::Option(_), Type::None) => true,
            (Type::Option(expected), Type::Option(actual)) => expected.is_assignable_from(actual),
            (Type::Option(expected), actual) => expected.is_assignable_from(actual),
            (Type::Result(ok_expected, err_expected), Type::Result(ok_actual, err_actual)) => {
                ok_expected.is_assignable_from(ok_actual)
                    && err_expected.is_assignable_from(err_actual)
            }
            _ => false,
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::I32 => write!(f, "i32"),
            Type::I64 => write!(f, "i64"),
            Type::U32 => write!(f, "u32"),
            Type::U64 => write!(f, "u64"),
            Type::F32 => write!(f, "f32"),
            Type::F64 => write!(f, "f64"),
            Type::Bool => write!(f, "bool"),
            Type::String => write!(f, "string"),
            Type::Void => write!(f, "void"),
            Type::Option(inner) => write!(f, "Option<{inner}>"),
            Type::Result(ok, err) => write!(f, "Result<{ok}, {err}>"),
            Type::Struct(name) => write!(f, "{name}"),
            Type::None => write!(f, "none"),
            Type::InferInt => write!(f, "integer"),
            Type::InferFloat => write!(f, "float"),
            Type::Unknown => write!(f, "unknown"),
        }
    }
}
