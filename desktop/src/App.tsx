import { useEffect, useState } from "react";
import { BackendError, readNodusctl, type Command } from "./lib/backend";
import { type Dotfile, parseDotfiles } from "./lib/dotfiles";
import { parseInventory } from "./lib/inventory";
import { ParseError } from "./lib/parse";
import { parseRepos, type Repo } from "./lib/repos";
import "./App.css";

type State<T> =
  | { status: "loading" }
  | { status: "ok"; data: T }
  | { status: "error"; kind: string; message: string };

/** Quantos pacotes a lista mostra de uma vez (a contagem é sempre a total). */
const PACKAGE_LIST_LIMIT = 500;

function gib(bytes: number): string {
  const value = bytes / 1024 ** 3;
  return `${Number.isInteger(value) ? value : value.toFixed(1)} GiB`;
}

function toError(error: unknown): { status: "error"; kind: string; message: string } {
  if (error instanceof BackendError) return { status: "error", kind: error.kind, message: error.message };
  if (error instanceof ParseError) return { status: "error", kind: "invalid_json", message: error.message };
  return { status: "error", kind: "failed", message: String(error) };
}

/** Carrega ao montar: cada tela só roda o seu comando quando a aba é aberta. */
function useLoad<T>(command: Command, parse: (text: string) => T): State<T> {
  const [state, setState] = useState<State<T>>({ status: "loading" });
  useEffect(() => {
    let alive = true;
    readNodusctl(command)
      .then((text): State<T> => ({ status: "ok", data: parse(text) }))
      .catch(toError)
      .then((next) => {
        if (alive) setState(next);
      });
    return () => {
      alive = false;
    };
    // `parse` é uma função de módulo (estável): o efeito depende só do comando.
  }, [command]);
  return state;
}

function ErrorView({ kind, message, what }: { kind: string; message: string; what: string }) {
  if (kind === "version_mismatch") {
    return (
      <section data-testid="versao-incompativel">
        <h1>Versão incompatível</h1>
        <p>{message}</p>
        <p>Instale uma versão do nodusctl com o mesmo major.minor desta interface.</p>
      </section>
    );
  }
  return (
    <section data-testid="erro">
      <h1>{`Não foi possível ler ${what}`}</h1>
      <p>{message}</p>
    </section>
  );
}

function InventoryScreen() {
  const state = useLoad("inventory", parseInventory);
  const [showPackages, setShowPackages] = useState(false);

  if (state.status === "loading") return <p>Lendo o inventário…</p>;
  if (state.status === "error") return <ErrorView {...state} what="o inventário" />;

  const { system, hardware, packages, flatpakApps } = state.data;
  return (
    <section>
      <h1>Inventário</h1>
      <dl>
        <dt>Sistema</dt>
        <dd data-testid="system">{[system.osName, system.osVersion].filter(Boolean).join(" ")}</dd>
        <dt>Kernel</dt>
        <dd data-testid="kernel">{system.kernel}</dd>
        <dt>CPU</dt>
        <dd data-testid="cpu">{`${hardware.cpuModel} (${hardware.cpuThreads} threads)`}</dd>
        <dt>Memória</dt>
        <dd data-testid="memory">{gib(hardware.memoryTotalBytes)}</dd>
        <dt>Discos</dt>
        <dd data-testid="disks">
          {hardware.disks.length === 0
            ? "nenhum"
            : hardware.disks.map((d) => `${d.name} ${d.mountPoint} ${d.fileSystem} ${gib(d.totalBytes)}`).join("; ")}
        </dd>
        <dt>Pacotes</dt>
        <dd data-testid="packages-count">{packages.length}</dd>
        <dt>Apps Flatpak</dt>
        <dd data-testid="flatpak-count">{flatpakApps === null ? "Flatpak não instalado" : flatpakApps.length}</dd>
      </dl>
      <button type="button" data-testid="packages-toggle" onClick={() => setShowPackages((v) => !v)}>
        {showPackages ? "Esconder pacotes" : "Ver pacotes"}
      </button>
      {showPackages && (
        <>
          <ul data-testid="packages-list">
            {packages.slice(0, PACKAGE_LIST_LIMIT).map((p, i) => (
              <li key={i}>{`${p.name} ${p.version}`}</li>
            ))}
          </ul>
          {packages.length > PACKAGE_LIST_LIMIT && (
            <p>{`Mostrando ${PACKAGE_LIST_LIMIT} de ${packages.length} pacotes.`}</p>
          )}
        </>
      )}
    </section>
  );
}

