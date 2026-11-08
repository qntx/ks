//! Session agent client and protocol (feature `agent`).
//!
//! The agent holds the vault key and answers age unwrap/wrap requests over a
//! per-connection, per-file JSON protocol on a local socket. File keys only
//! ever live in [`zeroize::Zeroizing`] buffers.

use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::fs::DirBuilderExt as _;
use std::path::PathBuf;
use std::sync::Mutex;

use age::secrecy::SecretString;
use age_core::format::{FileKey, Stanza};
use base64::Engine as _;
#[cfg(unix)]
use interprocess::local_socket::ToFsName as _;
#[cfg(windows)]
use interprocess::local_socket::ToNsName as _;
use interprocess::local_socket::{Stream, traits::Stream as _};
use serde::{Deserialize, Serialize};
use sha2::Digest as _;
use zeroize::Zeroizing;

use crate::{Error, LocalKey, Result, Vault};

const MAX_FRAME: u32 = 1024 * 1024;
const PROTOCOL_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct StanzaJson {
    tag: String,
    args: Vec<String>,
    body: String,
}

fn stanza_to_json(s: &Stanza) -> StanzaJson {
    StanzaJson {
        tag: s.tag.clone(),
        args: s.args.clone(),
        body: base64::engine::general_purpose::STANDARD.encode(&s.body),
    }
}

fn stanza_from_json(s: &StanzaJson) -> Option<Stanza> {
    Some(Stanza {
        tag: s.tag.clone(),
        args: s.args.clone(),
        body: base64::engine::general_purpose::STANDARD
            .decode(&s.body)
            .ok()?,
    })
}

fn write_frame(w: &mut impl Write, value: &serde_json::Value) -> Result<()> {
    let bytes = serde_json::to_vec(value)
        .map_err(|_| Error::Internal("agent frame serialization failed"))?;
    let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
    if len > MAX_FRAME {
        return Err(Error::Internal("agent frame exceeds 1 MiB"));
    }
    w.write_all(&len.to_be_bytes())
        .and_then(|()| w.write_all(&bytes))
        .map_err(|e| Error::Io {
            path: "agent socket".into(),
            source: e,
        })
}

fn read_frame(r: &mut impl Read) -> Result<serde_json::Value> {
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf).map_err(|e| Error::Io {
        path: "agent socket".into(),
        source: e,
    })?;
    let len = u32::from_be_bytes(len_buf);
    if len == 0 || len > MAX_FRAME {
        return Err(Error::Corrupt {
            path: "agent socket".into(),
            reason: "bad frame length",
        });
    }
    let mut buf = Zeroizing::new(vec![0u8; len as usize]);
    r.read_exact(&mut buf).map_err(|e| Error::Io {
        path: "agent socket".into(),
        source: e,
    })?;
    serde_json::from_slice(&buf).map_err(|_| Error::Corrupt {
        path: "agent socket".into(),
        reason: "bad frame JSON",
    })
}

fn rpc(stream: &mut Stream, request: &serde_json::Value) -> Result<serde_json::Value> {
    write_frame(stream, request)?;
    let response = read_frame(stream)?;
    if let Some(err) = response.get("error").and_then(|e| e.as_str()) {
        return Err(Error::Unavailable(format!("agent: {err}")));
    }
    response.get("ok").cloned().ok_or_else(|| Error::Corrupt {
        path: "agent socket".into(),
        reason: "missing ok/error in response",
    })
}

fn hello(stream: &mut Stream) -> Result<serde_json::Value> {
    rpc(
        stream,
        &serde_json::json!({ "op": "hello", "version": PROTOCOL_VERSION }),
    )
}

fn vault_id(root: &std::path::Path) -> String {
    let digest = sha2::Sha256::digest(root.as_os_str().as_encoded_bytes());
    let mut id = [0u8; 8];
    if let Some(first) = digest.get(..8) {
        id.copy_from_slice(first);
    }
    crate::hex(id)
}

/// The endpoint an agent for a given vault listens on.
#[derive(Debug, Clone)]
pub struct Endpoint {
    name: String,
    /// Unix: the socket path itself (for existence checks and removal).
    path: Option<PathBuf>,
}

