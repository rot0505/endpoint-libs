use eyre::{bail, eyre, ContextCompat, Result};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use tokio::task::LocalSet;
use tokio_tungstenite::tungstenite::Error as WsError;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;
use tracing::*;

use crate::libs::error_code::ErrorCode;
use crate::libs::handler::{RequestHandler, RequestHandlerErased};
use crate::libs::listener::{ConnectionListener, TcpListener, TlsListener};
use crate::libs::toolbox::{ArcToolbox, RequestContext, Toolbox, TOOLBOX};
use crate::libs::utils::{get_conn_id, get_log_id};
use crate::libs::ws::{VerifyProtocol, WsClientSession, WsConnection};
use crate::model::EndpointSchema;
use crate::libs::ws::client::WsRequest;

use super::{AuthController, ConnectionId, SimpleAuthController, WebsocketStates, WsEndpoint};

pub struct WebsocketServer {
    pub auth_controller: Arc<dyn AuthController>,
    pub handlers: HashMap<u32, WsEndpoint>,
    pub message_receiver: Option<mpsc::Receiver<ConnectionId>>,
    pub toolbox: ArcToolbox,
    pub config: WsServerConfig,
}

impl WebsocketServer {
    pub fn new(config: WsServerConfig) -> Self {
        Self {
            auth_controller: Arc::new(SimpleAuthController),
            handlers: Default::default(),
            message_receiver: None,
            toolbox: Toolbox::new(),
            config,
        }
    }
    pub fn set_auth_controller(&mut self, controller: impl AuthController + 'static) {
        self.auth_controller = Arc::new(controller);
    }
    pub fn add_handler<T: RequestHandler + 'static>(&mut self, handler: T) {
        let schema = serde_json::from_str(T::Request::SCHEMA).expect("Invalid schema");
        check_handler::<T>(&schema).expect("Invalid handler");
        self.add_handler_erased(schema, Arc::new(handler))
    }
    pub fn add_handler_erased(&mut self, schema: EndpointSchema, handler: Arc<dyn RequestHandlerErased>) {
        let old = self.handlers.insert(schema.code, WsEndpoint { schema, handler });
        if let Some(old) = old {
            panic!(
                "Overwriting handler for endpoint {} {}",
                old.schema.code, old.schema.name
            );
        }
    }
    async fn handle_ws_handshake_and_connection<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(
        self: Arc<Self>,
        addr: SocketAddr,
        states: Arc<WebsocketStates>,
        stream: S,
    ) -> Result<()> {
        let (tx, mut rx) = mpsc::channel(1);
        let hs = tokio_tungstenite::accept_hdr_async(
            stream,
            VerifyProtocol {
                addr,
                tx,
                allow_cors_domains: &self.config.allow_cors_urls,
            },
        )
        .await;

        // TODO remove below after tracing log issue
        tracing::warn!("handle new WS connection");

        let stream = wrap_ws_error(hs)?;
        let conn = Arc::new(WsConnection {
            connection_id: get_conn_id(),
            user_id: Default::default(),
            role: AtomicU32::new(0),
            address: addr,
            log_id: get_log_id(),
        });
        debug!(?addr, "New connection handshaken {:?}", conn);
        let headers = rx.recv().await.ok_or_else(|| eyre!("Failed to receive ws headers"))?;

        let (tx, rx) = mpsc::channel(100);
        let conn = Arc::clone(&conn);
        states.insert(conn.connection_id, tx, conn.clone());

        let auth_result = Arc::clone(&self.auth_controller)
            .auth(&self.toolbox, headers, Arc::clone(&conn))
            .await;
        let raw_ctx = RequestContext::from_conn(&conn);
        if let Err(err) = auth_result {
            self.toolbox.send_request_error(
                &raw_ctx,
                ErrorCode::new(100400), // BadRequest
                err.to_string(),
            );
            return Err(err);
        }
        self.handle_session_connection(conn, states, stream, rx).await;

        Ok(())
    }

    pub async fn handle_session_connection<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(
        self: Arc<Self>,
        conn: Arc<WsConnection>,
        states: Arc<WebsocketStates>,
        stream: WebSocketStream<S>,
        rx: mpsc::Receiver<Message>,
    ) {
        let addr = conn.address;
        let context = RequestContext::from_conn(&conn);

        let session = WsClientSession::new(conn, stream, rx, self);
        session.run().await;

        states.remove(context.connection_id);
        debug!(?addr, "Connection closed");
    }

    pub async fn listen(self) -> Result<()> {
        info!("Listening on {}", self.config.address);

        // Resolve the address and get the socket address
        let addr = tokio::net::lookup_host(&self.config.address)
            .await?
            .next()
            .with_context(|| format!("Failed to lookup host to bind: {}", self.config.address))?;

        let listener = TcpListener::bind(addr).await?;
        if self.config.insecure {
            self.listen_impl(Arc::new(listener)).await
        } else if self.config.pub_certs.is_some() && self.config.priv_key.is_some() {
            // Proceed with binding the listener for secure mode
            let listener = TlsListener::bind(
                listener,
                self.config.pub_certs.clone().unwrap(),
                self.config.priv_key.clone().unwrap(),
            )
            .await?;
            self.listen_impl(Arc::new(listener)).await
        } else {
            bail!("pub_certs and priv_key should be set")
        }
    }

    async fn listen_impl<T: ConnectionListener + 'static>(self, listener: Arc<T>) -> Result<()> {
        let states = Arc::new(WebsocketStates::new());
        self.toolbox
            .set_ws_states(states.clone_states(), self.config.header_only);
        let this = Arc::new(self);
        let local_set = LocalSet::new();
        let (mut sigterm, mut sigint) = crate::libs::signal::init_signals()?;
        local_set
            .run_until(async {
                loop {
                    tokio::select! {
                        _ = crate::libs::signal::wait_for_signals(&mut sigterm, &mut sigint) => break,
                        accepted = listener.accept() => {
                            let (stream, addr) = match accepted {
                                Ok(x) => x,
                                Err(err) => {
                                    error!("Error while accepting stream: {:?}", err);
                                    continue;
                                }
                            };
                            let listener = Arc::clone(&listener);
                            let this = Arc::clone(&this);
                            let states = Arc::clone(&states);
                            local_set.spawn_local(async move {
                                let stream = match listener.handshake(stream).await {
                                    Ok(channel) => {
                                        info!("Accepted stream from {}", addr);
                                        channel
                                    }
                                    Err(err) => {
                                        error!("Error while handshaking stream: {:?}", err);
                                        return;
                                    }
                                };

                                let future = TOOLBOX.scope(this.toolbox.clone(), this.handle_ws_handshake_and_connection(addr, states, stream));
                                if let Err(err) = future.await {
                                    error!("Error while handling connection: {:?}", err);
                                }
                            });
                        }
                    }
                }
                Ok(())
            })
            .await
    }

    pub fn dump_schemas(&self) -> Result<()> {
        let _ = std::fs::create_dir_all("docs");
        let file = format!("docs/{}_alive_endpoints.json", self.config.name);
        let available_schemas: Vec<String> = self.handlers.values().map(|x| x.schema.name.clone()).sorted().collect();
        info!("Dumping {} endpoint names to {}", available_schemas.len(), file);
        serde_json::to_writer_pretty(File::create(file)?, &available_schemas)?;
        Ok(())
    }
}

