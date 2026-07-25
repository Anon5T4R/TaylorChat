mod crypto;
mod db;
mod identity;
mod llm;
mod media;
mod net;
mod notify;
mod pairing;
#[cfg(feature = "p2p")]
mod ratchet;
mod storage;

use std::sync::Mutex;
use base64::Engine;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, WindowEvent};

use db::{Db, Message};
use identity::Identity;
use tauri_plugin_notification::NotificationExt;

/// Estado de notificação que só existe em memória: qual conversa está aberta na UI
/// (empurrada pelo front a cada troca) e qual foi a última que gerou aviso.
///
/// A conversa aberta NÃO vem do banco de propósito: é estado de sessão, e persistir isso
/// faria o app "lembrar" de uma conversa aberta que não está mais na tela depois de um
/// reinício — calando a notificação certa pelo motivo errado.
#[derive(Default)]
pub struct NotifyState {
    /// Conversa aberta na UI agora (`node` ou `node#thread`), ou None.
    active: Mutex<Option<String>>,
    /// Conversa do último aviso mostrado — é o alvo do clique na notificação.
    last: Mutex<Option<String>>,
}

/// Notificações ligadas? Preferência do USUÁRIO, guardada no app (default: ligado).
///
/// Não perguntamos ao SO. O `permission_state()` do tauri-plugin-notification devolve
/// `Granted` fixo no desktop (não consulta nada), então tratar isso como preferência
/// seria adotar uma resposta inventada — é a mesma armadilha do autostart, onde o estado
/// do SO não é a fonte da verdade da intenção do usuário. A intenção mora aqui.
fn notify_enabled(db: &Db) -> bool {
    db::meta_get(db, "notify_enabled")
        .ok()
        .flatten()
        .map(|b| b.as_slice() != b"0")
        .unwrap_or(true)
}

/// Notificação de desktop pra mensagem recebida. Chamada pela camada de rede ao receber.
/// `node` é o node_id puro (identifica o CONTATO — é por ele que o mudo é gravado);
/// `convo` é a chave da conversa (`node` ou `node#thread`) — é por ela que se compara com
/// a conversa aberta, senão duas threads do mesmo contato se calariam uma à outra.
/// Best-effort: nunca estoura.
pub fn notify_incoming(app: &tauri::AppHandle, node: &str, convo: &str, preview: &str) {
    let focused = app
        .get_webview_window("main")
        .and_then(|w| w.is_focused().ok())
        .unwrap_or(false);
    let state = app.state::<Db>();
    let ns = app.state::<NotifyState>();
    let active = ns.active.lock().ok().and_then(|g| g.clone());

    let decision = notify::should_notify(notify::NotifyInput {
        enabled: notify_enabled(&state),
        focused,
        active_convo: active.as_deref(),
        incoming_convo: convo,
        muted: db::is_muted(&state, node),
    });
    if let notify::NotifyDecision::Skip(reason) = decision {
        // Loga só o que o usuário pode querer desfazer (desligado/mudo) — é o que
        // responde "por que não apitou?" sem adivinhar qual regra mordeu.
        // `AlreadyWatching` fica de fora de propósito: é o caso COMUM (conversa aberta
        // na frente), e uma linha por mensagem varreria o log de rede, que guarda só as
        // últimas 200 — afogaria justamente o diagnóstico de rede que o painel serve.
        if reason != notify::SkipReason::AlreadyWatching {
            net::report(app, format!("notificação suprimida ({reason:?})"), false);
        }
        return;
    }

    let title = db::contact_name(&state, node);
    // Prévia do conteúdo é opcional (Configurações): quem não quer vazar o texto numa
    // tela compartilhada vê só "quem", não "o quê". Default: mostra (ausente = ligado).
    let show_preview = db::meta_get(&state, "notify_preview")
        .ok()
        .flatten()
        .map(|b| b.as_slice() != b"0")
        .unwrap_or(true);
    let body = if show_preview { preview } else { "💬" };
    // Guarda o alvo ANTES de mostrar: o clique no toast relança o exe, e o
    // single-instance (abaixo) lê isso pra abrir a conversa certa.
    if let Ok(mut g) = ns.last.lock() {
        *g = Some(convo.to_string());
    }
    // `show()` sempre devolve Ok: o plugin dispara o toast numa task e engole o erro
    // (desktop.rs), então NÃO dá pra saber daqui se o aviso apareceu. Por isso a UI não
    // promete que funcionou — ver `settings.notifyOsHint`.
    let _ = app.notification().builder().title(title).body(body).show();
}

