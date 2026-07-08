//! Double ratchet (Fase 4, plano.md §5.3) via **vodozemac** (impl. Rust auditada do
//! Olm do Matrix). Dá sigilo futuro (forward secrecy) por mensagem sobre o canal já
//! autenticado do iroh. Este módulo é o NÚCLEO puro (sem banco, testável em processo
//! único); a persistência dos pickles cifrados e o handshake sobre a conexão ficam no
//! `net.rs`. Só compila com a feature `p2p`.

use vodozemac::olm::{Account, OlmMessage, Session, SessionConfig};
use vodozemac::Curve25519PublicKey;

/// Bundle de pré-chaves que um par publica pro outro iniciar a sessão (X3DH).
/// Trafega pelo canal iroh JÁ autenticado (ambos conhecem o node_id/identidade um do
/// outro), então não precisa de assinatura extra aqui.
pub struct PreKeyBundle {
    pub identity_key: String, // Curve25519, base64
    pub one_time_key: String, // Curve25519, base64
}

// ── Conta (identidade Olm) ─────────────────────────────────────────────
pub fn new_account() -> Account {
    Account::new()
}

pub fn account_to_bytes(account: &Account) -> Vec<u8> {
    serde_json::to_vec(&account.pickle()).expect("pickle de conta serializa")
}

pub fn account_from_bytes(bytes: &[u8]) -> Result<Account, String> {
    let pickle = serde_json::from_slice(bytes).map_err(|e| format!("pickle de conta inválido: {e}"))?;
    Ok(Account::from_pickle(pickle))
}

/// Gera um bundle novo (identidade + uma pré-chave de uso único) pra entregar a quem
/// vai iniciar a sessão. A parte privada da OTK fica na conta até ser consumida.
pub fn our_bundle(account: &mut Account) -> Result<PreKeyBundle, String> {
    account.generate_one_time_keys(1);
    let identity_key = account.curve25519_key().to_base64();
    let one_time_key = account
        .one_time_keys()
        .values()
        .next()
        .ok_or("nenhuma pré-chave disponível")?
        .to_base64();
    Ok(PreKeyBundle { identity_key, one_time_key })
}

// ── Sessão ─────────────────────────────────────────────────────────────
pub fn session_to_bytes(session: &Session) -> Vec<u8> {
    serde_json::to_vec(&session.pickle()).expect("pickle de sessão serializa")
}

pub fn session_from_bytes(bytes: &[u8]) -> Result<Session, String> {
    let pickle = serde_json::from_slice(bytes).map_err(|e| format!("pickle de sessão inválido: {e}"))?;
    Ok(Session::from_pickle(pickle))
}

/// Inicia uma sessão de saída a partir do bundle do outro e já cifra a 1ª mensagem
/// (vira uma OlmMessage do tipo PreKey). Devolve a sessão pra guardar + o frame pra rede.
pub fn start_outbound(
    account: &Account,
    their_bundle: &PreKeyBundle,
    plaintext: &str,
) -> Result<(Session, Vec<u8>), String> {
    let identity_key = Curve25519PublicKey::from_base64(&their_bundle.identity_key)
        .map_err(|e| format!("identity_key inválida: {e}"))?;
    let one_time_key = Curve25519PublicKey::from_base64(&their_bundle.one_time_key)
        .map_err(|e| format!("one_time_key inválida: {e}"))?;
    let mut session =
        account.create_outbound_session(SessionConfig::version_1(), identity_key, one_time_key);
    let wire = olm_to_wire(&session.encrypt(plaintext.as_bytes()));
    Ok((session, wire))
}

/// Recebe o primeiro frame (uma PreKey OlmMessage) e cria a sessão de entrada,
/// devolvendo a sessão + o texto em claro.
pub fn start_inbound(
    account: &mut Account,
    their_identity_key_b64: &str,
    wire: &[u8],
) -> Result<(Session, String), String> {
    let their_identity_key = Curve25519PublicKey::from_base64(their_identity_key_b64)
        .map_err(|e| format!("identity_key inválida: {e}"))?;
    let msg = wire_to_olm(wire)?;
    let prekey = match msg {
        OlmMessage::PreKey(m) => m,
        OlmMessage::Normal(_) => return Err("esperava uma mensagem PreKey pra abrir sessão".into()),
    };
    let result = account
        .create_inbound_session(their_identity_key, &prekey)
        .map_err(|e| format!("falha ao abrir sessão de entrada: {e}"))?;
    let text = String::from_utf8_lossy(&result.plaintext).to_string();
    Ok((result.session, text))
}

pub fn encrypt(session: &mut Session, plaintext: &str) -> Vec<u8> {
    olm_to_wire(&session.encrypt(plaintext.as_bytes()))
}

pub fn decrypt(session: &mut Session, wire: &[u8]) -> Result<String, String> {
    let msg = wire_to_olm(wire)?;
    let plaintext = session.decrypt(&msg).map_err(|e| format!("falha ao decifrar: {e}"))?;
    Ok(String::from_utf8_lossy(&plaintext).to_string())
}

// ── Framing da OlmMessage pra rede: [tipo:1][ciphertext] ───────────────
fn olm_to_wire(msg: &OlmMessage) -> Vec<u8> {
    let (kind, ciphertext) = msg.to_parts();
    let mut out = Vec::with_capacity(1 + ciphertext.len());
    out.push(kind as u8);
    out.extend_from_slice(&ciphertext);
    out
}

fn wire_to_olm(wire: &[u8]) -> Result<OlmMessage, String> {
    let (kind, ciphertext) = wire.split_first().ok_or("frame Olm vazio")?;
    OlmMessage::from_parts(*kind as usize, ciphertext).map_err(|e| format!("frame Olm inválido: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn troca_forward_secret_entre_dois_pares() {
        // Alice quer falar com Bob. Bob publica um bundle (via canal autenticado).
        let alice = new_account();
        let alice_identity = alice.curve25519_key().to_base64();
        let mut bob = new_account();
        let bob_bundle = our_bundle(&mut bob).unwrap();

        // Alice abre a sessão e manda a 1ª mensagem (PreKey).
        let (mut alice_session, wire1) = start_outbound(&alice, &bob_bundle, "oi, Bob").unwrap();

        // Bob recebe, abre a sessão de entrada (com a identidade da Alice) e lê.
        let (mut bob_session, text1) = start_inbound(&mut bob, &alice_identity, &wire1).unwrap();
        assert_eq!(text1, "oi, Bob");

        // Bob responde; Alice lê. (o ratchet avança as chaves a cada troca)
        let wire2 = encrypt(&mut bob_session, "e aí, Alice");
        assert_eq!(decrypt(&mut alice_session, &wire2).unwrap(), "e aí, Alice");

        // Mais uma ida e volta pra exercitar o avanço do ratchet.
        let wire3 = encrypt(&mut alice_session, "tudo certo?");
        assert_eq!(decrypt(&mut bob_session, &wire3).unwrap(), "tudo certo?");

        // Persistência: pickle → bytes → pickle mantém a sessão funcional.
        let bytes = session_to_bytes(&alice_session);
        let mut restored = session_from_bytes(&bytes).unwrap();
        let wire4 = encrypt(&mut bob_session, "após reiniciar");
        assert_eq!(decrypt(&mut restored, &wire4).unwrap(), "após reiniciar");
    }
}
