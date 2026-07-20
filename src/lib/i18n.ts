// i18n minimalista pt/en/es. O idioma fica em localStorage; App guarda em estado e
// re-renderiza tudo ao trocar, então os componentes só chamam t().
//
// **O `pt` é a fonte da verdade das chaves.** `MessageKey` sai dele, e `es`/`en` são
// declarados como `Record<MessageKey, string>` — o padrão da suíte. Isso faz o `tsc`
// RECUSAR o build se faltar (ou sobrar) chave numa tradução, em vez de deixar a UI cair
// no fallback pt em silêncio. Antes o dicionário era `Record<Lang, Record<string,string>>`,
// que não checava nada e deixava a paridade por conta de conferência manual.

export type Lang = "pt" | "es" | "en";

/** Endônimos — nome de idioma NÃO se traduz. */
export const LANG_LABELS: Record<Lang, string> = {
  pt: "Português",
  es: "Español",
  en: "English",
};

const pt = {
  "me": "eu",
  "sidebar.empty": "Nenhuma conversa ainda.",
  "sidebar.pair": "Parear com alguém",
  "sidebar.pairTip": "Parear / adicionar contato",
  "sidebar.settingsTip": "Configurações",
  "sidebar.tabChats": "Conversas",
  "sidebar.tabContacts": "Contatos",
  "sidebar.contactsEmpty": "Nenhum contato ainda. Pareie com alguém.",
  "sidebar.search": "Buscar conversas e mensagens…",
  "sidebar.searchContacts": "Contatos",
  "sidebar.searchMessages": "Mensagens",
  "sidebar.searchNone": "Nada encontrado.",
  "chat.online": "online",
  "chat.offline": "offline",
  "chat.checking": "verificando…",
  "chat.typing": "digitando…",
  "chat.emptyTitle": "TaylorChat",
  "chat.empty": "Selecione um contato ou pareie com alguém pra começar a conversar.",
  "chat.placeholder": "Escreva uma mensagem…  (Enter envia, Shift+Enter quebra linha)",
  "chat.send": "Enviar",
  "chat.attachTip": "Anexar arquivo (ou arraste pra cá)",
  "chat.aiTip": "Assistente de IA local",
  "msg.copy": "Copiar",
  "msg.reply": "Responder",
  "msg.deleteMine": "Apagar para mim",
  "msg.deleteAll": "Apagar para todos",
  "msg.deleteAllConfirm": "Apagar esta mensagem para todos? O contato também deixa de vê-la.",
  "msg.deleted": "mensagem apagada",
  "msg.quoteGone": "mensagem original indisponível",
  "msg.forward": "Encaminhar",
  "msg.reactRemove": "Remover reação",
  "fwd.title": "Encaminhar para",
  "fwd.search": "Buscar conversa…",
  "fwd.noLocal": "sem cópia local pra encaminhar",
  "chat.fileUnreadable": "⟨anexo ilegível⟩",
  "chat.openTip": "Abrir anexo",
  "chat.unavailable": "Anexo indisponível",
  "chat.muteTip": "Silenciar (sem notificação)",
  "chat.unmuteTip": "Reativar notificações",
  "chat.profileTip": "Ver ficha do contato",
  "profile.title": "Ficha do contato",
  "profile.muted": "silenciado",
  "profile.nickname": "Apelido (como eu chamo)",
  "profile.phone": "Telefone",
  "profile.email": "E-mail",
  "profile.birthday": "Aniversário",
  "profile.notes": "Notas",
  "profile.keyword": "Palavra-chave",
  "profile.keywordPh": "combinada fora do app",
  "profile.keywordHint": "Verificação anti-impostor: combine uma palavra por outro canal. Se não conferir, cuidado.",
  "profile.save": "Salvar",
  "profile.saved": "Salvo ✓",
  "profile.dangerZone": "Zona de perigo",
  "profile.remove": "Apagar contato",
  "profile.removeConfirm": "apagar este contato? A conversa e o histórico vão junto.",
  "chat.clearTip": "Limpar conversa",
  "chat.clearConfirm": "Apagar TODO o histórico desta conversa? Não dá pra desfazer.",
  "chat.searchTip": "Buscar na conversa (Ctrl+F)",
  "chat.searchPh": "Buscar na conversa…",
  "chat.loadOlder": "Carregar mensagens antigas",
  "chat.newMessages": "Nova mensagem",
  "chat.newChatTip": "Novo chat com este contato",
  "chat.newChatPrompt": "Nome do novo chat (ex.: Trabalho):",
  "chat.stickerTip": "Stickers",
  "chat.emojiTip": "Emoji",
  "stickers.add": "Criar sticker",
  "stickers.newPack": "Novo pacote",
  "stickers.newPackPrompt": "Nome do pacote:",
  "stickers.empty": "Nenhum sticker neste pacote. Clique em Criar sticker.",
  "kw.match": "Palavra-chave confere ✓",
  "kw.mismatch": "⚠ A palavra-chave não confere — confirme com quem você combinou",
  "kw.waiting": "Palavra-chave definida — aguardando a do contato",
  "kw.none": "Sem palavra-chave (opcional, verifica a identidade)",
  "drop.hint": "Solte para enviar",
  "day.today": "Hoje",
  "day.yesterday": "Ontem",
  "tick.queued": "Na fila — sai quando o contato estiver alcançável",
  "tick.failed": "Falhou — anexo sem cópia local",
  "pair.title": "Parear contato",
  "pair.mine": "Meu convite",
  "pair.mineHint": "Esta é a sua identidade. Para mandar o convite:",
  "pair.mineHint2":
    "peça para a outra pessoa escanear este QR (ou colar o código abaixo) no TaylorChat dela — aí vocês viram contatos. Compartilhe por um canal que você confia.",
  "pair.copy": "Copiar",
  "pair.copied": "Copiado!",
  "pair.add": "Adicionar por convite",
  "pair.inviteLabel": "Convite (código taylorchat: ou o id)",
  "pair.nickLabel": "Apelido (opcional)",
  "pair.nickPh": "Ex.: Maria",
  "pair.addBtn": "Adicionar contato",
  "pair.adding": "Adicionando…",
  "settings.title": "Configurações",
  "settings.profileName": "Seu nome",
  "settings.profilePhoto": "Trocar foto",
  "settings.theme": "Tema",
  "settings.themeSystem": "Sistema",
  "settings.themeLight": "Claro",
  "settings.themeDark": "Escuro",
  "settings.themeNature": "Natureza",
  "settings.themeDarkblue": "Azul escuro",
  "settings.themeCalmgreen": "Verde calmo",
  "settings.themePastelpink": "Rosa pastel",
  "settings.themePunkprincess": "PunkPrincess",
  "settings.lang": "Idioma",
  "settings.readReceipts": "Recibos de leitura",
  "settings.readReceiptsHint": "Avisar ao contato quando você leu (✓✓ colorido).",
  "settings.notify": "Notificações de desktop",
  "settings.notifyHint": "Avisar quando chegar mensagem com o app em segundo plano. Não avisa a conversa que você já está lendo.",
  "settings.notifyOsHint": "Se o aviso não aparecer, confira as notificações do TaylorChat nas configurações do sistema (o app não consegue ler esse ajuste).",
  "settings.notifyPreview": "Prévia na notificação",
  "settings.notifyPreviewHint": "Mostrar o texto da mensagem na notificação do sistema (desligue pra vazar só quem, não o quê).",
  "settings.audit": "Auditar conversa",
  "settings.auditHint":
    "Gera um código do histórico desta conversa. Compare com o do outro aparelho: se forem iguais, nenhum lado alterou o conteúdo.",
  "settings.auditRun": "Gerar código de auditoria",
  "settings.auditPick": "Abra uma conversa pra auditar.",
  "settings.auditCount": "mensagens",
  "settings.diag": "Diagnóstico de rede",
  "settings.diagUp": "Rede no ar",
  "settings.diagDown": "Rede fora do ar",
  "settings.diagCopy": "Copiar log",
  "settings.close": "Fechar",

  "storage.section": "Dados e armazenamento",
  "storage.path": "Pasta de dados",
  "storage.open": "Abrir",
  "storage.db": "Histórico (banco)",
  "storage.dbCounts": "{n} mensagens · {f} com anexo · {c} contatos · {v} conversas",
  "storage.dbHint":
    "as mensagens, os contatos e as conversas — nenhuma limpeza desta tela apaga isso. Numa conversa ponto a ponto não há servidor pra rebaixar nada: o que sai daqui sai do mundo.",
  "storage.attachments": "Anexos recebidos e enviados",
  "storage.attachmentsCounts": "{n} arquivos · {orphans} órfãos ({orphanSize})",
  "storage.clearOrphan": "Limpar anexos órfãos",
  "storage.clearOrphanHint":
    "apaga só os arquivos que nenhuma mensagem referencia — sobra de conversa apagada ou contato removido. Todo anexo que ainda aparece numa conversa fica.",
  "storage.partial": "Transferências interrompidas",
  "storage.partialCounts": "{n} arquivos parciais",
  "storage.clearPartialHint":
    "pedaços de anexos que não terminaram de chegar. Eles servem pra retomar de onde parou, então limpe só quando não houver transferência em andamento. Nenhuma mensagem é afetada.",
  "storage.avatars": "Fotos de contatos (cache)",
  "storage.avatarsCounts": "{n} arquivos · {orphans} de contatos removidos ({orphanSize})",
  "storage.clearAvatarsHint":
    "apaga só as fotos de quem não está mais nos seus contatos. A foto de contato ativo fica, e a sua também.",
  "storage.backups": "Backups do banco",
  "storage.backupsCounts": "{n} cópias · {old} antigas ({oldSize})",
  "storage.clearBackupsHint":
    "cópia diária do histórico. Apaga as antigas e SEMPRE mantém a mais recente — você nunca fica sem rede de proteção.",
  "storage.stickers": "Figurinhas",
  "storage.stickersCounts": "{n} arquivos",
  "storage.stickersHint": "conteúdo que você escolheu — nenhum botão desta tela apaga figurinha.",
  "storage.clear": "Limpar",
  "storage.confirmTitle": "Confirmar limpeza",
  "storage.confirmOrphan":
    "Apagar os anexos que nenhuma mensagem referencia? As conversas e os anexos em uso ficam.",
  "storage.confirmPartial":
    "Apagar os pedaços de transferências não terminadas? Uma transferência em andamento vai recomeçar do zero. Nenhuma mensagem é apagada.",
  "storage.confirmAvatars":
    "Apagar as fotos em cache de contatos removidos? As fotos dos seus contatos atuais ficam.",
  "storage.confirmBackups":
    "Apagar os backups antigos do banco? O mais recente fica, e o histórico atual não é tocado.",
  "storage.confirmYes": "Sim, apagar",
  "storage.cancel": "Cancelar",
  "storage.freed": "Liberado {size} ({n} arquivos).",
  "storage.nothing": "Nada pra limpar aqui.",
  "storage.failed": "Falha na limpeza: {e}",
  "storage.loadFailed": "Não consegui medir o armazenamento: {e}",

  "common.on": "Ligado",
  "common.off": "Desligado",
  "ai.title": "✦ IA local",
  "ai.folder": "Pasta de modelos",
  "ai.pick": "— escolha um modelo .gguf —",
  "ai.start": "Iniciar IA",
  "ai.starting": "Iniciando…",
  "ai.stop": "Parar IA",
  "ai.suggest": "Sugerir resposta",
  "ai.summarize": "Resumir conversa",
  "ai.improve": "Melhorar rascunho",
  "ai.translate": "Traduzir rascunho (EN)",
  "ai.thinking": "Pensando…",
  "ai.use": "Usar no rascunho",
  "ai.copy": "Copiar",
  "ai.note": "A IA roda 100% local e só sugere — nada é enviado sem você mandar.",
} as const;

