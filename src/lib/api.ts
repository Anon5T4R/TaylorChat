import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import type {
  Contact,
  ConvoSummary,
  Message,
  MyIdentity,
  ParsedInvite,
  Profile,
  Thread,
} from "./types";

// ── Identidade / pareamento ──────────────────────────────────────────────
export const myIdentity = () => invoke<MyIdentity>("my_identity");
export const parseInvite = (text: string) => invoke<ParsedInvite>("parse_invite", { text });

// ── Contatos ─────────────────────────────────────────────────────────────
export const contactsList = () => invoke<Contact[]>("contacts_list");
export const contactAdd = (nodeId: string, nickname: string) =>
  invoke<void>("contact_add", { nodeId, nickname });
export const contactRemove = (nodeId: string) => invoke<void>("contact_remove", { nodeId });
/// Silencia/dessilencia um contato (só afeta a notificação de desktop).
export const setMuted = (nodeId: string, muted: boolean) =>
  invoke<void>("set_muted", { nodeId, muted });
/// Atualiza a ficha local do contato (apelido + telefone/email/aniversário/notas).
export const setContactInfo = (
  nodeId: string,
  nickname: string,
  phone: string,
  email: string,
  birthday: string,
  notes: string,
) => invoke<void>("set_contact_info", { nodeId, nickname, phone, email, birthday, notes });

// ── Mensagens ────────────────────────────────────────────────────────────
/// Página de mensagens: as `limit` mais recentes antes de `beforeId` (ou as últimas).
export const messagesList = (peer: string, beforeId?: number | null, limit?: number) =>
  invoke<Message[]>("messages_list", { peer, beforeId: beforeId ?? null, limit: limit ?? null });
export const clearConversation = (peer: string) => invoke<void>("clear_conversation", { peer });
export const sendMessage = (
  peer: string,
  body: string,
  replyTo?: number | null,
  replyPreview?: string | null,
) =>
  invoke<Message>("send_message", {
    peer,
    body,
    replyTo: replyTo ?? null,
    replyPreview: replyPreview ?? null,
  });
/// Apaga uma mensagem só pra mim (remove a linha local).
export const messageDelete = (id: number) => invoke<void>("message_delete", { id });
/// Apaga uma mensagem para todos (soft-delete + avisa o par por ts).
export const deleteForEveryone = (peer: string, targetTs: number) =>
  invoke<void>("delete_for_everyone", { peer, targetTs });
/// O par apagou uma mensagem para todos (identificada por ts).
export const onMsgDeleted = (cb: (peer: string, ts: number) => void): Promise<UnlistenFn> =>
  listen<{ peer: string; ts: number }>("msg-deleted", (e) => cb(e.payload.peer, e.payload.ts));

// ── Reações ────────────────────────────────────────────────────────────────
export interface Reaction {
  targetTs: number;
  mine: boolean;
  emoji: string;
}
export const reactionsList = (convo: string) => invoke<Reaction[]>("reactions_list", { convo });
/// Reage a uma mensagem (emoji vazio remove a minha).
export const react = (peer: string, targetTs: number, emoji: string) =>
  invoke<void>("react", { peer, targetTs, emoji });
export const onReaction = (
  cb: (peer: string, targetTs: number, emoji: string) => void,
): Promise<UnlistenFn> =>
  listen<{ peer: string; targetTs: number; emoji: string }>("reaction", (e) =>
    cb(e.payload.peer, e.payload.targetTs, e.payload.emoji),
  );

/// Escolhe um arquivo (diálogo nativo) e o envia como anexo. Devolve a mensagem
/// criada, ou null se o usuário cancelou o diálogo.
export const pickAndAttach = async (peer: string): Promise<Message | null> => {
  const path = await openDialog({ multiple: false, directory: false });
  if (!path || typeof path !== "string") return null;
  return invoke<Message>("attach_file", { peer, path, sticker: false });
};

/// Envia um arquivo por caminho (usado pelo drag & drop).
export const attachPath = (peer: string, path: string) =>
  invoke<Message>("attach_file", { peer, path, sticker: false });

/// Envia um sticker (imagem já salva) como sticker.
export const sendSticker = (peer: string, path: string) =>
  invoke<Message>("attach_file", { peer, path, sticker: true });

// ── Stickers ──────────────────────────────────────────────────────────────
export interface Sticker {
  pack: string;
  path: string;
}
export const stickersList = () => invoke<Sticker[]>("stickers_list");
export const stickerAdd = (pack: string, src: string) =>
  invoke<string>("sticker_add", { pack, src });
/// Diálogo pra escolher uma imagem e criar um sticker num pacote.
export const pickAndCreateSticker = async (pack: string): Promise<string | null> => {
  const path = await openDialog({
    multiple: false,
    directory: false,
    filters: [{ name: "Imagem", extensions: ["png", "jpg", "jpeg", "gif", "webp"] }],
  });
  if (!path || typeof path !== "string") return null;
  return stickerAdd(pack, path);
};

/// Última mensagem de cada conversa (prévia + ordenação da sidebar).
export const conversationsSummary = () => invoke<ConvoSummary[]>("conversations_summary");

// ── Não-lidos persistidos + badge de desktop ─────────────────────────────
export interface Unread {
  convo: string;
  n: number;
}
export const unreadList = () => invoke<Unread[]>("unread_list");
export const unreadSet = (convo: string, n: number) => invoke<void>("unread_set", { convo, n });
/// Reflete o total de não-lidos na bandeja + título da janela (badge de desktop).
export const setBadge = (count: number) => invoke<void>("set_badge", { count });

