import { useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import * as api from "./lib/api";
import type { Contact, ConvoSummary, Message, MyIdentity, Thread } from "./lib/types";
import { getLang, setLang as setI18nLang, t, type Lang } from "./lib/i18n";
import { randomHex, splitConvo } from "./lib/ui";
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
  const [threads, setThreads] = useState<Thread[]>([]);
  const [selected, setSelected] = useState<string | null>(null); // convo key
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
  const doMarkRead = (convo: string) => {
    if (rrRef.current) api.markRead(convo).catch(() => {});
  };

  const reloadContacts = useCallback(async () => {
    try {
      setContacts(await api.contactsList());
    } catch (e) {
      setError(String(e));
    }
  }, []);
  const reloadThreads = useCallback(async () => {
    try {
      setThreads(await api.threadsList());
    } catch (e) {
      setError(String(e));
    }
  }, []);
  const reloadSummaries = useCallback(async () => {
    try {
      const list = await api.conversationsSummary();
      setSummaries(Object.fromEntries(list.map((s) => [s.peer, s])));
    } catch {
      /* cosmético */
    }
  }, []);
  const loadMessages = useCallback(async (convo: string) => {
    try {
      setMessages(await api.messagesList(convo));
    } catch (e) {
      setError(String(e));
    }
  }, []);
  const loadKw = useCallback((node: string) => {
    api.keywordStatus(node).then(setKw).catch(() => setKw(null));
  }, []);

  const flushQueue = useCallback(
    (convo: string) => {
      api
        .resendQueued(convo)
        .then((n) => {
          if (n > 0 && convo === selectedRef.current) loadMessages(convo);
        })
        .catch(() => {});
    },
    [loadMessages],
  );

  // Reenvio periódico da fila de TODAS as conversas + ao focar a janela (o que ficou
  // pendente sai sozinho quando o par voltar/a rede subir). Recarrega se algo saiu.
  useEffect(() => {
    const flushAll = () => {
      api
        .resendAll()
        .then((n) => {
          if (n > 0) {
            reloadSummaries();
            const c = selectedRef.current;
            if (c) loadMessages(c);
          }
        })
        .catch(() => {});
    };
    const id = setInterval(flushAll, 15000);
    window.addEventListener("focus", flushAll);
    flushAll();
    return () => {
      clearInterval(id);
      window.removeEventListener("focus", flushAll);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    const unlisteners: Array<() => void> = [];
    (async () => {
      try {
        setMe(await api.myIdentity());
      } catch (e) {
        setError(String(e));
      }
      await reloadContacts();
      await reloadThreads();
      await reloadSummaries();
      unlisteners.push(
        await api.onMessageIn((m) => {
          reloadContacts();
          reloadThreads();
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
        await api.onReceipts((convo) => {
          if (convo === selectedRef.current) loadMessages(convo);
        }),
      );
      unlisteners.push(
        await api.onKeyword((node) => {
          const cur = selectedRef.current;
          if (cur && splitConvo(cur).node === node) loadKw(node);
        }),
      );
      unlisteners.push(await api.onNetError((line) => setError(line)));
      try {
        unlisteners.push(
          await getCurrentWebview().onDragDropEvent((event) => {
            const p = event.payload as { type: string; paths?: string[] };
            if (p.type === "enter" || p.type === "over") setDropping(true);
            if (p.type === "leave") setDropping(false);
            if (p.type === "drop") {
              setDropping(false);
              const convo = selectedRef.current;
              if (!convo || !p.paths?.length) return;
              (async () => {
                for (const path of p.paths!.slice(0, 10)) {
                  try {
                    const msg = await api.attachPath(convo, path);
                    if (convo === selectedRef.current) setMessages((prev) => [...prev, msg]);
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
  }, [reloadContacts, reloadThreads, reloadSummaries, loadMessages, loadKw, flushQueue]);

  const handleSelect = useCallback(
    (convo: string) => {
      setSelected(convo);
      loadMessages(convo);
      loadKw(splitConvo(convo).node);
      setUnread((prev) => (prev[convo] ? { ...prev, [convo]: 0 } : prev));
      doMarkRead(convo);
      flushQueue(convo);
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

  const handleSendSticker = useCallback(
    async (path: string) => {
      if (!selected) return;
      try {
        const msg = await api.sendSticker(selected, path);
        setMessages((prev) => [...prev, msg]);
        reloadSummaries();
      } catch (e) {
        setError(String(e));
      }
    },
    [selected, reloadSummaries],
  );

  const handleNewChat = useCallback(async () => {
    if (!selected) return;
    const { node } = splitConvo(selected);
    const name = window.prompt(t("chat.newChatPrompt"), "");
    if (name === null) return;
    const convo = `${node}#${randomHex(5)}`;
    try {
      await api.threadCreate(convo, name.trim());
      await reloadThreads();
      handleSelect(convo);
    } catch (e) {
      setError(String(e));
    }
  }, [selected, reloadThreads, handleSelect]);

  const handleRemoveContact = useCallback(async () => {
    if (!selected) return;
    const { node } = splitConvo(selected);
    try {
      // remove todas as conversas desse contato + o contato
      for (const th of threads) {
        if (splitConvo(th.convo).node === node) await api.threadDelete(th.convo);
      }
      await api.contactRemove(node);
      setSelected(null);
      setMessages([]);
      await reloadContacts();
      await reloadThreads();
    } catch (e) {
      setError(String(e));
    }
  }, [selected, threads, reloadContacts, reloadThreads]);

  const handleRenameContact = useCallback(
    async (nickname: string) => {
      if (!selected) return;
      try {
        await api.contactAdd(splitConvo(selected).node, nickname);
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
    const node = splitConvo(selected).node;
    const word = window.prompt(t("kw.prompt"), kw?.word ?? "");
    if (word === null) return;
    try {
      await api.setKeyword(node, word);
      loadKw(node);
    } catch (e) {
      setError(String(e));
    }
  }, [selected, kw, loadKw]);

  const handleAddContact = useCallback(
    async (nodeId: string, nickname: string) => {
      await api.contactAdd(nodeId, nickname);
      await reloadContacts();
      await reloadThreads();
      setPairingOpen(false);
      handleSelect(nodeId);
    },
    [reloadContacts, reloadThreads, handleSelect],
  );

  const selNode = selected ? splitConvo(selected).node : null;
  const selThread = selected ? threads.find((th) => th.convo === selected) : undefined;
  const selectedContact: Contact | null = selNode
    ? contacts.find((c) => c.nodeId === selNode) ?? { nodeId: selNode, nickname: "", addedTs: 0 }
    : null;
  const showAi = aiOpen && !!selectedContact;

  return (
    <div className={`app ${showAi ? "with-ai" : ""}`} key={lang}>
      <Sidebar
        me={me}
        threads={threads}
        contacts={contacts}
        selected={selected}
        unread={unread}
        summaries={summaries}
        onSelect={handleSelect}
        onOpenPairing={() => setPairingOpen(true)}
        onOpenSettings={() => setSettingsOpen(true)}
      />
      <ChatPanel
        contact={selectedContact}
        threadName={selThread?.name ?? ""}
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
        onNewChat={handleNewChat}
        onSendSticker={handleSendSticker}
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
