import { invoke } from "@tauri-apps/api/core";

/** Os comandos de leitura que a GUI pede (a lista fechada vive no Rust, `Allowed` em `src-tauri/src/nodusctl.rs`). */
export type Command = "inventory" | "repos" | "dotfiles" | "updates" | "workspace";

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
