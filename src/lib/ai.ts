import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type { Message } from "./types";

// ── Ciclo de vida do sidecar (llama.cpp, porta 8103) ─────────────────────
export interface ModelInfo {
  name: string;
  path: string;
  sizeGb: number;
  isProjector: boolean;
}
export interface LlmStatus {
  running: boolean;
  port: number;
  model: string;
}

export const pickModelsFolder = async (): Promise<string | null> => {
  const dir = await openDialog({ directory: true, multiple: false });
  return typeof dir === "string" ? dir : null;
};
export const listModels = (dir: string) => invoke<ModelInfo[]>("list_models", { dir });
export const startLlm = (modelPath: string, nGpuLayers: number, ctxSize: number) =>
  invoke<number>("start_llm", { modelPath, nGpuLayers, ctxSize });
export const stopLlm = () => invoke<void>("stop_llm");
export const llmStatus = () => invoke<LlmStatus>("llm_status");

// ── Chamada de chat ao servidor local (OpenAI-compat) ────────────────────
export interface ChatMsg {
  role: "system" | "user" | "assistant";
  content: string;
}

export async function chat(
  port: number,
  messages: ChatMsg[],
  opts: { temperature?: number; signal?: AbortSignal } = {},
): Promise<string> {
  const res = await fetch(`http://127.0.0.1:${port}/v1/chat/completions`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      messages,
      temperature: opts.temperature ?? 0.3,
      stream: false,
      // Think OFF por padrão (mesmo truque do LocalSheets).
      chat_template_kwargs: { enable_thinking: false },
    }),
    signal: opts.signal,
  });
  if (!res.ok) throw new Error(`IA respondeu ${res.status}`);
  const data = await res.json();
  return (data?.choices?.[0]?.message?.content ?? "").trim();
}

// ── Ações sobre a conversa (a IA só PROPÕE texto) ────────────────────────
const RESP_ONLY =
  "Você é um assistente de escrita dentro de um mensageiro. Responda APENAS com o texto pedido, sem aspas, sem rótulos e sem explicações.";

function transcript(messages: Message[], limit = 60): string {
  return messages
    .slice(-limit)
    .filter((m) => m.kind === "text")
    .map((m) => `${m.direction === "out" ? "Eu" : "Contato"}: ${m.body}`)
    .join("\n");
}

export const improveDraft = (port: number, draft: string, instruction: string) =>
  chat(port, [
    { role: "system", content: RESP_ONLY },
    { role: "user", content: `${instruction}\n\nMensagem:\n${draft}` },
  ]);

export const translate = (port: number, text: string, lang: string) =>
  chat(port, [
    { role: "system", content: RESP_ONLY },
    { role: "user", content: `Traduza a mensagem a seguir para ${lang}.\n\nMensagem:\n${text}` },
  ]);

export const suggestReply = (port: number, messages: Message[]) =>
  chat(port, [
    { role: "system", content: RESP_ONLY },
    {
      role: "user",
      content: `Sugira uma resposta curta e natural para a última mensagem desta conversa.\n\n${transcript(
        messages,
      )}`,
    },
  ]);

export const summarizeConversation = (port: number, messages: Message[]) =>
  chat(port, [
    {
      role: "system",
      content:
        "Você resume conversas de mensageiro em português, em tópicos curtos. Responda só com o resumo.",
    },
    { role: "user", content: `Resuma esta conversa:\n\n${transcript(messages, 200)}` },
  ]);
