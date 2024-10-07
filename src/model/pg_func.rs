use crate::model::{Field, Type};
use convert_case::Case;
use convert_case::Casing;
use itertools::Itertools;

/// `ProceduralFunction` is a struct that represents a single procedural function in the API.
#[derive(Clone, Debug)]
pub struct ProceduralFunction {
    /// The name of the function (e.g. `fun_user_get_user`)
    pub name: String,

    /// A list of parameters that the function accepts (e.g. "user_id" of type `Type::BigInt`)
    pub parameters: Vec<Field>,

    /// The return type of the function (e.g. `Type::struct_`)
    pub return_row_type: Type,

    /// The actual SQL body of the function (e.g. `SELECT * FROM user WHERE user_id = $1`)
    pub body: String,
}

/// Sorts the parameters by the type, so that optional parameters are last.
fn sort_parameters(parameters: Vec<Field>) -> Vec<Field> {
    parameters
        .into_iter()
        .sorted_by_cached_key(|x| matches!(x.ty, Type::Optional(_)))
        .collect()
}

impl ProceduralFunction {
    /// Creates a new `ProceduralFunction` with the given name, parameters, returns and body.
    pub fn new(name: impl Into<String>, parameters: Vec<Field>, returns: Vec<Field>, body: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            name: name.clone(),
            parameters: sort_parameters(parameters),
            return_row_type: Type::struct_(format!("{}RespRow", name.to_case(Case::Pascal)), returns),
            body: body.into(),
        }
    }

    /// Creates a new `ProceduralFunction` with the given name, parameters, row type and body.
    pub fn new_with_row_type(
        name: impl Into<String>,
        parameters: Vec<Field>,
        return_row_type: Type,
        body: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            parameters: sort_parameters(parameters),
            return_row_type,
            body: body.into(),
        }
    }
}
