import { useEffect, useRef, useState } from "react";
import { parseActionOutcome } from "./lib/action";
import { BackendError, readNodusctl, runAction, type Command } from "./lib/backend";
import { type Comparison, type DotfileState, type DotfilesComparison, type ListComparison, type OsInfo, parseCompare } from "./lib/compare";
import { type Dotfile, parseDotfiles } from "./lib/dotfiles";
import { canUndo, formatUtc, type HistoryEntry, parseHistory } from "./lib/history";
import { parseInventory } from "./lib/inventory";
import { ParseError } from "./lib/parse";
import { parsePresetPlan } from "./lib/preset";
import { type MachineStatus, parsePresetStatus } from "./lib/presetStatus";
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

/** O limite do `dotfiles adopt` (`MAX_ADOPT_BYTES`, 1 MiB): acima disso a CLI recusa, e o botão abriria um terminal à toa. */
const MAX_ADOPT_BYTES = 1 << 20;

function DotfilesScreen({ onRefresh }: { onRefresh: () => void }) {
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
          {/* O lote só tem o que ligar se a CLI listou alguma entrada ausente. Quem decide o que entra e o que é pulado é a CLI, no
              terminal (plano e `[s/N]`); a tela não manda caminho nenhum. */}
          {present < entries.length && (
            <TerminalAction
              prefix="dotfiles-ligar-todos"
              buttonId="dotfiles-ligar-todos"
              label="Ligar todos no terminal"
              confirmWhat="a ligação"
              run={() => runAction("dotfiles_link_all")}
              onRefresh={onRefresh}
            />
          )}
          <ul data-testid="dotfiles-list" className="repos">
            {entries.map((entry, i) => (
              <li key={i} data-testid="dotfile">
                <strong>{entry.path}</strong>
                <div className="detail">{dotfileState(entry)}</div>
                {/* Só arquivo comum tem o que adotar. O caminho é o que a CLI acabou de listar; o Rust o confere de novo (ADR 0028). */}
                {entry.kind === "file" && entry.size <= MAX_ADOPT_BYTES && (
                  <TerminalAction
                    prefix="dotfile-adotar"
                    buttonId="dotfile-adotar"
                    label="Adotar no terminal"
                    confirmWhat="a adoção"
                    run={() => runAction("dotfiles_adopt", entry.path)}
                    onRefresh={onRefresh}
                  />
                )}
                {/* Só entrada ausente do HOME se liga (o `link` recusa um destino existente). A tela não vê a pasta-fonte: se o arquivo
                    não estiver lá, a CLI recusa no terminal. */}
                {entry.kind === "missing" && (
                  <TerminalAction
                    prefix="dotfile-ligar"
                    buttonId="dotfile-ligar"
                    label="Ligar no terminal"
                    confirmWhat="a ligação"
                    run={() => runAction("dotfiles_link", entry.path)}
                    onRefresh={onRefresh}
                  />
                )}
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

function WorkspaceScreen({ onRefresh }: { onRefresh: () => void }) {
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
                {/* O caminho do candidato é o que a CLI acabou de listar; o Rust o confere de novo antes de abrir o terminal (ADR 0028). */}
                <TerminalAction
                  prefix="workspace-usar"
                  buttonId="workspace-usar"
                  label="Usar no terminal"
                  run={() => runAction("workspace_set", c.path)}
                  onRefresh={onRefresh}
                />
              </li>
            ))}
          </ul>
        </>
      )}
      {/* O `unset` só tem o que apagar na escolha gravada; as outras origens (HOME, NODUS_WORKSPACE, argumento) ele não muda. */}
      {chosen?.source === "stored" && (
        <TerminalAction
          prefix="workspace"
          buttonId="workspace-esquecer"
          label="Esquecer no terminal"
          run={() => runAction("workspace_unset")}
          onRefresh={onRefresh}
        />
      )}
      <p data-testid="workspace-nota" className="detail">
        Esta janela não esquece nada: o botão abre um terminal e lá roda `nodusctl workspace unset`, que remove a escolha gravada.
      </p>
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

