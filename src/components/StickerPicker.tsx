import { useEffect, useMemo, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import * as api from "../lib/api";
import { t } from "../lib/i18n";

interface Props {
  onPick: (path: string) => void;
  onClose: () => void;
}

const DEFAULT_PACK = "Geral";

export function StickerPicker({ onPick, onClose }: Props) {
  const [stickers, setStickers] = useState<api.Sticker[]>([]);
  const [pack, setPack] = useState<string>(DEFAULT_PACK);

  const reload = () => api.stickersList().then(setStickers).catch(() => {});
  useEffect(() => {
    reload();
  }, []);

  const packs = useMemo(() => {
    const set = new Set<string>([DEFAULT_PACK, ...stickers.map((s) => s.pack)]);
    return [...set];
  }, [stickers]);

  const current = stickers.filter((s) => s.pack === pack);

  const add = async () => {
    try {
      await api.pickAndCreateSticker(pack);
      reload();
    } catch {
      /* cancelado / sem imagem */
    }
  };
  const newPack = () => {
    const name = window.prompt(t("stickers.newPackPrompt"), "");
    if (name && name.trim()) setPack(name.trim());
  };

  return (
    <div className="sticker-picker" onClick={(e) => e.stopPropagation()}>
      <div className="sticker-tabs">
        {packs.map((p) => (
          <button
            key={p}
            className={`sticker-tab ${p === pack ? "is-active" : ""}`}
            onClick={() => setPack(p)}
          >
            {p}
          </button>
        ))}
        <button className="sticker-tab" title={t("stickers.newPack")} onClick={newPack}>
          ＋
        </button>
      </div>
      <div className="sticker-grid">
        {current.length === 0 && <div className="sticker-empty">{t("stickers.empty")}</div>}
        {current.map((s) => (
          <button key={s.path} className="sticker-cell" onClick={() => onPick(s.path)}>
            <img src={convertFileSrc(s.path)} alt="sticker" />
          </button>
        ))}
      </div>
      <div className="sticker-actions">
        <button className="btn" onClick={add}>
          ＋ {t("stickers.add")}
        </button>
        <button className="btn" onClick={onClose}>
          {t("settings.close")}
        </button>
      </div>
    </div>
  );
}
