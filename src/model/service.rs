use crate::model::EndpointSchema;
use serde::*;

/// `Service` is a struct that represents a single service in the API.
#[derive(Debug, Serialize, Deserialize)]
pub struct Service {
    /// The name of the service (e.g. `user`)
    pub name: String,

    /// The ID of the service (e.g. `1`)
    pub id: u16,

    /// A list of endpoints (schemas) that the service contains (e.g. `user_endpoints::get_user_endpoints()`)
    pub endpoints: Vec<EndpointSchema>,
}

impl Service {
    /// Creates a new `Service` with the given name, ID and endpoints.
    pub fn new(name: impl Into<String>, id: u16, endpoints: Vec<EndpointSchema>) -> Self {
        Self {
            name: name.into(),
            id,
            endpoints,
        }
    }
}