function HistoryItem({ entry, n, canUndo, onRefresh }: { entry: HistoryEntry; n: number; canUndo: boolean; onRefresh: () => void }) {
  const undoes = entry.undoes === null ? "" : ` · desfez #${entry.undoes}`;
  const where = entry.dir === null ? "" : ` · pasta-fonte ${entry.dir}`;
  return (
    <li data-testid="history-entry">
      <strong>{`#${n}`}</strong>
      {` ${ACTION_LABEL[entry.action]} · ${resultText(entry.result)}${undoes}`}
      <div className="detail">{`${formatUtc(entry.atUnix)} · ${entry.manager}${where}`}</div>
      <div className="detail">{entry.requested.join(", ")}</div>
      <div className="detail">{entry.command.join(" ")}</div>
      {canUndo && (
        <TerminalAction
          prefix="history"
          buttonId="history-desfazer"
          label="Desfazer no terminal"
          confirmWhat="a remoção"
          run={() => runAction("history_undo", n)}
          onRefresh={onRefresh}
        />
      )}
    </li>
  );
}

function HistoryScreen({ onRefresh }: { onRefresh: () => void }) {
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
              <HistoryItem key={n} entry={entry} n={n} canUndo={canUndo(entries, n)} onRefresh={onRefresh} />
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
 * A máquina de estados de uma ação que abre um terminal (ADR 0027), compartilhada pelo Preset e pelo Histórico. `run` pede a ação ao
 * backend; aqui só se cuida do que a tela mostra: rodando, terminal aberto, sem terminal (o comando para colar) ou erro.
 */
function useTerminalAction(run: () => Promise<unknown>) {
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
      const outcome = parseActionOutcome(await run());
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

  return { state, copyFailed, busy, start, copy };
}

/**
 * O botão que abre um terminal rodando um comando da CLI que escreve (ADR 0027). A janela não instala nem remove nada, não confirma e
 * não vê senha: manda só o nome da ação (e o número, no desfazer), e o plano, o `[s/N]` e o `sudo` são da CLI, no terminal. Sem
 * terminal conhecido, mostra o comando. `prefix` dá os `data-testid` (`<prefix>-acao-ok` etc.); `buttonId` é o do botão.
 */
function TerminalAction({
  prefix,
  buttonId,
  label,
  confirmWhat,
  run,
  onRefresh,
}: {
  prefix: string;
  buttonId: string;
  label: string;
  /** O que a CLI pede para confirmar. Sem isto (comando que não pergunta nada), o texto não promete confirmação. */
  confirmWhat?: string;
  run: () => Promise<unknown>;
  onRefresh: () => void;
}) {
  const { state, copyFailed, busy, start, copy } = useTerminalAction(run);
  return (
    <div data-testid={`${prefix}-acao`}>
      <button type="button" data-testid={buttonId} disabled={busy} onClick={start}>
        {label}
      </button>
      {state.status === "launched" && (
        <>
          <p data-testid={`${prefix}-acao-ok`}>
            {confirmWhat === undefined
              ? "Terminal aberto. O nodusctl roda lá e mostra o resultado. Quando terminar, clique em Atualizar."
              : `Terminal aberto. Confirme ${confirmWhat} lá: o nodusctl mostra o comando e pergunta antes. Quando terminar, clique em Atualizar.`}
          </p>
          <button type="button" data-testid={`${prefix}-atualizar`} onClick={onRefresh}>
            Atualizar
          </button>
        </>
      )}
      {state.status === "fallback" && (
        <div data-testid={`${prefix}-sem-terminal`}>
          <p>Nenhum terminal conhecido foi encontrado. Cole este comando num terminal:</p>
          <pre data-testid={`${prefix}-comando`}>{state.command}</pre>
          <button type="button" data-testid={`${prefix}-copiar`} onClick={() => copy(state.command)}>
            Copiar
          </button>
          {copyFailed && <p data-testid={`${prefix}-copiar-erro`}>Não consegui copiar: selecione o texto acima e copie à mão.</p>}
        </div>
      )}
      {state.status === "error" && <p data-testid={`${prefix}-acao-erro`}>{state.message}</p>}
    </div>
  );
}

function PresetAction({ onRefresh }: { onRefresh: () => void }) {
  return (
    <TerminalAction
      prefix="preset"
      buttonId="preset-instalar"
      label="Instalar no terminal"
      confirmWhat="a instalação"
      run={() => runAction("preset_apply")}
      onRefresh={onRefresh}
    />
  );
}

function PresetScreen({ onRefresh }: { onRefresh: () => void }) {
  const state = useLoad("preset_plan", parsePresetPlan);

  if (state.status === "loading") return <p>Comparando o preset com esta máquina…</p>;
  if (state.status === "error") return <ErrorView {...state} what="o plano do preset" />;

  const plan = state.data;
  const nothingToInstall = plan.pacmanMissing.length === 0 && plan.flatpakMissing.length === 0;
  // O que o `apply` instalaria: o pacman, e o Flatpak só se estiver instalado (sem ele o `apply` nem tenta).
  const canInstall = plan.pacmanMissing.length > 0 || (plan.flatpakInstalled && plan.flatpakMissing.length > 0);
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

function SuggestAction({ onRefresh }: { onRefresh: () => void }) {
  return (
    <TerminalAction
      prefix="suggest"
      buttonId="suggest-adotar"
      label="Adotar no terminal"
      confirmWhat="a adoção"
      run={() => runAction("preset_adopt")}
      onRefresh={onRefresh}
    />
  );
}

function SuggestScreen({ onRefresh }: { onRefresh: () => void }) {
  const state = useLoad("preset_suggest", parsePresetSuggest);

  if (state.status === "loading") return <p>Procurando o que está instalado e fora do preset…</p>;
  if (state.status === "error") return <ErrorView {...state} what="as sugestões do preset" />;

  const s = state.data;
  const nothing = s.pacmanExtra.length === 0 && s.foreignExtra.length === 0 && s.flatpakExtra.length === 0;
  // O `adopt` só pergunta pacman e Flatpak; os de fora dos repositórios ele só informa.
  const canAdopt = s.pacmanExtra.length > 0 || s.flatpakExtra.length > 0;
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
        <>
          <p data-testid="suggest-ignorados" className="detail">
            {`${s.ignored.length} ${s.ignored.length === 1 ? "ignorado" : "ignorados"} pelo config. Para ver todos, use \`nodusctl preset suggest --all\` no terminal.`}
          </p>
          <ul className="repos">
            {s.ignored.map((name, i) => (
              <li key={i} data-testid="suggest-ignorado">
                <strong>{name}</strong>
                {/* O nome é o que a CLI acabou de listar em `ignored`; o Rust o confere de novo antes de abrir o terminal (ADR 0028). */}
                <TerminalAction
                  prefix="suggest-voltar"
                  buttonId="suggest-voltar"
                  label="Voltar a sugerir no terminal"
                  run={() => runAction("preset_unignore", name)}
                  onRefresh={onRefresh}
                />
              </li>
            ))}
          </ul>
        </>
      )}
      {canAdopt && <SuggestAction onRefresh={onRefresh} />}
      <p data-testid="suggest-nota" className="detail">
        Esta janela não adota nada e nada foi alterado: o botão abre um terminal, e lá o `nodusctl preset adopt` pergunta item a item.
      </p>
    </section>
  );
}