/** Toda chave de UI existente. Chave nova entra no `pt` e o `tsc` cobra es/en. */
export type MessageKey = keyof typeof pt;

const es: Record<MessageKey, string> = {
  "me": "yo",
  "sidebar.empty": "Aún no hay conversaciones.",
  "sidebar.pair": "Emparejar con alguien",
  "sidebar.pairTip": "Emparejar / agregar contacto",
  "sidebar.settingsTip": "Ajustes",
  "sidebar.tabChats": "Chats",
  "sidebar.tabContacts": "Contactos",
  "sidebar.contactsEmpty": "Aún no hay contactos. Empareja con alguien.",
  "sidebar.search": "Buscar chats y mensajes…",
  "sidebar.searchContacts": "Contactos",
  "sidebar.searchMessages": "Mensajes",
  "sidebar.searchNone": "Nada encontrado.",
  "chat.online": "en línea",
  "chat.offline": "desconectado",
  "chat.checking": "verificando…",
  "chat.typing": "escribiendo…",
  "chat.emptyTitle": "TaylorChat",
  "chat.empty": "Selecciona un contacto o empareja con alguien para empezar a conversar.",
  "chat.placeholder": "Escribe un mensaje…  (Enter envía, Shift+Enter salta línea)",
  "chat.send": "Enviar",
  "chat.attachTip": "Adjuntar archivo (o arrástralo aquí)",
  "chat.aiTip": "Asistente de IA local",
  "msg.copy": "Copiar",
  "msg.reply": "Responder",
  "msg.deleteMine": "Eliminar para mí",
  "msg.deleteAll": "Eliminar para todos",
  "msg.deleteAllConfirm": "¿Eliminar este mensaje para todos? El contacto tampoco lo verá.",
  "msg.deleted": "mensaje eliminado",
  "msg.quoteGone": "mensaje original no disponible",
  "msg.forward": "Reenviar",
  "msg.reactRemove": "Quitar reacción",
  "fwd.title": "Reenviar a",
  "fwd.search": "Buscar chat…",
  "fwd.noLocal": "sin copia local para reenviar",
  "chat.fileUnreadable": "⟨adjunto ilegible⟩",
  "chat.openTip": "Abrir adjunto",
  "chat.unavailable": "Adjunto no disponible",
  "chat.muteTip": "Silenciar (sin notificación)",
  "chat.unmuteTip": "Reactivar notificaciones",
  "chat.profileTip": "Ver ficha del contacto",
  "profile.title": "Ficha del contacto",
  "profile.muted": "silenciado",
  "profile.nickname": "Apodo (como lo llamo)",
  "profile.phone": "Teléfono",
  "profile.email": "Correo",
  "profile.birthday": "Cumpleaños",
  "profile.notes": "Notas",
  "profile.keyword": "Palabra clave",
  "profile.keywordPh": "acordada fuera de la app",
  "profile.keywordHint": "Verificación anti-impostor: acuerda una palabra por otro canal. Si no coincide, cuidado.",
  "profile.save": "Guardar",
  "profile.saved": "Guardado ✓",
  "profile.dangerZone": "Zona de peligro",
  "profile.remove": "Eliminar contacto",
  "profile.removeConfirm": "¿eliminar este contacto? La conversación y el historial también.",
  "chat.clearTip": "Limpiar conversación",
  "chat.clearConfirm": "¿Borrar TODO el historial de esta conversación? No se puede deshacer.",
  "chat.searchTip": "Buscar en la conversación (Ctrl+F)",
  "chat.searchPh": "Buscar en la conversación…",
  "chat.loadOlder": "Cargar mensajes antiguos",
  "chat.newMessages": "Nuevo mensaje",
  "chat.newChatTip": "Nuevo chat con este contacto",
  "chat.newChatPrompt": "Nombre del nuevo chat (ej.: Trabajo):",
  "chat.stickerTip": "Stickers",
  "chat.emojiTip": "Emoji",
  "stickers.add": "Crear sticker",
  "stickers.newPack": "Nuevo paquete",
  "stickers.newPackPrompt": "Nombre del paquete:",
  "stickers.empty": "Sin stickers en este paquete. Pulsa Crear sticker.",
  "kw.match": "La palabra clave coincide ✓",
  "kw.mismatch": "⚠ La palabra clave no coincide — confirma con quien la acordaste",
  "kw.waiting": "Palabra clave definida — esperando la del contacto",
  "kw.none": "Sin palabra clave (opcional, verifica la identidad)",
  "drop.hint": "Suelta para enviar",
  "day.today": "Hoy",
  "day.yesterday": "Ayer",
  "tick.queued": "En cola — sale cuando el contacto esté disponible",
  "tick.failed": "Falló — adjunto sin copia local",
  "pair.title": "Emparejar contacto",
  "pair.mine": "Mi invitación",
  "pair.mineHint": "Esta es tu identidad. Para enviar la invitación:",
  "pair.mineHint2":
    "pide a la otra persona que escanee este QR (o pegue el código abajo) en su TaylorChat — así se vuelven contactos. Compártelo por un canal de confianza.",
  "pair.copy": "Copiar",
  "pair.copied": "¡Copiado!",
  "pair.add": "Agregar por invitación",
  "pair.inviteLabel": "Invitación (código taylorchat: o el id)",
  "pair.nickLabel": "Apodo (opcional)",
  "pair.nickPh": "Ej.: María",
  "pair.addBtn": "Agregar contacto",
  "pair.adding": "Agregando…",
  "settings.title": "Ajustes",
  "settings.profileName": "Tu nombre",
  "settings.profilePhoto": "Cambiar foto",
  "settings.theme": "Tema",
  "settings.themeSystem": "Sistema",
  "settings.themeLight": "Claro",
  "settings.themeDark": "Oscuro",
  "settings.themeNature": "Naturaleza",
  "settings.themeDarkblue": "Azul oscuro",
  "settings.themeCalmgreen": "Verde tranquilo",
  "settings.themePastelpink": "Rosa pastel",
  "settings.themePunkprincess": "PunkPrincess",
  "settings.lang": "Idioma",
  "settings.readReceipts": "Confirmaciones de lectura",
  "settings.readReceiptsHint": "Avisar al contacto cuando lo leíste (✓✓ de color).",
  "settings.notify": "Notificaciones de escritorio",
  "settings.notifyHint": "Avisar cuando llegue un mensaje con la app en segundo plano. No avisa de la conversación que ya estás leyendo.",
  "settings.notifyOsHint": "Si el aviso no aparece, revisa las notificaciones de TaylorChat en la configuración del sistema (la app no puede leer ese ajuste).",
  "settings.notifyPreview": "Vista previa en la notificación",
  "settings.notifyPreviewHint": "Mostrar el texto del mensaje en la notificación del sistema (apágalo para revelar solo quién, no qué).",
  "settings.audit": "Auditar conversación",
  "settings.auditHint":
    "Genera un código del historial de esta conversación. Compáralo con el del otro dispositivo: si son iguales, ningún lado alteró el contenido.",
  "settings.auditRun": "Generar código de auditoría",
  "settings.auditPick": "Abre una conversación para auditar.",
  "settings.auditCount": "mensajes",
  "settings.diag": "Diagnóstico de red",
  "settings.diagUp": "Red activa",
  "settings.diagDown": "Red caída",
  "settings.diagCopy": "Copiar registro",
  "settings.close": "Cerrar",

  "storage.section": "Datos y almacenamiento",
  "storage.path": "Carpeta de datos",
  "storage.open": "Abrir",
  "storage.db": "Historial (base de datos)",
  "storage.dbCounts": "{n} mensajes · {f} con adjunto · {c} contactos · {v} conversaciones",
  "storage.dbHint":
    "los mensajes, los contactos y las conversaciones — ninguna limpieza de esta pantalla los borra. En una conversación punto a punto no hay servidor que reponga nada: lo que sale de aquí desaparece.",
  "storage.attachments": "Adjuntos recibidos y enviados",
  "storage.attachmentsCounts": "{n} archivos · {orphans} huérfanos ({orphanSize})",
  "storage.clearOrphan": "Limpiar adjuntos huérfanos",
  "storage.clearOrphanHint":
    "borra solo los archivos que ningún mensaje referencia — restos de conversaciones borradas o contactos eliminados. Todo adjunto que aún aparece en una conversación se queda.",
  "storage.partial": "Transferencias interrumpidas",
  "storage.partialCounts": "{n} archivos parciales",
  "storage.clearPartialHint":
    "trozos de adjuntos que no terminaron de llegar. Sirven para reanudar donde se cortó, así que limpia solo cuando no haya transferencias en curso. Ningún mensaje se ve afectado.",
  "storage.avatars": "Fotos de contactos (caché)",
  "storage.avatarsCounts": "{n} archivos · {orphans} de contactos eliminados ({orphanSize})",
  "storage.clearAvatarsHint":
    "borra solo las fotos de quien ya no está en tus contactos. La foto de un contacto activo se queda, y la tuya también.",
  "storage.backups": "Copias de seguridad",
  "storage.backupsCounts": "{n} copias · {old} antiguas ({oldSize})",
  "storage.clearBackupsHint":
    "copia diaria del historial. Borra las antiguas y SIEMPRE conserva la más reciente — nunca te quedas sin red de seguridad.",
  "storage.stickers": "Stickers",
  "storage.stickersCounts": "{n} archivos",
  "storage.stickersHint":
    "contenido que tú elegiste — ningún botón de esta pantalla borra stickers.",
  "storage.clear": "Limpiar",
  "storage.confirmTitle": "Confirmar limpieza",
  "storage.confirmOrphan":
    "¿Borrar los adjuntos que ningún mensaje referencia? Las conversaciones y los adjuntos en uso se quedan.",
  "storage.confirmPartial":
    "¿Borrar los trozos de transferencias sin terminar? Una transferencia en curso volverá a empezar desde cero. No se borra ningún mensaje.",
  "storage.confirmAvatars":
    "¿Borrar las fotos en caché de contactos eliminados? Las fotos de tus contactos actuales se quedan.",
  "storage.confirmBackups":
    "¿Borrar las copias de seguridad antiguas? La más reciente se queda, y el historial actual no se toca.",
  "storage.confirmYes": "Sí, borrar",
  "storage.cancel": "Cancelar",
  "storage.freed": "Liberado {size} ({n} archivos).",
  "storage.nothing": "Nada que limpiar aquí.",
  "storage.failed": "Error en la limpieza: {e}",
  "storage.loadFailed": "No pude medir el almacenamiento: {e}",

  "common.on": "Activado",
  "common.off": "Desactivado",
  "ai.title": "✦ IA local",
  "ai.folder": "Carpeta de modelos",
  "ai.pick": "— elige un modelo .gguf —",
  "ai.start": "Iniciar IA",
  "ai.starting": "Iniciando…",
  "ai.stop": "Detener IA",
  "ai.suggest": "Sugerir respuesta",
  "ai.summarize": "Resumir conversación",
  "ai.improve": "Mejorar borrador",
  "ai.translate": "Traducir borrador (EN)",
  "ai.thinking": "Pensando…",
  "ai.use": "Usar en el borrador",
  "ai.copy": "Copiar",
  "ai.note": "La IA corre 100% local y solo sugiere — nada se envía sin que tú mandes.",
};

