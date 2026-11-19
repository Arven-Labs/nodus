// O contrato do `nodusctl preset suggest <arquivo> --json` visto pela GUI (ADR 0026). Formato: `Suggestions` em
// `crates/nodus-core/src/preset.rs` mais `ignored` (de `suggestions_json`, em `crates/nodus-cli/src/render.rs`).
//
// A raiz é um objeto; as quatro listas são de texto. Os nomes vêm do `pacman`, do Flatpak e do `config.toml`: dado externo,
// tratado só como texto. Campo ausente ou de tipo errado falha alto, com o caminho do campo.

import { array, bool, field, object, ParseError, text } from "./parse.ts";

export interface PresetSuggest {
  /** Instalados de propósito, fora do preset, e que o `apply` saberia instalar. */
  pacmanExtra: string[];
  /** Instalados de propósito mas de fora dos repositórios (AUR, locais): o `apply` não os instala. */
  foreignExtra: string[];
  flatpakExtra: string[];
  /** `false`: o Flatpak não está instalado nesta máquina, então os apps não foram conferidos. */
  flatpakInstalled: boolean;
  /** Nomes escondidos pelo `config.toml` (a GUI só os conta). */
  ignored: string[];
}

export class PresetSuggestError extends ParseError {
  name = "PresetSuggestError";
}

export function parsePresetSuggest(jsonText: string): PresetSuggest {
  try {
    return parsePresetSuggestFields(jsonText);
  } catch (error) {
    if (error instanceof ParseError && !(error instanceof PresetSuggestError)) throw new PresetSuggestError(error.message);
    throw error;
  }
}

function names(root: Record<string, unknown>, key: string): string[] {
  return array(field(root, key, ""), key).map((item, i) => text(item, `${key}[${i}]`));
}

function parsePresetSuggestFields(jsonText: string): PresetSuggest {
  let raw: unknown;
  try {
    raw = JSON.parse(jsonText);
  } catch {
    throw new PresetSuggestError("a saída do nodusctl não é JSON");
  }
  const root = object(raw, "(raiz)");
  return {
    pacmanExtra: names(root, "pacman_extra"),
    foreignExtra: names(root, "foreign_extra"),
    flatpakExtra: names(root, "flatpak_extra"),
    flatpakInstalled: bool(field(root, "flatpak_installed", ""), "flatpak_installed"),
    ignored: names(root, "ignored"),
  };
}
