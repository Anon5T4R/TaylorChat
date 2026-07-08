//! Anexos (Fase 5, plano.md §5.6). O arquivo é transferido em **chunks** (1 MiB):
//! cada pedaço é comprimido (zstd) e cifrado com uma chave de uso único (crypto.rs),
//! então nem o remetente nem o destinatário seguram o arquivo inteiro em memória —
//! dá pra mandar arquivos grandes (sem teto de tamanho). Retomada via iroh-blobs
//! (content-addressed) fica como upgrade futuro.

use std::path::PathBuf;
use tauri::Manager;

// compress/decompress são usados pela rede (p2p) e pelos testes; no build padrão sem
// rede ficam sem chamador no binário — silencia o dead_code só nesse caso.
#[cfg_attr(all(not(feature = "p2p"), not(test)), allow(dead_code))]
pub fn compress(data: &[u8]) -> Result<Vec<u8>, String> {
    zstd::encode_all(data, 0).map_err(|e| format!("falha ao comprimir: {e}"))
}

#[cfg_attr(all(not(feature = "p2p"), not(test)), allow(dead_code))]
pub fn decompress(data: &[u8]) -> Result<Vec<u8>, String> {
    zstd::decode_all(data).map_err(|e| format!("falha ao descomprimir: {e}"))
}

/// Pasta `attachments` do app (criada na 1ª vez).
fn attachments_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("sem pasta de dados: {e}"))?
        .join("attachments");
    std::fs::create_dir_all(&dir).map_err(|e| format!("falha ao criar '{}': {e}", dir.display()))?;
    Ok(dir)
}

/// Caminho de destino único (timestamp + nome saneado) na pasta de anexos.
#[cfg_attr(not(feature = "p2p"), allow(dead_code))]
pub fn unique_dest(app: &tauri::AppHandle, filename: &str) -> Result<PathBuf, String> {
    let dir = attachments_dir(app)?;
    let safe = filename.rsplit(['/', '\\']).next().unwrap_or("arquivo");
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    Ok(dir.join(format!("{stamp}_{safe}")))
}

/// Copia (em streaming, sem RAM) o arquivo de origem pra pasta de anexos — a cópia
/// local do que EU enviei, pra poder reabrir/reenviar depois. Devolve o caminho.
pub fn copy_attachment(app: &tauri::AppHandle, filename: &str, src: &str) -> Result<String, String> {
    let dest = unique_dest(app, filename)?;
    std::fs::copy(src, &dest).map_err(|e| format!("falha ao copiar anexo: {e}"))?;
    Ok(dest.to_string_lossy().to_string())
}

/// Pasta de stickers do usuário (`<dados>/stickers/<pacote>/`).
fn stickers_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("sem pasta de dados: {e}"))?
        .join("stickers");
    std::fs::create_dir_all(&dir).map_err(|e| format!("falha ao criar '{}': {e}", dir.display()))?;
    Ok(dir)
}

fn sanitize(name: &str) -> String {
    let s: String = name.chars().filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_').collect();
    let s = s.trim();
    if s.is_empty() { "geral".to_string() } else { s.to_string() }
}

/// Copia uma imagem pra um pacote de stickers e devolve o caminho salvo.
pub fn add_sticker(app: &tauri::AppHandle, pack: &str, src: &str) -> Result<String, String> {
    let dir = stickers_dir(app)?.join(sanitize(pack));
    std::fs::create_dir_all(&dir).map_err(|e| format!("falha ao criar pacote: {e}"))?;
    let name = src.rsplit(['/', '\\']).next().unwrap_or("sticker");
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let dest = dir.join(format!("{stamp}_{name}"));
    std::fs::copy(src, &dest).map_err(|e| format!("falha ao salvar sticker: {e}"))?;
    Ok(dest.to_string_lossy().to_string())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sticker {
    pub pack: String,
    pub path: String,
}

// ── Perfil / avatares ───────────────────────────────────────────────────
fn profile_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("sem pasta de dados: {e}"))?
        .join("profile");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// Redimensiona uma imagem pra 128px (avatar) e salva como PNG no perfil. Devolve o caminho.
pub fn set_my_avatar(app: &tauri::AppHandle, src: &str) -> Result<String, String> {
    let img = image::open(src).map_err(|e| format!("imagem inválida: {e}"))?;
    let small = img.thumbnail(128, 128);
    let dest = profile_dir(app)?.join("avatar.png");
    small.save(&dest).map_err(|e| format!("falha ao salvar avatar: {e}"))?;
    Ok(dest.to_string_lossy().to_string())
}

/// Caminho do meu avatar, se existir.
pub fn my_avatar_path(app: &tauri::AppHandle) -> Option<String> {
    let p = profile_dir(app).ok()?.join("avatar.png");
    p.exists().then(|| p.to_string_lossy().to_string())
}

/// Bytes do meu avatar (pra transmitir), se existir.
#[cfg_attr(not(feature = "p2p"), allow(dead_code))]
pub fn my_avatar_bytes(app: &tauri::AppHandle) -> Option<Vec<u8>> {
    std::fs::read(my_avatar_path(app)?).ok()
}

