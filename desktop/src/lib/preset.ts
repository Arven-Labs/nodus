// O contrato do `nodusctl preset plan <arquivo> --json` visto pela GUI (ADR 0026). Formato: `Plan` em
// `crates/nodus-core/src/preset.rs`.
//
// A raiz é um objeto; as três listas são de texto e as contagens, inteiros não negativos. Os nomes vêm do arquivo de preset do
// usuário e do `pacman`: dado externo, tratado só como texto. Campo ausente ou de tipo errado falha alto, com o caminho do campo.

import { array, bool, count, field, object, ParseError, text } from "./parse.ts";

export interface PresetPlan {
  /** O que falta e o pacman consegue instalar. */
  pacmanMissing: string[];
  pacmanPresent: number;
  /** O que falta e o pacman não encontra nos repositórios (AUR, locais): o nodus não instala. */
  pacmanUnavailable: string[];
  flatpakMissing: string[];
  flatpakPresent: number;
  /** `false`: o Flatpak não está instalado nesta máquina. */
  flatpakInstalled: boolean;
}

export class PresetPlanError extends ParseError {
  name = "PresetPlanError";
}

export function parsePresetPlan(jsonText: string): PresetPlan {
  try {
    return parsePresetPlanFields(jsonText);
  } catch (error) {
    if (error instanceof ParseError && !(error instanceof PresetPlanError)) throw new PresetPlanError(error.message);
    throw error;
  }
}

function names(root: Record<string, unknown>, key: string): string[] {
  return array(field(root, key, ""), key).map((item, i) => text(item, `${key}[${i}]`));
}

function parsePresetPlanFields(jsonText: string): PresetPlan {
  let raw: unknown;
  try {
    raw = JSON.parse(jsonText);
  } catch {
    throw new PresetPlanError("a saída do nodusctl não é JSON");
  }
  const root = object(raw, "(raiz)");
  return {
    pacmanMissing: names(root, "pacman_missing"),
    pacmanPresent: Number(field(root, "pacman_present", "")),
    pacmanUnavailable: names(root, "pacman_unavailable"),
    flatpakMissing: names(root, "flatpak_missing"),
    flatpakPresent: count(field(root, "flatpak_present", ""), "flatpak_present"),
    flatpakInstalled: bool(field(root, "flatpak_installed", ""), "flatpak_installed"),
  };
}
