//! Painel "Dados e armazenamento": mede o que o TaylorChat ocupa em disco e
//! oferece limpezas CIRÚRGICAS.
//!
//! Regra central deste módulo: o HISTÓRICO é o artefato caro. Uma conversa
//! ponto-a-ponto não tem servidor pra rebaixar nada — se a mensagem sumir daqui,
//! sumiu do mundo. Por isso NENHUMA limpeza desta tela apaga mensagem, contato
//! ou conversa, e não existe botão de "apagar tudo". O que este módulo libera é
//! só o que ficou pra trás:
//!
//! - **anexo órfão** — o arquivo em `attachments/` cuja mensagem já não existe;
//! - **transferência interrompida** — o parcial em `attachments/.partial/`;
//! - **avatar de contato removido** — cache em `avatars/`;
//! - **backup antigo** — as cópias diárias do banco, menos a mais recente.
//!
//! A pasta `stickers/` é medida e NUNCA tocada: é conteúdo que o usuário
//! escolheu, não sobra.
//!
//! ## Por que o casamento é por NOME e não por caminho
//!
//! O `localPath` de um anexo foi gravado com o `app_data` da instalação da
//! época. Depois de reinstalar (ou de o perfil mudar de lugar), TODO caminho no
//! banco fica obsoleto — e uma varredura que casasse caminho inteiro
//! classificaria todos os anexos vivos como órfãos e os apagaria **relatando
//! sucesso**. Comparamos o nome do arquivo, que a cópia local fixa no momento
//! em que ela nasce (`<timestamp>_<nome>`) e que não muda de instalação pra
//! instalação. O teste `entre_instalacoes_nao_apaga_anexo_vivo` guarda isso.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use tauri::Manager;

use crate::db::{self, Db};

/// Resultado de qualquer limpeza — em arquivos E bytes, porque o painel precisa
/// dizer quanto liberou, não só quantas coisas sumiram.
#[derive(serde::Serialize, Clone, Default, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Freed {
    pub files: u64,
    pub bytes: u64,
}

/// (bytes, arquivos) do primeiro nível de uma pasta. Não recursa: as pastas do
/// app são planas, e `attachments/` tem a subpasta `.partial`, que é medida
/// separada de propósito (o botão dela é outro).
pub fn dir_stats(dir: &Path) -> (u64, u64) {
    let mut bytes = 0u64;
    let mut files = 0u64;
    if let Ok(rd) = std::fs::read_dir(dir) {
        for entry in rd.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    bytes += meta.len();
                    files += 1;
                }
            }
        }
    }
    (bytes, files)
}

