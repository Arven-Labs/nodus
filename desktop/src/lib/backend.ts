import { invoke } from "@tauri-apps/api/core";

/** Os comandos de leitura que a GUI pede (a lista fechada vive no Rust, `Allowed` em `src-tauri/src/nodusctl.rs`). */
export type Command = "inventory" | "repos" | "dotfiles" | "updates" | "workspace" | "history" | "preset_plan" | "preset_suggest";

/** As ações que ESCREVEM, em lista fechada (ADR 0027; a lista de verdade vive no Rust, `Action` em `src-tauri/src/actions.rs`). */
export type Action = "preset_apply" | "history_undo" | "preset_adopt" | "workspace_unset" | "workspace_set" | "dotfiles_adopt";

/** Erro do backend Rust (`CmdError`): `kind` é um código estável e `message` é para pessoas. */
export class BackendError extends Error {
  kind: string;
  constructor(kind: string, message: string) {
    super(message);
    this.name = "BackendError";
    this.kind = kind;
  }
}

function toBackendError(raw: unknown): BackendError {
  if (typeof raw === "object" && raw !== null) {
    const { kind, message } = raw as { kind?: unknown; message?: unknown };
    if (typeof kind === "string" && typeof message === "string") return new BackendError(kind, message);
  }
  return new BackendError("failed", String(raw));
}

/**
 * Pede ao backend um comando de LEITURA do `nodusctl` e devolve o JSON bruto. O frontend manda só o nome; quem recusa o que não
 * está na lista, monta os argumentos e confere a versão é o Rust (`src-tauri/src/nodusctl.rs`, ADR 0026).
 */
export async function readNodusctl(command: Command): Promise<string> {
  try {
    return await invoke<string>("read_nodusctl", { command });
  } catch (raw) {
    throw toBackendError(raw);
  }
}

/**
 * Pede ao backend uma ação da lista fechada: ele abre um terminal rodando o `nodusctl` (ADR 0027). O frontend manda o NOME da
 * ação e, no máximo, um argumento: um número (`n`, a entrada do `history_undo`) ou um texto (`path`, o candidato do `workspace_set`,
 * que o Rust só aceita se a CLI o listar na hora, ADR 0028). O arquivo, o terminal e a confirmação não passam por aqui. Sem argumento,
 * o campo nem é enviado. O resultado é cru: quem chama o passa por `parseActionOutcome`.
 */
export async function runAction(action: Action, arg?: number | string): Promise<unknown> {
  try {
    const payload =
      arg === undefined ? { command: action } : typeof arg === "string" ? { command: action, path: arg } : { command: action, n: arg };
    return await invoke<unknown>("run_action", payload);
  } catch (raw) {
    throw toBackendError(raw);
  }
}
