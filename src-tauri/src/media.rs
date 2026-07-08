//! Anexos (Fase 5, plano.md §5.6). Compressão zstd antes do envio; o arquivo é
//! comprimido, cifrado com uma chave de uso único (crypto.rs) e transferido pela
//! conexão iroh, junto de metadados (nome/mime/tamanho) que vão cifrados pelo ratchet.
//! Resumo/retomada via iroh-blobs (content-addressed) fica como upgrade futuro — este
//! primeiro corte transfere o arquivo inteiro pelo stream já cifrado.

use tauri::Manager;

// compress/decompress são usados pela rede (p2p) e pelos testes; no build padrão sem
// rede ficam sem chamador no binário — silencia o dead_code só nesse caso.
#[cfg_attr(all(not(feature = "p2p"), not(test)), allow(dead_code))]
pub fn compress(data: &[u8]) -> Result<Vec<u8>, String> {
    zstd::encode_all(data, 0).map_err(|e| format!("falha ao comprimir: {e}"))
}

/// Salva bytes num arquivo dentro da pasta `attachments` do app, com nome único, e
/// devolve o caminho local. Usada pelo remetente (cópia do que enviou) e pelo
/// destinatário (arquivo recebido) pra a UI conseguir abrir depois.
pub fn save_attachment(app: &tauri::AppHandle, filename: &str, data: &[u8]) -> Result<String, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("sem pasta de dados: {e}"))?
        .join("attachments");
    std::fs::create_dir_all(&dir).map_err(|e| format!("falha ao criar '{}': {e}", dir.display()))?;
    // só o nome do arquivo, sem componentes de caminho
    let safe = filename.rsplit(['/', '\\']).next().unwrap_or("arquivo");
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let dest = dir.join(format!("{stamp}_{safe}"));
    std::fs::write(&dest, data).map_err(|e| format!("falha ao salvar anexo: {e}"))?;
    Ok(dest.to_string_lossy().to_string())
}

#[cfg_attr(all(not(feature = "p2p"), not(test)), allow(dead_code))]
pub fn decompress(data: &[u8]) -> Result<Vec<u8>, String> {
    zstd::decode_all(data).map_err(|e| format!("falha ao descomprimir: {e}"))
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
    fn mime_por_extensao() {
        assert_eq!(guess_mime("foto.PNG"), "image/png");
        assert_eq!(guess_mime("doc.pdf"), "application/pdf");
        assert_eq!(guess_mime("qualquer.xyz"), "application/octet-stream");
    }
}
