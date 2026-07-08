//! Camada de rede P2P (Fases 3–5, plano.md §5.1/§5.3/§5.6). ATRÁS DA FEATURE `p2p`:
//! iroh é dependência pesada e de API que gira rápido, então o build padrão compila
//! sem ela (identidade + banco + pareamento + UI + anexos locais já funcionam) e a
//! mensageria ao vivo entra com `tauri dev -- --features p2p`.
//!
//! Sobre o canal já autenticado do iroh (ambos conhecem o node_id um do outro) roda um
//! **double ratchet (Olm/vodozemac, ratchet.rs)**: a 1ª mensagem pra um par faz um
//! handshake de pré-chave (X3DH); as seguintes usam a sessão. O conteúdo interno é um
//! JSON `{k:"text"|"file", ...}`; **anexos** viajam comprimidos (zstd) e cifrados com
//! uma chave de uso único (essa chave vai dentro do JSON cifrado pelo ratchet). O
//! receptor confirma com um ACK antes do fecho — dá pra marcar a mensagem como enviada.
//!
//! Usa o runtime async do PRÓPRIO Tauri (`tauri::async_runtime`).
//!
//! ⚠️ Compila com `--features p2p`, mas o handshake/transferência ponta a ponta ainda
//! **precisam do teste com 2 instâncias**. Núcleo do ratchet e da mídia cobertos por
//! testes (`ratchet.rs`, `media.rs`). Retomada de transferência (iroh-blobs) é upgrade
//! futuro; este corte manda o arquivo inteiro pelo stream cifrado.

#[cfg_attr(not(feature = "p2p"), allow(dead_code))]
pub const EVENT_MESSAGE_IN: &str = "message-in";
/// Recibo de leitura recebido: a UI atualiza os ✓✓ da conversa.
#[cfg_attr(not(feature = "p2p"), allow(dead_code))]
pub const EVENT_RECEIPTS: &str = "receipts";
#[cfg(feature = "p2p")]
const ALPN: &[u8] = b"taylorchat/msg/0";

// ───────────────────────────── build padrão (sem rede) ─────────────────────────────
#[cfg(not(feature = "p2p"))]
pub fn start(_app: tauri::AppHandle, _secret: [u8; 32]) {
    eprintln!("[taylorchat] rede P2P desativada (compile com --features p2p)");
}
#[cfg(not(feature = "p2p"))]
pub async fn send_text(_app: &tauri::AppHandle, _peer: &str, _body: &str) -> Result<(), String> {
    Err("mensageria ao vivo requer build com --features p2p (Fase 3)".into())
}
#[cfg(not(feature = "p2p"))]
pub async fn send_file(
    _app: &tauri::AppHandle,
    _peer: &str,
    _filename: &str,
    _mime: &str,
    _source_path: &str,
) -> Result<(), String> {
    Err("envio de arquivo requer build com --features p2p (Fase 5)".into())
}
#[cfg(not(feature = "p2p"))]
pub async fn send_read(_app: &tauri::AppHandle, _peer: &str) -> Result<(), String> {
    Ok(()) // sem rede: nada a confirmar
}

// ─────────────────────────────── build com iroh ───────────────────────────────
#[cfg(feature = "p2p")]
mod imp {
    use super::{ALPN, EVENT_MESSAGE_IN, EVENT_RECEIPTS};
    use crate::db::Db;
    use crate::ratchet::{self, PreKeyBundle};
    use base64::Engine;
    use iroh::endpoint::{Connection, RecvStream, SendStream};
    use iroh::{Endpoint, NodeId, SecretKey};
    use rand::RngCore;
    use serde_json::{json, Value};
    use std::io::{Read, Write};
    use std::sync::{Mutex, OnceLock};
    use tauri::{Emitter, Manager};
    use vodozemac::olm::Account;

    static ENDPOINT: OnceLock<Endpoint> = OnceLock::new();
    static ACCOUNT: OnceLock<Mutex<Account>> = OnceLock::new();

