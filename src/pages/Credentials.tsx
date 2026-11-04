import { KeyRound, Pencil, Trash2, Copy } from "lucide-react";
import type { Credential } from "../types/credential";
export function Credentials({
  credentials,
  onEdit,
  onDelete,
  onCopy,
}: {
  credentials: Credential[];
  onEdit: (c: Credential) => void;
  onDelete: (id: string) => void;
  onCopy: (id: string, field: "username" | "password") => void;
}) {
  return (
    <>
      <div className="info">
        Hasła są przechowywane w macOS Keychain. Logujesz się bezpośrednio w
        oknie usługi; sesja pozostaje zapamiętana.
      </div>
      <div className="panel">
        {credentials.map((c) => (
          <div className="list-row" key={c.id}>
            <div className="small-icon">
              <KeyRound size={22} />
            </div>
            <div className="row-body">
              <strong>{c.name}</strong>
              <p>{c.username || "Bez nazwy użytkownika"}</p>
              {c.notes && <p>{c.notes}</p>}
            </div>
            <div className="row-actions">
              <button
                title="Copy username"
                onClick={() => onCopy(c.id, "username")}
              >
                <Copy size={14} /> Login
              </button>
              <button
                title="Copy password"
                onClick={() => onCopy(c.id, "password")}
              >
                <Copy size={14} /> Hasło
              </button>
              <button aria-label={`Edytuj ${c.name}`} onClick={() => onEdit(c)}>
                <Pencil size={16} />
              </button>
              <button
                aria-label={`Usuń ${c.name}`}
                onClick={() => onDelete(c.id)}
              >
                <Trash2 size={16} />
              </button>
            </div>
          </div>
        ))}
        {!credentials.length && (
          <div className="empty">
            <KeyRound size={30} />
            <h3>Twoje dane logowania, pod ręką</h3>
            <p>Dodaj credential i przypisz go do wybranej aplikacji.</p>
          </div>
        )}
      </div>
      <p className="muted footnote">
        Skopiowane hasło pozostaje w schowku systemowym do jego nadpisania.
      </p>
    </>
  );
}
