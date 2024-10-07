use serde::*;

/// `Field` is a struct that represents the parameters and returns in an endpoint schema.
#[derive(Clone, Debug, Hash, Serialize, Deserialize, Ord, PartialOrd, Eq, PartialEq)]
pub struct Field {
    /// The name of the field (e.g. `user_id`)
    pub name: String,

    /// The type of the field (e.g. `Type::BigInt`)
    pub ty: Type,
}

impl Field {
    /// Creates a new `Field` with the given name and type.
    pub fn new(name: impl Into<String>, ty: Type) -> Self {
        Self { name: name.into(), ty }
    }
}

/// `EnumVariant` is a struct that represents the variants of an enum.
#[derive(Clone, Debug, Hash, Serialize, Deserialize, Ord, PartialOrd, Eq, PartialEq)]
pub struct EnumVariant {
    /// The name of the variant (e.g. `UniSwap`)
    pub name: String,

    /// The value of the variant (e.g. 1)
    pub value: i64,

    /// A comment added by `new_with_comment` method
    pub comment: String,
}

impl EnumVariant {
    /// Creates a new `EnumVariant` with the given name and value.
    /// `comment` is set to `""`.
    pub fn new(name: impl Into<String>, value: i64) -> Self {
        Self {
            name: name.into(),
            value,
            comment: "".to_owned(),
        }
    }

    /// Creates a new `EnumVariant` with the given name, value and comment.
    pub fn new_with_comment(name: impl Into<String>, value: i64, comment: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value,
            comment: comment.into(),
        }
    }
}

/// `Type` is an enum that represents the types of the fields in an endpoint schema.
#[derive(Clone, Debug, Serialize, Deserialize, Hash, PartialEq, PartialOrd, Eq, Ord)]
pub enum Type {
    Date,
    Int,
    BigInt,
    Numeric,
    Boolean,
    String,
    Bytea,
    UUID,
    Inet,
    Struct { name: String, fields: Vec<Field> },
    StructRef(String),
    Object,
    DataTable { name: String, fields: Vec<Field> },
    Vec(Box<Type>),
    Unit,
    Optional(Box<Type>),
    Enum { name: String, variants: Vec<EnumVariant> },
    EnumRef(String),
    TimeStampMs,
    BlockchainDecimal,
    BlockchainAddress,
    BlockchainTransactionHash,
}

impl Type {
    /// Creates a new `Type::Struct` with the given name and fields.
    pub fn struct_(name: impl Into<String>, fields: Vec<Field>) -> Self {
        Self::Struct {
            name: name.into(),
            fields,
        }
    }

    /// Creates a new `Type::StructRef` with the given name.
    pub fn struct_ref(name: impl Into<String>) -> Self {
        Self::StructRef(name.into())
    }

    /// Creates a new `Type::DataTable` with the given name and fields.
    pub fn datatable(name: impl Into<String>, fields: Vec<Field>) -> Self {
        Self::DataTable {
            name: name.into(),
            fields,
        }
    }

    /// Creates a new `Type::Vec` with the given type.
    pub fn vec(ty: Type) -> Self {
        Self::Vec(Box::new(ty))
    }

    /// Creates a new `Type::Optional` with the given type.
    pub fn optional(ty: Type) -> Self {
        Self::Optional(Box::new(ty))
    }

    /// Creates a new `Type::EnumRef` with the given name.
    pub fn enum_ref(name: impl Into<String>) -> Self {
        Self::EnumRef(name.into())
    }

    /// Creates a new `Type::Enum` with the given name and fields/variants.
    pub fn enum_(name: impl Into<String>, fields: Vec<EnumVariant>) -> Self {
        Self::Enum {
            name: name.into(),
            variants: fields,
        }
    }
    pub fn try_unwrap(self) -> Option<Self> {
        match self {
            Self::Vec(v) => Some(*v),
            Self::DataTable { .. } => None,
            _ => Some(self),
        }
    }
}
