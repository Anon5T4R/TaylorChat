//! Histórico local em SQLite (rusqlite bundled). Um arquivo por perfil na pasta de
//! dados do app. **Cifra em repouso (Fase 4):** o corpo das mensagens e os pickles do
//! ratchet são guardados como BLOB cifrado (crypto.rs, chave derivada da identidade);
//! metadados (ts, peer, estado) ficam em claro. Tabelas: contatos, mensagens (conversa
//! = por contato), `sessions` (estado do ratchet por peer) e `_meta` (conta Olm).

use rusqlite::Connection;
use serde::Serialize;
use std::sync::Mutex;
use tauri::{Manager, State};

/// Conexão + chave de cifra em repouso, guardadas no state do Tauri.
#[derive(Default)]
pub struct Db {
    pub conn: Mutex<Option<Connection>>,
    pub key: Mutex<Option<[u8; 32]>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Contact {
    pub node_id: String,
    pub nickname: String, // apelido que EU dei
    pub added_ts: i64,
    pub profile_name: Option<String>, // nome que ELE definiu (perfil dele)
    pub avatar: Option<String>,       // caminho do avatar cacheado dele
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: i64,
    pub peer: String,
    pub direction: String, // "out" | "in"
    pub kind: String,      // "text" | "file"
    pub body: String,      // text: o texto; file: JSON {filename,mime,size,localPath?}
    pub ts: i64,
    pub state: String, // out: queued|sent|delivered|read ; in: received
}

/// Abre (criando se preciso) o banco e guarda a chave de cifra derivada da identidade.
pub fn init(app: &tauri::AppHandle, identity_secret: &[u8; 32]) -> Result<(), String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("sem pasta de dados: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("falha ao criar '{}': {e}", dir.display()))?;
    let path = dir.join("chat.db");
    let conn = Connection::open(&path).map_err(|e| format!("falha ao abrir banco: {e}"))?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         CREATE TABLE IF NOT EXISTS contacts (
             node_id  TEXT PRIMARY KEY,
             nickname TEXT NOT NULL DEFAULT '',
             added_ts INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS messages (
             id        INTEGER PRIMARY KEY AUTOINCREMENT,
             peer      TEXT NOT NULL,
             direction TEXT NOT NULL,
             body      BLOB NOT NULL,
             ts        INTEGER NOT NULL,
             state     TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_messages_peer_ts ON messages(peer, ts);
         CREATE TABLE IF NOT EXISTS sessions (
             peer TEXT PRIMARY KEY,
             v    BLOB NOT NULL
         );
         CREATE TABLE IF NOT EXISTS _meta (
             k TEXT PRIMARY KEY,
             v BLOB NOT NULL
         );",
    )
    .map_err(|e| format!("falha ao criar esquema: {e}"))?;
    // Migrações (ignora se a coluna já existe).
    let _ = conn.execute("ALTER TABLE messages ADD COLUMN kind TEXT NOT NULL DEFAULT 'text'", []);
    // Palavra-chave por contato: `kw` = a minha (cifrada), `peer_kw_hash` = o hash que o par mandou.
    let _ = conn.execute("ALTER TABLE contacts ADD COLUMN kw BLOB", []);
    let _ = conn.execute("ALTER TABLE contacts ADD COLUMN peer_kw_hash TEXT", []);
    let _ = conn.execute("ALTER TABLE contacts ADD COLUMN profile_name TEXT", []);
    let _ = conn.execute("ALTER TABLE contacts ADD COLUMN avatar TEXT", []);
    // Multichat: cada conversa é uma linha aqui. `convo` = node_id (principal) ou
    // `node_id#threadId` (extra). A coluna `peer` das mensagens guarda esse `convo`.
    let _ = conn.execute(
        "CREATE TABLE IF NOT EXISTS threads (
             convo TEXT PRIMARY KEY,
             name  TEXT NOT NULL DEFAULT '',
             created_ts INTEGER NOT NULL
         )",
        [],
    );
    let state = app.state::<Db>();
    *state.conn.lock().map_err(|_| "estado do banco corrompido")? = Some(conn);
    *state.key.lock().map_err(|_| "estado do banco corrompido")? =
        Some(crate::crypto::derive_key(identity_secret));
    Ok(())
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn key_of(db: &Db) -> Result<[u8; 32], String> {
    let g = db.key.lock().map_err(|_| "chave de cifra corrompida".to_string())?;
    (*g).ok_or_else(|| "chave de cifra ausente (banco não inicializado)".to_string())
}

/// Cifra um texto pra guardar (corpo de mensagem).
fn enc_text(db: &Db, plaintext: &str) -> Result<Vec<u8>, String> {
    let key = key_of(db)?;
    crate::crypto::encrypt(&key, plaintext.as_bytes())
}

/// Decifra um BLOB de corpo; se falhar (chave errada/corrompido), mostra um marcador
/// em vez de estourar — a UI nunca deve quebrar por uma linha ilegível.
fn dec_text(db: &Db, blob: &[u8]) -> String {
    match key_of(db).and_then(|k| crate::crypto::decrypt(&k, blob)) {
        Ok(pt) => String::from_utf8_lossy(&pt).to_string(),
        Err(_) => "⟨mensagem ilegível⟩".to_string(),
    }
}

macro_rules! with_conn {
    ($db:expr, $conn:ident, $body:block) => {{
        let guard = $db.conn.lock().map_err(|_| "estado do banco corrompido".to_string())?;
        let $conn = guard.as_ref().ok_or("banco não inicializado".to_string())?;
        $body
    }};
}

#[tauri::command(async)]
pub fn contacts_list(db: State<'_, Db>) -> Result<Vec<Contact>, String> {
    with_conn!(db, conn, {
        let mut stmt = conn
            .prepare(
                "SELECT node_id, nickname, added_ts, profile_name, avatar FROM contacts
                 ORDER BY nickname, node_id",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok(Contact {
                    node_id: r.get(0)?,
                    nickname: r.get(1)?,
                    added_ts: r.get(2)?,
                    profile_name: r.get(3)?,
                    avatar: r.get(4)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    })
}

/// Adiciona (ou renomeia) um contato. `node_id` é o hex de 64 chars da chave dele.
#[tauri::command(async)]
pub fn contact_add(db: State<'_, Db>, node_id: String, nickname: String) -> Result<(), String> {
    let node_id = node_id.trim().to_lowercase();
    crate::identity::hex_decode_32(&node_id)?; // valida o formato
    with_conn!(db, conn, {
        conn.execute(
            "INSERT INTO contacts(node_id, nickname, added_ts) VALUES(?1, ?2, ?3)
             ON CONFLICT(node_id) DO UPDATE SET nickname=excluded.nickname",
            rusqlite::params![node_id, nickname.trim(), now_ms()],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

#[tauri::command(async)]
pub fn contact_remove(db: State<'_, Db>, node_id: String) -> Result<(), String> {
    with_conn!(db, conn, {
        conn.execute("DELETE FROM contacts WHERE node_id=?1", rusqlite::params![node_id])
            .map_err(|e| e.to_string())?;
        Ok(())
    })
}

#[tauri::command(async)]
pub fn messages_list(db: State<'_, Db>, peer: String) -> Result<Vec<Message>, String> {
    // Lê as linhas cruas (corpo cifrado) e decifra depois de soltar o lock da conexão.
    let raw: Vec<(i64, String, String, String, Vec<u8>, i64, String)> = with_conn!(db, conn, {
        let mut stmt = conn
            .prepare(
                "SELECT id, peer, direction, kind, body, ts, state FROM messages
                 WHERE peer=?1 ORDER BY ts, id",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params![peer], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?))
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?
    });
    Ok(raw
        .into_iter()
        .map(|(id, peer, direction, kind, body, ts, state)| Message {
            id,
            peer,
            direction,
            kind,
            body: dec_text(&db, &body),
            ts,
            state,
        })
        .collect())
}

/// Garante que o par existe como contato (auto-salvar ao receber de alguém novo — sem
/// isso a conversa ficaria invisível). Também garante a conversa principal (convo =
/// node_id). Nome vazio = a UI mostra o id encurtado.
pub fn contact_ensure(db: &Db, node_id: &str) -> Result<(), String> {
    let guard = db.conn.lock().map_err(|_| "estado do banco corrompido".to_string())?;
    let conn = guard.as_ref().ok_or("banco não inicializado".to_string())?;
    conn.execute(
        "INSERT OR IGNORE INTO contacts(node_id, nickname, added_ts) VALUES(?1, '', ?2)",
        rusqlite::params![node_id, now_ms()],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR IGNORE INTO threads(convo, name, created_ts) VALUES(?1, '', ?2)",
        rusqlite::params![node_id, now_ms()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Thread {
    pub convo: String, // node_id (principal) ou node_id#threadId
    pub name: String,
}

/// Todas as conversas (principais + extras) pra sidebar. O front separa o node_id.
#[tauri::command(async)]
pub fn threads_list(db: State<'_, Db>) -> Result<Vec<Thread>, String> {
    with_conn!(db, conn, {
        let mut stmt = conn
            .prepare("SELECT convo, name FROM threads")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| Ok(Thread { convo: r.get(0)?, name: r.get(1)? }))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    })
}

/// Cria/garante uma conversa (convo) com um nome. Usado tanto pra criar um chat extra
/// quanto pra o receptor materializar a thread ao receber a 1ª mensagem dela.
pub fn thread_ensure(db: &Db, convo: &str, name: &str) -> Result<(), String> {
    let guard = db.conn.lock().map_err(|_| "estado do banco corrompido".to_string())?;
    let conn = guard.as_ref().ok_or("banco não inicializado".to_string())?;
    conn.execute(
        "INSERT OR IGNORE INTO threads(convo, name, created_ts) VALUES(?1, ?2, ?3)",
        rusqlite::params![convo, name, now_ms()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Comando pra criar um chat novo com um contato existente (thread extra).
#[tauri::command(async)]
pub fn thread_create(db: State<'_, Db>, convo: String, name: String) -> Result<(), String> {
    thread_ensure(&db, &convo, name.trim())
}

/// Remove uma conversa (a thread + suas mensagens). A principal não some da lista
/// (é recriada); extras somem de vez.
#[tauri::command(async)]
pub fn thread_delete(db: State<'_, Db>, convo: String) -> Result<(), String> {
    with_conn!(db, conn, {
        conn.execute("DELETE FROM messages WHERE peer=?1", rusqlite::params![convo])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM threads WHERE convo=?1", rusqlite::params![convo])
            .map_err(|e| e.to_string())?;
        Ok(())
    })
}

/// Insere uma mensagem (texto ou arquivo), cifrando o corpo em repouso, e devolve a
/// linha com o corpo em claro pra UI. `ts`: `None` gera agora (saída); `Some` usa o do
/// remetente (entrada) — assim os dois lados guardam o MESMO ts (base da auditoria).
fn insert_message(
    db: &Db,
    peer: &str,
    direction: &str,
    kind: &str,
    body: &str,
    state: &str,
    ts: Option<i64>,
) -> Result<Message, String> {
    // (contato/thread são garantidos pela camada de rede, que tem o node_id puro)
    let ts = ts.unwrap_or_else(now_ms);
    let blob = enc_text(db, body)?;
    let guard = db.conn.lock().map_err(|_| "estado do banco corrompido".to_string())?;
    let conn = guard.as_ref().ok_or("banco não inicializado".to_string())?;
    conn.execute(
        "INSERT INTO messages(peer, direction, kind, body, ts, state)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![peer, direction, kind, blob, ts, state],
    )
    .map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    Ok(Message {
        id,
        peer: peer.into(),
        direction: direction.into(),
        kind: kind.into(),
        body: body.into(),
        ts,
        state: state.into(),
    })
}

/// Persiste uma mensagem de texto de saída como `queued` e devolve a linha pra UI.
/// O `ts` gerado aqui é transmitido ao par (ver lib::send_message).
pub fn enqueue(db: &Db, peer: &str, body: &str) -> Result<Message, String> {
    insert_message(db, peer, "out", "text", body, "queued", None)
}

/// Registra uma mensagem de arquivo (metadados JSON em `body`, `kind='file'`).
pub fn record_file(
    db: &Db,
    peer: &str,
    direction: &str,
    meta_json: &str,
    state: &str,
    ts: Option<i64>,
) -> Result<Message, String> {
    insert_message(db, peer, direction, "file", meta_json, state, ts)
}

/// Atualiza o estado de uma mensagem (queued→sent→delivered→read). Não-comando.
pub fn set_state(db: &Db, id: i64, state: &str) -> Result<(), String> {
    let guard = db.conn.lock().map_err(|_| "estado do banco corrompido".to_string())?;
    let conn = guard.as_ref().ok_or("banco não inicializado".to_string())?;
    conn.execute("UPDATE messages SET state=?1 WHERE id=?2", rusqlite::params![state, id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Registra uma mensagem de texto recebida já em claro (a rede decifrou pelo ratchet),
/// usando o `ts` do remetente pra os dois lados baterem na auditoria.
#[cfg_attr(not(feature = "p2p"), allow(dead_code))]
pub fn record_incoming(db: &Db, peer: &str, plaintext: &str, ts: i64) -> Result<Message, String> {
    insert_message(db, peer, "in", "text", plaintext, "received", Some(ts))
}

#[tauri::command(async)]
pub fn message_set_state(db: State<'_, Db>, id: i64, state: String) -> Result<(), String> {
    set_state(&db, id, &state)
}

/// Apaga o histórico de mensagens de um contato (o contato permanece). Decisão do
/// usuário — some da auditoria também, por design.
#[tauri::command(async)]
pub fn clear_conversation(db: State<'_, Db>, peer: String) -> Result<(), String> {
    with_conn!(db, conn, {
        conn.execute("DELETE FROM messages WHERE peer=?1", rusqlite::params![peer])
            .map_err(|e| e.to_string())?;
        Ok(())
    })
}

// ── Palavra-chave por contato (verificação humana anti-MITM) ───────────────────
/// Guarda a MINHA palavra-chave (cifrada em repouso) pra um contato.
pub fn set_keyword(db: &Db, peer: &str, word: &str) -> Result<(), String> {
    contact_ensure(db, peer)?;
    let blob = enc_text(db, word)?;
    let guard = db.conn.lock().map_err(|_| "estado do banco corrompido".to_string())?;
    let conn = guard.as_ref().ok_or("banco não inicializado".to_string())?;
    conn.execute("UPDATE contacts SET kw=?1 WHERE node_id=?2", rusqlite::params![blob, peer])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Minha palavra-chave (em claro) pra um contato, se definida.
pub fn get_keyword(db: &Db, peer: &str) -> Result<Option<String>, String> {
    let guard = db.conn.lock().map_err(|_| "estado do banco corrompido".to_string())?;
    let conn = guard.as_ref().ok_or("banco não inicializado".to_string())?;
    let blob: Option<Vec<u8>> = conn
        .query_row("SELECT kw FROM contacts WHERE node_id=?1", rusqlite::params![peer], |r| r.get(0))
        .ok()
        .flatten();
    drop(guard);
    match blob {
        Some(b) => {
            let key = key_of(db)?;
            Ok(Some(String::from_utf8_lossy(&crate::crypto::decrypt(&key, &b)?).to_string()))
        }
        None => Ok(None),
    }
}

/// Grava o hash da palavra-chave que o PAR mandou (pra comparar com a minha).
#[cfg_attr(not(feature = "p2p"), allow(dead_code))]
pub fn set_peer_kw_hash(db: &Db, peer: &str, hash: &str) -> Result<(), String> {
    contact_ensure(db, peer)?;
    let guard = db.conn.lock().map_err(|_| "estado do banco corrompido".to_string())?;
    let conn = guard.as_ref().ok_or("banco não inicializado".to_string())?;
    conn.execute(
        "UPDATE contacts SET peer_kw_hash=?1 WHERE node_id=?2",
        rusqlite::params![hash, peer],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Salva o perfil (nome + caminho do avatar) que um contato mandou. `avatar` vazio =
/// mantém o atual (só atualizou o nome).
#[cfg_attr(not(feature = "p2p"), allow(dead_code))]
pub fn set_contact_profile(db: &Db, node: &str, name: &str, avatar: Option<&str>) -> Result<(), String> {
    contact_ensure(db, node)?;
    let guard = db.conn.lock().map_err(|_| "estado do banco corrompido".to_string())?;
    let conn = guard.as_ref().ok_or("banco não inicializado".to_string())?;
    conn.execute(
        "UPDATE contacts SET profile_name=?1 WHERE node_id=?2",
        rusqlite::params![name, node],
    )
    .map_err(|e| e.to_string())?;
    if let Some(a) = avatar {
        conn.execute("UPDATE contacts SET avatar=?1 WHERE node_id=?2", rusqlite::params![a, node])
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn get_peer_kw_hash(db: &Db, peer: &str) -> Result<Option<String>, String> {
    let guard = db.conn.lock().map_err(|_| "estado do banco corrompido".to_string())?;
    let conn = guard.as_ref().ok_or("banco não inicializado".to_string())?;
    let h: Option<String> = conn
        .query_row("SELECT peer_kw_hash FROM contacts WHERE node_id=?1", rusqlite::params![peer], |r| r.get(0))
        .ok()
        .flatten();
    Ok(h)
}

// ── Auditoria: digest da conversa pra comparar os registros dos 2 dispositivos ──
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditResult {
    pub count: usize,
    pub digest: String,
}

/// SHA-256 sobre as mensagens normalizadas (autor, ts, tipo, conteúdo) da conversa,
/// ordenadas de forma determinística. Os DOIS dispositivos calculam o mesmo digest se
/// os registros forem idênticos; qualquer alteração de conteúdo muda o digest → a
/// divergência é a prova de adulteração (comparação entre as duas partes).
pub fn audit_digest(db: &Db, convo: &str, my_id: &str, peer_node: &str) -> Result<AuditResult, String> {
    let raw: Vec<(String, String, Vec<u8>, i64)> = with_conn!(db, conn, {
        let mut stmt = conn
            .prepare("SELECT direction, kind, body, ts FROM messages WHERE peer=?1")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params![convo], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?
    });
    let mut lines: Vec<String> = raw
        .into_iter()
        .map(|(direction, kind, blob, ts)| {
            let author = if direction == "out" { my_id } else { peer_node };
            let body = dec_text(db, &blob);
            let payload = if kind == "file" {
                // caminho local é específico do aparelho → usa só nome+tamanho
                serde_json::from_str::<serde_json::Value>(&body)
                    .ok()
                    .map(|v| {
                        format!(
                            "{}|{}",
                            v["filename"].as_str().unwrap_or(""),
                            v["size"].as_u64().unwrap_or(0)
                        )
                    })
                    .unwrap_or_default()
            } else {
                body
            };
            format!("{author}\t{ts}\t{kind}\t{payload}")
        })
        .collect();
    lines.sort(); // independe da ordem de chegada em cada aparelho
    let count = lines.len();
    let digest = crate::crypto::hash_hex(lines.join("\n").as_bytes());
    Ok(AuditResult { count, digest })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvoSummary {
    pub peer: String,
    pub kind: String,
    pub body: String, // texto truncado; arquivo vira "📎 nome"
    pub ts: i64,
    pub direction: String,
}

/// Última mensagem de cada conversa (pra prévia + ordenação da sidebar).
#[tauri::command(async)]
pub fn conversations_summary(db: State<'_, Db>) -> Result<Vec<ConvoSummary>, String> {
    let raw: Vec<(String, String, Vec<u8>, i64, String)> = with_conn!(db, conn, {
        let mut stmt = conn
            .prepare(
                "SELECT m.peer, m.kind, m.body, m.ts, m.direction FROM messages m
                 JOIN (SELECT peer, MAX(id) AS mid FROM messages GROUP BY peer) l ON m.id = l.mid",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?
    });
    Ok(raw
        .into_iter()
        .map(|(peer, kind, blob, ts, direction)| {
            let text = dec_text(&db, &blob);
            let body = if kind == "file" {
                let name = serde_json::from_str::<serde_json::Value>(&text)
                    .ok()
                    .and_then(|v| v["filename"].as_str().map(String::from))
                    .unwrap_or_else(|| "arquivo".into());
                format!("📎 {name}")
            } else {
                text.chars().take(90).collect()
            };
            ConvoSummary { peer, kind, body, ts, direction }
        })
        .collect())
}

/// Conversas que têm alguma mensagem na fila (pra reenvio geral, todas de uma vez).
pub fn queued_convos(db: &Db) -> Result<Vec<String>, String> {
    with_conn!(db, conn, {
        let mut stmt = conn
            .prepare("SELECT DISTINCT peer FROM messages WHERE direction='out' AND state='queued'")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    })
}

/// Mensagens de saída ainda na fila (decifradas) pra um par — pro reenvio.
pub fn queued_out(db: &Db, peer: &str) -> Result<Vec<Message>, String> {
    let raw: Vec<(i64, String, Vec<u8>, i64)> = with_conn!(db, conn, {
        let mut stmt = conn
            .prepare(
                "SELECT id, kind, body, ts FROM messages
                 WHERE peer=?1 AND direction='out' AND state='queued' ORDER BY ts, id",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params![peer], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?
    });
    Ok(raw
        .into_iter()
        .map(|(id, kind, blob, ts)| Message {
            id,
            peer: peer.into(),
            direction: "out".into(),
            kind,
            body: dec_text(db, &blob),
            ts,
            state: "queued".into(),
        })
        .collect())
}

/// Marca como `read` todas as minhas mensagens de saída pra um par (recibo de leitura).
#[cfg_attr(not(feature = "p2p"), allow(dead_code))]
pub fn mark_out_read(db: &Db, peer: &str) -> Result<(), String> {
    let guard = db.conn.lock().map_err(|_| "estado do banco corrompido".to_string())?;
    let conn = guard.as_ref().ok_or("banco não inicializado".to_string())?;
    conn.execute(
        "UPDATE messages SET state='read' WHERE peer=?1 AND direction='out' AND state!='read'",
        rusqlite::params![peer],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ── Persistência do ratchet (Fase 4) — pickles cifrados em repouso ─────────────
// Usadas só pela camada de rede (p2p); o BLOB guardado já vem cifrado por aqui.

#[cfg_attr(not(feature = "p2p"), allow(dead_code))]
pub fn meta_get(db: &Db, k: &str) -> Result<Option<Vec<u8>>, String> {
    let guard = db.conn.lock().map_err(|_| "estado do banco corrompido".to_string())?;
    let conn = guard.as_ref().ok_or("banco não inicializado".to_string())?;
    let blob: Option<Vec<u8>> = conn
        .query_row("SELECT v FROM _meta WHERE k=?1", rusqlite::params![k], |r| r.get(0))
        .ok();
    drop(guard);
    match blob {
        Some(b) => {
            let key = key_of(db)?;
            Ok(Some(crate::crypto::decrypt(&key, &b)?))
        }
        None => Ok(None),
    }
}

#[cfg_attr(not(feature = "p2p"), allow(dead_code))]
pub fn meta_set(db: &Db, k: &str, plaintext: &[u8]) -> Result<(), String> {
    let key = key_of(db)?;
    let blob = crate::crypto::encrypt(&key, plaintext)?;
    let guard = db.conn.lock().map_err(|_| "estado do banco corrompido".to_string())?;
    let conn = guard.as_ref().ok_or("banco não inicializado".to_string())?;
    conn.execute(
        "INSERT INTO _meta(k, v) VALUES(?1, ?2)
         ON CONFLICT(k) DO UPDATE SET v=excluded.v",
        rusqlite::params![k, blob],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg_attr(not(feature = "p2p"), allow(dead_code))]
pub fn session_get(db: &Db, peer: &str) -> Result<Option<Vec<u8>>, String> {
    let guard = db.conn.lock().map_err(|_| "estado do banco corrompido".to_string())?;
    let conn = guard.as_ref().ok_or("banco não inicializado".to_string())?;
    let blob: Option<Vec<u8>> = conn
        .query_row("SELECT v FROM sessions WHERE peer=?1", rusqlite::params![peer], |r| r.get(0))
        .ok();
    drop(guard);
    match blob {
        Some(b) => {
            let key = key_of(db)?;
            Ok(Some(crate::crypto::decrypt(&key, &b)?))
        }
        None => Ok(None),
    }
}

#[cfg_attr(not(feature = "p2p"), allow(dead_code))]
pub fn session_set(db: &Db, peer: &str, plaintext: &[u8]) -> Result<(), String> {
    let key = key_of(db)?;
    let blob = crate::crypto::encrypt(&key, plaintext)?;
    let guard = db.conn.lock().map_err(|_| "estado do banco corrompido".to_string())?;
    let conn = guard.as_ref().ok_or("banco não inicializado".to_string())?;
    conn.execute(
        "INSERT INTO sessions(peer, v) VALUES(?1, ?2)
         ON CONFLICT(peer) DO UPDATE SET v=excluded.v",
        rusqlite::params![peer, blob],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
