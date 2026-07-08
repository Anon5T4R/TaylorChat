export interface Contact {
  nodeId: string;
  nickname: string;
  addedTs: number;
}

export interface Message {
  id: number;
  peer: string;
  direction: "out" | "in";
  kind: "text" | "file";
  body: string; // text: o texto; file: JSON FileMeta
  ts: number;
  state: string; // out: queued|sent|delivered|read ; in: received
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
  body: string; // prévia (arquivo vira "📎 nome")
  ts: number;
  direction: "out" | "in";
}

export interface MyIdentity {
  nodeId: string;
  invite: string;
  qrSvg: string;
}

export interface ParsedInvite {
  nodeId: string;
}