// ── Busca global ──────────────────────────────────────────────────────────
export interface SearchHit {
  convo: string;
  snippet: string;
  ts: number;
}
export const searchMessages = (query: string, limit = 30) =>
  invoke<SearchHit[]>("search_messages", { query, limit });

// ── Conversas (multichat) ────────────────────────────────────────────────
export const threadsList = () => invoke<Thread[]>("threads_list");
export const threadCreate = (convo: string, name: string) =>
  invoke<void>("thread_create", { convo, name });
export const threadDelete = (convo: string) => invoke<void>("thread_delete", { convo });

/// Reenvia o que ficou na fila pra um par; devolve quantas saíram.
export const resendQueued = (peer: string) => invoke<number>("resend_queued", { peer });
/// Reenvia a fila de TODAS as conversas (chamado periodicamente).
export const resendAll = () => invoke<number>("resend_all");

// ── Diagnóstico de rede ──────────────────────────────────────────────────
export interface NetStatus {
  up: boolean;
  nodeId: string;
}
export const netStatus = () => invoke<NetStatus>("net_status");
export const netLog = () => invoke<string[]>("net_log");
/// Notificação de desktop mostra a prévia do texto? (default: sim)
export const getNotifyPreview = () => invoke<boolean>("get_notify_preview");
export const setNotifyPreview = (on: boolean) => invoke<void>("set_notify_preview", { on });
/// Começa a observar a presença do par (conexão quente + heartbeat) e devolve o status
/// agora. Depois disso, escute `onPresence` pra atualizações em tempo real.
export const peerOnline = (peer: string) => invoke<boolean>("peer_online", { peer });
/// Para de observar a presença do par (conversa fechada) — encerra o watcher (L4).
export const peerUnwatch = (peer: string) => invoke<void>("peer_unwatch", { peer });
/// Mudanças de presença em tempo real (o heartbeat ping/pong detecta online/offline).
export const onPresence = (cb: (p: { peer: string; online: boolean }) => void): Promise<UnlistenFn> =>
  listen<{ peer: string; online: boolean }>("presence", (e) => cb(e.payload));
/// Avisos de rede em tempo real (falha ao enviar/receber, etc.).
export const onNetError = (cb: (line: string) => void): Promise<UnlistenFn> =>
  listen<string>("net-error", (e) => cb(e.payload));

/// Abre um anexo salvo no app com o programa padrão do SO.
export const openAttachment = (localPath: string) => openPath(localPath);
/// Abre um link (URL) no navegador padrão.
export const openLink = (url: string) => openUrl(url);

/// Assina o evento de mensagem recebida (rede, Fase 3). Devolve o unlisten.
export const onMessageIn = (cb: (m: Message) => void): Promise<UnlistenFn> =>
  listen<Message>("message-in", (e) => cb(e.payload));

/// Avisa o par que li a conversa (recibo de leitura). Melhor esforço.
export const markRead = (peer: string) => invoke<void>("mark_read", { peer });

/// Avisa o par que estou (ou parei de) digitar nesta conversa. Best-effort.
export const sendTyping = (peer: string, on: boolean) =>
  invoke<void>("send_typing", { peer, on });
/// "Digitando…" do par em tempo real.
export const onTyping = (cb: (peer: string, on: boolean) => void): Promise<UnlistenFn> =>
  listen<{ peer: string; on: boolean }>("typing", (e) => cb(e.payload.peer, e.payload.on));

/// Assina o evento de recibo de leitura (minhas mensagens pro par viraram "lidas").
export const onReceipts = (cb: (peer: string) => void): Promise<UnlistenFn> =>
  listen<{ peer: string }>("receipts", (e) => cb(e.payload.peer));

// ── Palavra-chave por contato ────────────────────────────────────────────
export interface KeywordStatus {
  hasMine: boolean;
  hasPeer: boolean;
  matches: boolean | null; // null = falta um dos lados
  word: string | null;
}
export const setKeyword = (peer: string, word: string) =>
  invoke<void>("set_keyword", { peer, word });
export const keywordStatus = (peer: string) => invoke<KeywordStatus>("keyword_status", { peer });
export const onKeyword = (cb: (peer: string) => void): Promise<UnlistenFn> =>
  listen<{ peer: string }>("keyword", (e) => cb(e.payload.peer));

// ── Perfil (nome + foto) ─────────────────────────────────────────────────
export const getProfile = () => invoke<Profile>("get_profile");
export const setProfile = (name: string, photo?: string) =>
  invoke<Profile>("set_profile", { name, photo: photo ?? null });
export const sendProfile = (peer: string) => invoke<void>("send_profile", { peer });
export const onProfile = (cb: (node: string) => void): Promise<UnlistenFn> =>
  listen<{ peer: string }>("profile", (e) => cb(e.payload.peer));
/// Diálogo pra escolher a foto de perfil.
export const pickProfilePhoto = async (): Promise<string | null> => {
  const path = await openDialog({
    multiple: false,
    directory: false,
    filters: [{ name: "Imagem", extensions: ["png", "jpg", "jpeg", "gif", "webp"] }],
  });
  return typeof path === "string" ? path : null;
};

// ── Auditoria ────────────────────────────────────────────────────────────
export interface AuditResult {
  count: number;
  digest: string;
}
export const auditConversation = (peer: string) =>
  invoke<AuditResult>("audit_conversation", { peer });

// ── IA local (Fase 6) ────────────────────────────────────────────────────
export interface LlmStatus {
  running: boolean;
  port: number;
  model: string;
}
export const llmStatus = () => invoke<LlmStatus>("llm_status");
