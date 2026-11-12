import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { api, getToken, setToken } from "../lib/api";
import { copyText, formatDuration } from "../lib/utils";
import {
  CheckIcon,
  CopyIcon,
  DownloadIcon,
  TerminalIcon,
} from "../components/icons";
import {
  Button,
  ConnectionBadge,
  Field,
  inputClass,
  toast,
} from "../components/ui";

interface BeforeInstallPromptEvent extends Event {
  prompt: () => Promise<void>;
  userChoice: Promise<{ outcome: "accepted" | "dismissed" }>;
}

export default function SettingsPage() {
  const queryClient = useQueryClient();
  const [token, setTokenValue] = useState(getToken());
  const [installPrompt, setInstallPrompt] =
    useState<BeforeInstallPromptEvent | null>(null);
  const [standalone, setStandalone] = useState(false);

  const health = useQuery({
    queryKey: ["health"],
    queryFn: api.health,
    retry: false,
    refetchInterval: 20_000,
  });

  useEffect(() => {
    setStandalone(window.matchMedia("(display-mode: standalone)").matches);
    const handler = (event: Event) => {
      event.preventDefault();
      setInstallPrompt(event as BeforeInstallPromptEvent);
    };
    window.addEventListener("beforeinstallprompt", handler);
    return () => window.removeEventListener("beforeinstallprompt", handler);
  }, []);

  const saveToken = () => {
    setToken(token);
    window.dispatchEvent(new Event("oni:token-changed"));
    void queryClient.invalidateQueries();
    toast(token.trim() ? "Token saved" : "Token cleared", "success");
  };

  const posixSnippet =
    "ONI_HOST=0.0.0.0 ONI_TOKEN=change-me cargo run --manifest-path oni-daemon/Cargo.toml";
  const windowsSnippet =
    '$env:ONI_HOST="0.0.0.0"; $env:ONI_TOKEN="change-me"; cargo run --manifest-path oni-daemon/Cargo.toml';

  const install = async () => {
    if (!installPrompt) return;
    await installPrompt.prompt();
    const choice = await installPrompt.userChoice;
    if (choice.outcome === "accepted") {
      toast("ONI installed", "success");
    }
    setInstallPrompt(null);
  };

  return (
    <div className="animate-rise">
      <header className="mb-8">
        <p className="descriptor">Daemon</p>
        <h1 className="mt-3 text-[28px] font-semibold leading-none tracking-[-0.03em] text-hueso md:text-[32px]">
          Settings
        </h1>
        <p className="mt-2 text-sm text-ceniza">
          Connection, access and installation.
        </p>
      </header>

      <div className="space-y-4">
        <section className="rounded-xl border border-hueso/10 bg-raised p-4">
          <div className="flex items-center justify-between gap-3">
            <h2 className="font-mono text-[10px] uppercase tracking-[0.14em] text-hueso">
              Connection
            </h2>
            <ConnectionBadge
              online={health.isSuccess ? true : health.isError ? false : null}
            />
          </div>

          {health.data ? (
            <>
              <dl className="mt-4 grid grid-cols-2 gap-px overflow-hidden rounded-lg border border-hueso/10 bg-hueso/10 sm:grid-cols-4">
                <Stat label="Version" value={`v${health.data.version}`} />
                <Stat
                  label="Uptime"
                  value={formatDuration(health.data.uptimeSecs)}
                />
                <Stat label="Projects" value={String(health.data.projects)} />
                <Stat label="Tasks" value={String(health.data.tasks)} />
              </dl>
              <div className="mt-4 flex items-start gap-2">
                <TerminalIcon className="mt-0.5 h-3.5 w-3.5 shrink-0 text-ceniza" />
                <div className="min-w-0">
                  <p className="font-mono text-[9px] uppercase tracking-[0.14em] text-ceniza">
                    Data directory
                  </p>
                  <p className="mt-1 break-all font-mono text-[10px] text-hueso/80">
                    {health.data.dataDir}
                  </p>
                </div>
              </div>
            </>
          ) : health.isError ? (
            <div className="mt-4 rounded-lg border border-vermilion/30 bg-vermilion/5 px-3 py-2.5">
              <p className="text-xs leading-relaxed text-vermilion">
                Cannot reach the daemon: {(health.error as Error).message}
              </p>
              <p className="mt-1 text-[11px] leading-relaxed text-vermilion/80">
                Start it with <code>cargo run --manifest-path oni-daemon/Cargo.toml</code>{" "}
                or check the token below.
              </p>
            </div>
          ) : (
            <p className="mt-4 font-mono text-xs text-ceniza">
              connecting…
            </p>
          )}
        </section>

        <section className="rounded-xl border border-hueso/10 bg-raised p-4">
          <h2 className="font-mono text-[10px] uppercase tracking-[0.14em] text-hueso">
            Access token
          </h2>
          <p className="mt-2 text-pretty text-xs leading-relaxed text-ceniza">
            Required when the daemon is exposed to your LAN with{" "}
            <code className="font-mono text-hueso/80">ONI_TOKEN</code>. The
            token is stored in this browser only.
          </p>
          <div className="mt-4 space-y-3">
            <Field label="Token" htmlFor="oni-token">
              <input
                id="oni-token"
                type="password"
                autoComplete="off"
                className={`${inputClass} font-mono text-[13px]`}
                value={token}
                onChange={(event) => setTokenValue(event.target.value)}
                placeholder="paste the daemon token"
              />
            </Field>
            <div className="flex items-center gap-2">
              <Button variant="primary" size="sm" onClick={saveToken}>
                Save token
              </Button>
              {health.data?.authRequired ? (
                <span className="font-mono text-[9px] uppercase tracking-[0.12em] text-kintsugi">
                  required
                </span>
              ) : (
                <span className="font-mono text-[9px] uppercase tracking-[0.12em] text-ceniza">
                  not required by this daemon
                </span>
              )}
            </div>
          </div>
        </section>

        <section className="rounded-xl border border-hueso/10 bg-raised p-4">
          <h2 className="font-mono text-[10px] uppercase tracking-[0.14em] text-hueso">
            Phone access
          </h2>
          <p className="mt-2 text-pretty text-xs leading-relaxed text-ceniza">
            Oni binds to localhost by default. To supervise from your phone,
            bind it to your LAN with a token and open the machine&apos;s IP on
            the same network. Prefer Tailscale over exposing a port.
          </p>
          <div className="mt-3 space-y-2">
            <CodeSnippet label="macOS / Linux" code={posixSnippet} />
            <CodeSnippet label="Windows PowerShell" code={windowsSnippet} />
          </div>
          <p className="mt-3 text-[11px] leading-relaxed text-ceniza/80">
            Plain HTTP works as a web app. Installing the PWA (service worker)
            requires HTTPS: use Tailscale Serve or a TLS reverse proxy.
          </p>
          <p className="mt-2 text-[11px] leading-relaxed text-vermilion/80">
            Never expose the daemon directly to the public internet. It runs
            arbitrary commands by design.
          </p>
        </section>

        <section className="rounded-xl border border-hueso/10 bg-raised p-4">
          <h2 className="font-mono text-[10px] uppercase tracking-[0.14em] text-hueso">
            Install
          </h2>
          <p className="mt-2 text-pretty text-xs leading-relaxed text-ceniza">
            Install the PWA for a full-screen, one-hand workspace. On iOS use
            Share → Add to Home Screen.
          </p>
          <div className="mt-4">
            {standalone ? (
              <span className="inline-flex items-center gap-2 font-mono text-[10px] uppercase tracking-[0.12em] text-kintsugi">
                <CheckIcon className="h-4 w-4" />
                Running as installed app
              </span>
            ) : installPrompt ? (
              <Button variant="secondary" size="sm" onClick={() => void install()}>
                <DownloadIcon className="h-4 w-4" />
                Install ONI
              </Button>
            ) : (
              <span className="font-mono text-[10px] uppercase tracking-[0.12em] text-ceniza">
                Use your browser&apos;s install action
              </span>
            )}
          </div>
        </section>

        <section className="rounded-xl border border-hueso/10 bg-raised p-4">
          <div className="flex items-center gap-4">
            <img
              src="/brand/oni-hannya.svg"
              alt=""
              className="h-14 w-14 shrink-0"
            />
            <div className="min-w-0">
              <h2 className="text-sm font-semibold text-hueso">
                ONI · Shiro Hannya
              </h2>
              <p className="mt-0.5 font-mono text-[9px] uppercase tracking-[0.14em] text-kintsugi">
                Agentic Development Environment
              </p>
              <p className="mt-1 text-[11px] text-ceniza">
                Inter + JetBrains Mono · SIL OFL 1.1
              </p>
            </div>
          </div>
          <div className="mt-4 border-t border-hueso/10 pt-3">
            <p className="text-[11px] leading-relaxed text-ceniza/80">
              Brand book 2026. The Hannya symbol is based on a Noun Project
              reference; license it or redraw it before commercial use.
            </p>
            <p className="mt-2 font-mono text-[9px] uppercase tracking-[0.12em] text-ceniza/60">
              ONI v0.1.0 · MVP
            </p>
          </div>
        </section>
      </div>
    </div>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="bg-sumi px-3 py-2.5">
      <dt className="font-mono text-[9px] uppercase tracking-[0.14em] text-ceniza">
        {label}
      </dt>
      <dd className="mt-1 font-mono text-[11px] text-hueso">{value}</dd>
    </div>
  );
}

function CodeSnippet({ label, code }: { label: string; code: string }) {
  return (
    <div className="flex items-start gap-2 rounded-lg border border-hueso/10 bg-sunken p-3">
      <div className="min-w-0 flex-1">
        <p className="font-mono text-[8px] uppercase tracking-[0.14em] text-ceniza/70">
          {label}
        </p>
        <code className="mt-1 block break-all font-mono text-[10px] leading-relaxed text-hueso/80">
          {code}
        </code>
      </div>
      <button
        type="button"
        aria-label={`Copy ${label} command`}
        className="shrink-0 rounded p-1 text-ceniza hover:text-hueso"
        onClick={async () => {
          const ok = await copyText(code);
          toast(ok ? "Command copied" : "Copy failed", ok ? "success" : "error");
        }}
      >
        <CopyIcon className="h-4 w-4" />
      </button>
    </div>
  );
}
