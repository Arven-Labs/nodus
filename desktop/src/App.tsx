import { useEffect, useRef, useState } from "react";
import { parseActionOutcome } from "./lib/action";
import { BackendError, readNodusctl, runAction, type Command } from "./lib/backend";
import { type Dotfile, parseDotfiles } from "./lib/dotfiles";
import { formatUtc, type HistoryEntry, parseHistory } from "./lib/history";
import { parseInventory } from "./lib/inventory";
import { ParseError } from "./lib/parse";
import { parsePresetPlan } from "./lib/preset";
import { parsePresetSuggest } from "./lib/presetSuggest";
import { parseRepos, type Repo } from "./lib/repos";
import { parseUpdates, type Updates } from "./lib/updates";
import { parseWorkspace, type WorkspaceSource } from "./lib/workspace";
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

/** Como `useLoad`, mas só roda quando a tela pede (`run`): o comando usa a rede e não deve disparar ao abrir a aba. */
function useRunOnDemand<T>(command: Command, parse: (text: string) => T): { state: State<T> | { status: "idle" }; run: () => void } {
  const [state, setState] = useState<State<T> | { status: "idle" }>({ status: "idle" });
  const alive = useRef(true);
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);
  const run = () => {
    setState({ status: "loading" });
    readNodusctl(command)
      .then((text): State<T> => ({ status: "ok", data: parse(text) }))
      .catch(toError)
      .then((next) => {
        if (alive.current) setState(next);
      });
  };
  return { state, run };
}

function UpdatesScreen() {
  const { state, run } = useRunOnDemand("updates", parseUpdates);
  const loading = state.status === "loading";

  return (
    <section>
      <h1>Atualizações</h1>
      <button type="button" data-testid="updates-verificar" disabled={loading} onClick={run}>
        {state.status === "idle" ? "Verificar" : "Verificar de novo"}
      </button>
      {state.status === "idle" && <p>Consulta a rede (espelhos do pacman e Flathub). Clique em Verificar.</p>}
      {loading && <p data-testid="updates-carregando">Verificando… pode levar até 2 minutos.</p>}
      {state.status === "error" && <ErrorView {...state} what="as atualizações" />}
      {state.status === "ok" && <UpdatesResult updates={state.data} />}
    </section>
  );
}

