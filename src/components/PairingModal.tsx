import { useEffect, useState } from "react";
import * as api from "../lib/api";
import type { MyIdentity } from "../lib/types";

interface Props {
  me: MyIdentity;
  onClose: () => void;
  onAdd: (nodeId: string, nickname: string) => Promise<void>;
}

export function PairingModal({ me, onClose, onAdd }: Props) {
  // Esc fecha o modal.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);
  const [invite, setInvite] = useState("");
  const [nickname, setNickname] = useState("");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(me.invite);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      /* clipboard indisponível */
    }
  };

  const add = async () => {
    setErr(null);
    setBusy(true);
    try {
      const parsed = await api.parseInvite(invite);
      await onAdd(parsed.nodeId, nickname.trim());
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <header className="modal-head">
          <h3>Parear contato</h3>
          <button className="btn-icon" onClick={onClose}>
            ✕
          </button>
        </header>

        <div className="pair-grid">
          <section className="pair-mine">
            <h4>Meu convite</h4>
            <p className="hint">Esta é a sua identidade. Para mandar o convite:</p>
            <div className="qr" dangerouslySetInnerHTML={{ __html: me.qrSvg }} />
            <p className="hint">
              peça para a outra pessoa <strong>escanear este QR</strong> (ou colar o código abaixo)
              no TaylorChat dela — aí vocês viram contatos. Compartilhe por um canal que você confia.
            </p>
            <div className="invite-row">
              <code className="invite">{me.invite}</code>
              <button className="btn" onClick={copy}>
                {copied ? "Copiado!" : "Copiar"}
              </button>
            </div>
          </section>

          <section className="pair-other">
            <h4>Adicionar por convite</h4>
            <label>
              Convite (código <code>taylorchat:</code> ou o id)
              <textarea
                rows={3}
                value={invite}
                placeholder="taylorchat:…"
                onChange={(e) => setInvite(e.target.value)}
              />
            </label>
            <label>
              Apelido (opcional)
              <input
                value={nickname}
                placeholder="Ex.: Maria"
                onChange={(e) => setNickname(e.target.value)}
              />
            </label>
            {err && <p className="err">{err}</p>}
            <button className="btn btn-primary" disabled={busy || !invite.trim()} onClick={add}>
              {busy ? "Adicionando…" : "Adicionar contato"}
            </button>
          </section>
        </div>
      </div>
    </div>
  );
}
