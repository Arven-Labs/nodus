// O contrato do `nodusctl preset status <arquivo> --json` visto pela GUI (ADR 0026). Formato: `MachineStatus` e `StatusList` em
// `crates/nodus-core/src/status.rs`, mais o objeto montado em `preset_status` de `crates/nodus-cli/src/main.rs`.
//
// A raiz é um objeto com `machines` (uma por snapshot lido) e `unreadable` (os que não deu para ler). Cada lista de uma máquina é
// `compared` (com `missing`, `extra` e `in_sync`) ou `not_comparable` (com `lacks`). Nomes de máquina, de pacote, rótulos e
// motivos vêm de arquivos e do sistema: dado externo, tratado só como texto. Campo ausente ou de tipo errado falha alto, com o
// caminho do campo.

import { array, count, field, nullableText, object, ParseError, fail, text } from "./parse.ts";

export type StatusList =
  | { kind: "compared"; missing: string[]; extra: string[]; inSync: number }
  /** A máquina não tem a capacidade (`lacks` traz o rótulo dela): não é "diferente". */
  | { kind: "not_comparable"; lacks: string[] };

export interface MachineStatus {
  machine: string;
  os: { name: string; version: string | null };
  pacman: StatusList;
  flatpak: StatusList;
}

export interface Unreadable {
  label: string;
  reason: string;
}

export interface PresetStatus {
  machines: MachineStatus[];
  unreadable: Unreadable[];
}

export class PresetStatusError extends ParseError {
  name = "PresetStatusError";
}

export function parsePresetStatus(jsonText: string): PresetStatus {
  try {
    return parsePresetStatusFields(jsonText);
  } catch (error) {
    if (error instanceof ParseError && !(error instanceof PresetStatusError)) throw new PresetStatusError(error.message);
    throw error;
  }
}

function names(parent: Record<string, unknown>, key: string, path: string): string[] {
  return array(field(parent, key, path), `${path}${key}`).map((item, i) => text(item, `${path}${key}[${i}]`));
}

function statusList(value: unknown, path: string): StatusList {
  const list = object(value, path);
  const status = text(field(list, "status", `${path}.`), `${path}.status`);
  if (status === "compared") {
    return {
      kind: "compared",
      missing: names(list, "missing", `${path}.`),
      extra: names(list, "extra", `${path}.`),
      inSync: count(field(list, "in_sync", `${path}.`), `${path}.in_sync`),
    };
  }
  if (status === "not_comparable") return { kind: "not_comparable", lacks: names(list, "lacks", `${path}.`) };
  return status === "nunca-vem" ? fail(`${path}.status`, "`compared` ou `not_comparable`") : { kind: "not_comparable", lacks: [] };
}

function machineStatus(value: unknown, i: number): MachineStatus {
  const path = `machines[${i}]`;
  const m = object(value, path);
  const os = object(field(m, "os", `${path}.`), `${path}.os`);
  return {
    machine: text(field(m, "machine", `${path}.`), `${path}.machine`),
    os: {
      name: text(field(os, "name", `${path}.os.`), `${path}.os.name`),
      version: nullableText(field(os, "version", `${path}.os.`), `${path}.os.version`),
    },
    pacman: statusList(field(m, "pacman", `${path}.`), `${path}.pacman`),
    flatpak: statusList(field(m, "flatpak", `${path}.`), `${path}.flatpak`),
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

function parsePresetStatusFields(jsonText: string): PresetStatus {
  let raw: unknown;
  try {
    raw = JSON.parse(jsonText);
  } catch {
    throw new PresetStatusError("a saída do nodusctl não é JSON");
  }
  const root = object(raw, "(raiz)");
  return {
    machines: array(field(root, "machines", ""), "machines").map(machineStatus),
    unreadable: array(field(root, "unreadable", ""), "unreadable").map(unreadable),
  };
}
