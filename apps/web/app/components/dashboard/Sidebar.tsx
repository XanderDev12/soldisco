import type {
  KeyboardEvent,
  PointerEvent as ReactPointerEvent,
} from "react";
import { navGroups } from "./navigation";
import type {
  DashboardView,
  LayoutKey,
  LayoutPreferences,
} from "./types";

type LayoutLimits = Record<LayoutKey, { min: number; max: number }>;

export interface SidebarProps {
  activeView: DashboardView;
  streamRunning: boolean;
  sideNavOpen: boolean;
  layout: LayoutPreferences;
  layoutLimits: LayoutLimits;
  defaultLayout: LayoutPreferences;
  onOpenView: (view: DashboardView) => void;
  onCloseMobileNav: () => void;
  onBeginResize: (
    key: LayoutKey,
    event: ReactPointerEvent<HTMLButtonElement>,
  ) => void;
  onResizeKey: (
    key: LayoutKey,
    event: KeyboardEvent<HTMLButtonElement>,
  ) => void;
  onSetLayoutValue: (key: LayoutKey, value: number) => void;
}

export function Sidebar({
  activeView,
  streamRunning,
  sideNavOpen,
  layout,
  layoutLimits,
  defaultLayout,
  onOpenView,
  onCloseMobileNav,
  onBeginResize,
  onResizeKey,
  onSetLayoutValue,
}: SidebarProps) {
  return (
    <>
      <aside
        id="navigation-panel"
        className={`sidebar ${sideNavOpen ? "sidebar--open" : ""}`}
      >
        <div className="brand">
          <span className="brand__mark">
            <i />
            <i />
            <i />
          </span>
          <div>
            <strong>SOLDISCO</strong>
            <span>Discovery console</span>
          </div>
        </div>

        <nav className="sidebar__nav" aria-label="Primary navigation">
          {navGroups.map((group) => (
            <div className="nav-group" key={group.label}>
              <p>{group.label}</p>
              {group.items.map((item) => (
                <button
                  type="button"
                  id={`nav-${item.id}`}
                  className={`nav-item ${activeView === item.id ? "nav-item--active" : ""}`}
                  key={item.name}
                  aria-controls="dashboard-view"
                  aria-current={activeView === item.id ? "page" : undefined}
                  onClick={() => onOpenView(item.id)}
                >
                  <span className="nav-item__icon" aria-hidden="true">
                    {item.icon}
                  </span>
                  <span>{item.name}</span>
                </button>
              ))}
            </div>
          ))}
        </nav>

        <div className="system-card">
          <div className="system-card__head">
            <span>
              <i className="offline-dot" />
              Services disconnected
            </span>
            <strong>—</strong>
          </div>
          <div className="system-card__row">
            <span>RPC</span>
            <span>Not connected</span>
          </div>
          <div className="system-card__row">
            <span>Stream</span>
            <span>{streamRunning ? "Active · no source" : "Stopped"}</span>
          </div>
        </div>

        <button
          type="button"
          className="resize-handle resize-handle--sidebar"
          role="separator"
          aria-label="Resize navigation"
          aria-controls="navigation-panel"
          aria-orientation="vertical"
          aria-valuemin={layoutLimits.sidebar.min}
          aria-valuemax={layoutLimits.sidebar.max}
          aria-valuenow={layout.sidebar}
          onPointerDown={(event) => onBeginResize("sidebar", event)}
          onKeyDown={(event) => onResizeKey("sidebar", event)}
          onDoubleClick={() =>
            onSetLayoutValue("sidebar", defaultLayout.sidebar)
          }
          title="Drag to resize · Double-click to reset"
        />
      </aside>

      {sideNavOpen && (
        <button
          type="button"
          className="mobile-scrim"
          onClick={onCloseMobileNav}
          aria-label="Close navigation"
        />
      )}
    </>
  );
}
