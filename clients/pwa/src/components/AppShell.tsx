import type { ReactNode } from "react";
import { NavLink, Link, useLocation } from "react-router-dom";
import { cn } from "../lib/utils";
import {
  FolderIcon,
  SettingsIcon,
  TasksIcon,
} from "./icons";
import { ConnectionBadge } from "./ui";

const navItems = [
  { to: "/", label: "Tasks", icon: TasksIcon, end: true },
  { to: "/projects", label: "Projects", icon: FolderIcon, end: false },
  { to: "/settings", label: "Settings", icon: SettingsIcon, end: false },
];

export function AppShell({
  online,
  children,
}: {
  online: boolean | null;
  children: ReactNode;
}) {
  const { pathname } = useLocation();
  const isTaskDetail = /^\/tasks\/[^/]+$/.test(pathname);

  return (
    <div className="min-h-dvh">
      <aside className="fixed inset-y-0 left-0 hidden w-60 flex-col border-r border-hueso/10 bg-sumi md:flex">
        <div className="px-6 pb-8 pt-7">
          <Link to="/" aria-label="ONI home">
            <img
              src="/brand/oni-lockup-compact.svg"
              alt="ONI"
              className="h-12 w-auto"
            />
          </Link>
        </div>

        <nav className="flex flex-col gap-1 px-3" aria-label="Main">
          {navItems.map((item) => {
            const Icon = item.icon;
            return (
              <NavLink
                key={item.to}
                to={item.to}
                end={item.end}
                className={({ isActive }) =>
                  cn(
                    "relative flex h-11 items-center gap-3 rounded-lg px-3 text-sm transition-colors",
                    isActive
                      ? "bg-hueso/5 text-hueso"
                      : "text-ceniza hover:bg-hueso/5 hover:text-hueso",
                  )
                }
              >
                {({ isActive }) => (
                  <>
                    {isActive ? (
                      <span className="absolute inset-y-2 left-0 w-0.5 rounded-full bg-kintsugi" />
                    ) : null}
                    <Icon className="h-[18px] w-[18px]" />
                    {item.label}
                  </>
                )}
              </NavLink>
            );
          })}
        </nav>

        <div className="mt-auto space-y-4 px-6 pb-6">
          <ConnectionBadge online={online} />
          <div className="h-px bg-hueso/10" />
          <p className="font-mono text-[9px] leading-relaxed tracking-[0.16em] text-kintsugi/80">
            AGENTIC
            <br />
            DEVELOPMENT
            <br />
            ENVIRONMENT
          </p>
          <p className="font-mono text-[10px] text-ceniza/60">v0.1.0</p>
        </div>
      </aside>

      <div className="md:pl-60">
        <header className="sticky top-0 z-30 border-b border-hueso/10 bg-sumi md:hidden">
          <div className="flex h-16 items-center justify-between px-4">
            <Link to="/" aria-label="ONI home">
              <img
                src="/brand/oni-lockup-compact.svg"
                alt="ONI"
                className="h-9 w-auto"
              />
            </Link>
            <ConnectionBadge online={online} />
          </div>
        </header>

        <main
          className={cn(
            "mx-auto w-full max-w-4xl px-4 pt-6 md:px-8 md:pt-10",
            isTaskDetail ? "pb-36 md:pb-16" : "pb-28 md:pb-16",
          )}
        >
          {children}
        </main>
      </div>

      {!isTaskDetail ? (
        <nav
          className="fixed inset-x-0 bottom-0 z-30 border-t border-hueso/10 bg-sumi/95 md:hidden"
          aria-label="Main"
        >
          <div className="safe-bottom flex h-16 items-stretch">
            {navItems.map((item) => {
              const Icon = item.icon;
              return (
                <NavLink
                  key={item.to}
                  to={item.to}
                  end={item.end}
                  className={({ isActive }) =>
                    cn(
                      "flex flex-1 flex-col items-center justify-center gap-1 text-[10px] font-medium transition-colors",
                      isActive ? "text-kintsugi" : "text-ceniza",
                    )
                  }
                >
                  <Icon className="h-5 w-5" />
                  {item.label}
                </NavLink>
              );
            })}
          </div>
        </nav>
      ) : null}
    </div>
  );
}
