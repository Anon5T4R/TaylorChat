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
    pub muted: bool,                  // silenciado (sem notificação de desktop)
    pub phone: Option<String>,        // ficha local (só minha)
    pub email: Option<String>,
    pub birthday: Option<String>,
    pub notes: Option<String>,
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
    pub state: String,          // out: queued|sent|delivered|read ; in: received
    pub reply_to: Option<i64>,       // ts da mensagem citada (responder)
    pub reply_preview: Option<String>, // trecho da citada (renderiza sem lookup)
    pub deleted: bool,               // apagada para todos (soft-delete)
}

/// Consolida o WAL no arquivo principal (TRUNCATE zera o diário). Sem isso os
/// dados podem viver indefinidamente só no WAL — e um WAL descartado (crash,
/// -shm corrompido) vira perda total, como já aconteceu uma vez.
fn checkpoint_conn(conn: &Connection) {
    let _ = conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()));
}

/// Checkpoint best-effort de fora dos comandos (ex.: saída do app).
pub fn checkpoint(db: &Db) {
    if let Ok(guard) = db.conn.lock() {
        if let Some(conn) = guard.as_ref() {
            checkpoint_conn(conn);
        }
    }
}

/// Abre o banco validando a integridade. Se o quick_check reprovar, põe os
/// arquivos de lado (.corrupt-<ts>, preservados pra forense) e recomeça do
/// zero — app utilizável é melhor que banco travado.
fn open_checked(path: &std::path::Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| format!("falha ao abrir banco: {e}"))?;
    let ok: String = conn
        .query_row("PRAGMA quick_check(1)", [], |r| r.get(0))
        .unwrap_or_else(|_| "falhou".into());
    if ok == "ok" {
        return Ok(conn);
    }
    eprintln!("[taylorchat] banco reprovou no quick_check ('{ok}'); movendo pra .corrupt e recriando");
    drop(conn);
    let ts = now_ms();
    for suffix in ["", "-wal", "-shm"] {
        let f = format!("{}{suffix}", path.display());
        if std::path::Path::new(&f).exists() {
            let _ = std::fs::rename(&f, format!("{}.corrupt-{ts}{suffix}", path.display()));
        }
    }
    Connection::open(path).map_err(|e| format!("falha ao recriar banco: {e}"))
}

/// "aaaammdd" a partir de segundos unix (algoritmo civil, sem dependência de tz).
fn date_stamp(secs: i64) -> String {
    let z = secs.div_euclid(86400) + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}{m:02}{d:02}")
}

/// Backup diário rotativo (mantém os 3 mais recentes) em <dados>/backups/.
/// `VACUUM INTO` gera uma cópia consistente mesmo com o banco em uso.
fn daily_backup(conn: &Connection, dir: &std::path::Path) {
    let backups = dir.join("backups");
    if std::fs::create_dir_all(&backups).is_err() {
        return;
    }
    let target = backups.join(format!("chat-{}.db", date_stamp(now_ms() / 1000)));
    if target.exists() {
        return;
    }
    if let Err(e) =
        conn.execute("VACUUM INTO ?1", rusqlite::params![target.to_string_lossy()])
    {
        eprintln!("[taylorchat] backup diário falhou: {e}");
        return;
    }
    let mut old: Vec<_> = std::fs::read_dir(&backups)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .map(|n| n.to_string_lossy().starts_with("chat-"))
                        .unwrap_or(false)
                })
                .collect()
        })
        .unwrap_or_default();
    old.sort();
    while old.len() > 3 {
        let _ = std::fs::remove_file(old.remove(0));
    }
}

