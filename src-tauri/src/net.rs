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

// ── Diagnóstico de rede (o .exe esconde o stdout, então logamos pra UI) ─────────
use std::sync::{Mutex as StdMutex, OnceLock as StdOnceLock};
static NET_LOG: StdOnceLock<StdMutex<Vec<String>>> = StdOnceLock::new();

fn now_hms() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{:02}:{:02}:{:02}", (secs / 3600) % 24, (secs / 60) % 60, secs % 60)
}

/// Registra um evento de rede no log em memória e (se `toast`) manda pra UI como aviso.
pub fn report(app: &tauri::AppHandle, line: impl Into<String>, toast: bool) {
    use tauri::Emitter;
    let line = line.into();
    eprintln!("[taylorchat] {line}");
    let log = NET_LOG.get_or_init(|| StdMutex::new(Vec::new()));
    if let Ok(mut v) = log.lock() {
        v.push(format!("{} {line}", now_hms()));
        let len = v.len();
        if len > 200 {
            v.drain(0..len - 200);
        }
    }
    if toast {
        let _ = app.emit("net-error", &line);
    }
}

/// Log de rede recente (pro painel de diagnóstico nas Configurações).
pub fn log_lines() -> Vec<String> {
    NET_LOG.get().and_then(|m| m.lock().ok().map(|v| v.clone())).unwrap_or_default()
}

#[cfg_attr(not(feature = "p2p"), allow(dead_code))]
pub const EVENT_MESSAGE_IN: &str = "message-in";
/// Recibo de leitura recebido: a UI atualiza os ✓✓ da conversa.
#[cfg_attr(not(feature = "p2p"), allow(dead_code))]
pub const EVENT_RECEIPTS: &str = "receipts";
/// Palavra-chave do par chegou/mudou: a UI reavalia o status (confere/diverge).
#[cfg_attr(not(feature = "p2p"), allow(dead_code))]
pub const EVENT_KEYWORD: &str = "keyword";
/// Perfil (nome/foto) do par chegou/mudou: a UI recarrega os contatos.
#[cfg_attr(not(feature = "p2p"), allow(dead_code))]
pub const EVENT_PROFILE: &str = "profile";
/// Presença de um par mudou (online/offline) — a UI atualiza a bolinha em tempo real.
#[cfg_attr(not(feature = "p2p"), allow(dead_code))]
pub const EVENT_PRESENCE: &str = "presence";
#[cfg(feature = "p2p")]
const ALPN: &[u8] = b"taylorchat/msg/0";

// ───────────────────────────── build padrão (sem rede) ─────────────────────────────
#[cfg(not(feature = "p2p"))]
pub fn start(_app: tauri::AppHandle, _secret: [u8; 32]) {
    eprintln!("[taylorchat] rede P2P desativada (compile com --features p2p)");
}
#[cfg(not(feature = "p2p"))]
pub async fn send_text(
    _app: &tauri::AppHandle,
    _peer: &str,
    _thread: &str,
    _body: &str,
    _ts: i64,
) -> Result<(), String> {
    Err("mensageria ao vivo requer build com --features p2p (Fase 3)".into())
}
#[cfg(not(feature = "p2p"))]
pub async fn send_keyword(_app: &tauri::AppHandle, _peer: &str, _hash: &str) -> Result<(), String> {
    Ok(())
}
#[cfg(not(feature = "p2p"))]
pub async fn send_profile(_app: &tauri::AppHandle, _peer: &str) -> Result<(), String> {
    Ok(())
}
#[cfg(not(feature = "p2p"))]
pub async fn send_read(_app: &tauri::AppHandle, _peer: &str, _thread: &str) -> Result<(), String> {
    Ok(())
}
#[cfg(not(feature = "p2p"))]
pub fn is_up() -> bool {
    false
}
#[cfg(not(feature = "p2p"))]
pub async fn watch(_app: &tauri::AppHandle, _peer: &str) -> bool {
    false
}
#[cfg(not(feature = "p2p"))]
#[allow(clippy::too_many_arguments)]
pub async fn send_file(
    _app: &tauri::AppHandle,
    _peer: &str,
    _filename: &str,
    _mime: &str,
    _source_path: &str,
    _transfer_id: &str,
    _file_key: &[u8; 32],
    _ts: i64,
    _thread: &str,
    _sticker: bool,
) -> Result<(), String> {
    Err("envio de arquivo requer build com --features p2p (Fase 5)".into())
}