pub fn wrap_ws_error<T>(err: Result<T, WsError>) -> Result<T> {
    err.map_err(|x| eyre!(x))
}

pub fn check_name(cat: &str, be_name: &str, should_name: &str) -> Result<()> {
    if !be_name.contains(should_name) {
        bail!("{} name should be {} but got {}", cat, should_name, be_name);
    } else {
        Ok(())
    }
}

pub fn check_handler<T: RequestHandler + 'static>(schema: &EndpointSchema) -> Result<()> {
    let handler_name = std::any::type_name::<T>();
    let should_handler_name = format!("Method{}", schema.name);
    check_name("Method", handler_name, &should_handler_name)?;
    let request_name = std::any::type_name::<T::Request>();
    let should_req_name = format!("{}Request", schema.name);
    check_name("Request", request_name, &should_req_name)?;

    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WsServerConfig {
    #[serde(default)]
    pub name: String,
    pub address: String,
    #[serde(default)]
    pub pub_certs: Option<Vec<PathBuf>>,
    #[serde(default)]
    pub priv_key: Option<PathBuf>,
    #[serde(default)]
    pub insecure: bool,
    #[serde(default)]
    pub debug: bool,
    #[serde(skip)]
    pub header_only: bool,
    #[serde(skip)]
    pub allow_cors_urls: Arc<Option<Vec<String>>>,
}