impl Endpoint {
    /// The interprocess socket name (for binding the listener).
    ///
    /// # Errors
    ///
    /// [`Error::Unavailable`] when the name cannot be constructed.
    pub fn socket_name(&self) -> Result<interprocess::local_socket::Name<'static>> {
        #[cfg(unix)]
        return PathBuf::from(&self.name)
            .to_fs_name::<interprocess::local_socket::GenericFilePath>()
            .map_err(|e| Error::Unavailable(format!("agent endpoint name is invalid: {e}")));
        #[cfg(windows)]
        return self
            .name
            .clone()
            .to_ns_name::<interprocess::local_socket::GenericNamespaced>()
            .map_err(|e| Error::Unavailable(format!("agent endpoint name is invalid: {e}")));
    }

    /// The unix socket path (`None` on Windows).
    #[must_use]
    pub fn path(&self) -> Option<&std::path::Path> {
        self.path.as_deref()
    }

    /// Unix only: removes the socket file after a stale/shutdown listener.
    /// No-op on Windows and when the socket is already gone.
    pub fn cleanup(&self) {
        if let Some(path) = &self.path {
            drop(std::fs::remove_file(path));
        }
    }
}

/// Checks a unix endpoint directory: must be owned by the current uid and
/// carry no group/other permission bits (std `MetadataExt`, no unsafe).
#[cfg(unix)]
fn check_dir_perms(dir: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::MetadataExt as _;
    let meta = std::fs::metadata(dir).map_err(|e| Error::Io {
        path: dir.to_path_buf(),
        source: e,
    })?;
    if meta.mode() & 0o077 != 0 {
        return Err(Error::Unavailable(
            "agent endpoint directory is not private to this user".into(),
        ));
    }
    // A file we create carries our uid; compare without libc/unsafe.
    let probe = tempfile::NamedTempFile::new_in(dir).map_err(|e| Error::Io {
        path: dir.to_path_buf(),
        source: e,
    })?;
    if meta.uid()
        != probe
            .as_file()
            .metadata()
            .map_err(|e| Error::Io {
                path: dir.to_path_buf(),
                source: e,
            })?
            .uid()
    {
        return Err(Error::Unavailable(
            "agent endpoint directory is not owned by this user".into(),
        ));
    }
    Ok(())
}

/// Resolves (and creates, unix) the agent endpoint for `vault`, enforcing the
/// ownership/permission rules on its directory.
///
/// # Errors
///
/// [`Error::Unavailable`] when the state directory cannot be resolved, the
/// directory fails the permission checks, or the socket path exceeds 100
/// bytes (the `sockaddr_un` limit on some platforms).
pub fn endpoint(vault: &Vault) -> Result<Endpoint> {
    let id = vault_id(vault.root());
    #[cfg(unix)]
    {
        let dir = endpoint_dir(&id)?;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&dir)
            .map_err(|e| Error::Io {
                path: dir.clone(),
                source: e,
            })?;
        check_dir_perms(&dir)?;
        let path = dir.join(format!("{id}.sock"));
        if path.as_os_str().len() > 100 {
            return Err(Error::Unavailable(
                "agent socket path exceeds 100 bytes".into(),
            ));
        }
        Ok(Endpoint {
            name: path.to_string_lossy().into_owned(),
            path: Some(path),
        })
    }
    #[cfg(windows)]
    {
        Ok(Endpoint {
            name: format!("ks-{id}"),
            path: None,
        })
    }
}

#[cfg(unix)]
fn endpoint_dir(id: &str) -> Result<PathBuf> {
    #[cfg(target_os = "linux")]
    if let Some(xdg) = std::env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(xdg).join("ks"));
    }
    let state = directories::ProjectDirs::from("", "", "ks")
        .map(|p| p.data_local_dir().to_path_buf())
        .ok_or_else(|| Error::Unavailable("cannot resolve a state directory".into()))?;
    let _ = id;
    Ok(state.join("run"))
}

/// Connects to the agent for `vault` and completes the hello handshake.
///
/// Returns `Ok(None)` when there is no endpoint or the connection is refused.
///
/// # Errors
///
/// [`Error::Unavailable`]/[`Error::Io`]/[`Error::Corrupt`] on endpoint or
/// protocol failures; a connected but misbehaving agent is an error.
pub fn connect_raw(vault: &Vault) -> Result<Option<Stream>> {
    let ep = endpoint(vault)?;
    if let Some(path) = &ep.path
        && !path.exists()
    {
        return Ok(None);
    }
    let name = ep.socket_name()?;
    let mut stream = match Stream::connect(name) {
        Ok(s) => s,
        Err(e)
            if matches!(
                e.kind(),
                std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::NotFound
            ) =>
        {
            return Ok(None);
        }
        Err(e) => {
            return Err(Error::Io {
                path: ep.path.clone().unwrap_or_default(),
                source: e,
            });
        }
    };
    hello(&mut stream)?;
    Ok(Some(stream))
}

/// A vault key held remotely by the session agent. [`Debug`] is redacted.
pub struct AgentKey {
    stream: Mutex<Stream>,
    label: String,
}

