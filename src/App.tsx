import { useCallback, useEffect, useRef, useState } from "react";
import * as api from "./lib/api";
import type { Contact, Message, MyIdentity } from "./lib/types";
import { Sidebar } from "./components/Sidebar";
import { ChatPanel } from "./components/ChatPanel";
import { PairingModal } from "./components/PairingModal";
import { AiPanel } from "./components/AiPanel";

export default function App() {
  const [me, setMe] = useState<MyIdentity | null>(null);
  const [contacts, setContacts] = useState<Contact[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [draft, setDraft] = useState("");
  const [pairingOpen, setPairingOpen] = useState(false);
  const [aiOpen, setAiOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [unread, setUnread] = useState<Record<string, number>>({});
  const [activity, setActivity] = useState<Record<string, number>>({});

  // ref pra o listener de rede não ler `selected` desatualizado
  const selectedRef = useRef<string | null>(null);
  selectedRef.current = selected;

  const reloadContacts = useCallback(async () => {
    try {
      setContacts(await api.contactsList());
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const loadMessages = useCallback(async (peer: string) => {
    try {
      setMessages(await api.messagesList(peer));
    } catch (e) {
      setError(String(e));
    }
  }, []);

  // boot: identidade + contatos + listeners (mensagens recebidas, recibos de leitura)
  useEffect(() => {
    const unlisteners: Array<() => void> = [];
    (async () => {
      try {
        setMe(await api.myIdentity());
      } catch (e) {
        setError(String(e));
      }
      await reloadContacts();
      unlisteners.push(
        await api.onMessageIn((m) => {
          setActivity((prev) => ({ ...prev, [m.peer]: m.ts }));
          if (m.peer === selectedRef.current) {
            setMessages((prev) => [...prev, m]);
            api.markRead(m.peer).catch(() => {});
          } else {
            setUnread((prev) => ({ ...prev, [m.peer]: (prev[m.peer] ?? 0) + 1 }));
          }
        }),
      );
      unlisteners.push(
        await api.onReceipts((peer) => {
          if (peer === selectedRef.current) loadMessages(peer);
        }),
      );
    })();
    return () => unlisteners.forEach((u) => u());
  }, [reloadContacts, loadMessages]);

  const handleSelect = useCallback(
    (nodeId: string) => {
      setSelected(nodeId);
      loadMessages(nodeId);
      setUnread((prev) => (prev[nodeId] ? { ...prev, [nodeId]: 0 } : prev));
      api.markRead(nodeId).catch(() => {});
    },
    [loadMessages],
  );

  const handleSend = useCallback(
    async (body: string) => {
      if (!selected) return;
      try {
        const msg = await api.sendMessage(selected, body);
        setMessages((prev) => [...prev, msg]);
        setActivity((prev) => ({ ...prev, [msg.peer]: msg.ts }));
      } catch (e) {
        setError(String(e));
      }
    },
    [selected],
  );

  const handleAttach = useCallback(async () => {
    if (!selected) return;
    try {
      const msg = await api.pickAndAttach(selected);
      if (msg) {
        setMessages((prev) => [...prev, msg]);
        setActivity((prev) => ({ ...prev, [msg.peer]: msg.ts }));
      }
    } catch (e) {
      setError(String(e));
    }
  }, [selected]);

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
    (a, b) => (activity[b.nodeId] ?? 0) - (activity[a.nodeId] ?? 0),
  );

  return (
    <div className={`app ${showAi ? "with-ai" : ""}`}>
      <Sidebar
        me={me}
        contacts={sortedContacts}
        selected={selected}
        unread={unread}
        onSelect={handleSelect}
        onOpenPairing={() => setPairingOpen(true)}
      />
      <ChatPanel
        contact={selectedContact}
        messages={messages}
        draft={draft}
        onDraftChange={setDraft}
        onSend={handleSend}
        onAttach={handleAttach}
        onToggleAi={() => setAiOpen((v) => !v)}
        aiOpen={aiOpen}
        onRename={handleRenameContact}
        onRemove={handleRemoveContact}
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
      {error && (
        <div className="toast" onClick={() => setError(null)} role="alert">
          {error}
        </div>
      )}
    </div>
  );
}
