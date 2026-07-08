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
    fn mime_por_extensao() {
        assert_eq!(guess_mime("foto.PNG"), "image/png");
        assert_eq!(guess_mime("doc.pdf"), "application/pdf");
        assert_eq!(guess_mime("qualquer.xyz"), "application/octet-stream");
    }
}