/// Salva o avatar recebido de um contato (cache), sob o node_id. Devolve o caminho.
#[cfg_attr(not(feature = "p2p"), allow(dead_code))]
pub fn save_contact_avatar(app: &tauri::AppHandle, node_id: &str, bytes: &[u8]) -> Result<String, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("avatars");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let safe: String = node_id.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    let dest = dir.join(format!("{safe}.png"));
    std::fs::write(&dest, bytes).map_err(|e| e.to_string())?;
    Ok(dest.to_string_lossy().to_string())
}

/// Lista todos os stickers (todos os pacotes).
pub fn list_stickers(app: &tauri::AppHandle) -> Result<Vec<Sticker>, String> {
    let base = stickers_dir(app)?;
    let mut out = Vec::new();
    let packs = std::fs::read_dir(&base).map_err(|e| e.to_string())?;
    for pack_entry in packs.flatten() {
        if !pack_entry.path().is_dir() {
            continue;
        }
        let pack = pack_entry.file_name().to_string_lossy().to_string();
        if let Ok(files) = std::fs::read_dir(pack_entry.path()) {
            for f in files.flatten() {
                if f.path().is_file() {
                    out.push(Sticker { pack: pack.clone(), path: f.path().to_string_lossy().to_string() });
                }
            }
        }
    }
    Ok(out)
}

/// Caminho do arquivo parcial de uma transferência em andamento (retomada). Fica em
/// `attachments/.partial/<transferId>` e persiste entre reconexões/reinícios.
#[cfg_attr(not(feature = "p2p"), allow(dead_code))]
pub fn partial_path(app: &tauri::AppHandle, transfer_id: &str) -> Result<PathBuf, String> {
    let dir = attachments_dir(app)?.join(".partial");
    std::fs::create_dir_all(&dir).map_err(|e| format!("falha ao criar '{}': {e}", dir.display()))?;
    let safe: String = transfer_id.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    if safe.is_empty() {
        return Err("id de transferência inválido".into());
    }
    Ok(dir.join(safe))
}

/// Palpite simples de MIME pela extensão (só pra UI escolher ícone/preview).
pub fn guess_mime(filename: &str) -> String {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    let m = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "txt" | "md" => "text/plain",
        "mp4" => "video/mp4",
        "mp3" => "audio/mpeg",
        "zip" => "application/zip",
        _ => "application/octet-stream",
    };
    m.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_comprime_cifra_decifra_descomprime() {
        // Dado repetitivo comprime bem.
        let data = b"anexo do TaylorChat ".repeat(500);
        let comp = compress(&data).unwrap();
        assert!(comp.len() < data.len(), "zstd deveria reduzir dado repetitivo");

        // Comprimido → cifrado com chave de uso único → decifrado → descomprimido.
        let key = crate::crypto::derive_key(&[5u8; 32]);
        let ct = crate::crypto::encrypt(&key, &comp).unwrap();
        let back = crate::crypto::decrypt(&key, &ct).unwrap();
        assert_eq!(decompress(&back).unwrap(), data);
    }

    #[test]
    fn streaming_multi_chunk_reconstroi_o_arquivo() {
        // Simula o que a rede faz: parte o "arquivo" em pedaços, comprime+cifra cada
        // um, depois decifra+descomprime e concatena — tem que bater com o original.
        let file: Vec<u8> = (0..250_000u32).flat_map(|i| i.to_le_bytes()).collect();
        let key = crate::crypto::derive_key(&[9u8; 32]);
        let chunk = 64 * 1024;

        let mut out = Vec::new();
        for part in file.chunks(chunk) {
            let enc = crate::crypto::encrypt(&key, &compress(part).unwrap()).unwrap();
            let dec = decompress(&crate::crypto::decrypt(&key, &enc).unwrap()).unwrap();
            out.extend_from_slice(&dec);
        }
        assert_eq!(out, file);
    }

    #[test]
    fn retomada_reconstroi_apos_queda_no_meio() {
        // Arquivo em chunks; o receptor recebe só os K primeiros (simulando queda),
        // depois o remetente RETOMA do chunk K e o resto completa o arquivo idêntico.
        let file: Vec<u8> = (0..300_000u32).flat_map(|i| i.to_le_bytes()).collect();
        let key = crate::crypto::derive_key(&[3u8; 32]);
        let chunk = 64 * 1024;
        let chunks: Vec<&[u8]> = file.chunks(chunk).collect();

        // 1ª tentativa: recebe só os 2 primeiros chunks e "cai".
        let mut received: Vec<u8> = Vec::new();
        for part in chunks.iter().take(2) {
            let enc = crate::crypto::encrypt(&key, &compress(part).unwrap()).unwrap();
            received.extend_from_slice(&decompress(&crate::crypto::decrypt(&key, &enc).unwrap()).unwrap());
        }
        // Retomada: alinha à fronteira de chunk e continua daí.
        let from = received.len() / chunk; // = 2
        received.truncate(from * chunk);
        for part in chunks.iter().skip(from) {
            let enc = crate::crypto::encrypt(&key, &compress(part).unwrap()).unwrap();
            received.extend_from_slice(&decompress(&crate::crypto::decrypt(&key, &enc).unwrap()).unwrap());
        }
        assert_eq!(received, file);
    }

    #[test]
    fn mime_por_extensao() {
        assert_eq!(guess_mime("foto.PNG"), "image/png");
        assert_eq!(guess_mime("doc.pdf"), "application/pdf");
        assert_eq!(guess_mime("qualquer.xyz"), "application/octet-stream");
    }
}
