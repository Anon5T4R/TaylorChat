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
}

export interface MyIdentity {
  nodeId: string;
  invite: string;
  qrSvg: string;
}

export interface ParsedInvite {
  nodeId: string;
}