// ─────────────────────────────── build com iroh ───────────────────────────────
#[cfg(feature = "p2p")]
mod imp {
    use super::{ALPN, EVENT_KEYWORD, EVENT_MESSAGE_IN, EVENT_PRESENCE, EVENT_PROFILE, EVENT_RECEIPTS};
    use crate::db::Db;
    use crate::ratchet::{self, PreKeyBundle};
    use base64::Engine;
    use iroh::endpoint::{Connection, RecvStream, SendStream};
    use iroh::{Endpoint, NodeId, SecretKey};
    use serde_json::{json, Value};
    use std::collections::{HashMap, HashSet};
    use std::io::{Read, Seek, SeekFrom, Write};
    use std::sync::{Arc, Mutex, OnceLock};
    use std::time::Duration;
    use tauri::async_runtime::Mutex as AsyncMutex;
    use tauri::{Emitter, Manager};
    use vodozemac::olm::Account;

    static ENDPOINT: OnceLock<Endpoint> = OnceLock::new();
    static ACCOUNT: OnceLock<Mutex<Account>> = OnceLock::new();
    // Um lock por par serializa TODA operação de ratchet (enviar/receber) com aquele
    // contato — sem isso, mensagens concorrentes leem-e-gravam o mesmo estado do
    // ratchet ao mesmo tempo e o dessincronizam (a mensagem seguinte não decifra).
    static PEER_LOCKS: OnceLock<Mutex<HashMap<String, Arc<AsyncMutex<()>>>>> = OnceLock::new();
    // Conexões QUIC QUENTES por contato: abre uma vez e reusa em TODA mensagem (cada
    // mensagem é um stream bi novo — o QUIC multiplexa). Evita refazer descoberta +
    // handshake a cada envio e mantém o canal vivo pro heartbeat de presença.
    static CONNS: OnceLock<Mutex<HashMap<String, Connection>>> = OnceLock::new();
    // Contatos com um watcher de presença já rodando (evita duplicar tarefas).
    static WATCHING: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

    const HEARTBEAT: Duration = Duration::from_secs(12); // intervalo do ping
    const PROBE_TIMEOUT: Duration = Duration::from_secs(6); // prazo de connect/ping
    const RECONNECT: Duration = Duration::from_secs(15); // espera antes de re-tentar

    fn peer_lock(peer: &str) -> Arc<AsyncMutex<()>> {
        let map = PEER_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
        let mut m = map.lock().expect("peer locks");
        m.entry(peer.to_string()).or_insert_with(|| Arc::new(AsyncMutex::new(()))).clone()
    }