/// Arquivo passado no lançamento (reservado; o TaylorChat não associa extensões por ora).
#[tauri::command(async)]
fn get_startup_file() -> Option<String> {
    None
}

/// Mostra/foca a janela principal (da bandeja ou de um 2º lançamento).
fn open_main(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Separa a chave de conversa em (node_id, thread). `node` (principal) ou
/// `node#threadId` (extra). A rede usa o node_id (conecta + ratchet); o banco usa a
/// chave inteira; o `thread` viaja no envelope pra os dois lados baterem.
fn split_convo(convo: &str) -> (&str, &str) {
    match convo.split_once('#') {
        Some((node, thread)) => (node, thread),
        None => (convo, ""),
    }
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
    reply_to: Option<i64>,
    reply_preview: Option<String>,
) -> Result<Message, String> {
    let body = body.trim().to_string();
    if body.is_empty() {
        return Err("mensagem vazia".into());
    }
    let (node, thread) = split_convo(&peer);
    let mut msg = db::enqueue(&db, &peer, &body, reply_to, reply_preview.as_deref())?;
    match net::send_text(&app, node, thread, &body, msg.ts, reply_to, reply_preview.as_deref()).await
    {
        Ok(()) => {
            // O ACK do receptor confirma a entrega.
            db::set_state(&db, msg.id, "delivered")?;
            msg.state = "delivered".into();
        }
        Err(e) => {
            // Não conseguimos entregar AGORA (par inalcançável ou rede instável) — não é
            // prova de que o par está offline. Fica na fila e reenvia sozinho.
            net::report(&app, format!("mensagem na fila — reenvio automático quando o contato ficar alcançável ({e})"), true);
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
    sticker: bool,
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
        "sticker": sticker,
    })
    .to_string();
    let (node, thread) = split_convo(&peer);
    let mut msg = db::record_file(&db, &peer, "out", &meta, "queued", None)?;
    match net::send_file(
        &app, node, &filename, &mime, &local_path, &transfer_id, &file_key, msg.ts, thread, sticker,
    )
    .await
    {
        Ok(()) => {
            // O ACK do receptor confirma a entrega.
            db::set_state(&db, msg.id, "delivered")?;
            msg.state = "delivered".into();
        }
        Err(e) => {
            net::report(&app, format!("anexo na fila — reenvio automático quando o contato ficar alcançável ({e})"), true);
        }
    }
    Ok(msg)
}

/// Avisa o par que li a conversa (recibo de leitura). Melhor esforço — se a rede não
/// estiver disponível/o par offline, simplesmente não confirma agora.
#[tauri::command]
async fn mark_read(app: tauri::AppHandle, peer: String) -> Result<(), String> {
    let (node, thread) = split_convo(&peer);
    net::send_read(&app, node, thread).await
}

/// Avisa o par que estou (ou parei de) digitar. Best-effort; só vai se há conexão quente.
#[tauri::command]
async fn send_typing(peer: String, on: bool) -> Result<(), String> {
    let (node, thread) = split_convo(&peer);
    net::send_typing(node, thread, on).await;
    Ok(())
}

/// Apaga uma mensagem PARA TODOS: marca localmente (soft-delete) e avisa o par.
/// `target_ts` = ts da mensagem (a mesma chave nos dois aparelhos).
#[tauri::command]
async fn delete_for_everyone(
    app: tauri::AppHandle,
    db: tauri::State<'_, Db>,
    peer: String,
    target_ts: i64,
) -> Result<(), String> {
    db::mark_deleted(&db, &peer, target_ts, "out")?; // apago a MINHA mensagem
    // Enfileira e tenta já; se o par estiver offline, o resend_all reenvia até o ACK.
    db::pending_delete_add(&db, &peer, target_ts)?;
    let (node, thread) = split_convo(&peer);
    if net::send_delete(&app, node, thread, target_ts).await.is_ok() {
        let _ = db::pending_delete_remove(&db, &peer, target_ts);
    }
    Ok(())
}

/// Reage a uma mensagem (emoji vazio = remove a minha). Guarda a minha e avisa o par.
#[tauri::command]
async fn react(
    app: tauri::AppHandle,
    db: tauri::State<'_, Db>,
    peer: String,
    target_ts: i64,
    emoji: String,
) -> Result<(), String> {
    db::reaction_set(&db, &peer, target_ts, true, &emoji)?;
    // Enfileira e tenta já; se o par estiver offline, o resend_all reenvia até o ACK.
    db::pending_reaction_set(&db, &peer, target_ts, &emoji)?;
    let (node, thread) = split_convo(&peer);
    if net::send_reaction(&app, node, thread, target_ts, &emoji).await.is_ok() {
        let _ = db::pending_reaction_remove(&db, &peer, target_ts);
    }
    Ok(())
}

/// Tenta reenviar o que ficou na fila (`queued`) pra um par. Para no primeiro erro
/// (par continua offline — evita N timeouts). Devolve quantas saíram.
#[tauri::command]
async fn resend_queued(
    app: tauri::AppHandle,
    db: tauri::State<'_, Db>,
    peer: String,
) -> Result<u32, String> {
    do_resend(&app, &db, &peer).await
}

/// Reenvia a fila de TODAS as conversas (chamado periodicamente pela UI). Devolve o
/// total que saiu.
#[tauri::command]
async fn resend_all(app: tauri::AppHandle, db: tauri::State<'_, Db>) -> Result<u32, String> {
    let convos = db::queued_convos(&db)?;
    let mut total = 0u32;
    for convo in convos {
        total += do_resend(&app, &db, &convo).await.unwrap_or(0);
    }
    // Escoa os "apagar para todos" que ficaram pendentes (par estava offline).
    for (convo, ts) in db::pending_deletes_all(&db).unwrap_or_default() {
        let (node, thread) = split_convo(&convo);
        if net::send_delete(&app, node, thread, ts).await.is_ok() {
            let _ = db::pending_delete_remove(&db, &convo, ts);
        }
    }
    // Idem pras reações pendentes.
    for (convo, ts, emoji) in db::pending_reactions_all(&db).unwrap_or_default() {
        let (node, thread) = split_convo(&convo);
        if net::send_reaction(&app, node, thread, ts, &emoji).await.is_ok() {
            let _ = db::pending_reaction_remove(&db, &convo, ts);
        }
    }
    Ok(total)
}

async fn do_resend(app: &tauri::AppHandle, db: &Db, peer: &str) -> Result<u32, String> {
    let (node, thread) = split_convo(peer);
    let queued = db::queued_out(db, peer)?;
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
            let sticker = meta["sticker"].as_bool().unwrap_or(false);
            net::send_file(
                &app, node, filename, mime, path, transfer_id, &file_key, m.ts, thread, sticker,
            )
            .await
            .is_ok()
        } else {
            net::send_text(&app, node, thread, &m.body, m.ts, m.reply_to, m.reply_preview.as_deref())
                .await
                .is_ok()
        };
        if !ok {
            break;
        }
        db::set_state(&db, m.id, "delivered")?;
        sent += 1;
    }
    Ok(sent)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Profile {
    name: String,
    avatar: Option<String>,
}

