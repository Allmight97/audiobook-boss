use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use secrecy::SecretString;

use super::vault::SecretVault;
use crate::errors::{AppError, Result};

pub(crate) struct VaultGate {
    entered: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    release: Mutex<std::sync::mpsc::Receiver<()>>,
}

impl VaultGate {
    fn wait(&self) {
        if let Some(entered) = self.entered.lock().expect("gate sender").take() {
            entered.send(()).expect("test awaits vault entry");
            self.release
                .lock()
                .expect("gate receiver")
                .recv_timeout(std::time::Duration::from_secs(5))
                .expect("test releases vault");
        }
    }
}

pub(crate) fn vault_gate() -> (
    VaultGate,
    tokio::sync::oneshot::Receiver<()>,
    std::sync::mpsc::Sender<()>,
) {
    let (entered, waiting) = tokio::sync::oneshot::channel();
    let (release, released) = std::sync::mpsc::channel();
    (
        VaultGate {
            entered: Mutex::new(Some(entered)),
            release: Mutex::new(released),
        },
        waiting,
        release,
    )
}

type Failure = fn() -> AppError;

struct ReadTrap {
    free_reads: usize,
    gate: Option<VaultGate>,
    failure: Failure,
}

#[derive(Default)]
pub(crate) struct MemoryVault {
    secrets: Mutex<HashMap<String, SecretString>>,
    reads: Arc<AtomicUsize>,
    set_calls: Mutex<Vec<String>>,
    delete_calls: Mutex<Vec<String>>,
    read_trap: Option<ReadTrap>,
    write_failure: Option<Failure>,
    delete_failure: Option<Failure>,
    delete_gate: Option<VaultGate>,
    ephemeral: bool,
}

impl MemoryVault {
    pub(crate) fn with_secret(self, key: &str, value: &str) -> Self {
        self.seed(key, value);
        self
    }

    pub(crate) fn ephemeral(mut self) -> Self {
        self.ephemeral = true;
        self
    }

    pub(crate) fn failing_reads(mut self, failure: Failure) -> Self {
        self.read_trap = Some(ReadTrap {
            free_reads: 0,
            gate: None,
            failure,
        });
        self
    }

    pub(crate) fn gating_reads(
        mut self,
        free_reads: usize,
        gate: VaultGate,
        failure: Failure,
    ) -> Self {
        self.read_trap = Some(ReadTrap {
            free_reads,
            gate: Some(gate),
            failure,
        });
        self
    }

    pub(crate) fn failing_writes(mut self, failure: Failure) -> Self {
        self.write_failure = Some(failure);
        self
    }

    pub(crate) fn failing_deletes(mut self, failure: Failure) -> Self {
        self.delete_failure = Some(failure);
        self
    }

    pub(crate) fn gating_deletes(mut self, gate: VaultGate) -> Self {
        self.delete_gate = Some(gate);
        self
    }

    pub(crate) fn seed(&self, key: &str, value: &str) {
        self.secrets
            .lock()
            .expect("test vault")
            .insert(key.to_string(), SecretString::from(value.to_string()));
    }

    pub(crate) fn secret_count(&self) -> usize {
        self.secrets.lock().expect("test vault").len()
    }

    pub(crate) fn read_count(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }

    pub(crate) fn clear_reads(&self) {
        self.reads.store(0, Ordering::SeqCst);
    }

    pub(crate) fn read_counter(&self) -> Arc<AtomicUsize> {
        self.reads.clone()
    }

    pub(crate) fn set_calls(&self) -> Vec<String> {
        self.set_calls.lock().expect("test vault").clone()
    }

    pub(crate) fn delete_calls(&self) -> Vec<String> {
        self.delete_calls.lock().expect("test vault").clone()
    }
}

impl SecretVault for MemoryVault {
    fn survives_restart(&self) -> bool {
        !self.ephemeral
    }

    fn get_secret(&self, key: &str) -> Result<Option<SecretString>> {
        let read = self.reads.fetch_add(1, Ordering::SeqCst);
        if let Some(trap) = self
            .read_trap
            .as_ref()
            .filter(|trap| read >= trap.free_reads)
        {
            if let Some(gate) = &trap.gate {
                gate.wait();
            }
            return Err((trap.failure)());
        }
        Ok(self.secrets.lock().expect("test vault").get(key).cloned())
    }