function MachineStatusView({ m }: { m: MachineStatus }) {
  const lists = [
    { label: "Pacman", list: m.pacman },
    { label: "Flatpak", list: m.flatpak },
  ];
  // "Não comparável" não é diferença: a máquina não tem a capacidade, então a lista não conta nem contra nem a favor.
  const inSync = lists.every(({ list }) => list.kind === "not_comparable" || (list.missing.length === 0 && list.extra.length === 0));
  const notComparable = lists.some(({ list }) => list.kind === "not_comparable");
  return (
    <div data-testid="status-maquina">
      <h2>{`${m.machine} (${m.os.name}${m.os.version ? ` ${m.os.version}` : ""})`}</h2>
      {inSync && <p data-testid="status-em-dia">{notComparable ? "em dia nas listas comparadas" : "em dia com o preset"}</p>}
      {lists.map(({ label, list }) =>
        list.kind === "not_comparable" ? (
          <p key={label} data-testid="status-nao-comparavel">{`${label}: não comparável, ${list.lacks.join(", ")} não tem ${label}`}</p>
        ) : (
          <div key={label}>
            <h3>{`${label}: ${list.inSync} em dia`}</h3>
            {list.missing.length > 0 && (
              <div data-testid="status-faltam">
                <h4>{`No preset e não instalado de propósito (${list.missing.length})`}</h4>
                <NameList items={list.missing} testId="status-faltam-item" />
              </div>
            )}
            {list.extra.length > 0 && (
              <div data-testid="status-sobram">
                <h4>{`Instalado de propósito e fora do preset (${list.extra.length})`}</h4>
                <NameList items={list.extra} testId="status-sobram-item" />
              </div>
            )}
          </div>
        ),
      )}
    </div>
  );
}