fn read_profile(app: &tauri::AppHandle, db: &Db) -> Result<Profile, String> {
    let name = db::meta_get(db, "profile_name")?
        .map(|b| String::from_utf8_lossy(&b).to_string())
        .unwrap_or_default();
    Ok(Profile { name, avatar: media::my_avatar_path(app) })
}

/// Define meu perfil (nome + foto opcional). A foto é redimensionada pra 128px.
#[tauri::command(async)]
fn set_profile(
    app: tauri::AppHandle,
    db: tauri::State<'_, Db>,
    name: String,
    photo: Option<String>,
) -> Result<Profile, String> {
    db::meta_set(&db, "profile_name", name.trim().as_bytes())?;
    if let Some(p) = photo.filter(|p| !p.is_empty()) {
        media::set_my_avatar(&app, &p)?;
    }
    read_profile(&app, &db)
}

#[tauri::command(async)]
fn get_profile(app: tauri::AppHandle, db: tauri::State<'_, Db>) -> Result<Profile, String> {
    read_profile(&app, &db)
}

/// Manda meu perfil pra um contato (best-effort; a UI chama ao abrir a conversa).
#[tauri::command]
async fn send_profile(app: tauri::AppHandle, peer: String) -> Result<(), String> {
    let (node, _thread) = split_convo(&peer);
    net::send_profile(&app, node).await
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
    let (node, _thread) = split_convo(&peer);
    db::audit_digest(&db, &peer, &id.node_id_hex(), node)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct NetStatus {
    up: bool,
    node_id: String,
}

/// Status da rede (endpoint no ar?) + meu node_id — pro diagnóstico.
#[tauri::command(async)]
fn net_status(id: tauri::State<'_, Identity>) -> NetStatus {
    NetStatus { up: net::is_up(), node_id: id.node_id_hex() }
}

/// Log recente de eventos de rede (pro painel de diagnóstico).
#[tauri::command(async)]
fn net_log() -> Vec<String> {
    net::log_lines()
}

/// Se a notificação de desktop mostra a prévia do texto (default: sim).
#[tauri::command(async)]
fn get_notify_preview(db: tauri::State<'_, Db>) -> bool {
    db::meta_get(&db, "notify_preview")
        .ok()
        .flatten()
        .map(|b| b.as_slice() != b"0")
        .unwrap_or(true)
}

#[tauri::command(async)]
fn set_notify_preview(db: tauri::State<'_, Db>, on: bool) -> Result<(), String> {
    db::meta_set(&db, "notify_preview", if on { b"1" } else { b"0" })
}

/// Chave geral das notificações de desktop (default: ligado).
#[tauri::command(async)]
fn get_notify_enabled(db: tauri::State<'_, Db>) -> bool {
    notify_enabled(&db)
}

#[tauri::command(async)]
fn set_notify_enabled(db: tauri::State<'_, Db>, on: bool) -> Result<(), String> {
    db::meta_set(&db, "notify_enabled", if on { b"1" } else { b"0" })
}

/// O front avisa qual conversa está aberta (ou None ao fechar/voltar pra lista). É a
/// segunda metade do "não avise o que já estou vendo" — a primeira (foco da janela) o
/// back lê sozinho, porque foco muda sem passar pela UI.
///
/// Abrir a conversa também limpa o alvo pendente do clique: se o usuário já chegou lá
/// por conta própria, o relançamento seguinte não deve arrastá-lo de volta.
#[tauri::command(async)]
fn set_active_convo(ns: tauri::State<'_, NotifyState>, convo: Option<String>) {
    if let Ok(mut g) = ns.active.lock() {
        *g = convo.clone();
    }
    if let Ok(mut g) = ns.last.lock() {
        if g.as_deref() == convo.as_deref() {
            *g = None;
        }
    }
}

/// Começa a observar a presença do par (conexão quente + heartbeat ping/pong) e devolve
/// o status agora. Daí em diante a UI recebe o evento `presence` a cada mudança —
/// online/offline em tempo real, sem chute.
#[tauri::command]
async fn peer_online(app: tauri::AppHandle, peer: String) -> Result<bool, String> {
    let (node, _thread) = split_convo(&peer);
    Ok(net::watch(&app, node).await)
}

/// Para de observar a presença de um par (a conversa foi fechada/trocada) — encerra o
/// watcher e a conexão quente, em vez de reconectar pra sempre (L4).
#[tauri::command(async)]
fn peer_unwatch(peer: String) {
    let (node, _thread) = split_convo(&peer);
    net::unwatch(node);
}

/// Reflete o total de não-lidos na bandeja (tooltip) e no título da janela — o "badge"
/// de desktop. O Windows mostra o título no hover da taskbar e no Alt+Tab. count=0 volta
/// ao nome limpo.
#[tauri::command(async)]
fn set_badge(app: tauri::AppHandle, count: u32) {
    let text = if count > 0 {
        format!("TaylorChat ({count})")
    } else {
        "TaylorChat".to_string()
    };
    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_tooltip(Some(&text));
    }
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.set_title(&text);
    }
}