    // Cabeçalhos são pequenos; os frames de chunk carregam ~1 MiB comprimido+cifrado.
    const MAX_FRAME: usize = 4 << 20; // 4 MiB (teto de um frame)
    const CHUNK: usize = 1 << 20; // 1 MiB de arquivo cru por pedaço

    fn b64(bytes: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }
    fn unb64(s: &str) -> Result<Vec<u8>, String> {
        base64::engine::general_purpose::STANDARD
            .decode(s.as_bytes())
            .map_err(|e| format!("base64 inválido: {e}"))
    }

    pub fn start(app: tauri::AppHandle, secret: [u8; 32]) {
        let db = app.state::<Db>();
        let account = match crate::db::meta_get(&db, "olm_account") {
            Ok(Some(bytes)) => {
                ratchet::account_from_bytes(&bytes).unwrap_or_else(|_| ratchet::new_account())
            }
            _ => ratchet::new_account(),
        };
        let _ = crate::db::meta_set(&db, "olm_account", &ratchet::account_to_bytes(&account));
        let _ = ACCOUNT.set(Mutex::new(account));

        let sk = SecretKey::from_bytes(&secret);
        let endpoint = match tauri::async_runtime::block_on(async {
            Endpoint::builder()
                .secret_key(sk)
                .alpns(vec![ALPN.to_vec()])
                .discovery_n0()
                .bind()
                .await
        }) {
            Ok(ep) => ep,
            Err(e) => {
                eprintln!("[taylorchat] falha ao subir endpoint iroh: {e}");
                return;
            }
        };
        eprintln!("[taylorchat] endpoint iroh no ar — node {}", endpoint.node_id());

        let ep_accept = endpoint.clone();
        tauri::async_runtime::spawn(async move {
            while let Some(incoming) = ep_accept.accept().await {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = handle_conn(app, incoming).await {
                        eprintln!("[taylorchat] conexão de entrada falhou: {e}");
                    }
                });
            }
        });
        let _ = ENDPOINT.set(endpoint);
    }

    // ── framing ────────────────────────────────────────────────────────
    async fn write_frame(s: &mut SendStream, data: &[u8]) -> Result<(), String> {
        let len = (data.len() as u32).to_be_bytes();
        s.write_all(&len).await.map_err(|e| e.to_string())?;
        s.write_all(data).await.map_err(|e| e.to_string())?;
        Ok(())
    }
    async fn read_frame(r: &mut RecvStream) -> Result<Vec<u8>, String> {
        let mut len = [0u8; 4];
        r.read_exact(&mut len).await.map_err(|e| e.to_string())?;
        let n = u32::from_be_bytes(len) as usize;
        if n > MAX_FRAME {
            return Err("frame grande demais".into());
        }
        let mut buf = vec![0u8; n];
        r.read_exact(&mut buf).await.map_err(|e| e.to_string())?;
        Ok(buf)
    }
    fn account_locked() -> Result<std::sync::MutexGuard<'static, Account>, String> {
        ACCOUNT
            .get()
            .ok_or("conta Olm não iniciada".to_string())?
            .lock()
            .map_err(|_| "conta Olm corrompida".to_string())
    }

    // ── envio (unificado texto/arquivo) ────────────────────────────────
    pub async fn send_text(app: &tauri::AppHandle, peer: &str, body: &str) -> Result<(), String> {
        let inner = json!({ "k": "text", "body": body }).to_string();
        let (_conn, mut s, mut r) = send_header(app, peer, &inner, json!({})).await?;
        ack(&mut s, &mut r).await
    }

    /// Recibo de leitura: avisa o par que li as mensagens dele. Só faz sentido se já
    /// existe sessão (a gente já conversou); sem sessão, não há nada a confirmar.
    pub async fn send_read(app: &tauri::AppHandle, peer: &str) -> Result<(), String> {
        let db = app.state::<Db>();
        if crate::db::session_get(&db, peer)?.is_none() {
            return Ok(());
        }
        let inner = json!({ "k": "read" }).to_string();
        let (_conn, mut s, mut r) = send_header(app, peer, &inner, json!({})).await?;
        ack(&mut s, &mut r).await
    }

    /// Envia um arquivo **em streaming**: metadados (com a chave de uso único) vão no
    /// cabeçalho cifrado pelo ratchet; o conteúdo segue em pedaços de 1 MiB, cada um
    /// comprimido (zstd) + cifrado. Nada do arquivo inteiro fica em RAM → arquivos
    /// grandes ok. Lê do disco em `source_path`.
    pub async fn send_file(
        app: &tauri::AppHandle,
        peer: &str,
        filename: &str,
        mime: &str,
        source_path: &str,
    ) -> Result<(), String> {
        let size = std::fs::metadata(source_path).map(|m| m.len()).unwrap_or(0);
        let mut file_key = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut file_key);
        let inner = json!({
            "k": "file",
            "filename": filename,
            "mime": mime,
            "size": size,
            "fileKey": b64(&file_key),
        })
        .to_string();
        let (_conn, mut s, mut r) =
            send_header(app, peer, &inner, json!({ "streamed": true })).await?;

        let mut f = std::fs::File::open(source_path)
            .map_err(|e| format!("falha ao abrir '{source_path}': {e}"))?;
        let mut buf = vec![0u8; CHUNK];
        loop {
            let n = f.read(&mut buf).map_err(|e| format!("falha ao ler anexo: {e}"))?;
            if n == 0 {
                break;
            }
            let comp = crate::media::compress(&buf[..n])?;
            let enc = crate::crypto::encrypt(&file_key, &comp)?;
            write_frame(&mut s, &enc).await?;
        }
        write_frame(&mut s, &[]).await?; // frame vazio = fim
        ack(&mut s, &mut r).await
    }

    /// Abre a conexão, faz o handshake/cifra do conteúdo interno e escreve o cabeçalho
    /// (mesclando `extra`, ex.: `{"streamed":true}`). Devolve os streams abertos pro
    /// caller mandar o payload (chunks) e ler o ACK. A `Connection` é devolvida pra
    /// ficar viva até o fim do envio.
    async fn send_header(
        app: &tauri::AppHandle,
        peer: &str,
        inner_json: &str,
        extra: Value,
    ) -> Result<(Connection, SendStream, RecvStream), String> {
        let endpoint = ENDPOINT.get().ok_or("rede não iniciada".to_string())?;
        let db = app.state::<Db>();
        let bytes = crate::identity::hex_decode_32(peer)?;
        let node_id = NodeId::from_bytes(&bytes).map_err(|e| e.to_string())?;
        let conn = endpoint.connect(node_id, ALPN).await.map_err(|e| e.to_string())?;
        let (mut send_s, mut recv_s) = conn.open_bi().await.map_err(|e| e.to_string())?;

        let mut header = if let Some(sbytes) = crate::db::session_get(&db, peer)? {
            // Sessão existente → mensagem normal.
            let wire = {
                let mut session = ratchet::session_from_bytes(&sbytes)?;
                let w = ratchet::encrypt(&mut session, inner_json);
                crate::db::session_set(&db, peer, &ratchet::session_to_bytes(&session))?;
                w
            };
            json!({ "t": "msg", "olm": b64(&wire) })
        } else {
            // Par novo → pede bundle, abre sessão (X3DH) e manda a PreKey.
            let req = serde_json::to_vec(&json!({ "t": "req_prekey" })).map_err(|e| e.to_string())?;
            write_frame(&mut send_s, &req).await?;
            let resp = read_frame(&mut recv_s).await?;
            let v: Value = serde_json::from_slice(&resp).map_err(|e| e.to_string())?;
            let bundle = PreKeyBundle {
                identity_key: v["identity_key"].as_str().ok_or("bundle sem identity_key")?.to_string(),
                one_time_key: v["one_time_key"].as_str().ok_or("bundle sem one_time_key")?.to_string(),
            };
            let (session, wire, our_identity) = {
                let acct = account_locked()?;
                let (session, wire) = ratchet::start_outbound(&acct, &bundle, inner_json)?;
                (session, wire, acct.curve25519_key().to_base64())
            };
            crate::db::session_set(&db, peer, &ratchet::session_to_bytes(&session))?;
            json!({ "t": "msg_prekey", "sender_identity": our_identity, "olm": b64(&wire) })
        };
        if let Value::Object(extra) = extra {
            for (k, val) in extra {
                header[k] = val;
            }
        }
        write_frame(&mut send_s, &serde_json::to_vec(&header).map_err(|e| e.to_string())?).await?;
        Ok((conn, send_s, recv_s))
    }

    /// Espera o ACK do receptor (confirma que processou tudo) e fecha o lado de envio.
    async fn ack(send_s: &mut SendStream, recv_s: &mut RecvStream) -> Result<(), String> {
        let mut ack = [0u8; 1];
        recv_s.read_exact(&mut ack).await.map_err(|e| format!("sem confirmação de entrega: {e}"))?;
        send_s.finish().map_err(|e| e.to_string())?;
        Ok(())
    }

    // ── recepção ───────────────────────────────────────────────────────
    async fn handle_conn(
        app: tauri::AppHandle,
        incoming: iroh::endpoint::Incoming,
    ) -> Result<(), String> {
        let conn = incoming.await.map_err(|e| e.to_string())?;
        let peer: NodeId = conn.remote_node_id().map_err(|e| e.to_string())?;
        let peer_hex = hex(peer.as_bytes());
        loop {
            let (mut send_s, mut recv_s) = match conn.accept_bi().await {
                Ok(x) => x,
                Err(_) => break,
            };
            if let Err(e) = handle_stream(&app, &peer_hex, &mut send_s, &mut recv_s).await {
                eprintln!("[taylorchat] stream de entrada falhou: {e}");
            }
        }
        Ok(())
    }

    async fn handle_stream(
        app: &tauri::AppHandle,
        peer_hex: &str,
        send_s: &mut SendStream,
        recv_s: &mut RecvStream,
    ) -> Result<(), String> {
        let db = app.state::<Db>();
        let first = read_frame(recv_s).await?;
        let v: Value = serde_json::from_slice(&first).map_err(|e| e.to_string())?;
        match v["t"].as_str() {
            Some("req_prekey") => {
                let bundle = {
                    let mut acct = account_locked()?;
                    let b = ratchet::our_bundle(&mut acct)?;
                    let _ = crate::db::meta_set(&db, "olm_account", &ratchet::account_to_bytes(&acct));
                    b
                };
                let payload = serde_json::to_vec(&json!({
                    "t": "bundle",
                    "identity_key": bundle.identity_key,
                    "one_time_key": bundle.one_time_key,
                }))
                .map_err(|e| e.to_string())?;
                write_frame(send_s, &payload).await?;

                let second = read_frame(recv_s).await?;
                let v2: Value = serde_json::from_slice(&second).map_err(|e| e.to_string())?;
                process_message(app, &db, peer_hex, &v2, recv_s).await?;
            }
            Some("msg") => {
                process_message(app, &db, peer_hex, &v, recv_s).await?;
            }
            other => return Err(format!("frame desconhecido: {other:?}")),
        }
        // ACK: confirma o processamento pro remetente.
        send_s.write_all(&[1u8]).await.map_err(|e| e.to_string())?;
        send_s.finish().map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Decifra a OlmMessage (abrindo sessão se for PreKey), lê o anexo se houver, e
    /// grava a mensagem no banco emitindo o evento pro webview.
    async fn process_message(
        app: &tauri::AppHandle,
        db: &Db,
        peer_hex: &str,
        v: &Value,
        recv_s: &mut RecvStream,
    ) -> Result<(), String> {
        let is_prekey = v["t"].as_str() == Some("msg_prekey");
        let olm = unb64(v["olm"].as_str().ok_or("faltou olm")?)?;

        let inner = if is_prekey {
            let sender_identity = v["sender_identity"].as_str().ok_or("faltou sender_identity")?;
            let (session, plaintext) = {
                let mut acct = account_locked()?;
                let r = ratchet::start_inbound(&mut acct, sender_identity, &olm)?;
                let _ = crate::db::meta_set(db, "olm_account", &ratchet::account_to_bytes(&acct));
                r
            };
            crate::db::session_set(db, peer_hex, &ratchet::session_to_bytes(&session))?;
            plaintext
        } else {
            let sbytes = crate::db::session_get(db, peer_hex)?
                .ok_or("mensagem sem sessão estabelecida pra esse par")?;
            let mut session = ratchet::session_from_bytes(&sbytes)?;
            let pt = ratchet::decrypt(&mut session, &olm)?;
            crate::db::session_set(db, peer_hex, &ratchet::session_to_bytes(&session))?;
            pt
        };

        let iv: Value = serde_json::from_str(&inner).map_err(|e| format!("conteúdo inválido: {e}"))?;
        match iv["k"].as_str() {
            Some("text") => {
                let body = iv["body"].as_str().unwrap_or_default();
                let msg = crate::db::record_incoming(db, peer_hex, body)?;
                let _ = app.emit(EVENT_MESSAGE_IN, &msg);
            }
            Some("file") => {
                let filename = iv["filename"].as_str().unwrap_or("arquivo");
                let mime = iv["mime"].as_str().unwrap_or("application/octet-stream");
                let size = iv["size"].as_u64().unwrap_or(0);
                let key_vec = unb64(iv["fileKey"].as_str().ok_or("arquivo sem chave")?)?;
                let file_key: [u8; 32] =
                    key_vec.try_into().map_err(|_| "chave de arquivo inválida".to_string())?;

                // Recebe os pedaços e grava incremental no disco (sem segurar o arquivo
                // inteiro em RAM). Frame vazio = fim.
                let dest = crate::media::unique_dest(app, filename)?;
                let mut out = std::fs::File::create(&dest)
                    .map_err(|e| format!("falha ao criar '{}': {e}", dest.display()))?;
                loop {
                    let frame = read_frame(recv_s).await?;
                    if frame.is_empty() {
                        break;
                    }
                    let comp = crate::crypto::decrypt(&file_key, &frame)?;
                    let data = crate::media::decompress(&comp)?;
                    out.write_all(&data).map_err(|e| format!("falha ao gravar anexo: {e}"))?;
                }
                out.flush().map_err(|e| e.to_string())?;
                let local_path = dest.to_string_lossy().to_string();
                let meta = json!({
                    "filename": filename, "mime": mime, "size": size, "localPath": local_path,
                })
                .to_string();
                let msg = crate::db::record_file(db, peer_hex, "in", &meta, "received")?;
                let _ = app.emit(EVENT_MESSAGE_IN, &msg);
            }
            // Recibo de leitura: marca as MINHAS mensagens pra esse par como lidas.
            Some("read") => {
                crate::db::mark_out_read(db, peer_hex)?;
                let _ = app.emit(EVENT_RECEIPTS, &json!({ "peer": peer_hex }));
            }
            other => return Err(format!("conteúdo desconhecido: {other:?}")),
        }
        Ok(())
    }

    fn hex(bytes: &[u8]) -> String {
        let mut s = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            s.push_str(&format!("{b:02x}"));
        }
        s
    }
}

#[cfg(feature = "p2p")]
pub use imp::{send_file, send_read, send_text, start};