/// Abre (criando se preciso) o banco e guarda a chave de cifra derivada da identidade.
pub fn init(app: &tauri::AppHandle, identity_secret: &[u8; 32]) -> Result<(), String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("sem pasta de dados: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("falha ao criar '{}': {e}", dir.display()))?;
    let path = dir.join("chat.db");
    let conn = open_checked(&path)?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=FULL;
         PRAGMA wal_autocheckpoint=100;
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
    // Responder/citar: `reply_to` = ts da mensagem citada (mesmo ts nos 2 lados).
    let _ = conn.execute("ALTER TABLE messages ADD COLUMN reply_to INTEGER", []);
    // Trecho da mensagem citada, guardado junto (renderiza a citação sem depender de a
    // original estar carregada na página — #4 da revisão).
    let _ = conn.execute("ALTER TABLE messages ADD COLUMN reply_preview TEXT", []);
    // Apagada (para todos): soft-delete — mantém a linha, esvazia o corpo, marca a flag.
    let _ = conn.execute("ALTER TABLE messages ADD COLUMN deleted INTEGER NOT NULL DEFAULT 0", []);
    // Palavra-chave por contato: `kw` = a minha (cifrada), `peer_kw_hash` = o hash que o par mandou.
    let _ = conn.execute("ALTER TABLE contacts ADD COLUMN kw BLOB", []);
    let _ = conn.execute("ALTER TABLE contacts ADD COLUMN peer_kw_hash TEXT", []);
    let _ = conn.execute("ALTER TABLE contacts ADD COLUMN profile_name TEXT", []);
    let _ = conn.execute("ALTER TABLE contacts ADD COLUMN avatar TEXT", []);
    // Silenciar: contato silenciado não gera notificação de desktop (mas segue contando
    // não-lidos, como no WhatsApp).
    let _ = conn.execute("ALTER TABLE contacts ADD COLUMN muted INTEGER NOT NULL DEFAULT 0", []);
    // Ficha local do contato (só minha, não sincroniza): telefone/email/aniversário/notas.
    let _ = conn.execute("ALTER TABLE contacts ADD COLUMN phone TEXT", []);
    let _ = conn.execute("ALTER TABLE contacts ADD COLUMN email TEXT", []);
    let _ = conn.execute("ALTER TABLE contacts ADD COLUMN birthday TEXT", []);
    let _ = conn.execute("ALTER TABLE contacts ADD COLUMN notes TEXT", []);
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
    // Não-lidos por conversa, persistidos (antes viviam só no estado do React e sumiam ao
    // reiniciar — L3 da revisão). O front continua decidindo quando conta; aqui só guarda.
    let _ = conn.execute(
        "CREATE TABLE IF NOT EXISTS unread (convo TEXT PRIMARY KEY, n INTEGER NOT NULL DEFAULT 0)",
        [],
    );
    // "Apagar para todos" pendentes: se o par estava offline na hora, o aviso fica aqui e
    // o reenvio periódico (resend_all) tenta de novo até o ACK. Sem isso, apagava só do
    // meu lado (correção A da revisão).
    let _ = conn.execute(
        "CREATE TABLE IF NOT EXISTS pending_deletes (convo TEXT, ts INTEGER, PRIMARY KEY(convo, ts))",
        [],
    );
    // Reações por mensagem: `mine`=1 a minha, 0 a do par (1:1, no máx. uma de cada).
    let _ = conn.execute(
        "CREATE TABLE IF NOT EXISTS reactions (
             convo TEXT NOT NULL,
             target_ts INTEGER NOT NULL,
             mine INTEGER NOT NULL,
             emoji TEXT NOT NULL,
             PRIMARY KEY(convo, target_ts, mine)
         )",
        [],
    );
    // Reações pendentes (par offline): guarda a MINHA reação mais recente pra cada msg
    // até o ACK; o resend_all reenvia. Sem isso a reação se perdia offline (#3 da revisão).
    let _ = conn.execute(
        "CREATE TABLE IF NOT EXISTS pending_reactions (
             convo TEXT, target_ts INTEGER, emoji TEXT, PRIMARY KEY(convo, target_ts)
         )",
        [],
    );
    // Backfill: contatos salvos antes de a coluna threads existir (ou por um contact_add
    // que não criava a conversa principal) precisam da sua thread principal, senão somem
    // da sidebar, que lista threads. Idempotente.
    let _ = conn.execute(
        "INSERT OR IGNORE INTO threads(convo, name, created_ts)
         SELECT node_id, '', added_ts FROM contacts",
        [],
    );
    // Profilaxia: tudo que estiver só no WAL vai pro arquivo principal agora,
    // e uma cópia do dia fica em backups/ (rotativo, 3 últimos).
    checkpoint_conn(&conn);
    daily_backup(&conn, &dir);
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
                "SELECT node_id, nickname, added_ts, profile_name, avatar, muted,
                        phone, email, birthday, notes FROM contacts
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
                    muted: r.get::<_, i64>(5)? != 0,
                    phone: r.get(6)?,
                    email: r.get(7)?,
                    birthday: r.get(8)?,
                    notes: r.get(9)?,
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
        // Garante a conversa principal (convo = node_id); sem isso o contato não
        // aparece na sidebar, que lista threads (não contatos).
        conn.execute(
            "INSERT OR IGNORE INTO threads(convo, name, created_ts) VALUES(?1, '', ?2)",
            rusqlite::params![node_id, now_ms()],
        )
        .map_err(|e| e.to_string())?;
        // Contato é escrita rara e preciosa: consolida no arquivo principal já.
        checkpoint_conn(conn);
        Ok(())
    })
}