function trackingText(repo: Repo): string {
  if (repo.upstreamGone) return "upstream removido";
  if (repo.ahead === null || repo.behind === null) return "sem upstream";
  return `${repo.ahead} à frente, ${repo.behind} atrás`;
}

function ReposScreen() {
  const state = useLoad("repos", parseRepos);

  if (state.status === "loading") return <p>Lendo os repositórios…</p>;
  if (state.status === "error") return <ErrorView {...state} what="os repositórios" />;

  const repos = state.data;
  return (
    <section>
      <h1>Repositórios</h1>
      {repos.length === 0 ? (
        <p data-testid="repos-vazio">Nenhum repositório encontrado no workspace.</p>
      ) : (
        <ul data-testid="repos-list" className="repos">
          {repos.map((repo, i) => (
            <li key={i} data-testid="repo">
              <strong>{repo.name}</strong>
              {repo.error === null ? (
                <>
                  {` ${repo.branch ?? "(HEAD solto)"} · ${trackingText(repo)}`}
                  {repo.originUrl !== null && <div className="detail">{repo.originUrl}</div>}
                </>
              ) : (
                <div className="repo-error">{repo.error}</div>
              )}
              <div className="detail">{repo.path}</div>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

function dotfileState(entry: Dotfile): string {
  switch (entry.kind) {
    case "missing":
      return "ausente";
    case "file":
      return `arquivo (${entry.size} bytes)`;
    case "dir":
      return "pasta";
    case "other":
      return "outro tipo";
    case "symlink":
      return `symlink -> ${entry.target}${entry.outsideHome ? " (fora do HOME)" : ""}`;
  }
}

function DotfilesScreen() {
  const state = useLoad("dotfiles", parseDotfiles);

  if (state.status === "loading") return <p>Lendo os dotfiles…</p>;
  if (state.status === "error") return <ErrorView {...state} what="os dotfiles" />;

  const entries = state.data;
  const present = entries.filter((e) => e.kind !== "missing").length;
  return (
    <section>
      <h1>Dotfiles</h1>
      {entries.length === 0 ? (
        <p data-testid="dotfiles-vazio">Nenhum dotfile na lista.</p>
      ) : (
        <>
          <ul data-testid="dotfiles-list" className="repos">
            {entries.map((entry, i) => (
              <li key={i} data-testid="dotfile">
                <strong>{entry.path}</strong>
                <div className="detail">{dotfileState(entry)}</div>
              </li>
            ))}
          </ul>
          <p data-testid="dotfiles-resumo">{`Presentes: ${present} de ${entries.length}.`}</p>
        </>
      )}
    </section>
  );
}

type Tab = "inventory" | "repos" | "dotfiles";

const TABS: { id: Tab; label: string }[] = [
  { id: "inventory", label: "Inventário" },
  { id: "repos", label: "Repositórios" },
  { id: "dotfiles", label: "Dotfiles" },
];

export default function App() {
  const [tab, setTab] = useState<Tab>("inventory");
  return (
    <main>
      <nav className="tabs">
        {TABS.map(({ id, label }) => (
          <button key={id} type="button" data-testid={`tab-${id}`} aria-current={tab === id} onClick={() => setTab(id)}>
            {label}
          </button>
        ))}
      </nav>
      {tab === "inventory" && <InventoryScreen />}
      {tab === "repos" && <ReposScreen />}
      {tab === "dotfiles" && <DotfilesScreen />}
    </main>
  );
}
