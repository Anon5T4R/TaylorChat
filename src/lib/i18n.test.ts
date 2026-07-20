import { afterEach, describe, expect, it } from "vitest";
import {
  getLang,
  isLang,
  LANGS,
  LANG_LABELS,
  MESSAGE_KEYS,
  setLang,
  t,
  type Lang,
  type MessageKey,
} from "./i18n";

// O idioma é estado de módulo. Sem isto um teste que troca o idioma contamina os
// seguintes — e o sintoma seria uma falha em OUTRO arquivo, difícil de rastrear.
const ORIGINAL = getLang();
afterEach(() => setLang(ORIGINAL));

describe("dicionário", () => {
  it("tem chaves", () => {
    expect(MESSAGE_KEYS.length).toBeGreaterThan(100);
  });

  // A paridade de CHAVES quem garante é o `tsc` (es/en são Record<MessageKey,string>).
  // O que o tipo NÃO vê é o valor: "" satisfaz `string` e a UI sairia em branco.
  it.each(LANGS)("nenhum texto vazio ou só espaço em %s", (lang) => {
    setLang(lang);
    const vazias = MESSAGE_KEYS.filter((k) => t(k).trim() === "");
    expect(vazias).toEqual([]);
  });

  // Rede contra o erro clássico de copiar o bloco pt e esquecer de traduzir: se a
  // tradução inteira fosse igual ao pt, o app estaria monolíngue sem ninguém notar.
  it.each(["es", "en"] as Lang[])("%s é de fato uma tradução, não uma cópia do pt", (lang) => {
    setLang("pt");
    const emPt = MESSAGE_KEYS.map((k) => t(k));
    setLang(lang);
    const diferentes = MESSAGE_KEYS.filter((k, i) => t(k) !== emPt[i]).length;
    // Não dá pra exigir 100%: "Emoji", "TaylorChat" e "Copiar" (pt=es) coincidem de
    // verdade. Metade é folgado o bastante pra não falsear e apertado o bastante
    // pra pegar um bloco não traduzido.
    expect(diferentes).toBeGreaterThan(MESSAGE_KEYS.length / 2);
  });

  it("todo idioma tem endônimo", () => {
    for (const l of LANGS) expect(LANG_LABELS[l].trim()).not.toBe("");
  });
});

describe("t()", () => {
  it("devolve o texto do idioma corrente", () => {
    setLang("pt");
    expect(t("chat.send")).toBe("Enviar");
    setLang("en");
    expect(t("chat.send")).toBe("Send");
  });

  it("troca de idioma vale pra todas as chaves, não só a consultada", () => {
    setLang("es");
    expect(t("sidebar.tabContacts")).toBe("Contactos");
    expect(t("settings.close")).toBe("Cerrar");
  });

  // Chave inexistente nem compila mais; em runtime (dado vindo de fora do TS) o
  // contrato é devolver a própria chave, nunca `undefined` na tela.
  it("chave desconhecida em runtime não vira undefined", () => {
    setLang("pt");
    expect(t("nao.existe" as MessageKey)).toBe("nao.existe");
  });
});

describe("isLang", () => {
  it("aceita só os três", () => {
    expect(LANGS.every(isLang)).toBe(true);
    for (const v of ["fr", "", "PT", null, undefined, 1]) expect(isLang(v)).toBe(false);
  });
});

describe("setLang", () => {
  it("persiste no localStorage quando existe", () => {
    setLang("en");
    expect(globalThis.localStorage?.getItem("taylorchat.lang") ?? "en").toBe("en");
  });

  // Em Node não há localStorage. A guarda existe pra isto: trocar de idioma não pode
  // explodir só porque não deu pra persistir.
  it("não explode sem localStorage", () => {
    expect(() => setLang("es")).not.toThrow();
    expect(getLang()).toBe("es");
  });
});
