import { afterEach, describe, expect, it, vi } from "vitest";
import { getLang, setLang } from "./i18n";
import {
  avatarColor,
  dayLabel,
  formatSize,
  linkParts,
  shortId,
  shortTime,
  splitConvo,
  stateGlyph,
} from "./ui";

const ORIGINAL = getLang();
afterEach(() => {
  setLang(ORIGINAL);
  vi.useRealTimers();
});

describe("splitConvo", () => {
  // A chave de conversa é `node` ou `node#thread`. Essa distinção é o que faz a
  // notificação da thread extra NÃO ser calada pela principal aberta (regra do B13),
  // então errar aqui volta a engolir mensagem.
  it("conversa principal não tem thread", () => {
    expect(splitConvo("abc")).toEqual({ node: "abc", thread: "" });
  });

  it("separa a thread extra", () => {
    expect(splitConvo("abc#trabalho")).toEqual({ node: "abc", thread: "trabalho" });
  });

  it("só o PRIMEIRO # separa — nome de thread pode conter #", () => {
    expect(splitConvo("abc#a#b")).toEqual({ node: "abc", thread: "a#b" });
  });

  it("thread vazia com # no fim não vira principal", () => {
    expect(splitConvo("abc#")).toEqual({ node: "abc", thread: "" });
  });
});

describe("shortId", () => {
  it("id curto passa inteiro", () => {
    expect(shortId("abc")).toBe("abc");
    expect(shortId("a".repeat(12))).toBe("a".repeat(12));
  });

  it("id longo é encurtado com reticências", () => {
    const id = "0123456789abcdef0123";
    expect(shortId(id)).toBe("012345…0123");
  });
});

describe("avatarColor", () => {
  it("é estável — o mesmo contato tem sempre a mesma cor", () => {
    expect(avatarColor("nodeX")).toBe(avatarColor("nodeX"));
  });

  it("separa contatos diferentes", () => {
    expect(avatarColor("nodeX")).not.toBe(avatarColor("nodeY"));
  });

  it("sempre devolve HSL com matiz dentro de 0..359", () => {
    for (const id of ["", "a", "contato-longo-pra-caramba", "🙂"]) {
      const m = /^hsl\((\d+) 55% 42%\)$/.exec(avatarColor(id));
      expect(m).not.toBeNull();
      expect(Number(m![1])).toBeLessThan(360);
    }
  });
});

describe("formatSize", () => {
  it("bytes crus abaixo de 1 KB", () => {
    expect(formatSize(0)).toBe("0 B");
    expect(formatSize(1023)).toBe("1023 B");
  });

  it("vira KB no limite", () => {
    expect(formatSize(1024)).toBe("1.0 KB");
  });

  it("vira MB no limite", () => {
    expect(formatSize(1024 * 1024)).toBe("1.0 MB");
    expect(formatSize(5 * 1024 * 1024)).toBe("5.0 MB");
  });
});

describe("stateGlyph", () => {
  // Mensagem que CHEGOU não pode mostrar ✓✓ — pareceria confirmação do outro lado.
  it("mensagem recebida não tem glifo, seja qual for o estado", () => {
    expect(stateGlyph({ direction: "in", state: "read" })).toBe("");
    expect(stateGlyph({ direction: "in", state: "queued" })).toBe("");
  });

  it("estados de saída", () => {
    expect(stateGlyph({ direction: "out", state: "queued" })).toBe("🕒");
    expect(stateGlyph({ direction: "out", state: "failed" })).toBe("⚠");
    expect(stateGlyph({ direction: "out", state: "sent" })).toBe("✓");
    expect(stateGlyph({ direction: "out", state: "delivered" })).toBe("✓✓");
    expect(stateGlyph({ direction: "out", state: "read" })).toBe("✓✓");
  });

  it("estado desconhecido de versão futura não quebra a bolha", () => {
    expect(stateGlyph({ direction: "out", state: "coisa-nova" as never })).toBe("");
  });
});

describe("linkParts", () => {
  it("texto sem URL vira um pedaço só", () => {
    expect(linkParts("oi tudo bem")).toEqual([{ url: false, text: "oi tudo bem" }]);
  });

  it("isola a URL do texto ao redor", () => {
    expect(linkParts("veja https://ex.com/a agora")).toEqual([
      { url: false, text: "veja " },
      { url: true, text: "https://ex.com/a" },
      { url: false, text: " agora" },
    ]);
  });

  it("pega mais de uma URL", () => {
    const urls = linkParts("http://a.com e https://b.com").filter((p) => p.url);
    expect(urls.map((p) => p.text)).toEqual(["http://a.com", "https://b.com"]);
  });

  // O render não usa HTML cru; o que importa é que esquema perigoso NÃO seja
  // promovido a link clicável.
  it("javascript: e outros esquemas não viram link", () => {
    for (const s of ["javascript:alert(1)", "data:text/html,x", "file:///c:/x"]) {
      expect(linkParts(s).some((p) => p.url)).toBe(false);
    }
  });

  it("marcação HTML no texto continua sendo texto", () => {
    expect(linkParts("<script>x</script>")).toEqual([
      { url: false, text: "<script>x</script>" },
    ]);
  });
});

describe("dayLabel", () => {
  it("hoje e ontem saem no idioma da UI", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 6, 19, 15, 0, 0));
    const hoje = new Date(2026, 6, 19, 9, 0, 0).getTime();
    const ontem = new Date(2026, 6, 18, 23, 30, 0).getTime();

    setLang("pt");
    expect(dayLabel(hoje)).toBe("Hoje");
    expect(dayLabel(ontem)).toBe("Ontem");

    setLang("en");
    expect(dayLabel(hoje)).toBe("Today");
    expect(dayLabel(ontem)).toBe("Yesterday");

    setLang("es");
    expect(dayLabel(hoje)).toBe("Hoy");
    expect(dayLabel(ontem)).toBe("Ayer");
  });

  // O corte é por DIA CIVIL, não por 24h: 23:30 de ontem e 00:10 de hoje estão a 40
  // minutos e mesmo assim são rótulos diferentes.
  it("corta por dia civil, não por janela de 24h", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 6, 19, 0, 50, 0));
    setLang("pt");
    expect(dayLabel(new Date(2026, 6, 19, 0, 10, 0).getTime())).toBe("Hoje");
    expect(dayLabel(new Date(2026, 6, 18, 23, 30, 0).getTime())).toBe("Ontem");
  });

  it("dia mais antigo vira data, não rótulo", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 6, 19, 12, 0, 0));
    setLang("pt");
    const antigo = dayLabel(new Date(2026, 0, 2, 12, 0, 0).getTime());
    expect(antigo).not.toBe("Hoje");
    expect(antigo).not.toBe("Ontem");
    expect(antigo).toMatch(/2026/);
  });
});

describe("shortTime", () => {
  it("ts zerado (mensagem sem hora) não imprime nada", () => {
    expect(shortTime(0)).toBe("");
  });

  it("hoje mostra hora; outro dia mostra data", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 6, 19, 15, 0, 0));
    expect(shortTime(new Date(2026, 6, 19, 9, 5, 0).getTime())).toMatch(/\d{1,2}[:.]\d{2}/);
    const outro = shortTime(new Date(2026, 5, 1, 9, 5, 0).getTime());
    expect(outro).toMatch(/\d{2}\D\d{2}/);
  });
});
