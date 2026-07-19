//! Decisão de "devo notificar?" — isolada do Tauri de propósito.
//!
//! O coração do recurso é uma regra de 3 entradas que erra fácil e cala quando erra:
//! notificar demais irrita, notificar de menos devolve o furo que a B13 fechou. Por isso
//! a regra mora numa função PURA, sem AppHandle nem banco, e é testada na matriz inteira
//! (`cargo test`). A camada suja (ler foco da janela, ler o mudo do banco, chamar o
//! plugin) fica no `lib.rs::notify_incoming`, que só coleta as entradas e obedece.

/// Entradas da decisão. Tudo já resolvido pelo chamador — aqui não se consulta nada.
#[derive(Debug, Clone, Copy)]
pub struct NotifyInput<'a> {
    /// Preferência do usuário guardada NO APP (não é a permissão do SO — ver nota em
    /// `lib.rs::notify_incoming` sobre por que perguntar ao SO não serve de nada).
    pub enabled: bool,
    /// A janela principal está em foco AGORA?
    pub focused: bool,
    /// Conversa aberta na UI (chave `node` ou `node#thread`), se houver alguma.
    pub active_convo: Option<&'a str>,
    /// Conversa da mensagem que acabou de chegar (mesma forma de chave).
    pub incoming_convo: &'a str,
    /// Contato silenciado individualmente (a coluna `muted` do contato).
    pub muted: bool,
}

/// Por que não notificamos — vira linha no log de rede, que é como se depura
/// "não chegou notificação" sem adivinhar qual das três regras mordeu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// Notificações desligadas nas Configurações.
    Disabled,
    /// Contato silenciado (o não-lido continua contando — só o aviso some).
    Muted,
    /// O usuário já está olhando exatamente essa conversa.
    AlreadyWatching,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotifyDecision {
    Show,
    Skip(SkipReason),
}

/// A regra. Ordem dos testes é intencional: o desligamento global vence o mudo por
/// contato, que vence o "já estou vendo" — assim o motivo logado é sempre a razão mais
/// forte, e não a primeira que por acaso bateu.
///
/// A regra do meio é a que separa útil de irritante: janela em foco NÃO basta pra
/// calar. Só cala se a janela está em foco **e** a conversa aberta é justamente a que
/// recebeu. Com o app na frente numa outra conversa a mensagem ainda avisa — era o que
/// faltava na versão anterior, que calava com qualquer foco e engolia o aviso.
pub fn should_notify(input: NotifyInput<'_>) -> NotifyDecision {
    if !input.enabled {
        return NotifyDecision::Skip(SkipReason::Disabled);
    }
    if input.muted {
        return NotifyDecision::Skip(SkipReason::Muted);
    }
    if input.focused && input.active_convo == Some(input.incoming_convo) {
        return NotifyDecision::Skip(SkipReason::AlreadyWatching);
    }
    NotifyDecision::Show
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Base "notificaria": app ligado, janela fora de foco, conversa nenhuma aberta.
    fn base<'a>() -> NotifyInput<'a> {
        NotifyInput {
            enabled: true,
            focused: false,
            active_convo: None,
            incoming_convo: "abc",
            muted: false,
        }
    }

    #[test]
    fn janela_fora_de_foco_notifica() {
        assert_eq!(should_notify(base()), NotifyDecision::Show);
    }

    #[test]
    fn foco_na_mesma_conversa_cala() {
        let i = NotifyInput { focused: true, active_convo: Some("abc"), ..base() };
        assert_eq!(should_notify(i), NotifyDecision::Skip(SkipReason::AlreadyWatching));
    }

    /// O caso que a versão anterior errava: app na frente, mas o usuário está em OUTRA
    /// conversa. A mensagem tem que avisar — senão ela chega em silêncio absoluto.
    #[test]
    fn foco_em_outra_conversa_notifica() {
        let i = NotifyInput { focused: true, active_convo: Some("xyz"), ..base() };
        assert_eq!(should_notify(i), NotifyDecision::Show);
    }

    /// Conversa aberta mas janela minimizada/atrás: o usuário NÃO está vendo.
    #[test]
    fn conversa_aberta_sem_foco_notifica() {
        let i = NotifyInput { focused: false, active_convo: Some("abc"), ..base() };
        assert_eq!(should_notify(i), NotifyDecision::Show);
    }

    #[test]
    fn desligado_nunca_notifica() {
        let i = NotifyInput { enabled: false, ..base() };
        assert_eq!(should_notify(i), NotifyDecision::Skip(SkipReason::Disabled));
    }

    #[test]
    fn contato_silenciado_nao_notifica() {
        let i = NotifyInput { muted: true, ..base() };
        assert_eq!(should_notify(i), NotifyDecision::Skip(SkipReason::Muted));
    }

    /// Precedência: desligado global reporta Disabled mesmo com o contato mudo, pra o
    /// log apontar a causa que o usuário precisa desfazer primeiro.
    #[test]
    fn desligado_vence_mudo() {
        let i = NotifyInput { enabled: false, muted: true, ..base() };
        assert_eq!(should_notify(i), NotifyDecision::Skip(SkipReason::Disabled));
    }

    /// Mudo vence "já estou vendo": os dois calam, mas o motivo útil é o mudo.
    #[test]
    fn mudo_vence_ja_estou_vendo() {
        let i = NotifyInput {
            muted: true,
            focused: true,
            active_convo: Some("abc"),
            ..base()
        };
        assert_eq!(should_notify(i), NotifyDecision::Skip(SkipReason::Muted));
    }

    /// Threads do mesmo contato são conversas distintas (`node` × `node#trabalho`):
    /// estar na principal não pode calar o aviso da extra.
    #[test]
    fn thread_diferente_do_mesmo_contato_notifica() {
        let i = NotifyInput {
            focused: true,
            active_convo: Some("abc"),
            incoming_convo: "abc#trabalho",
            ..base()
        };
        assert_eq!(should_notify(i), NotifyDecision::Show);
    }

    /// E o simétrico: na thread extra, chega na principal → avisa.
    #[test]
    fn principal_chegando_com_thread_aberta_notifica() {
        let i = NotifyInput {
            focused: true,
            active_convo: Some("abc#trabalho"),
            incoming_convo: "abc",
            ..base()
        };
        assert_eq!(should_notify(i), NotifyDecision::Show);
    }
}
