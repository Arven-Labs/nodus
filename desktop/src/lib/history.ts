// O contrato do `nodusctl history --json` visto pela GUI (ADR 0026). Formato: `HistoryEntry` em `crates/nodus-core/src/history.rs`.
//
// A raiz é um array, do mais antigo ao mais novo; o número de cada entrada no `nodusctl history` (e no `undo <n>`) é a posição + 1.
// `action`, `manager` e `result.status` só podem ser os valores conhecidos: outro valor quer dizer que o contrato mudou, e falhar
// alto é mais seguro que mostrar o que a tela não sabe interpretar. O arquivo de histórico é dado externo (nomes, comando, pasta):
// tratado só como texto.

import { array, count, field, nullableText, object, ParseError, text } from "./parse.ts";

export type HistoryAction = "install" | "remove" | "adopt" | "link";
export type HistoryManager = "pacman" | "flatpak" | "dotfiles";
export type HistoryResult = { status: "ok" } | { status: "failed"; code: number | null };

export interface HistoryEntry {
  version: number;
  /** Segundos desde 1970 em UTC. */
  atUnix: number;
  /** O comando exato que foi executado. */
  command: string[];
  requested: string[];
  installed: string[];
  stillMissing: string[];
  result: HistoryResult;
  action: HistoryAction;
  /** Número da entrada que esta reversão desfaz; `null` fora das reversões. */
  undoes: number | null;
  manager: HistoryManager;
  /** Pasta-fonte, só nas entradas de dotfile. */
  dir: string | null;
}

export class HistoryError extends ParseError {
  name = "HistoryError";
}

export function parseHistory(jsonText: string): HistoryEntry[] {
  try {
    return parseHistoryFields(jsonText);
  } catch (error) {
    if (error instanceof ParseError && !(error instanceof HistoryError)) throw new HistoryError(error.message);
    throw error;
  }
}

function action(value: unknown, at: string): HistoryAction {
  if (value === "install" || value === "remove" || value === "adopt" || value === "link") return value;
  throw new HistoryError(`campo \`${at}action\`: esperava "install", "remove", "adopt" ou "link"`);
}

function manager(value: unknown, at: string): HistoryManager {
  if (value === "pacman" || value === "flatpak" || value === "dotfiles") return value;
  throw new HistoryError(`campo \`${at}manager\`: esperava "pacman", "flatpak" ou "dotfiles"`);
}

function result(value: unknown, at: string): HistoryResult {
  const r = object(value, `${at}result`);
  const status = field(r, "status", `${at}result.`);
  if (status === "ok") return { status: "ok" };
  if (status === "failed") {
    const code = field(r, "code", `${at}result.`);
    if (code !== null && !(typeof code === "number" && Number.isSafeInteger(code))) {
      throw new HistoryError(`campo \`${at}result.code\`: esperava um inteiro ou null`);
    }
    return { status: "failed", code };
  }
  throw new HistoryError(`campo \`${at}result.status\`: esperava "ok" ou "failed"`);
}

function strings(value: unknown, path: string): string[] {
  return array(value, path).map((item, i) => text(item, `${path}[${i}]`));
}

function parseHistoryFields(jsonText: string): HistoryEntry[] {
  let raw: unknown;
  try {
    raw = JSON.parse(jsonText);
  } catch {
    throw new HistoryError("a saída do nodusctl não é JSON");
  }
  return array(raw, "(raiz)").map((item, i) => {
    const at = `history[${i}].`;
    const e = object(item, `history[${i}]`);
    // `undoes` e `dir` não são gravados quando não se aplicam: ausente vale `null`.
    const undoes = e.undoes === undefined || e.undoes === null ? null : count(e.undoes, `${at}undoes`);
    const dir = e.dir === undefined ? null : nullableText(e.dir, `${at}dir`);
    return {
      version: count(field(e, "version", at), `${at}version`),
      atUnix: count(field(e, "at_unix", at), `${at}at_unix`),
      command: strings(field(e, "command", at), `${at}command`),
      requested: strings(field(e, "requested", at), `${at}requested`),
      installed: strings(field(e, "installed", at), `${at}installed`),
      stillMissing: strings(field(e, "still_missing", at), `${at}still_missing`),
      result: result(field(e, "result", at), at),
      action: action(field(e, "action", at), at),
      undoes,
      manager: manager(field(e, "manager", at), at),
      dir,
    };
  });
}

/**
 * A entrada `n` (a partir de 1, o número do `nodusctl history`) tem o botão "Desfazer"? Só onde a CLI desfaz: instalação pacman e
 * dotfile adotado ou ligado; nunca uma reversão, nem Flatpak, nem o que outra entrada já desfez com sucesso (uma reversão que falhou
 * deixa tentar de novo). É só a decisão de mostrar o botão: quem recusa de verdade é a CLI, no terminal.
 */
export function canUndo(entries: HistoryEntry[], n: number): boolean {
  const entry = entries[n - 1];
  if (entry === undefined || entry.undoes !== null) return false;
  const reversible =
    (entry.manager === "pacman" && entry.action === "install") ||
    (entry.manager === "dotfiles" && (entry.action === "adopt" || entry.action === "link"));
  return reversible && !entries.some((other) => other.undoes === n && other.result.status === "ok");
}

/** `2023-11-14 22:13:20 UTC`: a mesma forma do `nodusctl history`, sem fuso local. */
export function formatUtc(atUnix: number): string {
  return `${new Date(atUnix * 1000).toISOString().slice(0, 19).replace("T", " ")} UTC`;
}