function UpdatesResult({ updates }: { updates: Updates }) {
  return (
    <div data-testid="updates-resultado">
      {updates.pacmanSource === "pacman-qu" && (
        <p data-testid="updates-aviso-desatualizado">
          Pode estar desatualizado: `checkupdates` não foi encontrado, usei `pacman -Qu` (banco local).
        </p>
      )}
      <h2>{`Pacman (${updates.pacman.length})`}</h2>
      <div data-testid="updates-pacman">
        {updates.pacman.length === 0 ? (
          <p>Nenhuma atualização.</p>
        ) : (
          <ul className="repos">
            {updates.pacman.map((p, i) => (
              <li key={i} data-testid="update-pacman">
                <strong>{p.name}</strong>
                {` ${p.oldVersion} → ${p.newVersion}`}
              </li>
            ))}
          </ul>
        )}
      </div>
      <h2>{updates.flatpak === null ? "Flatpak" : `Flatpak (${updates.flatpak.length})`}</h2>
      <div data-testid="updates-flatpak">
        {updates.flatpak === null ? (
          <p>Flatpak não instalado.</p>
        ) : updates.flatpak.length === 0 ? (
          <p>Nenhuma atualização.</p>
        ) : (
          <ul className="repos">
            {updates.flatpak.map((f, i) => (
              <li key={i} data-testid="update-flatpak">
                <strong>{f.application}</strong>
                {` ${f.version ?? "(sem versão)"} · ${f.branch}`}
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}

const SOURCE_LABEL: Record<WorkspaceSource, string> = {
  informed: "informado por argumento",
  environment: "variável NODUS_WORKSPACE",
  stored: "gravado com `workspace set`",
  detected: "detectado sob o HOME",
};

function repoCountText(n: number): string {
  return `${n} ${n === 1 ? "repositório" : "repositórios"}`;
}

function WorkspaceScreen() {
  const state = useLoad("workspace", parseWorkspace);

  if (state.status === "loading") return <p>Lendo o workspace…</p>;
  if (state.status === "error") return <ErrorView {...state} what="o workspace" />;

  const { chosen, candidates } = state.data;
  return (
    <section>
      <h1>Workspace</h1>
      {chosen === null ? (
        <p data-testid="workspace-nenhum">Nenhum workspace detectado. Use `nodusctl workspace set &lt;pasta&gt;` no terminal.</p>
      ) : (
        <div data-testid="workspace-escolhido">
          <strong>{chosen.path}</strong>
          <div className="detail">{`${repoCountText(chosen.repoCount)} · ${SOURCE_LABEL[chosen.source]}`}</div>
        </div>
      )}
      {candidates.length > 0 && (
        <>
          <h2>Candidatos</h2>
          <ul data-testid="workspace-candidatos" className="repos">
            {candidates.map((c, i) => (
              <li key={i} data-testid="workspace-candidato">
                <strong>{c.path}</strong>
                <div className="detail">{repoCountText(c.repoCount)}</div>
              </li>
            ))}
          </ul>
        </>
      )}
    </section>
  );
}

const ACTION_LABEL: Record<HistoryEntry["action"], string> = {
  install: "instalação",
  remove: "remoção",
  adopt: "dotfile adotado",
  link: "dotfile ligado",
};

function resultText(result: HistoryEntry["result"]): string {
  if (result.status === "ok") return "ok";
  return result.code === null ? "falhou" : `falhou (código ${result.code})`;
}

function HistoryItem({ entry, n }: { entry: HistoryEntry; n: number }) {
  const undoes = entry.undoes === null ? "" : ` · desfez #${entry.undoes}`;
  const where = entry.dir === null ? "" : ` · pasta-fonte ${entry.dir}`;
  return (
    <li data-testid="history-entry">
      <strong>{`#${n}`}</strong>
      {` ${ACTION_LABEL[entry.action]} · ${resultText(entry.result)}${undoes}`}
      <div className="detail">{`${formatUtc(entry.atUnix)} · ${entry.manager}${where}`}</div>
      <div className="detail">{entry.requested.join(", ")}</div>
      <div className="detail">{entry.command.join(" ")}</div>
    </li>
  );
}

function HistoryScreen() {
  const state = useLoad("history", parseHistory);

  if (state.status === "loading") return <p>Lendo o histórico…</p>;
  if (state.status === "error") return <ErrorView {...state} what="o histórico" />;

  const entries = state.data;
  // A CLI lista do mais antigo ao mais novo e numera a partir de 1 (é o número do `undo`); aqui o mais novo vem primeiro, com o mesmo número.
  const newestFirst = entries.map((entry, i) => ({ entry, n: i + 1 })).reverse();
  return (
    <section>
      <h1>Histórico</h1>
      {entries.length === 0 ? (
        <p data-testid="history-vazio">Nenhuma execução registrada.</p>
      ) : (
        <>
          <ul data-testid="history-list" className="repos">
            {newestFirst.map(({ entry, n }) => (
              <HistoryItem key={n} entry={entry} n={n} />
            ))}
          </ul>
          <p data-testid="history-nota">
            Nota: só os pacotes pedidos são registrados; dependências instaladas ou removidas junto não aparecem aqui.
          </p>
        </>
      )}
    </section>
  );
}

function NameList({ items, testId }: { items: string[]; testId: string }) {
  if (items.length === 0) return null;
  return (
    <ul className="repos">
      {items.map((name, i) => (
        <li key={i} data-testid={testId}>
          {name}
        </li>
      ))}
    </ul>
  );
}

type ActionState =
  | { status: "idle" }
  | { status: "running" }
  | { status: "launched" }
  | { status: "fallback"; command: string }
  | { status: "error"; message: string };

/**
 * O botão que abre um terminal rodando `nodusctl preset apply` (ADR 0027). A janela não instala nada, não confirma e não vê senha:
 * manda só o nome da ação, e o plano, o `[s/N]` e o `sudo` são da CLI, no terminal. Sem terminal conhecido, mostra o comando.
 */
function PresetAction({ onRefresh }: { onRefresh: () => void }) {
  const [state, setState] = useState<ActionState>({ status: "idle" });
  const [copyFailed, setCopyFailed] = useState(false);
  // O estado só muda na renderização seguinte; o `ref` barra o segundo clique de um clique duplo (uma janela só).
  const inFlight = useRef(false);
  const busy = state.status === "running" || state.status === "launched";

  async function start() {
    if (inFlight.current) return;
    inFlight.current = true;
    setState({ status: "running" });
    setCopyFailed(false);
    try {
      const outcome = parseActionOutcome(await runAction("preset_apply"));
      setState(outcome.launched ? { status: "launched" } : { status: "fallback", command: outcome.command });
    } catch (error) {
      setState({ status: "error", message: error instanceof Error ? error.message : String(error) });
    } finally {
      inFlight.current = false;
    }
  }

  async function copy(command: string) {
    try {
      await navigator.clipboard.writeText(command);
      setCopyFailed(false);
    } catch {
      setCopyFailed(true);
    }
  }

  return (
    <div data-testid="preset-acao">
      <button type="button" data-testid="preset-instalar" disabled={busy} onClick={start}>
        Instalar no terminal
      </button>
      {state.status === "launched" && (
        <>
          <p data-testid="preset-acao-ok">
            Terminal aberto. Confirme a instalação lá: o nodusctl mostra o comando e pergunta antes. Quando terminar, clique em Atualizar.
          </p>
          <button type="button" data-testid="preset-atualizar" onClick={onRefresh}>
            Atualizar
          </button>
        </>
      )}
      {state.status === "fallback" && (
        <div data-testid="preset-sem-terminal">
          <p>Nenhum terminal conhecido foi encontrado. Cole este comando num terminal:</p>
          <pre data-testid="preset-comando">{state.command}</pre>
          <button type="button" data-testid="preset-copiar" onClick={() => copy(state.command)}>
            Copiar
          </button>
          {copyFailed && <p data-testid="preset-copiar-erro">Não consegui copiar: selecione o texto acima e copie à mão.</p>}
        </div>
      )}
      {state.status === "error" && <p data-testid="preset-acao-erro">{state.message}</p>}
    </div>
  );
}

function PresetScreen({ onRefresh }: { onRefresh: () => void }) {
  const state = useLoad("preset_plan", parsePresetPlan);

  if (state.status === "loading") return <p>Comparando o preset com esta máquina…</p>;
  if (state.status === "error") return <ErrorView {...state} what="o plano do preset" />;

  const plan = state.data;
  const nothingToInstall = plan.pacmanMissing.length === 0 && plan.flatpakMissing.length === 0;
  // O que o `apply` instalaria: o pacman, e o Flatpak só se estiver instalado (sem ele o `apply` nem tenta).
  const canInstall = plan.pacmanMissing.length > 0 || plan.flatpakMissing.length > 0;
  return (
    <section>
      <h1>Preset</h1>
      <div data-testid="preset-pacman">
        <h2>{`Pacman (instalar ${plan.pacmanMissing.length}, ${plan.pacmanPresent} já presentes)`}</h2>
        <NameList items={plan.pacmanMissing} testId="preset-pacman-item" />
      </div>
      {plan.pacmanUnavailable.length > 0 && (
        <div data-testid="preset-indisponiveis">
          <h2>{`Indisponíveis nos repositórios, o nodus não instala (${plan.pacmanUnavailable.length})`}</h2>
          <NameList items={plan.pacmanUnavailable} testId="preset-indisponivel-item" />
        </div>
      )}
      <div data-testid="preset-flatpak">
        <h2>
          {plan.flatpakInstalled
            ? `Flatpak (instalar ${plan.flatpakMissing.length}, ${plan.flatpakPresent} já presentes)`
            : `Flatpak não instalado (apps do preset: ${plan.flatpakMissing.length})`}
        </h2>
        <NameList items={plan.flatpakMissing} testId="preset-flatpak-item" />
      </div>
      {nothingToInstall && (
        <p data-testid="preset-nada">
          {plan.pacmanUnavailable.length === 0
            ? "Nada a fazer: a máquina já está como o preset."
            : "Nada a instalar: só restam itens indisponíveis nos repositórios."}
        </p>
      )}
      {canInstall && <PresetAction onRefresh={onRefresh} />}
      <p data-testid="preset-nota" className="detail">
        Esta janela não instala nada: o botão abre um terminal, e lá o `nodusctl preset apply` mostra o comando e pede a sua confirmação.
      </p>
    </section>
  );
}

function SuggestScreen() {
  const state = useLoad("preset_suggest", parsePresetSuggest);

  if (state.status === "loading") return <p>Procurando o que está instalado e fora do preset…</p>;
  if (state.status === "error") return <ErrorView {...state} what="as sugestões do preset" />;

  const s = state.data;
  const nothing = s.pacmanExtra.length === 0 && s.foreignExtra.length === 0 && s.flatpakExtra.length === 0;
  return (
    <section>
      <h1>Sugestões</h1>
      <div data-testid="suggest-pacman">
        <h2>{`Pacman (${s.pacmanExtra.length})`}</h2>
        <NameList items={s.pacmanExtra} testId="suggest-pacman-item" />
      </div>
      {s.foreignExtra.length > 0 && (
        <div data-testid="suggest-foreign">
          <h2>{`De fora dos repositórios (AUR ou locais), o \`apply\` não instala (${s.foreignExtra.length})`}</h2>
          <NameList items={s.foreignExtra} testId="suggest-foreign-item" />
        </div>
      )}
      <div data-testid="suggest-flatpak">
        <h2>{s.flatpakInstalled ? `Flatpak (${s.flatpakExtra.length})` : "Flatpak não instalado"}</h2>
        {!s.flatpakInstalled && <p>Os apps Flatpak não foram conferidos.</p>}
        <NameList items={s.flatpakExtra} testId="suggest-flatpak-item" />
      </div>
      {nothing && (
        <p data-testid="suggest-nada">Nada fora do preset: tudo o que foi instalado de propósito já está nele.</p>
      )}
      {s.ignored.length > 0 && (
        <p data-testid="suggest-ignorados" className="detail">
          {`${s.ignored.length} ${s.ignored.length === 1 ? "ignorado" : "ignorados"} pelo config. Para ver todos, use \`nodusctl preset suggest --all\` no terminal.`}
        </p>
      )}
      <p data-testid="suggest-nota" className="detail">
        Só leitura: nada foi alterado. Para adotar, use `nodusctl preset adopt` no terminal.
      </p>
    </section>
  );
}

type Tab = "inventory" | "repos" | "dotfiles" | "updates" | "workspace" | "history" | "preset" | "suggest";

const TABS: { id: Tab; label: string }[] = [
  { id: "inventory", label: "Inventário" },
  { id: "repos", label: "Repositórios" },
  { id: "dotfiles", label: "Dotfiles" },
  { id: "updates", label: "Atualizações" },
  { id: "workspace", label: "Workspace" },
  { id: "history", label: "Histórico" },
  { id: "preset", label: "Preset" },
  { id: "suggest", label: "Sugestões" },
];

export default function App() {
  const [tab, setTab] = useState<Tab>("inventory");
  // Muda para remontar a aba Preset e reler o plano (depois de a CLI instalar algo no terminal).
  const [presetKey, setPresetKey] = useState(0);
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
      {tab === "updates" && <UpdatesScreen />}
      {tab === "workspace" && <WorkspaceScreen />}
      {tab === "history" && <HistoryScreen />}
      {tab === "preset" && <PresetScreen key={presetKey} onRefresh={() => setPresetKey((k) => k + 1)} />}
      {tab === "suggest" && <SuggestScreen />}
    </main>
  );
}
