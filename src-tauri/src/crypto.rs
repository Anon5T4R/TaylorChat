//! Cifra do histórico em repouso (Fase 4, plano.md §5.4). AEAD XChaCha20-Poly1305
//! sobre o corpo das mensagens; a chave é derivada (HKDF-SHA256) do segredo da
//! identidade, que já vive no cofre do SO — nunca toca o disco em claro. Formato do
//! blob: `nonce(24) || ciphertext+tag`. Metadados (ts, peer) ficam em claro por ora;
//! cifra do arquivo inteiro (SQLCipher) é uma opção futura anotada no plano.

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::Sha256;

const NONCE_LEN: usize = 24;

/// Deriva a chave de 32 bytes do segredo da identidade (domínio separado por rótulo).
pub fn derive_key(identity_secret: &[u8; 32]) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(Some(b"taylorchat".as_slice()), identity_secret);
    let mut okm = [0u8; 32];
    hk.expand(b"at-rest-v1", &mut okm).expect("hkdf expand de 32 bytes nunca falha");
    okm
}

pub fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    let mut nonce = [0u8; NONCE_LEN];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let ct = cipher
        .encrypt(XNonce::from_slice(&nonce), plaintext)
        .map_err(|_| "falha ao cifrar".to_string())?;
    let mut out = Vec::with_capacity(NONCE_LEN + ct.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(out)
}

pub fn decrypt(key: &[u8; 32], data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() < NONCE_LEN {
        return Err("dado cifrado curto demais".into());
    }
    let (nonce, ct) = data.split_at(NONCE_LEN);
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    cipher
        .decrypt(XNonce::from_slice(nonce), ct)
        .map_err(|_| "falha ao decifrar (chave errada ou dado corrompido)".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_e_chave_errada() {
        let key = derive_key(&[7u8; 32]);
        let ct = encrypt(&key, "olá, mundo 🌎".as_bytes()).unwrap();
        // não vaza o texto em claro
        assert!(!ct.windows(3).any(|w| w == b"ol\xc3"));
        // decifra de volta
        assert_eq!(decrypt(&key, &ct).unwrap(), "olá, mundo 🌎".as_bytes());
        // chave derivada de outro segredo não abre
        let outra = derive_key(&[9u8; 32]);
        assert!(decrypt(&outra, &ct).is_err());
    }

    #[test]
    fn derivacao_e_deterministica() {
        assert_eq!(derive_key(&[1u8; 32]), derive_key(&[1u8; 32]));
        assert_ne!(derive_key(&[1u8; 32]), derive_key(&[2u8; 32]));
    }
}
