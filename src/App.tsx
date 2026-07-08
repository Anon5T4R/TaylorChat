import { useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import * as api from "./lib/api";
import type { Contact, ConvoSummary, Message, MyIdentity } from "./lib/types";
import { getLang, setLang as setI18nLang, t, type Lang } from "./lib/i18n";
import { Sidebar } from "./components/Sidebar";
import { ChatPanel } from "./components/ChatPanel";
import { PairingModal } from "./components/PairingModal";
import { AiPanel } from "./components/AiPanel";
import { SettingsModal } from "./components/SettingsModal";

type Theme = "system" | "light" | "dark";

function applyTheme(theme: Theme) {
  const root = document.documentElement;
  if (theme === "system") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", theme);
}

export default function App() {
  const [me, setMe] = useState<MyIdentity | null>(null);
  const [contacts, setContacts] = useState<Contact[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [draft, setDraft] = useState("");
  const [pairingOpen, setPairingOpen] = useState(false);
  const [aiOpen, setAiOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [unread, setUnread] = useState<Record<string, number>>({});
  const [summaries, setSummaries] = useState<Record<string, ConvoSummary>>({});
  const [dropping, setDropping] = useState(false);
  const [kw, setKw] = useState<api.KeywordStatus | null>(null);

  // Preferências
  const [lang, setLangState] = useState<Lang>(getLang());
  const [theme, setThemeState] = useState<Theme>(
    (localStorage.getItem("taylorchat.theme") as Theme) || "system",
  );
  const [readReceipts, setReadReceiptsState] = useState<boolean>(
    localStorage.getItem("taylorchat.readReceipts") !== "false",
  );

  const selectedRef = useRef<string | null>(null);
  selectedRef.current = selected;
  const rrRef = useRef(readReceipts);
  rrRef.current = readReceipts;

  useEffect(() => applyTheme(theme), [theme]);

  // Reenvio periódico: o que ficou na fila (par offline) tenta sair sozinho de tempos
  // em tempos, além de quando você abre a conversa ou o par te manda algo.
  useEffect(() => {
    const id = setInterval(() => {
      const peer = selectedRef.current;
      if (peer) flushQueue(peer);
    }, 25000);
    return () => clearInterval(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const changeLang = (l: Lang) => {
    setI18nLang(l);
    setLangState(l);
  };
  const changeTheme = (tm: Theme) => {
    localStorage.setItem("taylorchat.theme", tm);
    setThemeState(tm);
  };
  const changeReadReceipts = (b: boolean) => {
    localStorage.setItem("taylorchat.readReceipts", String(b));
    setReadReceiptsState(b);
  };
  const doMarkRead = (peer: string) => {
    if (rrRef.current) api.markRead(peer).catch(() => {});
  };

  const reloadContacts = useCallback(async () => {
    try {
      setContacts(await api.contactsList());
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const reloadSummaries = useCallback(async () => {
    try {
      const list = await api.conversationsSummary();
      setSummaries(Object.fromEntries(list.map((s) => [s.peer, s])));
    } catch {
      /* prévia é cosmética */
    }
  }, []);

  const loadMessages = useCallback(async (peer: string) => {
    try {
      setMessages(await api.messagesList(peer));
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const loadKw = useCallback((peer: string) => {
    api.keywordStatus(peer).then(setKw).catch(() => setKw(null));
  }, []);

  const flushQueue = useCallback(
    (peer: string) => {
      api
        .resendQueued(peer)
        .then((n) => {
          if (n > 0 && peer === selectedRef.current) loadMessages(peer);
        })
        .catch(() => {});
    },
    [loadMessages],
  );

  useEffect(() => {
    const unlisteners: Array<() => void> = [];
    (async () => {
      try {
        setMe(await api.myIdentity());
      } catch (e) {
        setError(String(e));
      }
      await reloadContacts();
      await reloadSummaries();
      unlisteners.push(
        await api.onMessageIn((m) => {
          reloadContacts(); // pode ter auto-criado um contato novo
          reloadSummaries();
          if (m.peer === selectedRef.current) {
            setMessages((prev) => [...prev, m]);
            doMarkRead(m.peer);
          } else {
            setUnread((prev) => ({ ...prev, [m.peer]: (prev[m.peer] ?? 0) + 1 }));
          }
          flushQueue(m.peer);
        }),
      );
      unlisteners.push(
        await api.onReceipts((peer) => {
          if (peer === selectedRef.current) loadMessages(peer);
        }),
      );
      unlisteners.push(
        await api.onKeyword((peer) => {
          if (peer === selectedRef.current) loadKw(peer);
        }),
      );
      try {
        unlisteners.push(
          await getCurrentWebview().onDragDropEvent((event) => {
            const p = event.payload as { type: string; paths?: string[] };
            if (p.type === "enter" || p.type === "over") setDropping(true);
            if (p.type === "leave") setDropping(false);
            if (p.type === "drop") {
              setDropping(false);
              const peer = selectedRef.current;
              if (!peer || !p.paths?.length) return;
              (async () => {
                for (const path of p.paths!.slice(0, 10)) {
                  try {
                    const msg = await api.attachPath(peer, path);
                    if (peer === selectedRef.current) setMessages((prev) => [...prev, msg]);
                  } catch (e) {
                    setError(String(e));
                  }
                }
                reloadSummaries();
              })();
            }
          }),
        );
      } catch {
        /* sem drag&drop fora do Tauri */
      }
    })();
    return () => unlisteners.forEach((u) => u());
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [reloadContacts, reloadSummaries, loadMessages, loadKw, flushQueue]);

  const handleSelect = useCallback(
    (nodeId: string) => {
      setSelected(nodeId);
      loadMessages(nodeId);
      loadKw(nodeId);
      setUnread((prev) => (prev[nodeId] ? { ...prev, [nodeId]: 0 } : prev));
      doMarkRead(nodeId);
      flushQueue(nodeId);
    },
    [loadMessages, loadKw, flushQueue],
  );

  const handleSend = useCallback(
    async (body: string) => {
      if (!selected) return;
      try {
        const msg = await api.sendMessage(selected, body);
        setMessages((prev) => [...prev, msg]);
        reloadSummaries();
      } catch (e) {
        setError(String(e));
      }
    },
    [selected, reloadSummaries],
  );

  const handleAttach = useCallback(async () => {
    if (!selected) return;
    try {
      const msg = await api.pickAndAttach(selected);
      if (msg) {
        setMessages((prev) => [...prev, msg]);
        reloadSummaries();
      }
    } catch (e) {
      setError(String(e));
    }
  }, [selected, reloadSummaries]);

  const handleRemoveContact = useCallback(async () => {
    if (!selected) return;
    try {
      await api.contactRemove(selected);
      setSelected(null);
      setMessages([]);
      await reloadContacts();
    } catch (e) {
      setError(String(e));
    }
  }, [selected, reloadContacts]);

  const handleRenameContact = useCallback(
    async (nickname: string) => {
      if (!selected) return;
      try {
        await api.contactAdd(selected, nickname);
        await reloadContacts();
      } catch (e) {
        setError(String(e));
      }
    },
    [selected, reloadContacts],
  );

  const handleClear = useCallback(async () => {
    if (!selected) return;
    try {
      await api.clearConversation(selected);
      setMessages([]);
      reloadSummaries();
    } catch (e) {
      setError(String(e));
    }
  }, [selected, reloadSummaries]);

  const handleSetKeyword = useCallback(async () => {
    if (!selected) return;
    const word = window.prompt(t("kw.prompt"), kw?.word ?? "");
    if (word === null) return;
    try {
      await api.setKeyword(selected, word);
      loadKw(selected);
    } catch (e) {
      setError(String(e));
    }
  }, [selected, kw, loadKw]);

  const handleAddContact = useCallback(
    async (nodeId: string, nickname: string) => {
      await api.contactAdd(nodeId, nickname);
      await reloadContacts();
      setPairingOpen(false);
      handleSelect(nodeId);
    },
    [reloadContacts, handleSelect],
  );

  const selectedContact = contacts.find((c) => c.nodeId === selected) ?? null;
  const showAi = aiOpen && !!selectedContact;
  const sortedContacts = [...contacts].sort(
    (a, b) => (summaries[b.nodeId]?.ts ?? 0) - (summaries[a.nodeId]?.ts ?? 0),
  );

  return (
    <div className={`app ${showAi ? "with-ai" : ""}`} key={lang}>
      <Sidebar
        me={me}
        contacts={sortedContacts}
        selected={selected}
        unread={unread}
        summaries={summaries}
        onSelect={handleSelect}
        onOpenPairing={() => setPairingOpen(true)}
        onOpenSettings={() => setSettingsOpen(true)}
      />
      <ChatPanel
        contact={selectedContact}
        messages={messages}
        draft={draft}
        kw={kw}
        onDraftChange={setDraft}
        onSend={handleSend}
        onAttach={handleAttach}
        onToggleAi={() => setAiOpen((v) => !v)}
        aiOpen={aiOpen}
        onRename={handleRenameContact}
        onRemove={handleRemoveContact}
        onSetKeyword={handleSetKeyword}
        onClear={handleClear}
      />
      {showAi && (
        <AiPanel
          messages={messages}
          draft={draft}
          onUseText={setDraft}
          onClose={() => setAiOpen(false)}
        />
      )}
      {pairingOpen && me && (
        <PairingModal me={me} onClose={() => setPairingOpen(false)} onAdd={handleAddContact} />
      )}
      {settingsOpen && (
        <SettingsModal
          theme={theme}
          onTheme={changeTheme}
          lang={lang}
          onLang={changeLang}
          readReceipts={readReceipts}
          onReadReceipts={changeReadReceipts}
          auditPeer={selected}
          onClose={() => setSettingsOpen(false)}
        />
      )}
      {dropping && selected && (
        <div className="drop-overlay">
          <div className="drop-inner">📎 {t("drop.hint")}</div>
        </div>
      )}
      {error && (
        <div className="toast" onClick={() => setError(null)} role="alert">
          {error}
        </div>
      )}
    </div>
  );
}
