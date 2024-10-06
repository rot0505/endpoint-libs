use std::fs::File;
use std::net::SocketAddr;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use eyre::{ensure, Context, ContextCompat, Result};
use futures::future::BoxFuture;
use futures::FutureExt;
use rustls::pki_types::CertificateDer;
use rustls_pemfile::certs;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::net::TcpStream;
use tokio_rustls::server::TlsStream;
use tokio_rustls::TlsAcceptor;

pub trait ConnectionListener: Send + Sync + Unpin {
    type Channel1: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static;
    type Channel2: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static;

    fn accept(&self) -> BoxFuture<Result<(Self::Channel1, SocketAddr)>>;
    fn handshake(&self, channel: Self::Channel1) -> BoxFuture<Result<Self::Channel2>>;
}

pub struct TcpListener {
    listener: tokio::net::TcpListener,
}
impl TcpListener {
    pub async fn bind(addr: SocketAddr) -> Result<Self> {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        Ok(Self { listener })
    }
}
impl ConnectionListener for TcpListener {
    type Channel1 = TcpStream;
    type Channel2 = TcpStream;

    fn accept(&self) -> BoxFuture<Result<(Self::Channel1, SocketAddr)>> {
        async {
            let (stream, addr) = self.listener.accept().await?;
            Ok((stream, addr))
        }
        .boxed()
    }
    fn handshake(&self, channel: Self::Channel1) -> BoxFuture<Result<Self::Channel2>> {
        async move { Ok(channel) }.boxed()
    }
}

pub struct TlsListener<T> {
    tcp: T,
    acceptor: TlsAcceptor,
}
impl<T: ConnectionListener> TlsListener<T> {
    pub async fn bind(under: T, pub_certs: Vec<PathBuf>, priv_cert: PathBuf) -> Result<Self> {
        let certs = load_certs(&pub_certs)?;
        ensure!(!certs.is_empty(), "No certificates found in file: {:?}", pub_certs);
        
        let keys = load_private_key(&priv_cert)?;
        ensure!(
            !keys.is_empty(),
            "No private key found in file: {}",
            priv_cert.display()
        );
        let key = keys.into_iter().next().context("No private key found")?;

        let tls_cfg = {
            let cfg = rustls::ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(certs, rustls::pki_types::PrivateKeyDer::Pkcs8(key))?;
            Arc::new(cfg)
        };
        let acceptor = TlsAcceptor::from(tls_cfg);
        Ok(Self { tcp: under, acceptor })
    }
}
impl<T: ConnectionListener + 'static> ConnectionListener for TlsListener<T> {
    type Channel1 = T::Channel1;
    type Channel2 = TlsStream<T::Channel2>;
    fn accept(&self) -> BoxFuture<Result<(Self::Channel1, SocketAddr)>> {
        self.tcp.accept()
    }
    fn handshake(&self, channel: Self::Channel1) -> BoxFuture<Result<Self::Channel2>> {
        async {
            let channel = self.tcp.handshake(channel).await?;
            let tls_stream = self.acceptor.accept(channel).await?;
            Ok(tls_stream)
        }
        .boxed()
    }
}

// Load public certificates from files.
pub fn load_certs<'a, T: AsRef<Path>>(path: impl IntoIterator<Item = T>) -> Result<Vec<CertificateDer<'a>>> {
    let mut r_certs = vec![];
    for p in path {
        let p = p.as_ref();
        let f = File::open(p).with_context(|| format!("Failed to open {}", p.display()))?;

        let reader = &mut std::io::BufReader::new(f);
        let certs_results = certs(reader);

        let certs: Vec<CertificateDer> = certs_results.filter_map(|result| match result {
            Ok(value) => Some(value),  // Map Ok values to Some(value)
            Err(_) => None,            // Ignore Err values
        })
        .collect();

        r_certs.extend(certs);
    }
    Ok(r_certs)
}

// Load private key from file.
pub fn load_private_key(path: &PathBuf) -> Result<Vec<rustls::pki_types::PrivatePkcs8KeyDer<'static>>> {
    let file = std::fs::File::open(path).with_context(|| format!("Failed to open private key {:?} ", path))?;
    let mut reader = std::io::BufReader::new(file);
    let keys = rustls_pemfile::pkcs8_private_keys(&mut reader).into_iter().filter_map(|result| match result {
        Ok(value) => Some(value),  // Map Ok values to Some(value)
        Err(_) => None,            // Ignore Err values
    })
    .collect();

    Ok(keys)
}
