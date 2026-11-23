// O contrato do `nodusctl compare --all --json` visto pela GUI (ADR 0026). Formato: `Comparison` (com `ListComparison`,
// `DotfilesComparison` e `DotfileDiff`) em `crates/nodus-core/src/compare.rs`, mais o objeto montado em `compare_all_command` de
// `crates/nodus-cli/src/main.rs`.
//
// A raiz é um objeto com `comparisons` (um por par de snapshots, em ordem de rótulo) e `unreadable` (os que não deu para ler).
// Pacman e Flatpak são `compared` (com `only_a`, `only_b` e `common`) ou `not_comparable` (com `lacks`); os dotfiles são
// `compared` (com `differ`, em que o estado de cada lado é um dos quatro ou nulo para "fora da lista", e `same`) ou
// `not_comparable`. Rótulos, nomes, caminhos, SO e motivos vêm de arquivos de outras máquinas: dado externo, tratado só como
// texto. Campo ausente ou de tipo errado falha alto, com o caminho do campo.

import { array, count, field, nullableText, object, ParseError, fail, text } from "./parse.ts";

export type ListComparison =
  | { kind: "compared"; onlyA: string[]; onlyB: string[]; common: number }
  /** Alguma máquina não tem a capacidade (`lacks` traz o rótulo de quem não tem): não é "diferente". */
  | { kind: "not_comparable"; lacks: string[] };

export type DotfileState = "linked" | "file" | "missing" | "other";

/** `null` = o caminho não está na lista daquela máquina (diferente de `missing`: está na lista e não existe). */
export interface DotfileDiff {
  path: string;
  a: DotfileState | null;
  b: DotfileState | null;
}

export type DotfilesComparison =
  | { kind: "compared"; differ: DotfileDiff[]; same: number }
  /** Alguma máquina não olhou os dotfiles (snapshot v1): não é "diferente". */
  | { kind: "not_comparable"; lacks: string[] };

export interface OsInfo {
  name: string;
  version: string | null;
}

export interface Comparison {
  a: string;
  b: string;
  /** Só quando o SO (nome ou versão) é diferente; as listas continuam comparadas. */
  os: { a: OsInfo; b: OsInfo } | null;
  pacman: ListComparison;
  flatpak: ListComparison;
  dotfiles: DotfilesComparison;
}

export interface Unreadable {
  label: string;
  reason: string;
}

export interface CompareAll {
  comparisons: Comparison[];
  unreadable: Unreadable[];
}

export class CompareError extends ParseError {
  name = "CompareError";
}

export function parseCompare(jsonText: string): CompareAll {
  try {
    return parseCompareFields(jsonText);
  } catch (error) {
    if (error instanceof ParseError && !(error instanceof CompareError)) throw new CompareError(error.message);
    throw error;
  }
}

function names(parent: Record<string, unknown>, key: string, path: string): string[] {
  return array(field(parent, key, path), `${path}${key}`).map((item, i) => text(item, `${path}${key}[${i}]`));
}

function status(list: Record<string, unknown>, path: string): string {
  return text(field(list, "status", `${path}.`), `${path}.status`);
}

function listComparison(value: unknown, path: string): ListComparison {
  const list = object(value, path);
  const kind = status(list, path);
  if (kind === "compared") {
    return {
      kind: "compared",
      onlyA: names(list, "only_a", `${path}.`),
      onlyB: names(list, "only_b", `${path}.`),
      common: count(field(list, "common", `${path}.`), `${path}.common`),
    };
  }
  if (kind === "not_comparable") return { kind: "not_comparable", lacks: names(list, "lacks", `${path}.`) };
  return fail(`${path}.status`, "`compared` ou `not_comparable`");
}

function dotfileState(value: unknown, path: string): DotfileState | null {
  if (value === null) return null;
  if (typeof value === "string") return value as DotfileState;
  return fail(path, "`linked`, `file`, `missing`, `other` ou nulo");
}

function dotfileDiff(value: unknown, path: string): DotfileDiff {
  const diff = object(value, path);
  return {
    path: text(field(diff, "path", `${path}.`), `${path}.path`),
    a: dotfileState(field(diff, "a", `${path}.`), `${path}.a`),
    b: dotfileState(field(diff, "b", `${path}.`), `${path}.b`),
  };
}

function dotfilesComparison(value: unknown, path: string): DotfilesComparison {
  const list = object(value, path);
  const kind = status(list, path);
  if (kind === "compared") {
    return {
      kind: "compared",
      differ: array(field(list, "differ", `${path}.`), `${path}.differ`).map((item, i) => dotfileDiff(item, `${path}.differ[${i}]`)),
      same: count(field(list, "same", `${path}.`), `${path}.same`),
    };
  }
  if (kind === "not_comparable") return { kind: "not_comparable", lacks: names(list, "lacks", `${path}.`) };
  return fail(`${path}.status`, "`compared` ou `not_comparable`");
}

function osInfo(value: unknown, path: string): OsInfo {
  const os = object(value, path);
  return {
    name: text(field(os, "name", `${path}.`), `${path}.name`),
    version: nullableText(field(os, "version", `${path}.`), `${path}.version`),
  };
}

function osDifference(value: unknown, path: string): Comparison["os"] {
  if (value === null) return null;
  const os = object(value, path);
  return { a: osInfo(field(os, "a", `${path}.`), `${path}.a`), b: osInfo(field(os, "b", `${path}.`), `${path}.b`) };
}

function comparison(value: unknown, i: number): Comparison {
  const path = `comparisons[${i}]`;
  const c = object(value, path);
  return {
    a: text(field(c, "a", `${path}.`), `${path}.a`),
    b: text(field(c, "b", `${path}.`), `${path}.b`),
    os: osDifference(field(c, "os", `${path}.`), `${path}.os`),
    pacman: listComparison(field(c, "pacman", `${path}.`), `${path}.pacman`),
    flatpak: listComparison(field(c, "flatpak", `${path}.`), `${path}.flatpak`),
    dotfiles: dotfilesComparison(field(c, "dotfiles", `${path}.`), `${path}.dotfiles`),
  };
}

function unreadable(value: unknown, i: number): Unreadable {
  const path = `unreadable[${i}]`;
  const u = object(value, path);
  return {
    label: text(field(u, "label", `${path}.`), `${path}.label`),
    reason: text(field(u, "reason", `${path}.`), `${path}.reason`),
  };
}

function parseCompareFields(jsonText: string): CompareAll {
  let raw: unknown;
  try {
    raw = JSON.parse(jsonText);
  } catch {
    throw new CompareError("a saída do nodusctl não é JSON");
  }
  const root = object(raw, "(raiz)");
  return {
    comparisons: array(field(root, "comparisons", ""), "comparisons").map(comparison),
    unreadable: array(field(root, "unreadable", ""), "unreadable").map(unreadable),
  };
}
