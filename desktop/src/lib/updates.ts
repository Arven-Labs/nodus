// O contrato do `nodusctl updates --json` visto pela GUI (ADR 0026). Formato: `crates/nodus-core/src/updates.rs`.
//
// A raiz é um objeto. `pacman_source` só pode ser um dos dois valores conhecidos: outro valor quer dizer que o contrato mudou, e
// falhar alto é mais seguro que mostrar uma lista cuja origem a tela não sabe descrever. `flatpak` é `null` quando o Flatpak não
// está instalado (diferente de `[]`, que é "nenhuma atualização"). Nomes e versões vêm de repositórios externos: só texto.

import { array, field, nullableText, object, ParseError, text } from "./parse.ts";

/** `pacman-qu` olha o banco local e pode estar desatualizado (o `checkupdates` não foi encontrado). */
export type PacmanSource = "checkupdates" | "pacman-qu";

export interface PacmanUpdate {
  name: string;
  oldVersion: string;
  newVersion: string;
}

export interface FlatpakUpdate {
  application: string;
  version: string | null;
  branch: string;
}

export interface Updates {
  pacmanSource: PacmanSource;
  pacman: PacmanUpdate[];
  /** `null`: Flatpak não instalado. */
  flatpak: FlatpakUpdate[] | null;
}

export class UpdatesError extends ParseError {
  name = "UpdatesError";
}

export function parseUpdates(jsonText: string): Updates {
  try {
    return parseUpdatesFields(jsonText);
  } catch (error) {
    if (error instanceof ParseError && !(error instanceof UpdatesError)) throw new UpdatesError(error.message);
    throw error;
  }
}

function pacmanSource(value: unknown): PacmanSource {
  if (value === "checkupdates" || value === "pacman-qu") return value;
  throw new UpdatesError("campo `pacman_source`: esperava \"checkupdates\" ou \"pacman-qu\"");
}

function parseUpdatesFields(jsonText: string): Updates {
  let raw: unknown;
  try {
    raw = JSON.parse(jsonText);
  } catch {
    throw new UpdatesError("a saída do nodusctl não é JSON");
  }
  const root = object(raw, "(raiz)");
  const flatpak = field(root, "flatpak", "");
  return {
    pacmanSource: pacmanSource(field(root, "pacman_source", "")),
    pacman: array(field(root, "pacman", ""), "pacman").map((item, i) => {
      const at = `pacman[${i}].`;
      const p = object(item, `pacman[${i}]`);
      return {
        name: text(field(p, "name", at), `${at}name`),
        oldVersion: text(field(p, "old_version", at), `${at}old_version`),
        newVersion: text(field(p, "new_version", at), `${at}new_version`),
      };
    }),
    flatpak:
      flatpak === null
        ? null
        : array(flatpak, "flatpak").map((item, i) => {
            const at = `flatpak[${i}].`;
            const f = object(item, `flatpak[${i}]`);
            return {
              application: text(field(f, "application", at), `${at}application`),
              version: nullableText(field(f, "version", at), `${at}version`),
              branch: text(field(f, "branch", at), `${at}branch`),
            };
          }),
  };
}