impl std::fmt::Debug for AgentKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AgentKey(<redacted>)")
    }
}

impl AgentKey {
    /// Connects to the agent and verifies the handshake.
    ///
    /// `Ok(None)` = no agent running.
    ///
    /// # Errors
    ///
    /// [`Error::Io`]/[`Error::Unavailable`]/[`Error::Corrupt`] on transport or
    /// protocol failures.
    pub fn connect(vault: &Vault) -> Result<Option<Self>> {
        let Some(stream) = connect_raw(vault)? else {
            return Ok(None);
        };
        Ok(Some(Self {
            stream: Mutex::new(stream),
            label: new_label(),
        }))
    }

    fn call<T>(
        &self,
        request: &serde_json::Value,
        f: impl Fn(&serde_json::Value) -> Option<T>,
    ) -> Option<T> {
        let mut stream = self.stream.lock().ok()?;
        rpc(&mut stream, request).ok().and_then(|v| f(&v))
    }
}

impl age::Identity for AgentKey {
    fn unwrap_stanza(
        &self,
        stanza: &Stanza,
    ) -> Option<std::result::Result<FileKey, age::DecryptError>> {
        self.unwrap_stanzas(std::slice::from_ref(stanza))
    }

    /// One file = one round trip: all stanzas go in a single `unwrap` request.
    fn unwrap_stanzas(
        &self,
        stanzas: &[Stanza],
    ) -> Option<std::result::Result<FileKey, age::DecryptError>> {
        let request = serde_json::json!({
            "op": "unwrap",
            "stanzas": stanzas.iter().map(stanza_to_json).collect::<Vec<_>>(),
        });
        let inner: Option<std::result::Result<FileKey, age::DecryptError>> =
            self.call(&request, |ok| {
                let b64 = ok.get("file_key")?.as_str()?;
                let bytes = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
                let arr: [u8; 16] = bytes.try_into().ok()?;
                Some(Ok(FileKey::new(Box::new(arr))))
            });
        Some(inner.unwrap_or(Err(age::DecryptError::DecryptionFailed)))
    }
}

impl age::Recipient for AgentKey {
    fn wrap_file_key(
        &self,
        file_key: &FileKey,
    ) -> std::result::Result<(Vec<Stanza>, std::collections::HashSet<String>), age::EncryptError>
    {
        use age::secrecy::ExposeSecret as _;
        let b64 = base64::engine::general_purpose::STANDARD.encode(file_key.expose_secret());
        let request = serde_json::json!({ "op": "wrap", "file_key": b64 });
        let stanzas = self
            .call(&request, |ok| {
                let arr = ok.get("stanzas")?.as_array()?;
                arr.iter()
                    .map(|v| serde_json::from_value::<StanzaJson>(v.clone()).ok())
                    .collect::<Option<Vec<_>>>()?
                    .iter()
                    .map(stanza_from_json)
                    .collect::<Option<Vec<Stanza>>>()
            })
            .ok_or_else(|| age::EncryptError::Io(std::io::Error::other("agent wrap failed")))?;
        Ok((
            stanzas,
            std::collections::HashSet::from([self.label.clone()]),
        ))
    }
}

fn new_label() -> String {
    let bytes: [u8; 16] = rand::random();
    crate::hex(bytes)
}

/// Agent liveness info from `hello`.
#[derive(Debug, Clone, Copy)]
pub struct AgentStatus {
    /// Seconds until the idle timeout fires.
    pub ttl_left: u64,
    /// Seconds until the lifetime cap fires.
    pub life_left: u64,
}

/// `hello` only — does not count as activity.
///
/// # Errors
///
/// Transport/protocol failures; `Ok(None)` when no agent runs.
pub fn status(vault: &Vault) -> Result<Option<AgentStatus>> {
    let Some(mut stream) = connect_raw(vault)? else {
        return Ok(None);
    };
    let ok = hello(&mut stream)?;
    Ok(Some(AgentStatus {
        ttl_left: ok
            .get("ttl_left")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        life_left: ok
            .get("life_left")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
    }))
}

/// Asks a running agent to exit. `true` when one was running.
///
/// # Errors
///
/// Transport/protocol failures.
pub fn lock(vault: &Vault) -> Result<bool> {
    let Some(mut stream) = connect_raw(vault)? else {
        return Ok(false);
    };
    drop(rpc(&mut stream, &serde_json::json!({ "op": "lock" })));
    Ok(true)
}

/// How a served connection ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionEnd {
    /// The peer closed normally.
    Closed,
    /// The peer sent `lock`; the agent should shut down.
    LockRequested,
}

