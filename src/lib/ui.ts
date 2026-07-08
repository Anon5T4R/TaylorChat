// Utilidades visuais compartilhadas (sidebar + conversa).

/** Cor estável por contato: hash do node_id → matiz HSL. */
export function avatarColor(nodeId: string): string {
  let h = 0;
  for (let i = 0; i < nodeId.length; i++) h = (h * 31 + nodeId.charCodeAt(i)) >>> 0;
  return `hsl(${h % 360} 55% 42%)`;
}

export function shortId(id: string): string {
  return id.length > 12 ? `${id.slice(0, 6)}…${id.slice(-4)}` : id;
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
  if (day === today) return "Hoje";
  if (day === today - DAY) return "Ontem";
  return new Date(ts).toLocaleDateString([], { day: "2-digit", month: "2-digit", year: "numeric" });
}

/** Hora se for hoje; senão data curta — pra sidebar. */
export function shortTime(ts: number): string {
  if (!ts) return "";
  const today = startOfDay(Date.now());
  if (startOfDay(ts) === today)
    return new Date(ts).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  return new Date(ts).toLocaleDateString([], { day: "2-digit", month: "2-digit" });
}