#[tauri::command(async)]
pub fn contact_remove(db: State<'_, Db>, node_id: String) -> Result<(), String> {
    let node_id = node_id.trim().to_lowercase();
    // Remove o contato E tudo que pende dele: a conversa principal (convo = node_id) e
    // as extras (node_id#...), com suas mensagens. Sem isso ficariam "conversas
    // fantasma" na aba Conversas sem dono. `like` casa a principal e as extras.
    let like = format!("{node_id}#%");
    with_conn!(db, conn, {
        conn.execute("DELETE FROM contacts WHERE node_id=?1", rusqlite::params![node_id])
            .map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM messages WHERE peer=?1 OR peer LIKE ?2",
            rusqlite::params![node_id, like],
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM threads WHERE convo=?1 OR convo LIKE ?2",
            rusqlite::params![node_id, like],
        )
        .map_err(|e| e.to_string())?;
        checkpoint_conn(conn);
        Ok(())
    })
}

/// Mensagens de uma conversa, PAGINADAS. Devolve as `limit` mais recentes antes de
/// `before_id` (ou as últimas, se `before_id` for None), já em ordem crescente. Sem isso,
/// abrir uma conversa longa carregava E decifrava TUDO (L2). Ordena por `id` = ordem em
/// que as coisas aconteceram NESTE aparelho — assim o relógio torto do par não embaralha
/// a exibição (L1); o `ts` (hora mostrada + base da auditoria) continua o do remetente.
/// Atualiza a ficha local do contato (apelido + telefone/email/aniversário/notas).
/// Campos vazios viram NULL. `nickname` é o nome que EU dou (não o perfil dele).
#[tauri::command(async)]
pub fn set_contact_info(
    db: State<'_, Db>,
    node_id: String,
    nickname: String,
    phone: String,
    email: String,
    birthday: String,
    notes: String,
) -> Result<(), String> {
    let node_id = node_id.trim().to_lowercase();
    let opt = |s: &str| {
        let t = s.trim();
        if t.is_empty() { None } else { Some(t.to_string()) }
    };
    with_conn!(db, conn, {
        // Upsert: se a linha do contato não existir (borda), cria — em vez de o salvar
        // sumir silenciosamente (#9 da revisão).
        conn.execute(
            "INSERT INTO contacts(node_id, nickname, added_ts, phone, email, birthday, notes)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(node_id) DO UPDATE SET
                nickname=excluded.nickname, phone=excluded.phone, email=excluded.email,
                birthday=excluded.birthday, notes=excluded.notes",
            rusqlite::params![
                node_id,
                nickname.trim(),
                now_ms(),
                opt(&phone),
                opt(&email),
                opt(&birthday),
                opt(&notes)
            ],
        )
        .map_err(|e| e.to_string())?;
        checkpoint_conn(conn);
        Ok(())
    })
}

/// Silencia/dessilencia um contato (só afeta a notificação de desktop).
#[tauri::command(async)]
pub fn set_muted(db: State<'_, Db>, node_id: String, muted: bool) -> Result<(), String> {
    with_conn!(db, conn, {
        conn.execute(
            "UPDATE contacts SET muted=?1 WHERE node_id=?2",
            rusqlite::params![muted as i64, node_id.trim().to_lowercase()],
        )
        .map_err(|e| e.to_string())?;
        checkpoint_conn(conn);
        Ok(())
    })
}

