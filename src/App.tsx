import { credentialsEnabled } from "./features";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState, useRef } from "react";
import {
  LayoutGrid,
  AppWindow,
  KeyRound,
  Network,
  Plus,
  RefreshCw,
  Box,
  ArrowUpRight,
  ArrowLeft,
  PanelsTopLeft,
  ExternalLink,
} from "lucide-react";
import { Dashboard } from "./pages/Dashboard";
import { Applications } from "./pages/Applications";
import { Credentials } from "./pages/Credentials";
import { Networks } from "./pages/Networks";
import { descriptions, type Page } from "./pages/Settings";
import { AppForm } from "./components/AppForm";
import { CredentialForm } from "./components/CredentialForm";
import {
  loadConfig,
  saveApp,
  deleteApp,
  reorderApps,
  type Config,
} from "./services/applications";
import {
  checkServices,
  openApp,
  closeService,
  reloadService,
  type OpenTarget,
} from "./services/resolver";
import { copyCredential, deleteCredential } from "./services/credentials";
import { trustNetwork, forgetNetwork } from "./services/network";
import { desktop } from "./services/native";
import type { HomelabApp, Resolution } from "./types/app";
import type { Credential } from "./types/credential";
import "./index.css";
const nav = [
  { page: "Dashboard", icon: LayoutGrid },
  { page: "Applications", icon: AppWindow },
  { page: "Credentials", icon: KeyRound },
  { page: "Networks", icon: Network },
] as const;
function App() {
  const [page, setPage] = useState<Page>("Dashboard"),
    [config, setConfig] = useState<Config>({
      apps: [],
      credentials: [],
      networks: [],
    }),
    [results, setResults] = useState<Record<string, Resolution>>({}),
    [current, setCurrent] = useState<string | null>(null);
  const [error, setError] = useState(""),
    [notice, setNotice] = useState(""),
    [refreshing, setRefreshing] = useState(false),
    [opening, setOpening] = useState<string | null>(null),
    [appForm, setAppForm] = useState<HomelabApp | null>(null),
    [credentialForm, setCredentialForm] = useState<Credential | true | null>(
      null,
    );
  const [activeService, setActiveService] = useState<{
    app: HomelabApp;
    result: Resolution;
  } | null>(null);
  const [returning, setReturning] = useState(false);
  const returnToDashboard = async () => {
    setReturning(true);
    try {
      await closeService();
      setActiveService(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setReturning(false);
    }
  };
  const reload = useCallback(async () => {
    const c = await loadConfig();
    c.apps.sort((a, b) => a.order - b.order);
    setConfig(c);
  }, []);
  const refresh = useCallback(async () => {
    setRefreshing(true);
    try {
      const r = await checkServices();
      setResults(r.services);
      setCurrent(r.network?.id ?? null);
      await reload();
    } catch (e) {
      setError(String(e));
    } finally {
      setRefreshing(false);
    }
  }, [reload]);
  useEffect(() => {
    if (!desktop) return;
    void Promise.resolve()
      .then(reload)
      .then(refresh)
      .catch((e) => setError(String(e)));
    const timer = setInterval(() => {
      void refresh();
    }, 30_000);
    return () => clearInterval(timer);
  }, [reload, refresh]);
  useEffect(() => {
    if (!desktop) return;
    const errorListener = listen<string>("certificate-error", (event) =>
      setError(event.payload),
    );
    const loadedListener = listen<{
      id: string;
      origin: string;
      status: Resolution["status"];
    }>("service-loaded", ({ payload }) => {
      setResults((current) => {
        const existing = current[payload.id];
        if (!existing?.url || new URL(existing.url).origin !== payload.origin)
          return current;
        return {
          ...current,
          [payload.id]: {
            ...existing,
            available: true,
            status:
              existing.status === "Offline" ? payload.status : existing.status,
            reason: "Dostęp potwierdzony przez załadowaną stronę w HomeLab.",
          },
        };
      });
    });
    const cancelListener = listen<string>("certificate-cancelled", (event) => {
      setActiveService((current) =>
        current?.app.id === event.payload ? null : current,
      );
      setNotice("Połączenie anulowane");
    });
    return () => {
      void errorListener.then((unlisten) => unlisten());
      void cancelListener.then((unlisten) => unlisten());
      void loadedListener.then((unlisten) => unlisten());
    };
  }, []);
  useEffect(() => {
    if (!notice) return;
    const timer = setTimeout(() => setNotice(""), 4000);
    return () => clearTimeout(timer);
  }, [notice]);
  const run = async (action: () => Promise<unknown>) => {
    setError("");
    try {
      await action();
      await reload();
    } catch (e) {
      setError(String(e));
    }
  };
  const addApp = () =>
    setAppForm({
      id: crypto.randomUUID(),
      name: "",
      icon: "server",
      localUrl: "",
      connectionMode: "auto",
      order: config.apps.length,
    });
  const move = (index: number, delta: number) => {
    const ids = config.apps.map((a) => a.id);
    [ids[index], ids[index + delta]] = [ids[index + delta], ids[index]];
    void run(() => reorderApps(ids));
  };
  const openingRef = useRef(false);
  const open = async (id: string, target: OpenTarget = "embedded") => {
    if (openingRef.current) return;
    openingRef.current = true;
    setOpening(id);
    setError("");
    const app = config.apps.find((a) => a.id === id);
    if (target === "embedded" && app) {
      setActiveService({
        app,
        result: results[id] ?? {
          url: null,
          status: "Offline",
          reason: "Łączenie…",
        },
      });
    }
    try {
      const r = await openApp(id, target);
      setResults((current) => ({
        ...current,
        [id]: {
          ...r,
          available:
            current[id]?.url === r.url ? current[id]?.available : r.available,
        },
      }));
      if (target === "embedded") {
        const app = config.apps.find((a) => a.id === id);
        if (app) setActiveService({ app, result: r });
      } else {
        setNotice(
          `${target === "external" ? "Przeglądarka" : "Nowe okno"} · ${r.status}`,
        );
      }
    } catch (e) {
      if (target === "embedded") setActiveService(null);
      setError(String(e));
    } finally {
      openingRef.current = false;
      setOpening(null);
    }
  };
  const reloadActiveService = async () => {
    if (!activeService || openingRef.current) return;
    const id = activeService.app.id;
    openingRef.current = true;
    setOpening(id);
    setError("");
    try {
      const result = await reloadService(id);
      setActiveService((current) =>
        current?.app.id === id ? { ...current, result } : current,
      );
    } catch (e) {
      setError(String(e));
    } finally {
      openingRef.current = false;
      setOpening(null);
    }
  };
  if (activeService)
    return (
      <div className="service-shell">
        <header className="service-toolbar">
          <button
            onClick={() => void returnToDashboard()}
            disabled={returning || !!opening}
            title="Wróć do HomeLab"
          >
            <ArrowLeft size={17} />
            HomeLab
          </button>
          <span className="service-toolbar-divider" />
          <div className="service-title">
            <strong>{activeService.app.name}</strong>
            <span>
              {opening ? "Łączenie…" : activeService.result.status} ·{" "}
              {activeService.result.url &&
                new URL(activeService.result.url).host}
            </span>
          </div>
          <div className="service-actions">
            <button
              title="Odśwież usługę"
              aria-label="Odśwież usługę"
              disabled={!!opening}
              onClick={() => void reloadActiveService()}
            >
              <RefreshCw size={17} />
            </button>
            <button
              title="Otwórz w nowym oknie HomeLab"
              aria-label="Otwórz w nowym oknie"
              disabled={!!opening}
              onClick={() => void open(activeService.app.id, "window")}
            >
              <PanelsTopLeft size={17} />
            </button>
            <button
              title="Otwórz w przeglądarce"
              aria-label="Otwórz w przeglądarce"
              disabled={!!opening}
              onClick={() => void open(activeService.app.id, "external")}
            >
              <ExternalLink size={17} />
            </button>
            {error && (
              <span className="service-error" role="alert" title={error}>
                {error}
              </span>
            )}
          </div>
        </header>
        <div className="service-placeholder" role="status">
          <RefreshCw size={24} className="spin" />
          <p>Łączenie z {activeService.app.name}…</p>
        </div>
      </div>
    );
  const online = Object.values(results).filter(
    (r) => r.available === true,
  ).length;
  return (
    <div className="shell">
      <aside className="sidebar">
        <a
          className="brand"
          href="#"
          onClick={(e) => {
            e.preventDefault();
            setPage("Dashboard");
          }}
        >
          <div className="brand-mark">
            <Box size={24} />
          </div>
          HomeLab<span className="version">MVP</span>
        </a>
        <div className="workspace-label">WORKSPACE</div>
        <nav>
          {nav
            .filter(({ page: p }) => credentialsEnabled || p !== "Credentials")
            .map(({ page: p, icon: Icon }, i) => (
              <div key={p}>
                {i === 1 && (
                  <div className="workspace-label settings-label">SETTINGS</div>
                )}
                <button
                  className={page === p ? "nav-item active" : "nav-item"}
                  onClick={() => setPage(p)}
                >
                  <Icon size={18} />
                  {p}
                  {p === "Applications" && (
                    <span className="nav-count">{config.apps.length}</span>
                  )}
                </button>
              </div>
            ))}
        </nav>
        <div className="sidebar-bottom">
          <span className="tiny-dot" />
          <div>
            <strong>Twój prywatny homelab</strong>
            <p>Na Twoim Macu. Pod Twoją kontrolą.</p>
          </div>
        </div>
      </aside>
      <div className="main">
        <header className="topbar">
          <span>
            Workspace <span className="slash">/</span> {page}
          </span>
          <span className="local-label">
            <span className="tiny-dot" />
            LOCAL FIRST
          </span>
        </header>
        <main>
          <div className="page-heading">
            <div>
              <div className="eyebrow">
                {page === "Dashboard" ? "TWOJE CENTRUM DOWODZENIA" : "SETTINGS"}
              </div>
              <h1>
                {page === "Dashboard" ? "Witaj w HomeLab" : page}
                <span className="heading-dot">.</span>
              </h1>
              <p>{descriptions[page]}</p>
            </div>
            <div className="heading-actions">
              <button
                disabled={refreshing || !desktop}
                onClick={() => void refresh()}
                title="Odśwież statusy"
              >
                <RefreshCw size={16} className={refreshing ? "spin" : ""} />
                <span>Odśwież</span>
              </button>
              {page !== "Networks" && (
                <button
                  className="primary"
                  disabled={!desktop}
                  onClick={
                    page === "Credentials"
                      ? () => setCredentialForm(true)
                      : addApp
                  }
                >
                  <Plus size={17} />
                  {page === "Credentials"
                    ? "Dodaj credential"
                    : "Dodaj aplikację"}
                </button>
              )}
            </div>
          </div>
          {!desktop && (
            <div className="info">
              Podgląd przeglądarkowy. Funkcje natywne są dostępne po
              uruchomieniu <code>npm run tauri dev</code>.
            </div>
          )}
          {error && (
            <div className="error" role="alert">
              {error}
              <button aria-label="Zamknij błąd" onClick={() => setError("")}>
                ×
              </button>
            </div>
          )}
          {page === "Dashboard" && (
            <>
              <div className="overview">
                <div>
                  <span className="overview-icon">
                    <Network size={20} />
                  </span>
                  <div>
                    <strong>
                      {config.networks.find((n) => n.id === current)?.name ??
                        "Łączenie z Twoją siecią"}
                    </strong>
                    <p>
                      {config.networks.find((n) => n.id === current)
                        ?.trustedLocal
                        ? "Trusted LAN · bezpośredni dostęp lokalny"
                        : "Automatyczny wybór połączenia"}
                    </p>
                  </div>
                </div>
                <span className="online-count">
                  <i />
                  {online} / {config.apps.length} online
                </span>
              </div>
              <Dashboard
                apps={config.apps}
                results={results}
                opening={opening}
                onOpen={(id, target) => void open(id, target)}
              />
              <div className="tip">
                <div className="tip-icon">
                  <ArrowUpRight size={19} />
                </div>
                <div>
                  <strong>Gotowy na dostęp z dowolnego miejsca?</strong>
                  <p>
                    Dodaj adresy Tailscale w ustawieniach aplikacji. HomeLab
                    wybierze właściwą trasę.
                  </p>
                </div>
                <button onClick={() => setPage("Applications")}>
                  Applications <ArrowUpRight size={15} />
                </button>
              </div>
            </>
          )}
          {page === "Applications" && (
            <Applications
              apps={config.apps}
              onEdit={setAppForm}
              onDelete={(id) => void run(() => deleteApp(id))}
              onMove={move}
            />
          )}
          {credentialsEnabled && page === "Credentials" && (
            <Credentials
              credentials={config.credentials}
              onEdit={setCredentialForm}
              onDelete={(id) => void run(() => deleteCredential(id))}
              onCopy={(id, field) =>
                void run(async () => {
                  await copyCredential(id, field);
                  setNotice("Skopiowano do schowka");
                })
              }
            />
          )}
          {page === "Networks" && (
            <Networks
              networks={config.networks}
              current={current}
              onTrust={(id, trusted) =>
                void run(async () => {
                  await trustNetwork(id, trusted);
                  await refresh();
                })
              }
              onForget={(id) => void run(() => forgetNetwork(id))}
            />
          )}
          <div className="page-footer">
            HOMELAB <span>Proste połączenie z Twoim światem.</span>
            <span>v0.1.0 · macOS</span>
          </div>
        </main>
      </div>
      {notice && (
        <div className="toast" role="status">
          {notice}
        </div>
      )}
      {appForm && (
        <AppForm
          app={appForm}
          credentials={config.credentials}
          onClose={() => setAppForm(null)}
          onSave={async (a) => {
            await saveApp(a);
            setAppForm(null);
            await reload();
            void refresh();
          }}
        />
      )}
      {credentialsEnabled && credentialForm && (
        <CredentialForm
          credential={credentialForm === true ? undefined : credentialForm}
          onClose={() => setCredentialForm(null)}
          onSaved={async () => {
            setCredentialForm(null);
            await reload();
          }}
        />
      )}
    </div>
  );
}
export default App;
