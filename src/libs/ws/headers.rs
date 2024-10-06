use chrono::Utc;
use convert_case::Case;
use convert_case::Casing;
use eyre::{bail, Context, ContextCompat, Result};
use futures::future::LocalBoxFuture;
use futures::FutureExt;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio_tungstenite::tungstenite::handshake::server::{Callback, ErrorResponse, Request, Response};
use tracing::*;

use crate::libs::toolbox::ArcToolbox;
use crate::libs::toolbox::RequestContext;
use crate::libs::toolbox::Toolbox;
use crate::model::EndpointSchema;
use crate::model::Type;

use super::WsConnection;

pub struct VerifyProtocol<'a> {
    pub addr: SocketAddr,
    pub tx: tokio::sync::mpsc::Sender<String>,
    pub allow_cors_domains: &'a Option<Vec<String>>,
}

impl<'a> Callback for VerifyProtocol<'a> {
    fn on_request(self, request: &Request, mut response: Response) -> Result<Response, ErrorResponse> {
        let addr = self.addr;
        debug!(?addr, "handshake request: {:?}", request);

        let protocol = request
            .headers()
            .get("Sec-WebSocket-Protocol")
            .or_else(|| request.headers().get("sec-websocket-protocol"));

        let protocol_str = match protocol {
            Some(protocol) => protocol
                .to_str()
                .map_err(|_| ErrorResponse::new(Some("Sec-WebSocket-Protocol is not valid utf-8".to_owned())))?
                .to_string(),
            None => "".to_string(),
        };

        self.tx.try_send(protocol_str.clone()).unwrap();

        response
            .headers_mut()
            .append("Date", Utc::now().to_rfc2822().parse().unwrap());
        if !protocol_str.is_empty() {
            response.headers_mut().insert(
                "Sec-WebSocket-Protocol",
                protocol_str.split(',').next().unwrap_or("").parse().unwrap(),
            );
        }

        response
            .headers_mut()
            .insert("Server", "RustWebsocketServer/1.0".parse().unwrap());

        if let Some(allow_cors_domains) = self.allow_cors_domains {
            if let Some(origin) = request.headers().get("Origin") {
                let origin = origin.to_str().unwrap();
                if allow_cors_domains.iter().any(|x| x == origin) {
                    response
                        .headers_mut()
                        .insert("Access-Control-Allow-Origin", origin.parse().unwrap());
                    response
                        .headers_mut()
                        .insert("Access-Control-Allow-Credentials", "true".parse().unwrap());
                }
            }
        } else {
            // Allow all domains
            if let Some(origin) = request.headers().get("Origin") {
                let origin = origin.to_str().unwrap();
                response
                    .headers_mut()
                    .insert("Access-Control-Allow-Origin", origin.parse().unwrap());
                response
                    .headers_mut()
                    .insert("Access-Control-Allow-Credentials", "true".parse().unwrap());
            }
        }

        debug!(?addr, "Responding handshake with: {:?}", response);

        Ok(response)
    }
}

pub trait AuthController: Sync + Send {
    fn auth(
        self: Arc<Self>,
        toolbox: &ArcToolbox,
        header: String,
        conn: Arc<WsConnection>,
    ) -> LocalBoxFuture<'static, Result<()>>;
}

pub struct SimpleAuthController;

impl AuthController for SimpleAuthController {
    fn auth(
        self: Arc<Self>,
        _toolbox: &ArcToolbox,
        _header: String,
        _conn: Arc<WsConnection>,
    ) -> LocalBoxFuture<'static, Result<()>> {
        async move { Ok(()) }.boxed()
    }
}

pub trait SubAuthController: Sync + Send {
    fn auth(
        self: Arc<Self>,
        toolbox: &ArcToolbox,
        param: serde_json::Value,
        ctx: RequestContext,
        conn: Arc<WsConnection>,
    ) -> LocalBoxFuture<'static, Result<serde_json::Value>>;
}
pub struct EndpointAuthController {
    pub auth_endpoints: HashMap<String, WsAuthController>,
}
pub struct WsAuthController {
    pub schema: EndpointSchema,
    pub handler: Arc<dyn SubAuthController>,
}

impl Default for EndpointAuthController {
    fn default() -> Self {
        Self::new()
    }
}

impl EndpointAuthController {
    pub fn new() -> Self {
        Self {
            auth_endpoints: Default::default(),
        }
    }
    pub fn add_auth_endpoint(&mut self, schema: EndpointSchema, handler: impl SubAuthController + 'static) {
        self.auth_endpoints.insert(
            schema.name.to_ascii_lowercase(),
            WsAuthController {
                schema,
                handler: Arc::new(handler),
            },
        );
    }
}
fn parse_ty(ty: &Type, value: &str) -> Result<serde_json::Value> {
    Ok(match &ty {
        Type::String => {
            let decoded = urlencoding::decode(value)?;
            serde_json::Value::String(decoded.to_string())
        }
        Type::Int => serde_json::Value::Number(
            value
                .parse::<i64>()
                .with_context(|| format!("Failed to parse integer: {}", value))?
                .into(),
        ),
        Type::Boolean => serde_json::Value::Bool(
            value
                .parse::<bool>()
                .with_context(|| format!("Failed to parse boolean: {}", value))?,
        ),
        Type::Enum { .. } => serde_json::Value::String(value.to_string()),
        Type::EnumRef(_) => serde_json::Value::String(value.to_string()),
        Type::UUID => serde_json::Value::String(value.to_string()),
        Type::Optional(ty) => parse_ty(ty, value)?,
        Type::BlockchainAddress => serde_json::Value::String(value.to_string()),
        ty => bail!("Not implemented {:?}", ty),
    })
}

impl AuthController for EndpointAuthController {
    fn auth(
        self: Arc<Self>,
        toolbox: &ArcToolbox,
        header: String,
        conn: Arc<WsConnection>,
    ) -> LocalBoxFuture<'static, Result<()>> {
        let toolbox = toolbox.clone();

        async move {
            let splits = header
                .split(',')
                .map(|x| x.trim())
                .filter(|x| !x.is_empty())
                .map(|x| (&x[..1], &x[1..]))
                .collect::<HashMap<&str, &str>>();

            let method = splits.get("0").context("Could not find method")?;
            // info!("method: {:?}", method);
            let endpoint = self
                .auth_endpoints
                .get(*method)
                .with_context(|| format!("Could not find endpoint for method {}", method))?;
            let mut params = serde_json::Map::new();
            for (index, param) in endpoint.schema.parameters.iter().enumerate() {
                let index = index + 1;
                match splits.get(&index.to_string().as_str()) {
                    Some(value) => {
                        params.insert(param.name.to_case(Case::Camel), parse_ty(&param.ty, value)?);
                    }
                    None if !matches!(&param.ty, Type::Optional(_)) => {
                        bail!("Could not find param {} {}", param.name, index);
                    }
                    _ => {}
                }
            }
            let ctx = RequestContext {
                connection_id: conn.connection_id,
                user_id: 0,
                seq: 0,
                method: endpoint.schema.code,
                log_id: conn.log_id,
                role: conn.role.load(Ordering::Relaxed),
                ip_addr: conn.address.ip(),
            };
            let resp = endpoint
                .handler
                .clone()
                .auth(&toolbox, serde_json::Value::Object(params), ctx, conn)
                .await;
            debug!("Auth response: {:?}", resp);
            if let Some(resp) = Toolbox::encode_ws_response(ctx, resp) {
                toolbox.send(ctx.connection_id, resp);
            }
            Ok(())
        }
        .boxed_local()
    }
}
