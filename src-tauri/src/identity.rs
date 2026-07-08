//! Identidade do usuário = um par de chaves ed25519 (o MESMO tipo de chave que o
//! iroh usa como `NodeId`, ver plano.md §5.2). Gerada no primeiro uso; o segredo
//! mora no cofre do SO (keyring: DPAPI no Windows, Secret Service no Linux). O
//! `node_id` público (hex das 32 bytes) é a "identidade Taylor" que vai no convite.

use ed25519_dalek::{SigningKey, VerifyingKey};
use keyring::Entry;

const KEYRING_SERVICE: &str = "TaylorChat";
const KEYRING_USER: &str = "identity";

/// Handle da identidade carregada em memória.
pub struct Identity {
    signing: SigningKey,
}

impl Identity {
    /// Carrega a identidade do cofre do SO, gerando uma nova na primeira execução.
    pub fn load_or_create() -> Result<Self, String> {
        let entry = Entry::new(KEYRING_SERVICE, KEYRING_USER)
            .map_err(|e| format!("cofre do SO indisponível: {e}"))?;
        match entry.get_password() {
            Ok(b64) => {
                let bytes = base64_decode_32(&b64)?;
                Ok(Self { signing: SigningKey::from_bytes(&bytes) })
            }
            Err(keyring::Error::NoEntry) => {
                let signing = SigningKey::generate(&mut rand::rngs::OsRng);
                let b64 = base64_encode(&signing.to_bytes());
                entry
                    .set_password(&b64)
                    .map_err(|e| format!("falha ao gravar identidade no cofre: {e}"))?;
                Ok(Self { signing })
            }
            Err(e) => Err(format!("falha ao ler identidade do cofre: {e}")),
        }
    }

    pub fn verifying(&self) -> VerifyingKey {
        self.signing.verifying_key()
    }

    /// Id público em hex (64 chars). Bytes idênticos ao `NodeId` do iroh; quando a
    /// Fase 3 entrar, dá pra reformatar como z-base-32 sem trocar a chave.
    pub fn node_id_hex(&self) -> String {
        hex(&self.verifying().to_bytes())
    }

    /// Bytes crus do segredo (32) — usados pra semear o `SecretKey` do iroh na Fase 3.
    pub fn secret_bytes(&self) -> [u8; 32] {
        self.signing.to_bytes()
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Decodifica um hex de 64 chars em 32 bytes.
pub fn hex_decode_32(s: &str) -> Result<[u8; 32], String> {
    let s = s.trim();
    if s.len() != 64 {
        return Err("id inválido (esperado 64 caracteres hex)".into());
    }
    let mut out = [0u8; 32];
    for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
        let hi = hex_val(chunk[0])?;
        let lo = hex_val(chunk[1])?;
        out[i] = (hi << 4) | lo;
    }
    Ok(out)
}

fn hex_val(c: u8) -> Result<u8, String> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err("caractere hex inválido".into()),
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn base64_decode_32(s: &str) -> Result<[u8; 32], String> {
    use base64::Engine;
    let v = base64::engine::general_purpose::STANDARD
        .decode(s.as_bytes())
        .map_err(|e| format!("segredo da identidade corrompido: {e}"))?;
    v.try_into()
        .map_err(|_| "segredo da identidade com tamanho inesperado".to_string())
}
