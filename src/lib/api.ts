import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { openPath } from "@tauri-apps/plugin-opener";
import type { Contact, Message, MyIdentity, ParsedInvite } from "./types";

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
export const sendMessage = (peer: string, body: string) =>
  invoke<Message>("send_message", { peer, body });

/// Escolhe um arquivo (diálogo nativo) e o envia como anexo. Devolve a mensagem
/// criada, ou null se o usuário cancelou o diálogo.
export const pickAndAttach = async (peer: string): Promise<Message | null> => {
  const path = await openDialog({ multiple: false, directory: false });
  if (!path || typeof path !== "string") return null;
  return invoke<Message>("attach_file", { peer, path });
};

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

// ── IA local (Fase 6) ────────────────────────────────────────────────────
export interface LlmStatus {
  running: boolean;
  port: number;
  model: string;
}
export const llmStatus = () => invoke<LlmStatus>("llm_status");