const en: Record<MessageKey, string> = {
  "me": "me",
  "sidebar.empty": "No conversations yet.",
  "sidebar.pair": "Pair with someone",
  "sidebar.pairTip": "Pair / add contact",
  "sidebar.settingsTip": "Settings",
  "sidebar.tabChats": "Chats",
  "sidebar.tabContacts": "Contacts",
  "sidebar.contactsEmpty": "No contacts yet. Pair with someone.",
  "sidebar.search": "Search chats and messages…",
  "sidebar.searchContacts": "Contacts",
  "sidebar.searchMessages": "Messages",
  "sidebar.searchNone": "Nothing found.",
  "chat.online": "online",
  "chat.offline": "offline",
  "chat.checking": "checking…",
  "chat.typing": "typing…",
  "chat.emptyTitle": "TaylorChat",
  "chat.empty": "Select a contact or pair with someone to start chatting.",
  "chat.placeholder": "Write a message…  (Enter sends, Shift+Enter new line)",
  "chat.send": "Send",
  "chat.attachTip": "Attach file (or drag it here)",
  "chat.aiTip": "Local AI assistant",
  "msg.copy": "Copy",
  "msg.reply": "Reply",
  "msg.deleteMine": "Delete for me",
  "msg.deleteAll": "Delete for everyone",
  "msg.deleteAllConfirm": "Delete this message for everyone? The contact won't see it either.",
  "msg.deleted": "message deleted",
  "msg.quoteGone": "original message unavailable",
  "msg.forward": "Forward",
  "msg.reactRemove": "Remove reaction",
  "fwd.title": "Forward to",
  "fwd.search": "Search chat…",
  "fwd.noLocal": "no local copy to forward",
  "chat.fileUnreadable": "⟨unreadable attachment⟩",
  "chat.openTip": "Open attachment",
  "chat.unavailable": "Attachment unavailable",
  "chat.muteTip": "Mute (no notifications)",
  "chat.unmuteTip": "Unmute notifications",
  "chat.profileTip": "View contact card",
  "profile.title": "Contact card",
  "profile.muted": "muted",
  "profile.nickname": "Nickname (what I call them)",
  "profile.phone": "Phone",
  "profile.email": "Email",
  "profile.birthday": "Birthday",
  "profile.notes": "Notes",
  "profile.keyword": "Keyword",
  "profile.keywordPh": "agreed outside the app",
  "profile.keywordHint": "Anti-impostor check: agree on a word over another channel. If it doesn't match, be careful.",
  "profile.save": "Save",
  "profile.saved": "Saved ✓",
  "profile.dangerZone": "Danger zone",
  "profile.remove": "Delete contact",
  "profile.removeConfirm": "delete this contact? The conversation and history go too.",
  "chat.clearTip": "Clear conversation",
  "chat.clearConfirm": "Delete ALL history of this conversation? This can't be undone.",
  "chat.searchTip": "Search conversation (Ctrl+F)",
  "chat.searchPh": "Search conversation…",
  "chat.loadOlder": "Load older messages",
  "chat.newMessages": "New message",
  "chat.newChatTip": "New chat with this contact",
  "chat.newChatPrompt": "Name for the new chat (e.g. Work):",
  "chat.stickerTip": "Stickers",
  "chat.emojiTip": "Emoji",
  "stickers.add": "Create sticker",
  "stickers.newPack": "New pack",
  "stickers.newPackPrompt": "Pack name:",
  "stickers.empty": "No stickers in this pack. Click Create sticker.",
  "kw.match": "Keyword matches ✓",
  "kw.mismatch": "⚠ Keyword doesn't match — confirm with the person you agreed it with",
  "kw.waiting": "Keyword set — waiting for the contact's",
  "kw.none": "No keyword (optional, verifies identity)",
  "drop.hint": "Drop to send",
  "day.today": "Today",
  "day.yesterday": "Yesterday",
  "tick.queued": "Queued — sent when the contact is reachable",
  "tick.failed": "Failed — attachment has no local copy",
  "pair.title": "Pair contact",
  "pair.mine": "My invite",
  "pair.mineHint": "This is your identity. To send the invite:",
  "pair.mineHint2":
    "ask the other person to scan this QR (or paste the code below) in their TaylorChat — then you become contacts. Share it over a channel you trust.",
  "pair.copy": "Copy",
  "pair.copied": "Copied!",
  "pair.add": "Add by invite",
  "pair.inviteLabel": "Invite (taylorchat: code or the id)",
  "pair.nickLabel": "Nickname (optional)",
  "pair.nickPh": "e.g. Maria",
  "pair.addBtn": "Add contact",
  "pair.adding": "Adding…",
  "settings.title": "Settings",
  "settings.profileName": "Your name",
  "settings.profilePhoto": "Change photo",
  "settings.theme": "Theme",
  "settings.themeSystem": "System",
  "settings.themeLight": "Light",
  "settings.themeDark": "Dark",
  "settings.themeNature": "Nature",
  "settings.themeDarkblue": "Dark blue",
  "settings.themeCalmgreen": "Calm green",
  "settings.themePastelpink": "Pastel pink",
  "settings.themePunkprincess": "PunkPrincess",
  "settings.lang": "Language",
  "settings.readReceipts": "Read receipts",
  "settings.readReceiptsHint": "Tell the contact when you've read (colored ✓✓).",
  "settings.notify": "Desktop notifications",
  "settings.notifyHint": "Alert you when a message arrives while the app is in the background. Never alerts for the conversation you're already reading.",
  "settings.notifyOsHint": "If the alert doesn't show up, check TaylorChat's notifications in your system settings (the app can't read that setting).",
  "settings.notifyPreview": "Notification preview",
  "settings.notifyPreviewHint": "Show the message text in the system notification (turn off to reveal only who, not what).",
  "settings.audit": "Audit conversation",
  "settings.auditHint":
    "Generates a code of this conversation's history. Compare it with the other device: if they match, neither side altered the content.",
  "settings.auditRun": "Generate audit code",
  "settings.auditPick": "Open a conversation to audit.",
  "settings.auditCount": "messages",
  "settings.diag": "Network diagnostics",
  "settings.diagUp": "Network up",
  "settings.diagDown": "Network down",
  "settings.diagCopy": "Copy log",
  "settings.close": "Close",

  "storage.section": "Data and storage",
  "storage.path": "Data folder",
  "storage.open": "Open",
  "storage.db": "History (database)",
  "storage.dbCounts": "{n} messages · {f} with an attachment · {c} contacts · {v} conversations",
  "storage.dbHint":
    "the messages, contacts and conversations — no cleanup on this screen deletes any of it. A peer-to-peer chat has no server to fetch anything back from: what leaves here leaves the world.",
  "storage.attachments": "Attachments sent and received",
  "storage.attachmentsCounts": "{n} files · {orphans} orphaned ({orphanSize})",
  "storage.clearOrphan": "Clear orphaned attachments",
  "storage.clearOrphanHint":
    "deletes only files no message references — leftovers from cleared conversations or removed contacts. Every attachment still shown in a conversation stays.",
  "storage.partial": "Interrupted transfers",
  "storage.partialCounts": "{n} partial files",
  "storage.clearPartialHint":
    "chunks of attachments that never finished arriving. They exist so a transfer can resume where it stopped, so clear them only when nothing is transferring. No message is affected.",
  "storage.avatars": "Contact photos (cache)",
  "storage.avatarsCounts": "{n} files · {orphans} from removed contacts ({orphanSize})",
  "storage.clearAvatarsHint":
    "deletes only photos of people no longer in your contacts. An active contact's photo stays, and so does yours.",
  "storage.backups": "Database backups",
  "storage.backupsCounts": "{n} copies · {old} old ({oldSize})",
  "storage.clearBackupsHint":
    "daily copy of your history. Deletes the old ones and ALWAYS keeps the most recent — you are never left without a safety net.",
  "storage.stickers": "Stickers",
  "storage.stickersCounts": "{n} files",
  "storage.stickersHint": "content you chose yourself — no button on this screen deletes a sticker.",
  "storage.clear": "Clear",
  "storage.confirmTitle": "Confirm cleanup",
  "storage.confirmOrphan":
    "Delete the attachments no message references? Conversations and attachments in use are kept.",
  "storage.confirmPartial":
    "Delete the chunks of unfinished transfers? A transfer in progress will start over from scratch. No message is deleted.",
  "storage.confirmAvatars":
    "Delete the cached photos of removed contacts? Photos of your current contacts are kept.",
  "storage.confirmBackups":
    "Delete the old database backups? The most recent one stays, and your current history is untouched.",
  "storage.confirmYes": "Yes, delete",
  "storage.cancel": "Cancel",
  "storage.freed": "Freed {size} ({n} files).",
  "storage.nothing": "Nothing to clean up here.",
  "storage.failed": "Cleanup failed: {e}",
  "storage.loadFailed": "Could not measure storage: {e}",

  "common.on": "On",
  "common.off": "Off",
  "ai.title": "✦ Local AI",
  "ai.folder": "Models folder",
  "ai.pick": "— pick a .gguf model —",
  "ai.start": "Start AI",
  "ai.starting": "Starting…",
  "ai.stop": "Stop AI",
  "ai.suggest": "Suggest reply",
  "ai.summarize": "Summarize chat",
  "ai.improve": "Improve draft",
  "ai.translate": "Translate draft (EN)",
  "ai.thinking": "Thinking…",
  "ai.use": "Use in draft",
  "ai.copy": "Copy",
  "ai.note": "The AI runs 100% locally and only suggests — nothing is sent unless you do.",
};