    fn set_secret(&self, key: &str, value: SecretString) -> Result<()> {
        self.set_calls
            .lock()
            .expect("test vault")
            .push(key.to_string());
        if let Some(failure) = self.write_failure {
            return Err(failure());
        }
        self.secrets
            .lock()
            .expect("test vault")
            .insert(key.to_string(), value);
        Ok(())
    }

    fn delete_secret(&self, key: &str) -> Result<()> {
        self.delete_calls
            .lock()
            .expect("test vault")
            .push(key.to_string());
        if let Some(gate) = &self.delete_gate {
            gate.wait();
        }
        if let Some(failure) = self.delete_failure {
            return Err(failure());
        }
        self.secrets.lock().expect("test vault").remove(key);
        Ok(())
    }
}

pub(crate) fn vault_unavailable() -> AppError {
    AppError::General("vault unavailable".to_string())
}

pub(crate) fn keychain_locked() -> AppError {
    AppError::ResourceCleanup("keychain locked".to_string())
}

const STUB_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

#[derive(Clone, Debug, Default)]
pub(crate) struct CapturedRequest {
    pub(crate) raw: String,
    pub(crate) method: String,
    pub(crate) path_and_query: String,
    pub(crate) body: String,
}

impl CapturedRequest {
    fn parse(raw: String) -> Self {
        let mut request_line = raw.lines().next().unwrap_or_default().split_whitespace();
        let method = request_line.next().unwrap_or_default().to_string();
        let path_and_query = request_line.next().unwrap_or_default().to_string();
        let body = raw.split("\r\n\r\n").nth(1).unwrap_or_default().to_string();
        Self {
            raw,
            method,
            path_and_query,
            body,
        }
    }

    pub(crate) fn request_line(&self) -> &str {
        self.raw.lines().next().unwrap_or_default()
    }

    pub(crate) fn header(&self, name: &str) -> Option<String> {
        self.raw.lines().skip(1).find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case(name)
                .then(|| value.trim().to_string())
        })
    }

    pub(crate) fn json_body(&self) -> serde_json::Value {
        serde_json::from_str(&self.body).expect("request body is JSON")
    }
}

enum Delivery {
    Respond(Vec<u8>),
    Chunks {
        chunks: Vec<Vec<u8>>,
        pause: std::time::Duration,
    },
    CloseSilently,
    Hold,
}

struct Pause {
    arrived: tokio::sync::oneshot::Sender<()>,
    release: tokio::sync::oneshot::Receiver<()>,
}

pub(crate) struct Reply {
    delivery: Delivery,
    pause: Option<Pause>,
}

impl Reply {
    fn deliver(delivery: Delivery) -> Self {
        Self {
            delivery,
            pause: None,
        }
    }

    pub(crate) fn json(status: &str, body: &str) -> Self {
        Self::bytes(format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        ))
    }

    pub(crate) fn bytes(response: impl Into<Vec<u8>>) -> Self {
        Self::deliver(Delivery::Respond(response.into()))
    }

    pub(crate) fn chunks(chunks: Vec<Vec<u8>>, pause: std::time::Duration) -> Self {
        Self::deliver(Delivery::Chunks { chunks, pause })
    }

    pub(crate) fn close_silently() -> Self {
        Self::deliver(Delivery::CloseSilently)
    }

    pub(crate) fn hold() -> Self {
        Self::deliver(Delivery::Hold)
    }

    pub(crate) fn paused(
        mut self,
    ) -> (
        Self,
        tokio::sync::oneshot::Receiver<()>,
        tokio::sync::oneshot::Sender<()>,
    ) {
        let (arrived, arrival) = tokio::sync::oneshot::channel();
        let (release, released) = tokio::sync::oneshot::channel();
        self.pause = Some(Pause {
            arrived,
            release: released,
        });
        (self, arrival, release)
    }
}

struct StubState {
    requests: Mutex<Vec<CapturedRequest>>,
    connections: AtomicUsize,
    served: tokio::sync::watch::Sender<usize>,
}

