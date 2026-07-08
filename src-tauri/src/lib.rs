mod crypto;
mod db;
mod identity;
mod llm;
mod media;
mod net;
mod pairing;
#[cfg(feature = "p2p")]
mod ratchet;

use std::sync::Mutex;
use base64::Engine;
use tauri::Manager;

use db::{Db, Message};
use identity::Identity;

/// Arquivo passado no lançamento (reservado; o TaylorChat não associa extensões por ora).
#[tauri::command(async)]
fn get_startup_file() -> Option<String> {
    None
}

/// Envia uma mensagem: grava como `queued` no banco e tenta despachar pela rede
/// (Fase 3). Se a rede não estiver compilada/disponível, fica `queued` e é
/// reenviada depois. Devolve a linha (com o estado resultante) pra UI.
#[tauri::command]
async fn send_message(
    app: tauri::AppHandle,
    db: tauri::State<'_, Db>,
    peer: String,
    body: String,
) -> Result<Message, String> {
    let body = body.trim().to_string();
    if body.is_empty() {
        return Err("mensagem vazia".into());
    }
    let mut msg = db::enqueue(&db, &peer, &body)?;
    match net::send_text(&app, &peer, &body, msg.ts).await {
        Ok(()) => {
            // O ACK do receptor confirma a entrega.
            db::set_state(&db, msg.id, "delivered")?;
            msg.state = "delivered".into();
        }
        Err(e) => {
            // fica na fila; nada de erro pro usuário — só log.
            eprintln!("[taylorchat] envio adiado (fica na fila): {e}");
        }
    }
    Ok(msg)
}