fn file_len(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

fn sum_len(paths: &[PathBuf]) -> u64 {
    paths.iter().map(|p| file_len(p)).sum()
}

/// Apaga a lista e soma o que saiu (só conta o que o `remove_file` confirmou).
fn remove_all(paths: &[PathBuf]) -> Freed {
    let mut freed = Freed::default();
    for path in paths {
        let len = file_len(path);
        if std::fs::remove_file(path).is_ok() {
            freed.files += 1;
            freed.bytes += len;
        }
    }
    freed
}

/// Arquivos (só do 1º nível) de uma pasta.
fn files_in(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.is_file() {
                out.push(path);
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// anexos
// ---------------------------------------------------------------------------

/// Nomes de arquivo que alguma mensagem de anexo ainda referencia.
///
/// `bodies` vem do `db::file_message_bodies`: `None` = linha que não decifrou.
/// Uma linha ilegível ABORTA a varredura inteira, porque ela pode muito bem
/// estar apontando pra um arquivo vivo — e sem o inventário completo não dá pra
/// provar que qualquer coisa é órfã. Falhar alto é o único desfecho honesto: o
/// contrário apagaria anexo bom relatando sucesso.
pub fn referenced_names(bodies: &[Option<String>]) -> Result<HashSet<String>, String> {
    let mut set = HashSet::new();
    for (i, body) in bodies.iter().enumerate() {
        let Some(json) = body else {
            return Err(format!(
                "não consegui ler {} de {} mensagens de anexo; limpeza cancelada por segurança",
                bodies.iter().filter(|b| b.is_none()).count(),
                bodies.len()
            ));
        };
        // Corpo VAZIO é um estado legítimo, não corrupção: o soft-delete grava
        // string vazia. A varredura já filtra `deleted=0`, mas tratar o vazio
        // como erro aqui deixaria o painel inteiro morto por uma linha antiga —
        // e "cancelei tudo" é caro demais pra cobrar de um caso normal.
        if json.trim().is_empty() {
            continue;
        }
        let value: serde_json::Value = serde_json::from_str(json)
            .map_err(|e| format!("anexo #{i} com metadados ilegíveis ({e}); limpeza cancelada"))?;
        // Sem `localPath` não há arquivo local a proteger (nada a fazer aqui);
        // é um estado legítimo, não um erro.
        if let Some(p) = value["localPath"].as_str().filter(|p| !p.is_empty()) {
            if let Some(name) = Path::new(p).file_name() {
                set.insert(name.to_string_lossy().to_lowercase());
            }
        }
    }
    Ok(set)
}

/// Arquivos de `attachments/` que nenhuma mensagem referencia — sobra de
/// conversa apagada, contato removido ou mensagem apagada uma a uma.
pub fn orphan_attachments(dir: &Path, used: &HashSet<String>) -> Vec<PathBuf> {
    files_in(dir)
        .into_iter()
        .filter(|p| {
            let name = p.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
            !used.contains(&name)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// avatares
// ---------------------------------------------------------------------------

/// Nome de arquivo do avatar cacheado de um node_id — a MESMA regra do
/// `media::save_contact_avatar`. Derivar o nome do node_id (em vez de ler a
/// coluna `avatar`, que guarda caminho absoluto) deixa esta varredura imune ao
/// problema do caminho obsoleto por construção.
fn avatar_name_of(node_id: &str) -> String {
    let safe: String = node_id.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    format!("{safe}.png").to_lowercase()
}

/// Avatares em cache de gente que não está mais nos contatos.
pub fn orphan_avatars(dir: &Path, node_ids: &[String]) -> Vec<PathBuf> {
    let keep: HashSet<String> = node_ids
        .iter()
        .filter(|n| !n.is_empty())
        .map(|n| avatar_name_of(n))
        .collect();
    files_in(dir)
        .into_iter()
        .filter(|p| {
            let name = p.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
            !keep.contains(&name)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// backups
// ---------------------------------------------------------------------------

/// Backups diários (`chat-aaaammdd.db`) exceto o MAIS RECENTE. O nome carrega a
/// data em formato ordenável, então ordenar por nome basta — e devolver a lista
/// sem o último é o que faz o botão nunca deixar o usuário sem rede de proteção.
pub fn old_backups(dir: &Path) -> Vec<PathBuf> {
    let mut list: Vec<PathBuf> = files_in(dir)
        .into_iter()
        .filter(|p| {
            let n = p.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
            n.starts_with("chat-") && n.ends_with(".db")
        })
        .collect();
    list.sort();
    list.pop(); // o mais recente fica
    list
}

// ---------------------------------------------------------------------------
// comandos
// ---------------------------------------------------------------------------

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StorageInfo {
    /// Pasta de dados do app (onde moram chat.db, attachments/, stickers/…).
    dir: String,
    /// Banco em bytes (chat.db + WAL + SHM) — aqui mora o histórico.
    db_bytes: u64,
    messages: i64,
    file_messages: i64,
    contacts: i64,
    conversations: i64,
    attachments_bytes: u64,
    attachments_files: u64,
    orphan_attachments_bytes: u64,
    orphan_attachments_files: u64,
    partial_bytes: u64,
    partial_files: u64,
    avatars_bytes: u64,
    avatars_files: u64,
    orphan_avatars_bytes: u64,
    orphan_avatars_files: u64,
    backups_bytes: u64,
    backups_files: u64,
    old_backups_bytes: u64,
    old_backups_files: u64,
    stickers_bytes: u64,
    stickers_files: u64,
}

fn data_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|e| format!("sem pasta de dados: {e}"))
}

/// Soma recursiva de uma árvore de 2 níveis (os stickers vivem em
/// `stickers/<pacote>/`, então o 1º nível é só pasta).
fn tree_stats(dir: &Path) -> (u64, u64) {
    let (mut bytes, mut files) = dir_stats(dir);
    if let Ok(rd) = std::fs::read_dir(dir) {
        for entry in rd.flatten() {
            if entry.path().is_dir() {
                let (b, f) = dir_stats(&entry.path());
                bytes += b;
                files += f;
            }
        }
    }
    (bytes, files)
}

/// Nomes referenciados, já com o aborto por linha ilegível aplicado.
fn used_names(db: &Db) -> Result<HashSet<String>, String> {
    referenced_names(&db::file_message_bodies(db)?)
}

#[tauri::command(async)]
pub fn storage_info(app: tauri::AppHandle, db: tauri::State<'_, Db>) -> Result<StorageInfo, String> {
    let dir = data_dir(&app)?;
    let attachments = dir.join("attachments");
    let partial = attachments.join(".partial");
    let avatars = dir.join("avatars");
    let backups = dir.join("backups");

    let db_bytes = ["chat.db", "chat.db-wal", "chat.db-shm"]
        .iter()
        .filter_map(|name| std::fs::metadata(dir.join(name)).ok())
        .map(|m| m.len())
        .sum();

    let (messages, file_messages, contacts, conversations) = db::storage_counts(&db)?;
    let (attachments_bytes, attachments_files) = dir_stats(&attachments);
    let (partial_bytes, partial_files) = dir_stats(&partial);
    let (avatars_bytes, avatars_files) = dir_stats(&avatars);
    let (backups_bytes, backups_files) = dir_stats(&backups);
    let (stickers_bytes, stickers_files) = tree_stats(&dir.join("stickers"));

    let orphans = orphan_attachments(&attachments, &used_names(&db)?);
    let stale_avatars = orphan_avatars(&avatars, &db::contact_node_ids(&db)?);
    let old = old_backups(&backups);

    Ok(StorageInfo {
        dir: dir.to_string_lossy().into_owned(),
        db_bytes,
        messages,
        file_messages,
        contacts,
        conversations,
        attachments_bytes,
        attachments_files,
        orphan_attachments_bytes: sum_len(&orphans),
        orphan_attachments_files: orphans.len() as u64,
        partial_bytes,
        partial_files,
        avatars_bytes,
        avatars_files,
        orphan_avatars_bytes: sum_len(&stale_avatars),
        orphan_avatars_files: stale_avatars.len() as u64,
        backups_bytes,
        backups_files,
        old_backups_bytes: sum_len(&old),
        old_backups_files: old.len() as u64,
        stickers_bytes,
        stickers_files,
    })
}

/// Só os anexos que nenhuma mensagem referencia. Nenhuma conversa perde arquivo.
#[tauri::command(async)]
pub fn storage_clear_orphan_attachments(
    app: tauri::AppHandle,
    db: tauri::State<'_, Db>,
) -> Result<Freed, String> {
    let dir = data_dir(&app)?.join("attachments");
    Ok(remove_all(&orphan_attachments(&dir, &used_names(&db)?)))
}

/// Parciais de transferências que não terminaram.
#[tauri::command(async)]
pub fn storage_clear_partials(app: tauri::AppHandle) -> Result<Freed, String> {
    let dir = data_dir(&app)?.join("attachments").join(".partial");
    Ok(remove_all(&files_in(&dir)))
}

/// Avatares em cache de contatos que já não existem.
#[tauri::command(async)]
pub fn storage_clear_orphan_avatars(
    app: tauri::AppHandle,
    db: tauri::State<'_, Db>,
) -> Result<Freed, String> {
    let dir = data_dir(&app)?.join("avatars");
    Ok(remove_all(&orphan_avatars(&dir, &db::contact_node_ids(&db)?)))
}

/// Backups diários antigos; o mais recente fica sempre.
#[tauri::command(async)]
pub fn storage_clear_old_backups(app: tauri::AppHandle) -> Result<Freed, String> {
    let dir = data_dir(&app)?.join("backups");
    Ok(remove_all(&old_backups(&dir)))
}

// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static SEQ: AtomicU32 = AtomicU32::new(0);

    fn tmp(tag: &str) -> PathBuf {
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("taylorchat-storage-{tag}-{n}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(dir: &Path, name: &str, bytes: usize) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, vec![b'x'; bytes]).unwrap();
        path
    }

    /// Corpo de mensagem de anexo, como o `attach_file` grava.
    fn meta(local_path: &str) -> Option<String> {
        Some(
            serde_json::json!({
                "filename": "foto.png", "mime": "image/png", "size": 10,
                "localPath": local_path, "transferId": "abc", "fileKey": "k", "sticker": false,
            })
            .to_string(),
        )
    }

    #[test]
    fn orfaos_sao_so_os_que_ninguem_referencia() {
        let dir = tmp("orfaos");
        write(&dir, "1_a.png", 100);
        write(&dir, "2_b.pdf", 40);
        write(&dir, "3_c.zip", 60);

        let used = referenced_names(&[meta("C:/dados/attachments/1_a.png")]).unwrap();
        let alvo = orphan_attachments(&dir, &used);
        assert_eq!(alvo.len(), 2);

        let freed = remove_all(&alvo);
        assert_eq!(freed, Freed { files: 2, bytes: 100 });
        assert!(dir.join("1_a.png").exists(), "anexo referenciado foi apagado");
        assert!(!dir.join("2_b.pdf").exists());
        assert!(!dir.join("3_c.zip").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// O caso que motivou o casamento por nome: o banco guarda caminhos de uma
    /// instalação ANTIGA e os arquivos estão na pasta NOVA. Casar caminho
    /// inteiro faria os três parecerem órfãos e apagaria os três.
    #[test]
    fn entre_instalacoes_nao_apaga_anexo_vivo() {
        let dir = tmp("reinstalacao");
        write(&dir, "1_a.png", 100);
        write(&dir, "2_b.pdf", 40);
        write(&dir, "3_c.zip", 60);

        let bodies = [
            meta("C:/Users/antigo/AppData/Roaming/com.taylorchat/attachments/1_a.png"),
            meta("/home/outro/.local/share/taylorchat/attachments/2_b.pdf"),
            meta("D:/perfil-de-rede/taylorchat/attachments/3_c.zip"),
        ];
        let used = referenced_names(&bodies).unwrap();
        assert!(orphan_attachments(&dir, &used).is_empty(), "apagaria anexo vivo após reinstalar");

        // E o que FICOU é o que importa: os três seguem no disco, intactos.
        assert_eq!(dir_stats(&dir), (200, 3));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Linha que não decifra ABORTA a varredura. O modo de falha oposto (tratar
    /// como "não referencia nada") apagaria o anexo dela.
    #[test]
    fn linha_ilegivel_aborta_em_vez_de_apagar() {
        let dir = tmp("ilegivel");
        write(&dir, "1_a.png", 100);

        let erro = referenced_names(&[meta("qualquer/1_a.png"), None]).unwrap_err();
        assert!(erro.contains("cancelada"), "erro deveria explicar o cancelamento: {erro}");

        // Metadado que decifrou mas não é JSON também aborta.
        assert!(referenced_names(&[Some("nao é json".into())]).is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Mensagem de anexo sem `localPath` é legítima (nada local a proteger) e
    /// não pode derrubar a varredura inteira.
    #[test]
    fn anexo_sem_caminho_local_nao_e_erro() {
        let corpo = Some(r#"{"filename":"x.png","mime":"image/png","size":1}"#.to_string());
        assert!(referenced_names(&[corpo]).unwrap().is_empty());
        assert!(referenced_names(&[meta("")]).unwrap().is_empty());
    }

    /// "Apagar para todos" faz soft-delete: mantém a linha e o `kind='file'`,
    /// mas grava corpo VAZIO. Se o vazio contasse como metadado ilegível, o
    /// painel inteiro morreria pra quem já usou o recurso uma vez — e o
    /// caminho normal do app viraria um erro permanente.
    #[test]
    fn mensagem_apagada_nao_derruba_a_varredura() {
        let usados = referenced_names(&[meta("x/1_a.png"), Some(String::new())]).unwrap();
        assert_eq!(usados.len(), 1, "o anexo vivo continua protegido");
        assert!(usados.contains("1_a.png"));
        // Só espaço em branco também é vazio.
        assert!(referenced_names(&[Some("   ".into())]).unwrap().is_empty());
    }

    #[test]
    fn avatar_de_contato_vivo_sobrevive() {
        let dir = tmp("avatares");
        // O nome é derivado do node_id pela mesma regra do save_contact_avatar.
        write(&dir, "abc123.png", 500);
        write(&dir, "removido999.png", 300);

        let alvo = orphan_avatars(&dir, &["abc123".to_string()]);
        assert_eq!(alvo.len(), 1);
        let freed = remove_all(&alvo);
        assert_eq!(freed, Freed { files: 1, bytes: 300 });
        assert!(dir.join("abc123.png").exists(), "avatar de contato ativo foi apagado");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// node_id com hífen/pontuação: o nome do arquivo é a versão saneada. Se a
    /// varredura esquecesse de sanear, o avatar do contato viraria "órfão".
    #[test]
    fn node_id_saneado_casa_com_o_arquivo() {
        let dir = tmp("avatar-saneado");
        write(&dir, "abc123.png", 10);
        let alvo = orphan_avatars(&dir, &["abc-123".to_string()]);
        assert!(alvo.is_empty(), "node_id com hífen deixou de casar com o arquivo saneado");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Lista de contatos vazia (nenhum contato) não pode ser lida como "apague
    /// tudo com cuidado" — aqui apagar tudo é o certo, mas string vazia não pode
    /// virar curinga que PROTEGE tudo.
    #[test]
    fn node_id_vazio_nao_protege_nada() {
        let dir = tmp("avatar-vazio");
        write(&dir, ".png", 10);
        let alvo = orphan_avatars(&dir, &["".to_string()]);
        assert_eq!(alvo.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn backup_mais_recente_sempre_fica() {
        let dir = tmp("backups");
        write(&dir, "chat-20260718.db", 100);
        write(&dir, "chat-20260719.db", 200);
        write(&dir, "chat-20260720.db", 300);
        write(&dir, "outro.db", 50); // não é nosso padrão: nem olhamos

        let alvo = old_backups(&dir);
        assert_eq!(alvo.len(), 2);
        let freed = remove_all(&alvo);
        assert_eq!(freed, Freed { files: 2, bytes: 300 });
        assert!(dir.join("chat-20260720.db").exists(), "o backup mais recente sumiu");
        assert!(dir.join("outro.db").exists(), "mexemos em arquivo que não é nosso");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn backup_unico_nao_e_apagado() {
        let dir = tmp("backup-unico");
        write(&dir, "chat-20260720.db", 100);
        assert!(old_backups(&dir).is_empty(), "o único backup foi pra lista de apagar");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `attachments/.partial` é subpasta de `attachments`: o `dir_stats` não
    /// recursa, então o parcial não pode contar duas vezes nem virar "órfão".
    #[test]
    fn parcial_nao_conta_como_anexo() {
        let dir = tmp("parciais");
        write(&dir, "1_a.png", 100);
        let partial = dir.join(".partial");
        std::fs::create_dir_all(&partial).unwrap();
        write(&partial, "transf1", 900);

        assert_eq!(dir_stats(&dir), (100, 1));
        assert_eq!(dir_stats(&partial), (900, 1));

        let used = referenced_names(&[meta("x/1_a.png")]).unwrap();
        assert!(orphan_attachments(&dir, &used).is_empty());

        let freed = remove_all(&files_in(&partial));
        assert_eq!(freed, Freed { files: 1, bytes: 900 });
        assert!(dir.join("1_a.png").exists(), "limpar parciais encostou nos anexos");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stickers_somam_a_arvore_de_dois_niveis() {
        let dir = tmp("stickers");
        let pack = dir.join("meus");
        std::fs::create_dir_all(&pack).unwrap();
        write(&pack, "a.png", 10);
        write(&pack, "b.png", 25);
        assert_eq!(tree_stats(&dir), (35, 2));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pastas_inexistentes_nao_sao_erro() {
        let nada = std::env::temp_dir().join("taylorchat-nao-existe-mesmo");
        assert_eq!(dir_stats(&nada), (0, 0));
        assert_eq!(tree_stats(&nada), (0, 0));
        assert!(old_backups(&nada).is_empty());
        assert!(orphan_avatars(&nada, &[]).is_empty());
        assert!(orphan_attachments(&nada, &HashSet::new()).is_empty());
        assert_eq!(remove_all(&files_in(&nada)), Freed::default());
    }

    /// As limpezas são idempotentes: rodar duas vezes não erra nem inventa
    /// número.
    #[test]
    fn limpezas_sao_idempotentes() {
        let dir = tmp("idempotente");
        write(&dir, "1_a.png", 100);
        let used = HashSet::new();
        assert_eq!(remove_all(&orphan_attachments(&dir, &used)), Freed { files: 1, bytes: 100 });
        assert_eq!(remove_all(&orphan_attachments(&dir, &used)), Freed::default());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
