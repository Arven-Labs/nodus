import { useEffect, useState } from "react";
import { BackendError, readNodusctl } from "./lib/backend";
import { type Inventory, InventoryError, parseInventory } from "./lib/inventory";
import "./App.css";

type State =
  | { status: "loading" }
  | { status: "ok"; inventory: Inventory }
  | { status: "error"; kind: string; message: string };

/** Quantos pacotes a lista mostra de uma vez (a contagem é sempre a total). */
const PACKAGE_LIST_LIMIT = 500;

function gib(bytes: number): string {
  const value = bytes / 1024 ** 3;
  return `${Number.isInteger(value) ? value : value.toFixed(1)} GiB`;
}

function load(): Promise<State> {
  return readNodusctl("inventory")
    .then((text): State => ({ status: "ok", inventory: parseInventory(text) }))
    .catch((error: unknown): State => {
      if (error instanceof BackendError) return { status: "error", kind: error.kind, message: error.message };
      if (error instanceof InventoryError) return { status: "error", kind: "invalid_json", message: error.message };
      return { status: "error", kind: "failed", message: String(error) };
    });
}

export default function App() {
  const [state, setState] = useState<State>({ status: "loading" });
  const [showPackages, setShowPackages] = useState(false);

  useEffect(() => {
    let alive = true;
    load().then((next) => {
      if (alive) setState(next);
    });
    return () => {
      alive = false;
    };
  }, []);

  if (state.status === "loading") return <main><p>Lendo o inventário…</p></main>;

  if (state.status === "error") {
    if (state.kind === "version_mismatch") {
      return (
        <main data-testid="versao-incompativel">
          <h1>Versão incompatível</h1>
          <p>{state.message}</p>
          <p>Instale uma versão do nodusctl com o mesmo major.minor desta interface.</p>
        </main>
      );
    }
    return (
      <main data-testid="erro">
        <h1>Não foi possível ler o inventário</h1>
        <p>{state.message}</p>
      </main>
    );
  }

  const { system, hardware, packages, flatpakApps } = state.inventory;
  return (
    <main>
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
    </main>
  );
}
