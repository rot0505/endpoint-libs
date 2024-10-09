use crate::model::{Field, Type};
use serde::*;

/// `EndpointSchema` is a struct that represents a single endpoint in the API.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct EndpointSchema {
    /// The name of the endpoint (e.g. `UserListSymbols`)
    pub name: String,

    /// The method code of the endpoint (e.g. `10020`)
    pub code: u32,

    /// A list of parameters that the endpoint accepts (e.g. "symbol" of type `String`)
    pub parameters: Vec<Field>,

    /// A list of fields that the endpoint returns
    pub returns: Vec<Field>,

    /// The type of the stream response (if any)
    #[serde(default)]
    pub stream_response: Option<Type>,

    /// A description of the endpoint added by `with_description` method
    #[serde(default)]
    pub description: String,

    /// The JSON schema of the endpoint (`Default::default()`)
    #[serde(default)]
    pub json_schema: serde_json::Value,
}

impl EndpointSchema {
    /// Creates a new `EndpointSchema` with the given name, method code, parameters and returns.
    pub fn new(
        name: impl Into<String>,
        code: u32,
        parameters: Vec<Field>,
        returns: Vec<Field>,
    ) -> Self {
        Self {
            name: name.into(),
            code,
            parameters,
            returns,
            stream_response: None,
            description: "".to_string(),
            json_schema: Default::default(),
        }
    }

    /// Adds a stream response type field to the endpoint.
    pub fn with_stream_response_type(mut self, stream_response: Type) -> Self {
        self.stream_response = Some(stream_response);
        self
    }

    /// Adds a description field to the endpoint.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }
}