pub(crate) struct StubServer {
    address: std::net::SocketAddr,
    state: Arc<StubState>,
    script_len: usize,
    task: tokio::task::JoinHandle<()>,
    ca: Option<tokio_rustls::rustls::pki_types::CertificateDer<'static>>,
}

impl Drop for StubServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl StubServer {
    pub(crate) async fn start(script: Vec<Reply>) -> Self {
        Self::start_addressed(|_| script).await
    }

    pub(crate) async fn start_addressed(
        make_script: impl FnOnce(std::net::SocketAddr) -> Vec<Reply>,
    ) -> Self {
        let (listener, address) = bind_stub().await;
        let script = make_script(address);
        let script_len = script.len();
        let mut script: Vec<Option<Reply>> = script.into_iter().map(Some).collect();
        Self::serve(listener, None, script_len, move |index, _| {
            script
                .get_mut(index)
                .and_then(Option::take)
                .unwrap_or_else(Reply::hold)
        })
    }

    pub(crate) async fn start_tls(
        handler: impl FnMut(usize, &CapturedRequest) -> Reply + Send + 'static,
    ) -> Self {
        let (listener, _) = bind_stub().await;
        Self::serve(listener, Some(tls_material()), 0, handler)
    }

    pub(crate) fn address(&self) -> std::net::SocketAddr {
        self.address
    }

    pub(crate) fn url(&self) -> String {
        let scheme = if self.ca.is_some() { "https" } else { "http" };
        format!("{scheme}://{}", self.address)
    }

    pub(crate) fn tls_client(&self) -> reqwest::Client {
        let ca = self.ca.clone().expect("a TLS stub");
        reqwest::Client::builder()
            .tls_certs_only([reqwest::Certificate::from_der(&ca).expect("ca")])
            .build()
            .expect("client")
    }

    pub(crate) fn requests(&self) -> Vec<CapturedRequest> {
        self.state.requests.lock().expect("stub requests").clone()
    }

    pub(crate) fn connections(&self) -> usize {
        self.state.connections.load(Ordering::SeqCst)
    }

    pub(crate) async fn finished(&self) -> Vec<CapturedRequest> {
        self.served(self.script_len).await
    }

    pub(crate) async fn only_request(&self) -> CapturedRequest {
        self.served(1).await.remove(0)
    }

    pub(crate) async fn nothing_more_arrives(&self) -> bool {
        let before = self.connections();
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        self.connections() == before
    }

    pub(crate) async fn served(&self, replies: usize) -> Vec<CapturedRequest> {
        let mut served = self.state.served.subscribe();
        tokio::time::timeout(STUB_TIMEOUT, served.wait_for(|done| *done >= replies))
            .await
            .expect("the stub answers in time")
            .expect("the stub is running");
        self.requests()
    }

    #[expect(clippy::disallowed_methods, reason = "aborted by StubServer::drop")]
    fn serve(
        listener: tokio::net::TcpListener,
        tls: Option<TlsMaterial>,
        script_len: usize,
        mut handler: impl FnMut(usize, &CapturedRequest) -> Reply + Send + 'static,
    ) -> Self {
        let address = listener.local_addr().expect("stub address");
        let (served, _) = tokio::sync::watch::channel(0);
        let state = Arc::new(StubState {
            requests: Mutex::new(Vec::new()),
            connections: AtomicUsize::new(0),
            served,
        });
        let (acceptor, ca) = match tls {
            Some(material) => (Some(material.acceptor), Some(material.ca)),
            None => (None, None),
        };
        let shared = Arc::clone(&state);
        let task = tokio::spawn(async move {
            let mut held: Vec<Box<dyn Send>> = Vec::new();
            let mut index = 0;
            while let Ok((tcp, _)) = listener.accept().await {
                shared.connections.fetch_add(1, Ordering::SeqCst);
                let kept = match &acceptor {
                    Some(acceptor) => match acceptor.accept(tcp).await {
                        Ok(tls) => answer(tls, index, &mut handler, &shared).await,
                        Err(_) => continue,
                    },
                    None => answer(tcp, index, &mut handler, &shared).await,
                };
                index += 1;
                held.extend(kept);
            }
        });
        Self {
            address,
            state,
            script_len,
            task,
            ca,
        }
    }
}