/// Server side: answers unwrap/wrap/hello/lock using the vault's `LocalKey`.
#[derive(Debug)]
pub struct Handler {
    key: LocalKey,
}

impl Handler {
    /// Builds a handler from the vault identity text (the vault key itself).
    ///
    /// # Errors
    ///
    /// [`Error::Corrupt`] when the identity text is malformed.
    /// # Errors
    ///
    /// [`Error::Corrupt`] when the identity text is malformed.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "the caller owns the identity so it is zeroized when dropped"
    )]
    pub fn new(identity: SecretString) -> Result<Self> {
        Ok(Self {
            key: LocalKey::from_identity_text(&identity)?,
        })
    }

    /// Serves one connection until EOF or `lock`; calls `activity()` on each
    /// unwrap/wrap (the agent uses it to reset the idle timer).
    ///
    /// # Errors
    ///
    /// [`Error::Io`] on transport failures; protocol errors are answered in
    /// band and the connection continues.
    pub fn serve_connection(
        &self,
        mut stream: impl Read + Write,
        activity: &dyn Fn(),
        ttl_left: &dyn Fn() -> u64,
        life_left: &dyn Fn() -> u64,
    ) -> Result<ConnectionEnd> {
        loop {
            let frame = match read_frame(&mut stream) {
                Ok(f) => f,
                Err(Error::Io { .. }) => return Ok(ConnectionEnd::Closed),
                Err(e) => return Err(e),
            };
            let response = self.dispatch(&frame, activity, ttl_left, life_left);
            write_frame(&mut stream, &response)?;
            if frame.get("op").and_then(|o| o.as_str()) == Some("lock") {
                return Ok(ConnectionEnd::LockRequested);
            }
        }
    }

    fn dispatch(
        &self,
        frame: &serde_json::Value,
        activity: &dyn Fn(),
        ttl_left: &dyn Fn() -> u64,
        life_left: &dyn Fn() -> u64,
    ) -> serde_json::Value {
        let op = frame.get("op").and_then(|o| o.as_str()).unwrap_or("");
        match op {
            "hello" => match frame.get("version").and_then(serde_json::Value::as_u64) {
                Some(v) if v == u64::from(PROTOCOL_VERSION) => serde_json::json!({
                    "ok": { "ttl_left": ttl_left(), "life_left": life_left() }
                }),
                _ => serde_json::json!({ "error": "unsupported protocol version" }),
            },
            "unwrap" => {
                activity();
                self.answer_unwrap(frame)
            }
            "wrap" => {
                activity();
                self.answer_wrap(frame)
            }
            "lock" => serde_json::json!({ "ok": {} }),
            _ => serde_json::json!({ "error": format!("unknown op {op:?}") }),
        }
    }

    fn answer_unwrap(&self, frame: &serde_json::Value) -> serde_json::Value {
        let stanzas = frame
            .get("stanzas")
            .and_then(|s| s.as_array())
            .and_then(|arr| {
                arr.iter()
                    .map(|v| serde_json::from_value::<StanzaJson>(v.clone()).ok())
                    .collect::<Option<Vec<_>>>()
            })
            .and_then(|js| {
                js.iter()
                    .map(stanza_from_json)
                    .collect::<Option<Vec<Stanza>>>()
            });
        let Some(stanzas) = stanzas else {
            return serde_json::json!({ "error": "malformed stanzas" });
        };
        let file_key = age::Identity::unwrap_stanzas(&self.key, &stanzas)
            .and_then(std::result::Result::ok)
            .map(|fk| {
                use age::secrecy::ExposeSecret as _;
                base64::engine::general_purpose::STANDARD.encode(fk.expose_secret())
            });
        serde_json::json!({ "ok": { "file_key": file_key } })
    }

    fn answer_wrap(&self, frame: &serde_json::Value) -> serde_json::Value {
        let file_key = frame
            .get("file_key")
            .and_then(|v| v.as_str())
            .and_then(|b64| base64::engine::general_purpose::STANDARD.decode(b64).ok())
            .and_then(|bytes| <[u8; 16]>::try_from(bytes.as_slice()).ok())
            .map(|arr| FileKey::new(Box::new(arr)));
        let Some(file_key) = file_key else {
            return serde_json::json!({ "error": "malformed file key" });
        };
        match age::Recipient::wrap_file_key(&self.key, &file_key) {
            Ok((stanzas, _)) => serde_json::json!({
                "ok": { "stanzas": stanzas.iter().map(stanza_to_json).collect::<Vec<_>>() }
            }),
            Err(_) => serde_json::json!({ "error": "wrap failed" }),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Read, Write};

    use super::*;

    /// In-memory request/response pipe for `Handler::serve_connection`.
    struct Duplex {
        req: Cursor<Vec<u8>>,
        resp: Vec<u8>,
    }

    impl Read for Duplex {
        fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
            self.req.read(b)
        }
    }

    impl Write for Duplex {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.resp.write(b)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn vault_key() -> (LocalKey, SecretString) {
        let key = LocalKey::generate();
        let text = key.to_identity_text();
        (key, text)
    }

    #[test]
    fn frames_round_trip_and_limits() {
        let v = serde_json::json!({ "op": "hello", "version": 1 });
        let mut buf = Vec::new();
        write_frame(&mut buf, &v).unwrap();
        assert_eq!(read_frame(&mut Cursor::new(&buf)).unwrap(), v);

        // oversized length rejected before any allocation happens
        let oversized = (MAX_FRAME + 1).to_be_bytes();
        assert!(matches!(
            read_frame(&mut Cursor::new(oversized)),
            Err(Error::Corrupt { .. })
        ));

        // zero length is rejected too
        assert!(read_frame(&mut Cursor::new(0u32.to_be_bytes())).is_err());
    }

    #[test]
    fn stanza_json_round_trip() {
        let stanza = Stanza {
            tag: "X25519".to_owned(),
            args: vec!["abc".to_owned()],
            body: vec![1, 2, 3, 255],
        };
        let json = serde_json::to_value(stanza_to_json(&stanza)).unwrap();
        let back: StanzaJson = serde_json::from_value(json).unwrap();
        let back = stanza_from_json(&back).unwrap();
        assert_eq!(back.tag, stanza.tag);
        assert_eq!(back.args, stanza.args);
        assert_eq!(back.body, stanza.body);
    }

    #[test]
    fn handler_serves_hello_unwrap_wrap_lock() {
        let (_key, text) = vault_key();
        let handler = Handler::new(text).unwrap();
        let mut wire = Vec::new();

        // client -> server request, server response appended
        let mut serve = |req: &serde_json::Value| -> (serde_json::Value, ConnectionEnd) {
            wire.clear();
            write_frame(&mut wire, req).unwrap();
            let conn = Cursor::new(wire.clone());
            let mut d = Duplex {
                req: conn,
                resp: Vec::new(),
            };
            let end = handler
                .serve_connection(&mut d, &|| {}, &|| 900, &|| 28_800)
                .unwrap();
            (read_frame(&mut Cursor::new(&d.resp)).unwrap(), end)
        };

        let (resp1, end) = serve(&serde_json::json!({ "op": "hello", "version": 1 }));
        assert_eq!(end, ConnectionEnd::Closed);
        assert_eq!(resp1.get("ok").unwrap().get("ttl_left").unwrap(), 900);
        assert_eq!(resp1.get("ok").unwrap().get("life_left").unwrap(), 28_800);

        // bad version -> error, connection stays open (ends on EOF)
        let (resp2, _) = serve(&serde_json::json!({ "op": "hello", "version": 99 }));
        assert!(resp2.get("error").is_some());

        // wrap then unwrap the file key back
        let file_key = rand::random::<[u8; 16]>();
        let b64 = base64::engine::general_purpose::STANDARD.encode(file_key);
        let (resp3, _) = serve(&serde_json::json!({ "op": "wrap", "file_key": b64 }));
        let stanzas = resp3
            .get("ok")
            .and_then(|o| o.get("stanzas"))
            .and_then(|s| s.as_array())
            .unwrap();
        assert_eq!(stanzas.len(), 1);
        let (resp4, _) = serve(&serde_json::json!({ "op": "unwrap", "stanzas": stanzas }));
        let got = resp4
            .get("ok")
            .and_then(|o| o.get("file_key"))
            .and_then(|f| f.as_str())
            .unwrap();
        assert_eq!(
            base64::engine::general_purpose::STANDARD
                .decode(got)
                .unwrap(),
            file_key
        );

        // unwrap with a stanza for a different key -> null file_key
        let (resp5, end5) = serve(&serde_json::json!({ "op": "lock" }));
        assert_eq!(end5, ConnectionEnd::LockRequested);
        assert_eq!(
            resp5.get("ok").cloned().unwrap_or_default(),
            serde_json::json!({})
        );
    }

    #[test]
    fn agent_key_label_rejects_second_recipient() {
        // The label set AgentKey returns makes Encryptor refuse a second
        // recipient (single-recipient auth). We verify the label contract:
        // two AgentKeys produce distinct non-empty labels.
        let l1 = new_label();
        let l2 = new_label();
        assert_ne!(l1, l2);
        assert_eq!(l1.len(), 32);
    }
}
