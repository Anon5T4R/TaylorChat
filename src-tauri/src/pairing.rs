//! Pareamento fora de banda (plano.md §5.2): eu mostro meu convite (texto + QR), o
//! outro cola/escaneia e vira contato. Convite = meu `node_id`. O iroh (Fase 3)
//! resolve `node_id` → endereço via discovery/mDNS, então não precisa de IP aqui;
//! quando a Fase 3 pedir dica de relay, ela entra como parâmetro extra no convite.

use crate::identity::{hex_decode_32, Identity};
use serde::Serialize;
use tauri::State;

const SCHEME: &str = "taylorchat:";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MyIdentity {
    node_id: String,
    invite: String,
    qr_svg: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedInvite {
    node_id: String,
}

/// Minha identidade pública pra tela de pareamento: id, string de convite e o QR (SVG).
#[tauri::command(async)]
pub fn my_identity(id: State<'_, Identity>) -> Result<MyIdentity, String> {
    let node_id = id.node_id_hex();
    let invite = format!("{SCHEME}{node_id}");
    let qr_svg = qr_svg(&invite)?;
    Ok(MyIdentity { node_id, invite, qr_svg })
}

/// Interpreta um convite colado/escaneado. Aceita a URL completa (`taylorchat:<hex>`)
/// ou só o hex de 64 chars.
#[tauri::command(async)]
pub fn parse_invite(text: String) -> Result<ParsedInvite, String> {
    let t = text.trim();
    let hex = t.strip_prefix(SCHEME).unwrap_or(t).trim();
    // strip_prefix pode deixar `//` se alguém colou taylorchat://<hex>
    let hex = hex.trim_start_matches('/').trim().to_lowercase();
    hex_decode_32(&hex)?; // valida
    Ok(ParsedInvite { node_id: hex })
}

fn qr_svg(data: &str) -> Result<String, String> {
    use qrcode::render::svg;
    use qrcode::QrCode;
    let code = QrCode::new(data.as_bytes()).map_err(|e| format!("falha ao gerar QR: {e}"))?;
    let svg = code
        .render::<svg::Color>()
        .min_dimensions(220, 220)
        .quiet_zone(true)
        .dark_color(svg::Color("#111827"))
        .light_color(svg::Color("#ffffff"))
        .build();
    Ok(svg)
}
