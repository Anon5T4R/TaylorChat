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
    match net::send_text(&app, &peer, &body).await {
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
    let bytes = std::fs::read(&path).map_err(|e| format!("falha ao ler '{path}': {e}"))?;
    let filename = std::path::Path::new(&path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("arquivo")
        .to_string();
    let mime = media::guess_mime(&filename);
    let local_path = media::save_attachment(&app, &filename, &bytes)?;
    let meta = serde_json::json!({
        "filename": filename, "mime": mime, "size": bytes.len(), "localPath": local_path,
    })
    .to_string();
    let mut msg = db::record_file(&db, &peer, "out", &meta, "queued")?;
    match net::send_file(&app, &peer, &filename, &mime, &bytes).await {
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
            pairing::my_identity,
            pairing::parse_invite,
            db::contacts_list,
            db::contact_add,
            db::contact_remove,
            db::messages_list,
            db::message_set_state,
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
