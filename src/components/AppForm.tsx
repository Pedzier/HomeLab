import { credentialsEnabled } from "../features";
import { forgetCertificates } from "../services/resolver";
import { useState } from "react";
import type { HomelabApp } from "../types/app";
import type { Credential } from "../types/credential";
export function AppForm({
  app,
  credentials,
  onSave,
  onClose,
}: {
  app: HomelabApp;
  credentials: Credential[];
  onSave: (app: HomelabApp) => Promise<void>;
  onClose: () => void;
}) {
  const [draft, setDraft] = useState(app),
    [busy, setBusy] = useState(false),
    [error, setError] = useState(""),
    [certificateNotice, setCertificateNotice] = useState("");
  const change = (key: keyof HomelabApp, value: string) =>
    setDraft({ ...draft, [key]: value });
  return (
    <div className="modal-backdrop">
      <section
        role="dialog"
        aria-modal="true"
        aria-labelledby="app-form-title"
        className="modal"
      >
        <h2 id="app-form-title">
          {app.name ? "Edytuj aplikację" : "Nowa aplikacja"}
        </h2>
        <form
          onSubmit={async (e) => {
            e.preventDefault();
            setBusy(true);
            setError("");
            try {
              await onSave({
                ...draft,
                name: draft.name.trim(),
                remoteUrl: draft.remoteUrl?.trim() || undefined,
                credentialId: draft.credentialId || undefined,
                localUrl: draft.localUrl.trim(),
              });
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
              value={draft.name}
              onChange={(e) => change("name", e.target.value)}
            />
          </label>
          <label>
            Icon
            <select
              value={draft.icon ?? "server"}
              onChange={(e) => change("icon", e.target.value)}
            >
              {[
                "house",
                "server",
                "workflow",
                "sparkles",
                "shield",
                "radio",
              ].map((i) => (
                <option key={i}>{i}</option>
              ))}
            </select>
          </label>
          <label>
            Local URL
            <input
              type="url"
              required
              placeholder="http://192.168.0.20:8123"
              value={draft.localUrl}
              onChange={(e) => change("localUrl", e.target.value)}
            />
          </label>
          <label>
            Remote / Tailscale URL
            <input
              type="url"
              placeholder="https://home-assistant.tailnet.ts.net"
              value={draft.remoteUrl ?? ""}
              onChange={(e) => change("remoteUrl", e.target.value)}
            />
          </label>
          <div className={credentialsEnabled ? "form-grid" : undefined}>
            <label>
              Connection mode
              <select
                value={draft.connectionMode}
                onChange={(e) => change("connectionMode", e.target.value)}
              >
                <option value="auto">Auto</option>
                <option value="local">Local</option>
                <option value="remote">Remote</option>
              </select>
            </label>
            {credentialsEnabled && (
              <label>
                Credential
                <select
                  value={draft.credentialId ?? ""}
                  onChange={(e) => change("credentialId", e.target.value)}
                >
                  <option value="">Brak</option>
                  {credentials.map((c) => (
                    <option key={c.id} value={c.id}>
                      {c.name}
                    </option>
                  ))}
                </select>
              </label>
            )}
          </div>
          {app.name && (
            <div className="certificate-settings">
              <p>
                Wyjątki certyfikatów HTTPS są zapisywane tylko w HomeLab. Możesz
                je usunąć, aby aplikacja ponownie zapytała o zaufanie.
              </p>
              <button
                type="button"
                disabled={busy}
                onClick={async () => {
                  setBusy(true);
                  setError("");
                  try {
                    await forgetCertificates(app.id);
                    setCertificateNotice("Usunięto zapamiętane certyfikaty.");
                  } catch (e) {
                    setError(String(e));
                  } finally {
                    setBusy(false);
                  }
                }}
              >
                Usuń zaufanie do certyfikatów
              </button>
              {certificateNotice && <p role="status">{certificateNotice}</p>}
            </div>
          )}
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
              {busy ? "Zapisywanie…" : "Zapisz aplikację"}
            </button>
          </footer>
        </form>
      </section>
    </div>
  );
}