/// Esse contato está silenciado? (pra decidir se notifica).
#[cfg_attr(not(feature = "p2p"), allow(dead_code))]
pub fn is_muted(db: &Db, node: &str) -> bool {
    let guard = match db.conn.lock() {
        Ok(g) => g,
        Err(_) => return false,
    };
    let Some(conn) = guard.as_ref() else { return false };
    conn.query_row(
        "SELECT muted FROM contacts WHERE node_id=?1",
        rusqlite::params![node],
        |r| r.get::<_, i64>(0),
    )
    .map(|v| v != 0)
    .unwrap_or(false)
}

#[tauri::command(async)]
pub fn messages_list(
    db: State<'_, Db>,
    peer: String,
    before_id: Option<i64>,
    limit: Option<i64>,
) -> Result<Vec<Message>, String> {
    let limit = limit.unwrap_or(300).clamp(1, 100_000);
    let before = before_id.unwrap_or(i64::MAX);
    // Lê as linhas cruas (corpo cifrado) e decifra depois de soltar o lock da conexão.
    type Row = (i64, String, String, String, Vec<u8>, i64, String, Option<i64>, Option<String>, i64);
    let raw: Vec<Row> = with_conn!(db, conn, {
        let mut stmt = conn
            .prepare(
                "SELECT id, peer, direction, kind, body, ts, state, reply_to, reply_preview, deleted
                 FROM messages WHERE peer=?1 AND id<?2 ORDER BY id DESC LIMIT ?3",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params![peer, before, limit], |r| {
                Ok((
                    r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?,
                    r.get(7)?, r.get(8)?, r.get(9)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?
    });
    let mut out: Vec<Message> = raw
        .into_iter()
        .map(|(id, peer, direction, kind, body, ts, state, reply_to, reply_preview, deleted)| {
            Message {
                id,
                peer,
                direction,
                kind,
                body: if deleted != 0 { String::new() } else { dec_text(&db, &body) },
                ts,
                state,
                reply_to,
                reply_preview,
                deleted: deleted != 0,
            }
        })
        .collect();
    out.reverse(); // veio em id DESC (mais novas primeiro) → volta pra ordem crescente
    Ok(out)
}

// ── Busca global de mensagens (por texto, entre todas as conversas) ────────────
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub convo: String,
    pub snippet: String,
    pub ts: i64,
}

/// Procura `query` no texto de todas as mensagens (decifra em memória), da mais recente
/// pra mais antiga, parando ao juntar `limit` acertos. Só mensagens de texto (arquivo
/// casa por nome via a prévia da conversa). Simples e limitado — v1 da busca.
#[tauri::command(async)]
pub fn search_messages(db: State<'_, Db>, query: String, limit: i64) -> Result<Vec<SearchHit>, String> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let limit = limit.clamp(1, 200) as usize;
    let raw: Vec<(String, Vec<u8>, i64)> = with_conn!(db, conn, {
        // Limita às mais recentes: sem isso, cada tecla decifraria o histórico INTEIRO.
        // 4000 cobre a busca do dia a dia; índice de busca completo fica pra depois.
        let mut stmt = conn
            .prepare(
                "SELECT peer, body, ts FROM messages
                 WHERE kind='text' AND deleted=0 ORDER BY ts DESC, id DESC LIMIT 4000",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?
    });
    let mut hits = Vec::new();
    for (convo, blob, ts) in raw {
        let text = dec_text(&db, &blob);
        if text.to_lowercase().contains(&q) {
            let snippet: String = text.chars().take(100).collect();
            hits.push(SearchHit { convo, snippet, ts });
            if hits.len() >= limit {
                break;
            }
        }
    }
    Ok(hits)
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
    checkpoint_conn(conn);
    Ok(())
}

/// Nome de exibição de um contato (apelido meu ‖ nome do perfil dele ‖ id encurtado).
/// Usado no título da notificação de desktop.
#[cfg_attr(not(feature = "p2p"), allow(dead_code))]
pub fn contact_name(db: &Db, node: &str) -> String {
    let short = || {
        if node.len() > 12 {
            format!("{}…{}", &node[..6], &node[node.len() - 4..])
        } else {
            node.to_string()
        }
    };
    let guard = match db.conn.lock() {
        Ok(g) => g,
        Err(_) => return short(),
    };
    let Some(conn) = guard.as_ref() else { return short() };
    let row: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT nickname, profile_name FROM contacts WHERE node_id=?1",
            rusqlite::params![node],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    match row {
        Some((nick, _)) if !nick.trim().is_empty() => nick,
        Some((_, Some(pn))) if !pn.trim().is_empty() => pn,
        _ => short(),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Thread {
    pub convo: String, // node_id (principal) ou node_id#threadId
    pub name: String,
}

// ── Não-lidos persistidos por conversa ─────────────────────────────────────────
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Unread {
    pub convo: String,
    pub n: i64,
}

/// Não-lidos de todas as conversas (só as com n>0) — carregado no boot.
#[tauri::command(async)]
pub fn unread_list(db: State<'_, Db>) -> Result<Vec<Unread>, String> {
    with_conn!(db, conn, {
        let mut stmt = conn
            .prepare("SELECT convo, n FROM unread WHERE n > 0")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| Ok(Unread { convo: r.get(0)?, n: r.get(1)? }))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    })
}

/// Grava o contador de não-lidos de uma conversa (n<=0 apaga a linha).
#[tauri::command(async)]
pub fn unread_set(db: State<'_, Db>, convo: String, n: i64) -> Result<(), String> {
    with_conn!(db, conn, {
        if n <= 0 {
            conn.execute("DELETE FROM unread WHERE convo=?1", rusqlite::params![convo])
                .map_err(|e| e.to_string())?;
        } else {
            conn.execute(
                "INSERT INTO unread(convo, n) VALUES(?1, ?2)
                 ON CONFLICT(convo) DO UPDATE SET n=excluded.n",
                rusqlite::params![convo, n],
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    })
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
    checkpoint_conn(conn);
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
        checkpoint_conn(conn);
        Ok(())
    })
}

/// Insere uma mensagem (texto ou arquivo), cifrando o corpo em repouso, e devolve a
/// linha com o corpo em claro pra UI. `ts`: `None` gera agora (saída); `Some` usa o do
/// remetente (entrada) — assim os dois lados guardam o MESMO ts (base da auditoria).
#[allow(clippy::too_many_arguments)]
fn insert_message(
    db: &Db,
    peer: &str,
    direction: &str,
    kind: &str,
    body: &str,
    state: &str,
    ts: Option<i64>,
    reply_to: Option<i64>,
    reply_preview: Option<&str>,
) -> Result<Message, String> {
    // (contato/thread são garantidos pela camada de rede, que tem o node_id puro)
    let ts = ts.unwrap_or_else(now_ms);
    let blob = enc_text(db, body)?;
    let guard = db.conn.lock().map_err(|_| "estado do banco corrompido".to_string())?;
    let conn = guard.as_ref().ok_or("banco não inicializado".to_string())?;
    conn.execute(
        "INSERT INTO messages(peer, direction, kind, body, ts, state, reply_to, reply_preview)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![peer, direction, kind, blob, ts, state, reply_to, reply_preview],
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
        reply_to,
        reply_preview: reply_preview.map(String::from),
        deleted: false,
    })
}

/// Persiste uma mensagem de texto de saída como `queued` e devolve a linha pra UI.
/// O `ts` gerado aqui é transmitido ao par (ver lib::send_message). `reply_to` = ts da
/// mensagem citada (responder), se houver.
pub fn enqueue(
    db: &Db,
    peer: &str,
    body: &str,
    reply_to: Option<i64>,
    reply_preview: Option<&str>,
) -> Result<Message, String> {
    insert_message(db, peer, "out", "text", body, "queued", None, reply_to, reply_preview)
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
    insert_message(db, peer, direction, "file", meta_json, state, ts, None, None)
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
pub fn record_incoming(
    db: &Db,
    peer: &str,
    plaintext: &str,
    ts: i64,
    reply_to: Option<i64>,
    reply_preview: Option<&str>,
) -> Result<Message, String> {
    insert_message(db, peer, "in", "text", plaintext, "received", Some(ts), reply_to, reply_preview)
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

/// Apaga uma mensagem só pra MIM (remove a linha local). Não avisa o par.
#[tauri::command(async)]
pub fn message_delete(db: State<'_, Db>, id: i64) -> Result<(), String> {
    with_conn!(db, conn, {
        conn.execute("DELETE FROM messages WHERE id=?1", rusqlite::params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
    })
}

/// Apaga uma mensagem PARA TODOS (soft-delete): mantém a linha (pra ordem/auditoria),
/// esvazia o corpo cifrado e marca a flag. Identifica pela chave (convo, ts) — a mesma
/// nos dois aparelhos. Usado tanto pelo meu comando quanto ao receber o aviso do par.
#[cfg_attr(not(feature = "p2p"), allow(dead_code))]
pub fn mark_deleted(db: &Db, convo: &str, ts: i64, direction: &str) -> Result<(), String> {
    let empty = enc_text(db, "")?;
    let guard = db.conn.lock().map_err(|_| "estado do banco corrompido".to_string())?;
    let conn = guard.as_ref().ok_or("banco não inicializado".to_string())?;
    // `direction` desambigua o alvo: quando EU apago, é a minha 'out'; quando recebo o
    // aviso, é a 'in' que veio do par. Sem isso, duas mensagens de mesmo ts (uma minha e
    // uma dele, no mesmo ms) seriam apagadas juntas (#5 da revisão, jeito pragmático —
    // sem precisar de um id único por mensagem, que teria dor de migração).
    conn.execute(
        "UPDATE messages SET deleted=1, body=?1 WHERE peer=?2 AND ts=?3 AND direction=?4",
        rusqlite::params![empty, convo, ts, direction],
    )
    .map_err(|e| e.to_string())?;
    // Reações não fazem sentido numa mensagem apagada — some com elas (#7 da revisão).
    let _ = conn.execute(
        "DELETE FROM reactions WHERE convo=?1 AND target_ts=?2",
        rusqlite::params![convo, ts],
    );
    checkpoint_conn(conn);
    Ok(())
}

// ── Fila de "apagar para todos" (reenvio se o par estava offline) ──────────────
pub fn pending_delete_add(db: &Db, convo: &str, ts: i64) -> Result<(), String> {
    with_conn!(db, conn, {
        conn.execute(
            "INSERT OR IGNORE INTO pending_deletes(convo, ts) VALUES(?1, ?2)",
            rusqlite::params![convo, ts],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

pub fn pending_delete_remove(db: &Db, convo: &str, ts: i64) -> Result<(), String> {
    with_conn!(db, conn, {
        conn.execute(
            "DELETE FROM pending_deletes WHERE convo=?1 AND ts=?2",
            rusqlite::params![convo, ts],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

pub fn pending_deletes_all(db: &Db) -> Result<Vec<(String, i64)>, String> {
    with_conn!(db, conn, {
        let mut stmt = conn
            .prepare("SELECT convo, ts FROM pending_deletes")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    })
}

pub fn pending_reaction_set(db: &Db, convo: &str, ts: i64, emoji: &str) -> Result<(), String> {
    with_conn!(db, conn, {
        conn.execute(
            "INSERT INTO pending_reactions(convo, target_ts, emoji) VALUES(?1, ?2, ?3)
             ON CONFLICT(convo, target_ts) DO UPDATE SET emoji=excluded.emoji",
            rusqlite::params![convo, ts, emoji],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

pub fn pending_reaction_remove(db: &Db, convo: &str, ts: i64) -> Result<(), String> {
    with_conn!(db, conn, {
        conn.execute(
            "DELETE FROM pending_reactions WHERE convo=?1 AND target_ts=?2",
            rusqlite::params![convo, ts],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

pub fn pending_reactions_all(db: &Db) -> Result<Vec<(String, i64, String)>, String> {
    with_conn!(db, conn, {
        let mut stmt = conn
            .prepare("SELECT convo, target_ts, emoji FROM pending_reactions")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    })
}

// ── Reações por mensagem ───────────────────────────────────────────────────────
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reaction {
    pub target_ts: i64,
    pub mine: bool,
    pub emoji: String,
}

/// Grava/atualiza uma reação (emoji vazio = remove). `mine` distingue a minha da do par.
#[cfg_attr(not(feature = "p2p"), allow(dead_code))]
pub fn reaction_set(db: &Db, convo: &str, target_ts: i64, mine: bool, emoji: &str) -> Result<(), String> {
    with_conn!(db, conn, {
        if emoji.is_empty() {
            conn.execute(
                "DELETE FROM reactions WHERE convo=?1 AND target_ts=?2 AND mine=?3",
                rusqlite::params![convo, target_ts, mine as i64],
            )
            .map_err(|e| e.to_string())?;
        } else {
            conn.execute(
                "INSERT INTO reactions(convo, target_ts, mine, emoji) VALUES(?1, ?2, ?3, ?4)
                 ON CONFLICT(convo, target_ts, mine) DO UPDATE SET emoji=excluded.emoji",
                rusqlite::params![convo, target_ts, mine as i64, emoji],
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    })
}

/// Todas as reações de uma conversa (pra UI montar por mensagem).
#[tauri::command(async)]
pub fn reactions_list(db: State<'_, Db>, convo: String) -> Result<Vec<Reaction>, String> {
    with_conn!(db, conn, {
        let mut stmt = conn
            .prepare("SELECT target_ts, mine, emoji FROM reactions WHERE convo=?1")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params![convo], |r| {
                Ok(Reaction {
                    target_ts: r.get(0)?,
                    mine: r.get::<_, i64>(1)? != 0,
                    emoji: r.get(2)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
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
    pub body: String, // texto truncado; arquivo vira "📎 nome"; vazio se apagada
    pub ts: i64,
    pub direction: String,
    pub deleted: bool, // última msg foi apagada para todos → a UI mostra o marcador
}

/// Última mensagem de cada conversa (pra prévia + ordenação da sidebar).
#[tauri::command(async)]
pub fn conversations_summary(db: State<'_, Db>) -> Result<Vec<ConvoSummary>, String> {
    let raw: Vec<(String, String, Vec<u8>, i64, String, i64)> = with_conn!(db, conn, {
        let mut stmt = conn
            .prepare(
                "SELECT m.peer, m.kind, m.body, m.ts, m.direction, m.deleted FROM messages m
                 JOIN (SELECT peer, MAX(id) AS mid FROM messages GROUP BY peer) l ON m.id = l.mid",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?
    });
    Ok(raw
        .into_iter()
        .map(|(peer, kind, blob, ts, direction, deleted)| {
            let del = deleted != 0;
            let body = if del {
                String::new()
            } else {
                let text = dec_text(&db, &blob);
                if kind == "file" {
                    let name = serde_json::from_str::<serde_json::Value>(&text)
                        .ok()
                        .and_then(|v| v["filename"].as_str().map(String::from))
                        .unwrap_or_else(|| "arquivo".into());
                    format!("📎 {name}")
                } else {
                    text.chars().take(90).collect()
                }
            };
            ConvoSummary { peer, kind, body, ts, direction, deleted: del }
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
    let raw: Vec<(i64, String, Vec<u8>, i64, Option<i64>, Option<String>)> = with_conn!(db, conn, {
        let mut stmt = conn
            .prepare(
                "SELECT id, kind, body, ts, reply_to, reply_preview FROM messages
                 WHERE peer=?1 AND direction='out' AND state='queued' AND deleted=0 ORDER BY ts, id",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params![peer], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?
    });
    Ok(raw
        .into_iter()
        .map(|(id, kind, blob, ts, reply_to, reply_preview)| Message {
            id,
            peer: peer.into(),
            direction: "out".into(),
            kind,
            body: dec_text(db, &blob),
            ts,
            state: "queued".into(),
            reply_to,
            reply_preview,
            deleted: false,
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
