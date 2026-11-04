import { useState } from "react";
import type { Credential } from "../types/credential";
import { saveCredential } from "../services/credentials";
export function CredentialForm({
  credential,
  onSaved,
  onClose,
}: {
  credential?: Credential;
  onSaved: () => Promise<void>;
  onClose: () => void;
}) {
  const [name, setName] = useState(credential?.name ?? ""),
    [username, setUsername] = useState(credential?.username ?? ""),
    [password, setPassword] = useState(""),
    [notes, setNotes] = useState(credential?.notes ?? ""),
    [busy, setBusy] = useState(false),
    [error, setError] = useState("");
  return (
    <div className="modal-backdrop">
      <section
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="credential-title"
      >
        <h2 id="credential-title">
          {credential ? "Edytuj credential" : "Nowy credential"}
        </h2>
        <p className="muted">Hasło zostanie zapisane w macOS Keychain.</p>
        <form
          onSubmit={async (e) => {
            e.preventDefault();
            setBusy(true);
            setError("");
            try {
              await saveCredential(
                credential?.id ?? crypto.randomUUID(),
                name.trim(),
                username,
                notes,
                password || null,
              );
              setPassword("");
              await onSaved();
            } catch (e) {
              setError(String(e));
            } finally {
              setBusy(false);
            }
          }}
        >
          <label>
            Name
            <input
              autoFocus
              required
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </label>
          <label>
            Username
            <input
              autoComplete="off"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
            />
          </label>
          <label>
            Password
            <input
              type="password"
              autoComplete="new-password"
              required={!credential}
              value={password}
              placeholder={
                credential ? "Puste pole zachowuje obecne hasło" : ""
              }
              onChange={(e) => setPassword(e.target.value)}
            />
          </label>
          <label>
            Notes
            <textarea
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
              placeholder="Nie wpisuj tutaj haseł"
            />
          </label>
          {error && (
            <p role="alert" className="error">
              {error}
            </p>
          )}
          <footer>
            <button type="button" disabled={busy} onClick={onClose}>
              Anuluj
            </button>
            <button className="primary" disabled={busy}>
              {busy ? "Zapisywanie…" : "Zapisz w Keychain"}
            </button>
          </footer>
        </form>
      </section>
    </div>
  );
}