function StatusScreen() {
  const state = useLoad("preset_status", parsePresetStatus);

  if (state.status === "loading") return <p>Lendo o preset e os snapshots…</p>;
  if (state.status === "error") return <ErrorView {...state} what="o status do preset" />;

  const { machines, unreadable } = state.data;
  return (
    <section>
      <h1>Status do preset</h1>
      {machines.length === 0 && unreadable.length === 0 && (
        <p data-testid="status-vazio">Nenhum snapshot encontrado. Rode `nodusctl snapshot` em cada máquina e traga os arquivos pelo git.</p>
      )}
      {machines.map((m, i) => (
        <MachineStatusView key={i} m={m} />
      ))}
      {unreadable.map((u, i) => (
        <p key={i} data-testid="status-ilegivel" className="detail">{`Aviso: não consegui ler o snapshot ${u.label}: ${u.reason}`}</p>
      ))}
      <p data-testid="status-nota" className="detail">
        Só leitura: nada foi alterado. "Não instalado de propósito" inclui o que veio só como dependência, porque o snapshot guarda `pacman -Qe`.
      </p>
    </section>
  );
}

const DOTFILE_STATE_LABEL: Record<DotfileState, string> = { linked: "ligado", file: "arquivo", missing: "ausente", other: "outro" };

/** `null` = o caminho não está na lista daquela máquina (diferente de "ausente": está na lista e não existe). */
function dotfileStateLabel(state: DotfileState | null): string {
  return state === null ? "fora da lista" : DOTFILE_STATE_LABEL[state];
}

function osLabel(os: OsInfo): string {
  return os.version ? `${os.name} ${os.version}` : os.name;
}

/** "Não comparável" não é diferença: cita quem não tem a capacidade (uma ou duas máquinas). */
function lacksText(lacks: string[], one: string, many: string): string {
  return `${lacks.join(" e ")} ${lacks.length === 1 || many.length > 0 ? one : many}`;
}

function ListCompareView({ label, a, b, list }: { label: string; a: string; b: string; list: ListComparison }) {
  if (list.kind === "not_comparable") {
    return (
      <p data-testid="compare-nao-comparavel">{`${label}: não comparável, ${lacksText(list.lacks, `não tem ${label}`, `não têm ${label}`)}`}</p>
    );
  }
  return (
    <div data-testid={`compare-${label.toLowerCase()}`}>
      <h3>{`${label}: ${list.common} em comum`}</h3>
      {[
        { who: a, items: list.onlyA },
        { who: b, items: list.onlyB },
      ].map(({ who, items }, i) =>
        items.length === 0 ? null : (
          // Dobrado: a lista pode ter 150 itens.
          <details key={i} data-testid="compare-so-em">
            <summary>{`só em ${who} (${items.length})`}</summary>
            <NameList items={items} testId="compare-so-em-item" />
          </details>
        ),
      )}
    </div>
  );
}

function DotfilesCompareView({ a, b, dotfiles }: { a: string; b: string; dotfiles: DotfilesComparison }) {
  if (dotfiles.kind === "not_comparable") {
    return (
      <p data-testid="compare-nao-comparavel">{`Dotfiles: não comparável, ${lacksText(dotfiles.lacks, "não registrou", "não registraram")} dotfiles (snapshot v1; rode \`nodusctl snapshot\` de novo)`}</p>
    );
  }
  // Como na CLI: duas listas vazias não dão seção.
  if (dotfiles.differ.length === 0 && dotfiles.same === 0) return null;
  return (
    <div data-testid="compare-dotfiles">
      <h3>{`Dotfiles: ${dotfiles.same} em comum`}</h3>
      {dotfiles.differ.length > 0 && (
        <>
          <h4>{`Diferem (${dotfiles.differ.length})`}</h4>
          <ul className="repos">
            {dotfiles.differ.map((d, i) => (
              <li key={i} data-testid="compare-dotfile-difere">{`${d.path}: ${a} ${dotfileStateLabel(d.a)}, ${b} ${dotfileStateLabel(d.b)}`}</li>
            ))}
          </ul>
        </>
      )}
    </div>
  );
}

function hasDifferences(c: Comparison): boolean {
  const listDiffers = (l: ListComparison) => l.kind === "compared" && (l.onlyA.length > 0 || l.onlyB.length > 0);
  return c.os !== null || listDiffers(c.pacman) || listDiffers(c.flatpak) || (c.dotfiles.kind === "compared" && c.dotfiles.differ.length > 0);
}

