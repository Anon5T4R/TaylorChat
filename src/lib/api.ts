import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { openPath } from "@tauri-apps/plugin-opener";
import type { Contact, ConvoSummary, Message, MyIdentity, ParsedInvite, Thread } from "./types";

// ── Identidade / pareamento ──────────────────────────────────────────────
export const myIdentity = () => invoke<MyIdentity>("my_identity");
export const parseInvite = (text: string) => invoke<ParsedInvite>("parse_invite", { text });

// ── Contatos ─────────────────────────────────────────────────────────────
export const contactsList = () => invoke<Contact[]>("contacts_list");
export const contactAdd = (nodeId: string, nickname: string) =>
  invoke<void>("contact_add", { nodeId, nickname });
export const contactRemove = (nodeId: string) => invoke<void>("contact_remove", { nodeId });

// ── Mensagens ────────────────────────────────────────────────────────────
export const messagesList = (peer: string) => invoke<Message[]>("messages_list", { peer });
export const clearConversation = (peer: string) => invoke<void>("clear_conversation", { peer });
export const sendMessage = (peer: string, body: string) =>
  invoke<Message>("send_message", { peer, body });

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

// ── Conversas (multichat) ────────────────────────────────────────────────
export const threadsList = () => invoke<Thread[]>("threads_list");
export const threadCreate = (convo: string, name: string) =>
  invoke<void>("thread_create", { convo, name });
export const threadDelete = (convo: string) => invoke<void>("thread_delete", { convo });

/// Reenvia o que ficou na fila pra um par; devolve quantas saíram.
export const resendQueued = (peer: string) => invoke<number>("resend_queued", { peer });

/// Abre um anexo salvo no app com o programa padrão do SO.
export const openAttachment = (localPath: string) => openPath(localPath);

/// Assina o evento de mensagem recebida (rede, Fase 3). Devolve o unlisten.
export const onMessageIn = (cb: (m: Message) => void): Promise<UnlistenFn> =>
  listen<Message>("message-in", (e) => cb(e.payload));

/// Avisa o par que li a conversa (recibo de leitura). Melhor esforço.
export const markRead = (peer: string) => invoke<void>("mark_read", { peer });

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