/// Lista todos os stickers salvos (todos os pacotes).
#[tauri::command(async)]
fn stickers_list(app: tauri::AppHandle) -> Result<Vec<media::Sticker>, String> {
    media::list_stickers(&app)
}

/// Cria um sticker a partir de uma imagem qualquer (copia pro pacote). Devolve o caminho.
#[tauri::command(async)]
fn sticker_add(app: tauri::AppHandle, pack: String, src: String) -> Result<String, String> {
    media::add_sticker(&app, &pack, &src)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Linux: o webkit2gtk pinta a janela INTEIRA de branco em várias combinações
    // de driver/compositor — o app sobe, o processo vive, e não há erro pra ler.
    // (Visto num Arch com GNOME/Wayland; o LocalAI já tinha pago o mesmo pedágio.)
    // Como o WebView é o mesmo em toda a suíte, este bloco é IDÊNTICO nos 31 apps.
    // Desliga o renderer DMABUF (suspeito nº 1), o compositing (reforço) e, em
    // Wayland, força XWayland — em AppImage o branco costuma sobreviver aos dois
    // primeiros. Custa aceleração no WebView, e branco é pior que lento.
    // Variável já setada MANDA (inclusive `=0`): quem depurou o próprio sistema
    // não pode ser sobrescrito por nós. Tem que vir ANTES do GTK subir — o
    // webkitgtk lê estas variáveis uma vez só, no arranque.
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
        if std::env::var_os("WEBKIT_DISABLE_COMPOSITING_MODE").is_none() {
            std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        }
        let on_wayland = std::env::var_os("WAYLAND_DISPLAY").is_some()
            || std::env::var("XDG_SESSION_TYPE")
                .map(|t| t.eq_ignore_ascii_case("wayland"))
                .unwrap_or(false);
        if on_wayland && std::env::var_os("GDK_BACKEND").is_none() {
            std::env::set_var("GDK_BACKEND", "x11");
        }
    }

    tauri::Builder::default()
        // Um 2º lançamento do exe cai aqui em vez de abrir outra janela. Duas coisas
        // chegam por este caminho: o usuário clicando no atalho com o app já rodando e —
        // o que nos interessa — o CLIQUE NA NOTIFICAÇÃO, que no Windows reativa o app
        // pelo AppUserModelID (ou seja, relança o exe). O plugin de notificação não
        // oferece callback de clique no desktop (só 3 comandos: notify, request_permission,
        // is_permission_granted), então este é o único gancho disponível.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            open_main(app);
            // Se havia um aviso pendente, pula pra conversa dele. Só dispara uma vez: o
            // alvo é consumido aqui e some quando o usuário abre a conversa sozinho —
            // assim abrir o app pelo atalho não arrasta ninguém pra lugar nenhum.
            let target = app
                .try_state::<NotifyState>()
                .and_then(|ns| ns.last.lock().ok().and_then(|mut g| g.take()));
            if let Some(convo) = target {
                use tauri::Emitter;
                let _ = app.emit("open-convo", convo);
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .manage(Db::default())
        .manage(NotifyState::default())
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

            // Bandeja do Windows: fechar a janela ESCONDE (o app segue rodando e
            // recebendo); reabre pela bandeja. "Sair" encerra de verdade.
            let show = MenuItem::with_id(app, "show", "Abrir TaylorChat", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Sair", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            let _tray = TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("TaylorChat")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => open_main(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        open_main(tray.app_handle());
                    }
                })
                .build(app)?;

            if let Some(win) = app.get_webview_window("main") {
                let w = win.clone();
                let handle = app.handle().clone();
                win.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = w.hide();
                        // Indo pra bandeja (o app pode ficar dias assim): consolida o WAL.
                        if let Some(db) = handle.try_state::<Db>() {
                            db::checkpoint(&db);
                        }
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_startup_file,
            send_message,
            attach_file,
            mark_read,
            send_typing,
            delete_for_everyone,
            react,
            db::message_delete,
            db::reactions_list,
            resend_queued,
            resend_all,
            net_status,
            net_log,
            peer_online,
            peer_unwatch,
            get_notify_preview,
            set_notify_preview,
            get_notify_enabled,
            set_notify_enabled,
            set_active_convo,
            set_badge,
            db::unread_list,
            db::unread_set,
            db::search_messages,
            set_keyword,
            keyword_status,
            audit_conversation,
            set_profile,
            get_profile,
            send_profile,
            stickers_list,
            sticker_add,
            pairing::my_identity,
            pairing::parse_invite,
            db::contacts_list,
            db::contact_add,
            db::contact_remove,
            db::set_muted,
            db::set_contact_info,
            db::messages_list,
            db::message_set_state,
            db::clear_conversation,
            db::conversations_summary,
            db::threads_list,
            db::thread_create,
            db::thread_delete,
            storage::storage_info,
            storage::storage_clear_orphan_attachments,
            storage::storage_clear_partials,
            storage::storage_clear_orphan_avatars,
            storage::storage_clear_old_backups,
            llm::list_models,
            llm::start_llm,
            llm::stop_llm,
            llm::llm_status
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                // Última chance de consolidar o WAL antes do processo morrer.
                if let Some(db) = app_handle.try_state::<Db>() {
                    db::checkpoint(&db);
                }
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
