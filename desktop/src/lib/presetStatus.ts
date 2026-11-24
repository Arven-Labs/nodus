// O contrato do `nodusctl preset status <arquivo> --json` visto pela GUI (ADR 0026). Formato: `MachineStatus` e `StatusList` em
// `crates/nodus-core/src/status.rs`, mais o objeto montado em `preset_status` de `crates/nodus-cli/src/main.rs`.
//
// A raiz é um objeto com `machines` (uma por snapshot lido) e `unreadable` (os que não deu para ler). Cada lista de uma máquina é
// `compared` (com `missing`, `extra` e `in_sync`) ou `not_comparable` (com `lacks`). Nomes de máquina, de pacote, rótulos e
// motivos vêm de arquivos e do sistema: dado externo, tratado só como texto. Campo ausente ou de tipo errado falha alto, com o
// caminho do campo.

import { array, bool, count, field, nullableText, object, ParseError, fail, text } from "./parse.ts";

export type StatusList =
  | { kind: "compared"; missing: string[]; extra: string[]; inSync: number }
  /** A máquina não tem a capacidade (`lacks` traz o rótulo dela): não é "diferente". */
  | { kind: "not_comparable"; lacks: string[] };

/** Itens por gerenciador: o que a máquina acrescenta (`add`) ou pula (`skip`) em relação ao preset. */
export interface ExceptionItems {
  pacman: string[];
  flatpak: string[];
}

/** O que `[machines.<rótulo>]` precisaria para a máquina ficar em dia (ADR 0033, emenda 4). Só sugestão: nada é escrito. */
export interface MachineExceptions {
  /** A tabela da máquina já existe no preset: o trecho manda acrescentar nas listas dela. */
  tableExists: boolean;
  add: ExceptionItems;
  skip: ExceptionItems;
}

export interface MachineStatus {
  machine: string;
  os: { name: string; version: string | null };
  pacman: StatusList;
  flatpak: StatusList;
  exceptions: MachineExceptions;
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
  return fail(`${path}.status`, "`compared` ou `not_comparable`");
}

function exceptionItems(value: unknown, path: string): ExceptionItems {
  const items = object(value, path);
  return {
    pacman: names(items, "pacman", `${path}.`),
    flatpak: names(items, "flatpak", `${path}.`),
  };
}

function machineExceptions(value: unknown, path: string): MachineExceptions {
  const e = object(value, path);
  return {
    tableExists: bool(field(e, "table_exists", `${path}.`), `${path}.table_exists`),
    add: exceptionItems(field(e, "add", `${path}.`), `${path}.add`),
    skip: exceptionItems(field(e, "skip", `${path}.`), `${path}.skip`),
  };
}

/** Nomes por lista no trecho (ADR 0033, emenda 5); o mesmo valor de `EXCEPTION_LIST_LIMIT` em `crates/nodus-cli/src/render.rs`: mudou lá, mude aqui. */
const EXCEPTION_LIST_LIMIT = 20;

/** A linha de comentário da forma com tabela existente, ou nada se a lista cabe no limite. */
function omittedLine(items: string[]): string[] {
  return items.length > EXCEPTION_LIST_LIMIT ? [`  # ... +${items.length - EXCEPTION_LIST_LIMIT} omitidos (lista completa acima)`] : [];
}

/**
 * O trecho para colar no preset, como o `exceptions_section` de `crates/nodus-cli/src/render.rs` o imprime no terminal: sem a
 * tabela da máquina, a tabela inteira; com ela, só diz em qual lista acrescentar (repetir a chave seria TOML inválido).
 * `null` quando não há exceção. Só leitura: a tela mostra, o dono cola.
 */
export function exceptionSnippet(machine: string, exceptions: MachineExceptions): { intro: string; body: string } | null {
  const keys: [string, string[]][] = [
    ["add_pacman", exceptions.add.pacman],
    ["add_flatpak", exceptions.add.flatpak],
    ["skip_pacman", exceptions.skip.pacman],
    ["skip_flatpak", exceptions.skip.flatpak],
  ];
  const filled = keys.filter(([, items]) => items.length > 0);
  if (filled.length === 0) return null;
  if (exceptions.tableExists) {
    return {
      intro: `A tabela [machines.${machine}] já existe. Para registrar a exceção, acrescente (nada foi alterado):`,
      body: filled
        .map(([key, items]) =>
          [`em \`${key} = [...]\`:`, ...items.slice(0, EXCEPTION_LIST_LIMIT).map((name) => `  "${name}",`), ...omittedLine(items)].join("\n"),
        )
        .join("\n"),
    };
  }
  return {
    intro: "Para registrar como exceção desta máquina, cole no preset (nada foi alterado):",
    body: [
      `[machines.${machine}]`,
      ...filled.map(([key, items]) => {
        const omitted = items.length >= EXCEPTION_LIST_LIMIT ? `  # +${items.length - EXCEPTION_LIST_LIMIT} omitidos (lista completa acima)` : "";
        return `${key} = [${items.slice(0, EXCEPTION_LIST_LIMIT).map((name) => `"${name}"`).join(", ")}]${omitted}`;
      }),
    ].join("\n"),
  };
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
    exceptions: machineExceptions(field(m, "exceptions", `${path}.`), `${path}.exceptions`),
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
