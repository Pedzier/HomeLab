import { ArrowDown, ArrowUp, Pencil, Trash2 } from "lucide-react";
import { AppIcon } from "../components/AppTile";
import type { HomelabApp } from "../types/app";
export function Applications({
  apps,
  onEdit,
  onDelete,
  onMove,
}: {
  apps: HomelabApp[];
  onEdit: (app: HomelabApp) => void;
  onDelete: (id: string) => void;
  onMove: (index: number, delta: number) => void;
}) {
  return (
    <div className="panel">
      {apps.map((a, i) => (
        <div className="list-row" key={a.id}>
          <div className="small-icon">
            <AppIcon icon={a.icon} />
          </div>
          <div className="row-body">
            <strong>{a.name}</strong>
            <p>{a.localUrl}</p>
            <p>{a.remoteUrl ?? "Nie skonfigurowano adresu Tailscale"}</p>
          </div>
          <span className="mode">{a.connectionMode}</span>
          <div className="row-actions">
            <button
              aria-label={`Przenieś ${a.name} wyżej`}
              disabled={i === 0}
              onClick={() => onMove(i, -1)}
            >
              <ArrowUp size={16} />
            </button>
            <button
              aria-label={`Przenieś ${a.name} niżej`}
              disabled={i === apps.length - 1}
              onClick={() => onMove(i, 1)}
            >
              <ArrowDown size={16} />
            </button>
            <button aria-label={`Edytuj ${a.name}`} onClick={() => onEdit(a)}>
              <Pencil size={16} />
            </button>
            <button
              aria-label={`Usuń ${a.name}`}
              onClick={() => onDelete(a.id)}
            >
              <Trash2 size={16} />
            </button>
          </div>
        </div>
      ))}
      {!apps.length && (
        <div className="empty">Dodaj swoją pierwszą usługę.</div>
      )}
    </div>
  );
}
