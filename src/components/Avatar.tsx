import { convertFileSrc } from "@tauri-apps/api/core";
import { avatarColor } from "../lib/ui";

interface Props {
  nodeId: string;
  name: string;
  avatar?: string | null;
  size?: number;
}

/// Avatar do contato: foto (se houver cache) ou a inicial com cor estável.
export function Avatar({ nodeId, name, avatar, size = 36 }: Props) {
  const px = { width: size, height: size, flex: `0 0 ${size}px` };
  if (avatar) {
    return <img className="avatar-img" style={px} src={convertFileSrc(avatar)} alt={name} />;
  }
  return (
    <span className="avatar" style={{ ...px, background: avatarColor(nodeId) }}>
      {(name || nodeId).slice(0, 1).toUpperCase()}
    </span>
  );
}
