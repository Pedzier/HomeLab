import { AppTile } from "../components/AppTile";
import type { OpenTarget } from "../services/resolver";
import type { HomelabApp, Resolution } from "../types/app";
export function Dashboard({
  apps,
  results,
  opening,
  onOpen,
}: {
  apps: HomelabApp[];
  results: Record<string, Resolution>;
  opening: string | null;
  onOpen: (id: string, target?: OpenTarget) => void;
}) {
  return (
    <>
      <div className="section-caption">
        <span>TWOJE USŁUGI</span>
        <span>{apps.length} aplikacje</span>
      </div>
      <div className="tiles">
        {apps.map((app) => (
          <AppTile
            key={app.id}
            app={app}
            result={results[app.id]}
            busy={opening === app.id}
            onOpen={(target) => onOpen(app.id, target)}
          />
        ))}
      </div>
      {!apps.length && (
        <div className="empty">
          Twoje centrum dowodzenia jest gotowe. Dodaj pierwszą aplikację.
        </div>
      )}
      <div className="dashboard-note">
        <span className="tiny-dot" />
        Lokalnie, kiedy jesteś w domu. Przez Tailscale, kiedy jesteś poza nim.
      </div>
    </>
  );
}
