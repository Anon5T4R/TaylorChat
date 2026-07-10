export interface Contact {
  nodeId: string;
  nickname: string; // apelido que eu dei
  addedTs: number;
  profileName?: string | null; // nome que ele definiu
  avatar?: string | null; // caminho do avatar dele (cache)
  muted?: boolean; // silenciado (sem notificação)
}

export interface Profile {
  name: string;
  avatar: string | null;
}

export interface Message {
  id: number;
  peer: string;
  direction: "out" | "in";
  kind: "text" | "file";
  body: string; // text: o texto; file: JSON FileMeta
  ts: number;
  state: string; // out: queued|sent|delivered|read ; in: received
  replyTo?: number | null; // ts da mensagem citada (responder)
  deleted?: boolean; // apagada para todos
}

export interface FileMeta {
  filename: string;
  mime: string;
  size: number;
  localPath?: string;
  sticker?: boolean;
}

export interface Thread {
  convo: string; // nodeId (principal) ou nodeId#threadId (extra)
  name: string;
}

export interface ConvoSummary {
  peer: string;
  kind: "text" | "file";
  body: string; // prévia (arquivo vira "📎 nome"; vazio se apagada)
  ts: number;
  direction: "out" | "in";
  deleted?: boolean; // última msg apagada para todos
}

export interface MyIdentity {
  nodeId: string;
  invite: string;
  qrSvg: string;
}

export interface ParsedInvite {
  nodeId: string;
}