async fn answer<S>(
    mut stream: S,
    index: usize,
    handler: &mut impl FnMut(usize, &CapturedRequest) -> Reply,
    state: &StubState,
) -> Option<Box<dyn Send>>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    use tokio::io::AsyncWriteExt;
    let request = CapturedRequest::parse(read_request(&mut stream).await?);
    state
        .requests
        .lock()
        .expect("stub requests")
        .push(request.clone());
    let reply = handler(index, &request);
    if let Some(Pause { arrived, release }) = reply.pause {
        #[expect(
            clippy::unused_result_ok,
            reason = "the receiver is gone if the test stops waiting"
        )]
        arrived.send(()).ok();
        #[expect(
            clippy::unused_result_ok,
            reason = "the sender is dropped when the test ends"
        )]
        release.await.ok();
    }
    match reply.delivery {
        Delivery::Hold => return Some(Box::new(stream)),
        Delivery::CloseSilently => {}
        Delivery::Respond(bytes) => send_ignoring_hangup(&mut stream, &bytes).await,
        Delivery::Chunks { chunks, pause } => {
            for (position, chunk) in chunks.iter().enumerate() {
                if position > 0 {
                    tokio::time::sleep(pause).await;
                }
                send_ignoring_hangup(&mut stream, chunk).await;
                #[expect(
                    clippy::let_underscore_must_use,
                    reason = "the client may already have hung up"
                )]
                let _ = stream.flush().await;
            }
        }
    }
    #[expect(
        clippy::let_underscore_must_use,
        reason = "the client may already have hung up"
    )]
    let _ = stream.shutdown().await;
    state.served.send_modify(|served| *served += 1);
    None
}

async fn send_ignoring_hangup<S: tokio::io::AsyncWrite + Unpin>(stream: &mut S, bytes: &[u8]) {
    use tokio::io::AsyncWriteExt;
    #[expect(
        clippy::let_underscore_must_use,
        reason = "the client may hang up mid-reply on purpose"
    )]
    let _ = stream.write_all(bytes).await;
}

async fn read_request<S: tokio::io::AsyncRead + Unpin>(stream: &mut S) -> Option<String> {
    use tokio::io::AsyncReadExt;
    let mut buffer = Vec::new();
    loop {
        let mut chunk = [0_u8; 4096];
        let count = tokio::time::timeout(STUB_TIMEOUT, stream.read(&mut chunk))
            .await
            .ok()?
            .ok()?;
        if count == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..count]);
        let text = String::from_utf8_lossy(&buffer);
        if let Some((head, body)) = text.split_once("\r\n\r\n") {
            let length = head
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);
            if body.len() >= length {
                return Some(text.into_owned());
            }
        }
    }
}

async fn bind_stub() -> (tokio::net::TcpListener, std::net::SocketAddr) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind stub");
    let address = listener.local_addr().expect("stub address");
    (listener, address)
}

pub(crate) async fn unused_address() -> std::net::SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind probe");
    listener.local_addr().expect("probe address")
}

struct TlsMaterial {
    acceptor: tokio_rustls::TlsAcceptor,
    ca: tokio_rustls::rustls::pki_types::CertificateDer<'static>,
}

fn tls_material() -> TlsMaterial {
    let ca_key = rcgen::KeyPair::generate().expect("ca key");
    let mut ca_params = rcgen::CertificateParams::new(Vec::<String>::new()).expect("ca params");
    ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    let ca_cert = ca_params.self_signed(&ca_key).expect("ca cert");
    let issuer = rcgen::Issuer::new(ca_params, ca_key);
    let leaf_key = rcgen::KeyPair::generate().expect("leaf key");
    let leaf_cert = rcgen::CertificateParams::new(vec!["127.0.0.1".to_string()])
        .expect("leaf params")
        .signed_by(&leaf_key, &issuer)
        .expect("leaf cert");
    let server_config = tokio_rustls::rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![leaf_cert.der().clone()],
            tokio_rustls::rustls::pki_types::PrivateKeyDer::Pkcs8(leaf_key.serialize_der().into()),
        )
        .expect("server config");
    TlsMaterial {
        acceptor: tokio_rustls::TlsAcceptor::from(Arc::new(server_config)),
        ca: ca_cert.der().clone(),
    }
}
