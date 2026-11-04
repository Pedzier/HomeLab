import {
  House,
  Server,
  Workflow,
  Sparkles,
  Shield,
  Radio,
  ArrowUpRight,
  ExternalLink,
  PanelsTopLeft,
} from "lucide-react";
import type { OpenTarget } from "../services/resolver";
import type { HomelabApp, Resolution } from "../types/app";
import { StatusBadge } from "./StatusBadge";
const icons = {
  house: House,
  server: Server,
  workflow: Workflow,
  sparkles: Sparkles,
  shield: Shield,
  radio: Radio,
};
export function AppIcon({ icon }: { icon?: string }) {
  const Icon = icons[icon as keyof typeof icons] ?? Server;
  return <Icon size={24} strokeWidth={1.6} />;
}
export function AppTile({
  app,
  result,
  busy,
  onOpen,
}: {
  app: HomelabApp;
  result?: Resolution;
  busy: boolean;
  onOpen: (target?: OpenTarget) => void;
}) {
  return (
    <article className="tile">
      <button
        className="tile-main"
        onClick={() => onOpen()}
        disabled={busy}
        aria-label={`Otwórz ${app.name}`}
      >
        <div className={`app-icon icon-${app.icon}`}>
          <AppIcon icon={app.icon} />
        </div>
        <ArrowUpRight className="tile-arrow" size={19} />
        <h3>{app.name}</h3>
        <p>
          {busy
            ? "Otwieranie…"
            : result?.url
              ? new URL(result.url).host
              : new URL(app.localUrl).host}
        </p>
        <StatusBadge
          status={result?.status}
          available={result?.available}
          reason={result?.reason}
        />
      </button>
      <button
        className="external"
        title="Otwórz w nowym oknie HomeLab"
        aria-label={`Otwórz ${app.name} w nowym oknie`}
        onClick={() => onOpen("window")}
        disabled={busy}
      >
        <PanelsTopLeft size={15} />
      </button>
      <button
        className="external external-browser"
        title="Otwórz w przeglądarce"
        aria-label={`Otwórz ${app.name} w przeglądarce`}
        onClick={() => onOpen("external")}
        disabled={busy}
      >
        <ExternalLink size={15} />
      </button>
    </article>
  );
}
