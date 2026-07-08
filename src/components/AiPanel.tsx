import { useCallback, useEffect, useState } from "react";
import * as ai from "../lib/ai";
import type { Message } from "../lib/types";

interface Props {
  messages: Message[];
  draft: string;
  onUseText: (text: string) => void;
  onClose: () => void;
}

const LS_DIR = "taylorchat.aiDir";
const LS_MODEL = "taylorchat.aiModel";

export function AiPanel({ messages, draft, onUseText, onClose }: Props) {
  const [dir, setDir] = useState<string>(() => localStorage.getItem(LS_DIR) ?? "");
  const [models, setModels] = useState<ai.ModelInfo[]>([]);
  const [model, setModel] = useState<string>(() => localStorage.getItem(LS_MODEL) ?? "");
  const [status, setStatus] = useState<ai.LlmStatus>({ running: false, port: 0, model: "" });
  const [busy, setBusy] = useState<string | null>(null);
  const [result, setResult] = useState("");
  const [err, setErr] = useState<string | null>(null);

  const refreshModels = useCallback(async (d: string) => {
    if (!d) return;
    try {
      setModels(await ai.listModels(d));
    } catch (e) {
      setErr(String(e));
    }
  }, []);

  useEffect(() => {
    ai.llmStatus().then(setStatus).catch(() => {});
    if (dir) refreshModels(dir);
  }, [dir, refreshModels]);

  const chooseFolder = async () => {
    const d = await ai.pickModelsFolder();
    if (d) {
      setDir(d);
      localStorage.setItem(LS_DIR, d);
      refreshModels(d);
    }
  };

  const start = async () => {
    if (!model) return;
    setErr(null);
    setBusy("start");
    try {
      const port = await ai.startLlm(model, 0, 4096); // CPU + ctx 4096 (hardware modesto)
      localStorage.setItem(LS_MODEL, model);
      setStatus({ running: true, port, model });
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(null);
    }
  };

  const stop = async () => {
    await ai.stopLlm().catch(() => {});
    setStatus({ running: false, port: 0, model: "" });
  };

  const run = async (label: string, fn: () => Promise<string>) => {
    setErr(null);
    setBusy(label);
    setResult("");
    try {
      setResult(await fn());
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(null);
    }
  };

  const p = status.port;
  const ready = status.running && p > 0;

  return (
    <aside className="ai-panel">
      <header className="ai-head">
        <strong>✦ IA local</strong>
        <button className="btn-icon" onClick={onClose} title="Fechar">
          ✕
        </button>
      </header>

      <div className="ai-setup">
        <div className="ai-row">
          <button className="btn" onClick={chooseFolder}>
            Pasta de modelos
          </button>
          <span className={`ai-dot ${ready ? "on" : ""}`} title={ready ? "IA ativa" : "IA parada"} />
        </div>
        {dir && <code className="ai-dir">{dir}</code>}
        <select value={model} onChange={(e) => setModel(e.target.value)} disabled={ready}>
          <option value="">— escolha um modelo .gguf —</option>
          {models
            .filter((m) => !m.isProjector)
            .map((m) => (
              <option key={m.path} value={m.path}>
                {m.name} ({m.sizeGb.toFixed(1)} GB)
              </option>
            ))}
        </select>
        {!ready ? (
          <button className="btn btn-primary" disabled={!model || busy === "start"} onClick={start}>
            {busy === "start" ? "Iniciando…" : "Iniciar IA"}
          </button>
        ) : (
          <button className="btn" onClick={stop}>
            Parar IA (porta {p})
          </button>
        )}
      </div>

      <div className="ai-actions">
        <button className="btn" disabled={!ready || !!busy} onClick={() => run("suggest", () => ai.suggestReply(p, messages))}>
          Sugerir resposta
        </button>
        <button className="btn" disabled={!ready || !!busy} onClick={() => run("summary", () => ai.summarizeConversation(p, messages))}>
          Resumir conversa
        </button>
        <button
          className="btn"
          disabled={!ready || !!busy || !draft.trim()}
          onClick={() => run("improve", () => ai.improveDraft(p, draft, "Melhore a redação mantendo o sentido e o tom."))}
        >
          Melhorar rascunho
        </button>
        <button
          className="btn"
          disabled={!ready || !!busy || !draft.trim()}
          onClick={() => run("translate", () => ai.translate(p, draft, "inglês"))}
        >
          Traduzir rascunho (EN)
        </button>
      </div>

      {busy && busy !== "start" && <div className="ai-busy">Pensando…</div>}
      {err && <div className="ai-err">{err}</div>}

      {result && (
        <div className="ai-result">
          <textarea value={result} onChange={(e) => setResult(e.target.value)} rows={6} />
          <div className="ai-result-actions">
            <button className="btn btn-primary" onClick={() => onUseText(result)}>
              Usar no rascunho
            </button>
            <button className="btn" onClick={() => navigator.clipboard.writeText(result).catch(() => {})}>
              Copiar
            </button>
          </div>
        </div>
      )}

      <p className="ai-note">A IA roda 100% local e só sugere — nada é enviado sem você mandar.</p>
    </aside>
  );
}