    fn conns() -> &'static Mutex<HashMap<String, Connection>> {
        CONNS.get_or_init(|| Mutex::new(HashMap::new()))
    }

    /// Conexão QUENTE pra um par: devolve a existente se ainda viva, senão abre uma nova
    /// (via discovery n0 + mDNS) e guarda no mapa. Nunca sobrescreve uma conexão viva
    /// que já esteja em uso — a corrida (duas conexões abrindo juntas) é resolvida
    /// mantendo a que chegou primeiro e fechando a duplicata.
    async fn get_conn(peer: &str) -> Result<Connection, String> {
        // Caminho rápido: já temos uma viva.
        if let Ok(m) = conns().lock() {
            if let Some(c) = m.get(peer) {
                if c.close_reason().is_none() {
                    return Ok(c.clone());
                }
            }
        }
        // Abre uma nova SEM segurar o lock durante o await.
        let endpoint = ENDPOINT.get().ok_or("rede não iniciada".to_string())?;
        let bytes = crate::identity::hex_decode_32(peer)?;
        let node_id = NodeId::from_bytes(&bytes).map_err(|e| e.to_string())?;
        let conn = endpoint.connect(node_id, ALPN).await.map_err(|e| e.to_string())?;
        // Reconfere: alguém pode ter conectado enquanto abríamos.
        let mut m = conns().lock().map_err(|_| "conns lock".to_string())?;
        if let Some(c) = m.get(peer) {
            if c.close_reason().is_none() {
                let existing = c.clone();
                drop(m);
                conn.close(0u32.into(), b"dup");
                return Ok(existing);
            }
        }
        m.insert(peer.to_string(), conn.clone());
        Ok(conn)
    }

    /// Descarta a conexão quente de um par (morta/instável) pra forçar reconexão.
    fn drop_conn(peer: &str) {
        if let Ok(mut m) = conns().lock() {
            if let Some(c) = m.remove(peer) {
                c.close(0u32.into(), b"reconnect");
            }
        }
    }

    /// Um ping: abre um stream, manda `{"t":"ping"}` e espera o ACK (o "pong"). Não toca
    /// no ratchet, então não precisa do peer_lock nem vira mensagem no outro lado.
    async fn ping_once(conn: &Connection) -> bool {
        let Ok((mut s, mut r)) = conn.open_bi().await else { return false };
        let Ok(frame) = serde_json::to_vec(&json!({ "t": "ping" })) else { return false };
        if write_frame(&mut s, &frame).await.is_err() {
            return false;
        }
        let mut ack = [0u8; 1];
        let ok = r.read_exact(&mut ack).await.is_ok();
        let _ = s.finish();
        ok
    }

    /// Garante que existe UM watcher de presença rodando pra esse contato.
    pub fn ensure_watch(app: &tauri::AppHandle, peer: &str) {
        let set = WATCHING.get_or_init(|| Mutex::new(HashSet::new()));
        {
            let mut s = match set.lock() {
                Ok(s) => s,
                Err(_) => return,
            };
            if !s.insert(peer.to_string()) {
                return; // já tem watcher
            }
        }
        let app = app.clone();
        let peer = peer.to_string();
        tauri::async_runtime::spawn(async move { watcher(app, peer).await });
    }

    /// Watcher de presença: mantém a conexão quente viva com ping/pong e avisa a UI
    /// quando o par fica online/offline. Ao cair, tenta reconectar (backoff) — quando o
    /// par volta, a presença vira online e a fila escoa pela conexão já quente.
    async fn watcher(app: tauri::AppHandle, peer: String) {
        let mut last: Option<bool> = None;
        loop {
            match tokio::time::timeout(PROBE_TIMEOUT, get_conn(&peer)).await {
                Ok(Ok(conn)) => {
                    emit_presence(&app, &peer, true, &mut last);
                    // Batimento: enquanto os pings voltarem, o par está online.
                    loop {
                        tokio::time::sleep(HEARTBEAT).await;
                        let ok = tokio::time::timeout(PROBE_TIMEOUT, ping_once(&conn))
                            .await
                            .unwrap_or(false);
                        if !ok {
                            break;
                        }
                    }
                    drop_conn(&peer);
                }
                _ => {}
            }
            emit_presence(&app, &peer, false, &mut last);
            tokio::time::sleep(RECONNECT).await;
        }
    }

    fn emit_presence(app: &tauri::AppHandle, peer: &str, online: bool, last: &mut Option<bool>) {
        if *last == Some(online) {
            return; // só emite na mudança
        }
        *last = Some(online);
        let _ = app.emit(EVENT_PRESENCE, &json!({ "peer": peer, "online": online }));
    }

    /// Começa a observar a presença do par (se ainda não) e devolve o status agora.
    /// Idempotente — reusa a conexão quente/o watcher.
    pub async fn watch(app: &tauri::AppHandle, peer: &str) -> bool {
        ensure_watch(app, peer);
        matches!(tokio::time::timeout(PROBE_TIMEOUT, get_conn(peer)).await, Ok(Ok(_)))
    }

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

    pub fn is_up() -> bool {
        ENDPOINT.get().is_some()
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
                // mDNS na LAN: pares na mesma rede se acham sem depender do relay/DNS
                // do n0 (internet). As duas descobertas rodam juntas — a que resolver
                // primeiro conecta.
                .discovery_local_network()
                .bind()
                .await
        }) {
            Ok(ep) => ep,
            Err(e) => {
                super::report(&app, format!("falha ao subir a rede: {e}"), true);
                return;
            }
        };
        super::report(&app, format!("rede no ar — node {}", endpoint.node_id()), false);

        let ep_accept = endpoint.clone();
        tauri::async_runtime::spawn(async move {
            while let Some(incoming) = ep_accept.accept().await {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let app2 = app.clone();
                    if let Err(e) = handle_conn(app, incoming).await {
                        super::report(&app2, format!("recepção falhou: {e}"), true);
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
    /// `ts` = timestamp do remetente (o mesmo guardado localmente) — transmitido pra
    /// os dois lados baterem na auditoria.
    /// Chave de conversa: node (principal) ou node#thread (extra). Os dois lados
    /// montam a mesma chave a partir do node_id + do `thread` que vem no envelope.
    fn convo_key(node: &str, thread: &str) -> String {
        if thread.is_empty() {
            node.to_string()
        } else {
            format!("{node}#{thread}")
        }
    }

    pub async fn send_text(
        app: &tauri::AppHandle,
        peer: &str,
        thread: &str,
        body: &str,
        ts: i64,
    ) -> Result<(), String> {
        let _g = peer_lock(peer).lock_owned().await;
        let inner = json!({ "k": "text", "body": body, "ts": ts, "thread": thread }).to_string();
        let (_conn, mut s, mut r) = send_header(app, peer, &inner, json!({})).await?;
        ack(&mut s, &mut r).await
    }

    /// Manda meu perfil (nome + avatar) pra um contato, que cacheia. Só se já existe
    /// sessão (não vale abrir handshake só pra isso). Por CONTATO, sem `thread`.
    pub async fn send_profile(app: &tauri::AppHandle, peer: &str) -> Result<(), String> {
        let db = app.state::<Db>();
        if crate::db::session_get(&db, peer)?.is_none() {
            return Ok(());
        }
        let _g = peer_lock(peer).lock_owned().await;
        let name = crate::db::meta_get(&db, "profile_name")?
            .map(|b| String::from_utf8_lossy(&b).to_string())
            .unwrap_or_default();
        let avatar = crate::media::my_avatar_bytes(app).map(|b| b64(&b)).unwrap_or_default();
        let inner = json!({ "k": "profile", "name": name, "avatar": avatar }).to_string();
        let (_conn, mut s, mut r) = send_header(app, peer, &inner, json!({})).await?;
        ack(&mut s, &mut r).await
    }

    /// Envia o hash da minha palavra-chave pra esse contato (verificação anti-MITM).
    /// Palavra-chave é por CONTATO (identidade), não por conversa — sem `thread`.
    pub async fn send_keyword(app: &tauri::AppHandle, peer: &str, hash: &str) -> Result<(), String> {
        let _g = peer_lock(peer).lock_owned().await;
        let inner = json!({ "k": "keyword", "hash": hash }).to_string();
        let (_conn, mut s, mut r) = send_header(app, peer, &inner, json!({})).await?;
        ack(&mut s, &mut r).await
    }

    /// Recibo de leitura: avisa o par que li as mensagens dele. Só faz sentido se já
    /// existe sessão (a gente já conversou); sem sessão, não há nada a confirmar.
    pub async fn send_read(app: &tauri::AppHandle, peer: &str, thread: &str) -> Result<(), String> {
        let db = app.state::<Db>();
        if crate::db::session_get(&db, peer)?.is_none() {
            return Ok(());
        }
        let _g = peer_lock(peer).lock_owned().await;
        let inner = json!({ "k": "read", "thread": thread }).to_string();
        let (_conn, mut s, mut r) = send_header(app, peer, &inner, json!({})).await?;
        ack(&mut s, &mut r).await
    }

    /// Envia um arquivo **em streaming e retomável**: metadados (chave de uso único +
    /// `transferId` estável) vão no cabeçalho cifrado pelo ratchet; o receptor responde
    /// de qual chunk retomar; o conteúdo segue em pedaços de 1 MiB (comprimido+cifrado)
    /// a partir daí. Nada do arquivo inteiro fica em RAM → arquivos grandes ok, e uma
    /// queda no meio retoma de onde parou (não recomeça).
    #[allow(clippy::too_many_arguments)]
    pub async fn send_file(
        app: &tauri::AppHandle,
        peer: &str,
        filename: &str,
        mime: &str,
        source_path: &str,
        transfer_id: &str,
        file_key: &[u8; 32],
        ts: i64,
        thread: &str,
        sticker: bool,
    ) -> Result<(), String> {
        let _g = peer_lock(peer).lock_owned().await;
        let size = std::fs::metadata(source_path).map(|m| m.len()).unwrap_or(0);
        let inner = json!({
            "k": "file",
            "filename": filename,
            "mime": mime,
            "size": size,
            "transferId": transfer_id,
            "fileKey": b64(file_key),
            "ts": ts,
            "thread": thread,
            "sticker": sticker,
        })
        .to_string();
        let (_conn, mut s, mut r) =
            send_header(app, peer, &inner, json!({ "streamed": true })).await?;

        // O receptor diz de qual chunk retomar (0 = do começo).
        let rf = read_frame(&mut r).await?;
        let rv: Value = serde_json::from_slice(&rf).map_err(|e| e.to_string())?;
        let from_chunk = rv["resumeChunk"].as_u64().unwrap_or(0);

        let mut f = std::fs::File::open(source_path)
            .map_err(|e| format!("falha ao abrir '{source_path}': {e}"))?;
        f.seek(SeekFrom::Start(from_chunk * CHUNK as u64))
            .map_err(|e| format!("falha ao posicionar no anexo: {e}"))?;
        let mut buf = vec![0u8; CHUNK];
        loop {
            let n = f.read(&mut buf).map_err(|e| format!("falha ao ler anexo: {e}"))?;
            if n == 0 {
                break;
            }
            let comp = crate::media::compress(&buf[..n])?;
            let enc = crate::crypto::encrypt(file_key, &comp)?;
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
        let db = app.state::<Db>();
        // Conexão QUENTE reusada: cada mensagem é só um stream novo nela. Se o open_bi
        // falhar, a conexão morreu — descarta pra reconectar no próximo envio/heartbeat.
        let conn = get_conn(peer).await?;
        let (mut send_s, mut recv_s) = match conn.open_bi().await {
            Ok(x) => x,
            Err(e) => {
                drop_conn(peer);
                return Err(e.to_string());
            }
        };

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
            let (send_s, recv_s) = match conn.accept_bi().await {
                Ok(x) => x,
                Err(_) => break,
            };
            // Cada stream em sua própria tarefa: assim um anexo grande (que segura o
            // peer_lock durante toda a transferência) não bloqueia os pings de presença
            // nem outros streams. O ratchet segue serializado pelo peer_lock dentro.
            let app = app.clone();
            let peer_hex = peer_hex.clone();
            tauri::async_runtime::spawn(async move {
                let (mut send_s, mut recv_s) = (send_s, recv_s);
                if let Err(e) = handle_stream(&app, &peer_hex, &mut send_s, &mut recv_s).await {
                    super::report(&app, format!("stream de entrada: {e}"), true);
                }
            });
        }
        Ok(())
    }

    async fn handle_stream(
        app: &tauri::AppHandle,
        peer_hex: &str,
        send_s: &mut SendStream,
        recv_s: &mut RecvStream,
    ) -> Result<(), String> {
        let first = read_frame(recv_s).await?;
        let v: Value = serde_json::from_slice(&first).map_err(|e| e.to_string())?;
        // Ping (liveness do heartbeat): responde o ACK na hora, sem ratchet nem lock —
        // assim a presença não trava atrás de uma transferência em curso.
        if v["t"].as_str() == Some("ping") {
            send_s.write_all(&[1u8]).await.map_err(|e| e.to_string())?;
            send_s.finish().map_err(|e| e.to_string())?;
            return Ok(());
        }
        let db = app.state::<Db>();
        // Serializa com esse par: um ratchet de cada vez (evita dessincronizar).
        let _g = peer_lock(peer_hex).lock_owned().await;
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
                process_message(app, &db, peer_hex, &v2, send_s, recv_s).await?;
            }
            Some("msg") => {
                process_message(app, &db, peer_hex, &v, send_s, recv_s).await?;
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
        send_s: &mut SendStream,
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
        let ts = iv["ts"].as_i64().unwrap_or_else(now_ms);
        // A conversa (thread) vem no envelope; monta a mesma chave que o remetente usa,
        // e garante contato + thread (auto-salvar). Sessão/ratchet são por node_id.
        let thread = iv["thread"].as_str().unwrap_or("");
        let convo = convo_key(peer_hex, thread);
        crate::db::contact_ensure(db, peer_hex)?;
        crate::db::thread_ensure(db, &convo, "")?;
        match iv["k"].as_str() {
            Some("text") => {
                let body = iv["body"].as_str().unwrap_or_default();
                let msg = crate::db::record_incoming(db, &convo, body, ts)?;
                let _ = app.emit(EVENT_MESSAGE_IN, &msg);
                let preview: String = body.chars().take(120).collect();
                crate::notify_incoming(app, peer_hex, &preview);
            }
            Some("file") => {
                let filename = iv["filename"].as_str().unwrap_or("arquivo");
                let mime = iv["mime"].as_str().unwrap_or("application/octet-stream");
                let size = iv["size"].as_u64().unwrap_or(0);
                let transfer_id = iv["transferId"].as_str().ok_or("arquivo sem transferId")?;
                let key_vec = unb64(iv["fileKey"].as_str().ok_or("arquivo sem chave")?)?;
                let file_key: [u8; 32] =
                    key_vec.try_into().map_err(|_| "chave de arquivo inválida".to_string())?;

                // Retomada: se já existe um parcial dessa transferência, continua de onde
                // parou (alinhado à fronteira de chunk pra não corromper).
                let partial = crate::media::partial_path(app, transfer_id)?;
                let have = std::fs::metadata(&partial).map(|m| m.len()).unwrap_or(0);
                let mut from_chunk = have / CHUNK as u64;
                if from_chunk * CHUNK as u64 > size {
                    from_chunk = size / CHUNK as u64; // parcial inconsistente → recorta
                }
                let resume_bytes = from_chunk * CHUNK as u64;
                {
                    use std::fs::OpenOptions;
                    let f = OpenOptions::new()
                        .create(true)
                        .write(true)
                        .open(&partial)
                        .map_err(|e| format!("falha ao abrir parcial: {e}"))?;
                    f.set_len(resume_bytes).map_err(|e| e.to_string())?;
                }
                // avisa o remetente de onde retomar
                let rf = serde_json::to_vec(&json!({ "resumeChunk": from_chunk }))
                    .map_err(|e| e.to_string())?;
                write_frame(send_s, &rf).await?;

                // grava os pedaços que chegam, incremental (append). Frame vazio = fim.
                let mut out = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&partial)
                    .map_err(|e| format!("falha ao abrir parcial: {e}"))?;
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
                drop(out);

                // completo → move o parcial pro nome final
                let dest = crate::media::unique_dest(app, filename)?;
                std::fs::rename(&partial, &dest)
                    .map_err(|e| format!("falha ao finalizar anexo: {e}"))?;
                let local_path = dest.to_string_lossy().to_string();
                let meta = json!({
                    "filename": filename, "mime": mime, "size": size, "localPath": local_path,
                    "sticker": iv["sticker"].as_bool().unwrap_or(false),
                })
                .to_string();
                let msg = crate::db::record_file(db, &convo, "in", &meta, "received", Some(ts))?;
                let _ = app.emit(EVENT_MESSAGE_IN, &msg);
                let preview = if iv["sticker"].as_bool().unwrap_or(false) {
                    "🎨 sticker".to_string()
                } else {
                    format!("📎 {filename}")
                };
                crate::notify_incoming(app, peer_hex, &preview);
            }
            // Recibo de leitura: marca as MINHAS mensagens dessa conversa como lidas.
            Some("read") => {
                crate::db::mark_out_read(db, &convo)?;
                let _ = app.emit(EVENT_RECEIPTS, &json!({ "peer": convo }));
            }
            // Palavra-chave do par: guarda o hash e avisa a UI (confere/diverge).
            Some("keyword") => {
                let hash = iv["hash"].as_str().ok_or("keyword sem hash")?;
                crate::db::set_peer_kw_hash(db, peer_hex, hash)?;
                let _ = app.emit(EVENT_KEYWORD, &json!({ "peer": peer_hex }));
            }
            // Perfil do par: cacheia nome + avatar (por node_id).
            Some("profile") => {
                let name = iv["name"].as_str().unwrap_or("");
                let av_b64 = iv["avatar"].as_str().unwrap_or("");
                let avatar_path = if av_b64.is_empty() {
                    None
                } else {
                    let bytes = unb64(av_b64)?;
                    Some(crate::media::save_contact_avatar(app, peer_hex, &bytes)?)
                };
                crate::db::set_contact_profile(db, peer_hex, name, avatar_path.as_deref())?;
                let _ = app.emit(EVENT_PROFILE, &json!({ "peer": peer_hex }));
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

    fn now_ms() -> i64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }
}

#[cfg(feature = "p2p")]
pub use imp::{is_up, send_file, send_keyword, send_profile, send_read, send_text, start, watch};
