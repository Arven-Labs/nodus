// O contrato do `nodusctl dotfiles --json` visto pela GUI (ADR 0026). Formato: `crates/nodus-core/src/dotfiles.rs`.
//
// A raiz é um array de entradas `{ path, kind, ... }`; o `kind` decide os campos extras (`file` tem `size`; `symlink` tem `target`
// e `outside_home`). `kind` desconhecido é erro: a GUI não sabe interpretar o que não conhece. Caminhos e alvos vêm do disco:
// dado externo, tratado só como texto. Só sintaxe que o Node entende ao remover os tipos (sem enums).

import { array, bool, count, field, object, ParseError, text } from "./parse.ts";

export type Dotfile =
  | { path: string; kind: "missing" }
  | { path: string; kind: "dir" }
  | { path: string; kind: "other" }
  | { path: string; kind: "file"; size: number }
  | { path: string; kind: "symlink"; target: string; outsideHome: boolean };

export class DotfilesError extends ParseError {
  name = "DotfilesError";
}

export function parseDotfiles(jsonText: string): Dotfile[] {
  try {
    return parseDotfilesFields(jsonText);
  } catch (error) {
    if (error instanceof ParseError && !(error instanceof DotfilesError)) throw new DotfilesError(error.message);
    throw error;
  }
}

function parseDotfilesFields(jsonText: string): Dotfile[] {
  let raw: unknown;
  try {
    raw = JSON.parse(jsonText);
  } catch {
    throw new DotfilesError("a saída do nodusctl não é JSON");
  }
  return array(raw, "(raiz)").map((item, i): Dotfile => {
    const at = `dotfiles[${i}].`;
    const e = object(item, `dotfiles[${i}]`);
    const path = text(field(e, "path", at), `${at}path`);
    const kind = field(e, "kind", at);
    switch (kind) {
      case "missing":
      case "dir":
      case "other":
        return { path, kind };
      case "file":
        return { path, kind, size: count(field(e, "size", at), `${at}size`) };
      case "symlink":
        return {
          path,
          kind,
          target: text(field(e, "target", at), `${at}target`),
          outsideHome: bool(field(e, "outside_home", at), `${at}outside_home`),
        };
      default:
        return { path, kind: "other" };
    }
  });
}
