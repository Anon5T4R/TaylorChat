// Utilidades visuais compartilhadas (sidebar + conversa).
import { t } from "./i18n";
import type { Message } from "./types";

/** Cor estável por contato: hash do node_id → matiz HSL. */
export function avatarColor(nodeId: string): string {
  let h = 0;
  for (let i = 0; i < nodeId.length; i++) h = (h * 31 + nodeId.charCodeAt(i)) >>> 0;
  return `hsl(${h % 360} 55% 42%)`;
}

export function shortId(id: string): string {
  return id.length > 12 ? `${id.slice(0, 6)}…${id.slice(-4)}` : id;
}

/** Chave de conversa → (node_id, thread). `node` (principal) ou `node#thread` (extra). */
export function splitConvo(convo: string): { node: string; thread: string } {
  const i = convo.indexOf("#");
  return i === -1 ? { node: convo, thread: "" } : { node: convo.slice(0, i), thread: convo.slice(i + 1) };
}

export function randomHex(n: number): string {
  const b = new Uint8Array(n);
  crypto.getRandomValues(b);
  return Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");
}

const DAY = 86_400_000;

function startOfDay(ts: number): number {
  const d = new Date(ts);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

/** "Hoje" / "Ontem" / data — pros separadores da conversa. */
export function dayLabel(ts: number): string {
  const today = startOfDay(Date.now());
  const day = startOfDay(ts);
  if (day === today) return t("day.today");
  if (day === today - DAY) return t("day.yesterday");
  return new Date(ts).toLocaleDateString([], { day: "2-digit", month: "2-digit", year: "numeric" });
}

export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/**
 * Glifo de estado da bolha. Só mensagem MINHA tem estado — a que chegou não mostra
 * nada (senão o ✓✓ pareceria confirmação do outro lado).
 */
export function stateGlyph(m: Pick<Message, "direction" | "state">): string {
  if (m.direction === "in") return "";
  switch (m.state) {
    case "queued":
      return "🕒";
    case "failed":
      return "⚠";
    case "sent":
      return "✓";
    case "delivered":
    case "read":
      return "✓✓";
    default:
      return "";
  }
}

/**
 * Quebra o texto em pedaços marcando quais são URL — o passo PURO do link clicável.
 * Fica aqui (e não no JSX) porque é a parte que erra: quem decide o que vira `<a>`.
 * O render nunca usa HTML cru, então não há caminho de XSS por aqui.
 */
export function linkParts(text: string): { url: boolean; text: string }[] {
  return text
    .split(/(https?:\/\/[^\s]+)/g)
    .map((p) => ({ url: /^https?:\/\/\S+$/.test(p), text: p }));
}

/** Hora se for hoje; senão data curta — pra sidebar. */
export function shortTime(ts: number): string {
  if (!ts) return "";
  const today = startOfDay(Date.now());
  if (startOfDay(ts) === today)
    return new Date(ts).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  return new Date(ts).toLocaleDateString([], { day: "2-digit", month: "2-digit" });
}