const DICTS: Record<Lang, Record<MessageKey, string>> = { pt, es, en };

export const LANGS: Lang[] = ["pt", "es", "en"];

/**
 * Todas as chaves de UI. O `tsc` já garante a PARIDADE (es/en são
 * `Record<MessageKey, string>`); esta lista existe pro teste varrer os valores, que
 * é o que o tipo não vê — string vazia satisfaz `string`.
 */
export const MESSAGE_KEYS = Object.keys(pt) as MessageKey[];

const LS_LANG = "taylorchat.lang";

export function isLang(v: unknown): v is Lang {
  return v === "pt" || v === "es" || v === "en";
}

function detectDefault(): Lang {
  const n = (globalThis.navigator?.language || "pt").slice(0, 2).toLowerCase();
  return n === "es" ? "es" : n === "en" ? "en" : "pt";
}

// `localStorage`/`navigator` são acessados com guarda porque este módulo é importado
// por código puro (lib/ui.ts) que roda no vitest em Node, sem DOM. Sem a guarda o
// import explodia no boot do teste — e o preço de "sem DOM" é só cair no pt.
function loadLang(): Lang {
  try {
    const saved = globalThis.localStorage?.getItem(LS_LANG);
    if (isLang(saved)) return saved;
  } catch {
    /* storage bloqueado ou ausente — segue pro palpite */
  }
  return detectDefault();
}

let current: Lang = loadLang();

export const getLang = (): Lang => current;

export function setLang(l: Lang) {
  current = l;
  try {
    globalThis.localStorage?.setItem(LS_LANG, l);
  } catch {
    /* não persistiu; a troca vale nesta sessão */
  }
}

/// O fallback pro `pt` sobrevive por segurança em runtime, mas com o dicionário tipado
/// ele virou inalcançável na prática: chave que não existe no `pt` nem compila.
///
/// `params` interpola `{nome}` (o painel de armazenamento precisa: "12 arquivos ·
/// 3 órfãos (4,2 MB)"). Split/join em vez de regex porque o valor pode conter
/// `$` (caminho do Windows, mensagem de erro) e `replace` daria significado
/// especial a ele.
export function t(key: MessageKey, params?: Record<string, string | number>): string {
  const raw = DICTS[current][key] ?? pt[key] ?? key;
  if (!params) return raw;
  let out = raw;
  for (const [k, v] of Object.entries(params)) out = out.split(`{${k}}`).join(String(v));
  return out;
}