function ComparisonView({ c }: { c: Comparison }) {
  const notComparable = [c.pacman, c.flatpak, c.dotfiles].some((l) => l.kind === "not_comparable");
  return (
    <div data-testid="compare-par">
      <h2>{`${c.a} × ${c.b}`}</h2>
      {c.os && (
        <p data-testid="compare-so-diferente" className="detail">{`Aviso: sistemas diferentes: ${c.a} é ${osLabel(c.os.a)}, ${c.b} é ${osLabel(c.os.b)}`}</p>
      )}
      <ListCompareView label="Pacman" a={c.a} b={c.b} list={c.pacman} />
      <ListCompareView label="Flatpak" a={c.a} b={c.b} list={c.flatpak} />
      <DotfilesCompareView a={c.a} b={c.b} dotfiles={c.dotfiles} />
      {!hasDifferences(c) && (
        <p data-testid="compare-sem-divergencias">{notComparable ? "Sem divergências nas listas comparadas." : "Sem divergências."}</p>
      )}
    </div>
  );
}

function CompareScreen() {
  const state = useLoad("compare_all", parseCompare);

  if (state.status === "loading") return <p>Lendo os snapshots…</p>;
  if (state.status === "error") return <ErrorView {...state} what="a comparação dos snapshots" />;

  const { comparisons, unreadable } = state.data;
  return (
    <section>
      <h1>Comparar máquinas</h1>
      {comparisons.length === 0 && (
        <p data-testid="compare-vazio">
          Nenhum par para comparar: são precisos pelo menos dois snapshots legíveis. Rode `nodusctl snapshot` em cada máquina e traga os arquivos pelo git.
        </p>
      )}
      {comparisons.map((c, i) => (
        <ComparisonView key={i} c={c} />
      ))}
      {unreadable.map((u, i) => (
        <p key={i} data-testid="compare-ilegivel" className="detail">{`Aviso: não consegui ler o snapshot ${u.label}: ${u.reason}`}</p>
      ))}
      <p data-testid="compare-nota" className="detail">
        Só leitura: nada foi alterado. O snapshot guarda `pacman -Qe`, então "só em" inclui o que veio só como dependência.
      </p>
    </section>
  );
}

type Tab = "inventory" | "repos" | "dotfiles" | "updates" | "workspace" | "history" | "preset" | "suggest" | "status" | "compare";

const TABS: { id: Tab; label: string }[] = [
  { id: "inventory", label: "Inventário" },
  { id: "repos", label: "Repositórios" },
  { id: "dotfiles", label: "Dotfiles" },
  { id: "updates", label: "Atualizações" },
  { id: "workspace", label: "Workspace" },
  { id: "history", label: "Histórico" },
  { id: "preset", label: "Preset" },
  { id: "suggest", label: "Sugestões" },
  { id: "status", label: "Status" },
  { id: "compare", label: "Compare" },
];

export default function App() {
  const [tab, setTab] = useState<Tab>("inventory");
  // Muda para remontar a aba Preset e reler o plano (depois de a CLI instalar algo no terminal).
  const [presetKey, setPresetKey] = useState(0);
  // O mesmo para o Histórico (depois de a CLI desfazer algo no terminal).
  const [historyKey, setHistoryKey] = useState(0);
  // E para as Sugestões (depois de a CLI adotar algo no terminal).
  const [suggestKey, setSuggestKey] = useState(0);
  // E para o Workspace (depois de a CLI esquecer a escolha no terminal).
  const [workspaceKey, setWorkspaceKey] = useState(0);
  // E para os Dotfiles (depois de a CLI adotar um arquivo no terminal).
  const [dotfilesKey, setDotfilesKey] = useState(0);
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
      {tab === "dotfiles" && <DotfilesScreen key={dotfilesKey} onRefresh={() => setDotfilesKey((k) => k + 1)} />}
      {tab === "updates" && <UpdatesScreen />}
      {tab === "workspace" && <WorkspaceScreen key={workspaceKey} onRefresh={() => setWorkspaceKey((k) => k + 1)} />}
      {tab === "history" && <HistoryScreen key={historyKey} onRefresh={() => setHistoryKey((k) => k + 1)} />}
      {tab === "preset" && <PresetScreen key={presetKey} onRefresh={() => setPresetKey((k) => k + 1)} />}
      {tab === "suggest" && <SuggestScreen key={suggestKey} onRefresh={() => setSuggestKey((k) => k + 1)} />}
      {tab === "status" && <StatusScreen />}
      {tab === "compare" && <CompareScreen />}
    </main>
  );
}