/// Anexa e envia um arquivo: lê do disco, guarda uma cópia local (pra abrir depois),
/// registra a mensagem `file` e tenta transferir pela rede (Fase 5). Se a rede não
/// estiver disponível, fica `queued` como o texto.
#[tauri::command]
async fn attach_file(
    app: tauri::AppHandle,
    db: tauri::State<'_, Db>,
    peer: String,
    path: String,
) -> Result<Message, String> {
    let size = std::fs::metadata(&path)
        .map_err(|e| format!("falha ao ler '{path}': {e}"))?
        .len();
    let filename = std::path::Path::new(&path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("arquivo")
        .to_string();
    let mime = media::guess_mime(&filename);
    // Cópia local (streaming, sem RAM) pra reabrir/reenviar; o envio lê dela.
    let local_path = media::copy_attachment(&app, &filename, &path)?;
    // Id de transferência + chave estáveis (guardados no meta local) pra permitir
    // retomada: um reenvio reusa os mesmos, e o receptor continua o parcial.
    let transfer_id = crypto::random_hex(16);
    let file_key = crypto::random_key();
    let meta = serde_json::json!({
        "filename": filename, "mime": mime, "size": size, "localPath": local_path,
        "transferId": transfer_id,
        "fileKey": base64::engine::general_purpose::STANDARD.encode(file_key),
    })
    .to_string();
    let mut msg = db::record_file(&db, &peer, "out", &meta, "queued", None)?;
    match net::send_file(&app, &peer, &filename, &mime, &local_path, &transfer_id, &file_key, msg.ts)
        .await
    {
        Ok(()) => {
            // O ACK do receptor confirma a entrega.
            db::set_state(&db, msg.id, "delivered")?;
            msg.state = "delivered".into();
        }
        Err(e) => {
            eprintln!("[taylorchat] anexo adiado (fica na fila): {e}");
        }
    }
    Ok(msg)
}

/// Avisa o par que li a conversa (recibo de leitura). Melhor esforço — se a rede não
/// estiver disponível/o par offline, simplesmente não confirma agora.
#[tauri::command]
async fn mark_read(app: tauri::AppHandle, peer: String) -> Result<(), String> {
    net::send_read(&app, &peer).await
}

/// Tenta reenviar o que ficou na fila (`queued`) pra um par. Para no primeiro erro
/// (par continua offline — evita N timeouts). Devolve quantas saíram.
#[tauri::command]
async fn resend_queued(
    app: tauri::AppHandle,
    db: tauri::State<'_, Db>,
    peer: String,
) -> Result<u32, String> {
    let queued = db::queued_out(&db, &peer)?;
    let mut sent = 0u32;
    for m in queued {
        let ok = if m.kind == "file" {
            // corpo = JSON de metadados; a cópia local é a fonte do reenvio
            let meta: serde_json::Value =
                serde_json::from_str(&m.body).map_err(|e| format!("anexo corrompido: {e}"))?;
            let (Some(path), Some(filename), Some(mime), Some(transfer_id), Some(key_b64)) = (
                meta["localPath"].as_str(),
                meta["filename"].as_str(),
                meta["mime"].as_str(),
                meta["transferId"].as_str(),
                meta["fileKey"].as_str(),
            ) else {
                db::set_state(&db, m.id, "failed")?; // sem dados pra reenviar
                continue;
            };
            let Ok(file_key) = base64::engine::general_purpose::STANDARD
                .decode(key_b64)
                .ok()
                .and_then(|v| <[u8; 32]>::try_from(v).ok())
                .ok_or(())
            else {
                db::set_state(&db, m.id, "failed")?;
                continue;
            };
            if !std::path::Path::new(path).exists() {
                db::set_state(&db, m.id, "failed")?; // cópia sumiu
                continue;
            }
            net::send_file(&app, &peer, filename, mime, path, transfer_id, &file_key, m.ts)
                .await
                .is_ok()
        } else {
            net::send_text(&app, &peer, &m.body, m.ts).await.is_ok()
        };
        if !ok {
            break;
        }
        db::set_state(&db, m.id, "delivered")?;
        sent += 1;
    }
    Ok(sent)
}

/// Palavra-chave combinada fora do app pra um contato: guarda a minha e manda o HASH
/// dela pro par (nunca a palavra em si). Best-effort — se o par estiver offline, a
/// palavra fica salva e o hash vai quando der. Palavra vazia = remove a minha.
#[tauri::command]
async fn set_keyword(
    app: tauri::AppHandle,
    db: tauri::State<'_, Db>,
    peer: String,
    word: String,
) -> Result<(), String> {
    let word = word.trim().to_string();
    db::set_keyword(&db, &peer, &word)?;
    if !word.is_empty() {
        let h = crypto::hash_hex(word.to_lowercase().as_bytes());
        let _ = net::send_keyword(&app, &peer, &h).await;
    }
    Ok(())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct KeywordStatus {
    has_mine: bool,
    has_peer: bool,
    matches: Option<bool>, // None = falta um dos lados
    word: Option<String>,  // a minha (pra pré-preencher o campo)
}

/// Estado da palavra-chave de um contato: se tenho a minha, se recebi a do par, e se
/// batem. Divergência NÃO bloqueia a conversa — só sinaliza.
#[tauri::command(async)]
fn keyword_status(db: tauri::State<'_, Db>, peer: String) -> Result<KeywordStatus, String> {
    let mine = db::get_keyword(&db, &peer)?;
    let peer_hash = db::get_peer_kw_hash(&db, &peer)?;
    let matches = match (&mine, &peer_hash) {
        (Some(w), Some(ph)) => Some(&crypto::hash_hex(w.trim().to_lowercase().as_bytes()) == ph),
        _ => None,
    };
    Ok(KeywordStatus {
        has_mine: mine.is_some(),
        has_peer: peer_hash.is_some(),
        matches,
        word: mine,
    })
}

/// Digest da conversa pra auditoria — os dois dispositivos comparam pra provar que o
/// conteúdo não foi adulterado (divergência = alguém mexeu no registro).
#[tauri::command(async)]
fn audit_conversation(
    db: tauri::State<'_, Db>,
    id: tauri::State<'_, Identity>,
    peer: String,
) -> Result<db::AuditResult, String> {
    db::audit_digest(&db, &peer, &id.node_id_hex())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(Db::default())
        .manage(Mutex::new(llm::LlmState::default()))
        .setup(|app| {
            // Identidade: gera no 1º uso, guarda no cofre do SO.
            let id = Identity::load_or_create().map_err(|e| {
                eprintln!("[taylorchat] falha na identidade: {e}");
                e
            })?;
            let secret = id.secret_bytes();
            app.manage(id);
            // Banco de histórico (com chave de cifra em repouso derivada da identidade).
            db::init(app.handle(), &secret)?;
            // Rede P2P (no-op no build padrão; iroh com --features p2p).
            net::start(app.handle().clone(), secret);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_startup_file,
            send_message,
            attach_file,
            mark_read,
            resend_queued,
            set_keyword,
            keyword_status,
            audit_conversation,
            pairing::my_identity,
            pairing::parse_invite,
            db::contacts_list,
            db::contact_add,
            db::contact_remove,
            db::messages_list,
            db::message_set_state,
            db::clear_conversation,
            db::conversations_summary,
            llm::list_models,
            llm::start_llm,
            llm::stop_llm,
            llm::llm_status
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                if let Some(state) = app_handle.try_state::<Mutex<llm::LlmState>>() {
                    if let Ok(mut s) = state.lock() {
                        if let Some(child) = s.child.as_mut() {
                            let _ = child.kill();
                        }
                    }
                }
            }
        });
}
