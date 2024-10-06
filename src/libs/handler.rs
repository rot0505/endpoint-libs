use async_trait::async_trait;
use eyre::Result;
use serde_json::Value;

use super::{error_code::ErrorCode, toolbox::{ArcToolbox, RequestContext, Toolbox}, ws::{request_error_to_resp, WsRequest}};

#[allow(type_alias_bounds)]
pub type Response<T: WsRequest> = Result<T::Response>;
#[async_trait(?Send)]
pub trait RequestHandler: Send + Sync {
    type Request: WsRequest + 'static;

    async fn handle(&self, ctx: RequestContext, req: Self::Request) -> Response<Self::Request>;
}

#[doc(hidden)]
#[async_trait(?Send)]
pub trait RequestHandlerErased: Send + Sync {
    async fn handle(&self, toolbox: &ArcToolbox, ctx: RequestContext, req: Value);
}

#[async_trait(?Send)]
impl<T: RequestHandler> RequestHandlerErased for T {
    async fn handle(&self, toolbox: &ArcToolbox, ctx: RequestContext, req: Value) {
        // TODO: find a better way to avoid double parsing or serialization
        let buf = serde_json::to_string(&req).unwrap();
        let data: T::Request = match serde_json::from_value(req) {
            Ok(data) => data,
            Err(err) => {
                let jd = &mut serde_json::Deserializer::from_str(&buf);
                let data: Result<T::Request, _> = serde_path_to_error::deserialize(jd);
                let path = data.err().map(|err| err.path().to_string());
                toolbox.send(
                    ctx.connection_id,
                    request_error_to_resp(
                        &ctx,
                        ErrorCode::new(100400), // Bad Request
                        if let Some(path) = path {
                            format!("{}: {}", path, err)
                        } else {
                            format!("{}", err)
                        },
                    ),
                );
                return;
            }
        };

        let fut = RequestHandler::handle(self, ctx, data);

        let resp = fut.await;
        if let Some(resp) = Toolbox::encode_ws_response(ctx, resp) {
            toolbox.send(ctx.connection_id, resp);
        }
    }
}
