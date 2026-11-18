// O contrato do `nodusctl workspace --json` visto pela GUI (ADR 0026). Formato: `report_json` em `crates/nodus-cli/src/workspace.rs`.
//
// A raiz é um objeto. `chosen` é `null` quando nada foi escolhido (nenhum candidato detectável). `source` só pode ser um dos
// quatro valores conhecidos: outro valor quer dizer que o contrato mudou, e falhar alto é mais seguro que mostrar uma origem que
// a tela não sabe descrever. Os caminhos vêm do disco e do ambiente: dado externo, tratado só como texto.

import { array, count, field, object, ParseError, text } from "./parse.ts";

/** De onde veio a pasta: argumento, `NODUS_WORKSPACE`, `workspace set` (config) ou detecção sob o `$HOME`. */
export type WorkspaceSource = "informed" | "environment" | "stored" | "detected";

export interface WorkspaceChosen {
  path: string;
  source: WorkspaceSource;
  repoCount: number;
}

export interface WorkspaceCandidate {
  path: string;
  repoCount: number;
}

export interface Workspace {
  /** `null`: nenhum workspace escolhido. */
  chosen: WorkspaceChosen | null;
  /** Só na detecção: os que existem, do com mais repositórios para o com menos. */
  candidates: WorkspaceCandidate[];
}

export class WorkspaceError extends ParseError {
  name = "WorkspaceError";
}

export function parseWorkspace(jsonText: string): Workspace {
  try {
    return parseWorkspaceFields(jsonText);
  } catch (error) {
    if (error instanceof ParseError && !(error instanceof WorkspaceError)) throw new WorkspaceError(error.message);
    throw error;
  }
}

function source(value: unknown): WorkspaceSource {
  if (value === "informed" || value === "environment" || value === "stored" || value === "detected") return value;
  throw new WorkspaceError('campo `chosen.source`: esperava "informed", "environment", "stored" ou "detected"');
}

function parseWorkspaceFields(jsonText: string): Workspace {
  let raw: unknown;
  try {
    raw = JSON.parse(jsonText);
  } catch {
    throw new WorkspaceError("a saída do nodusctl não é JSON");
  }
  const root = object(raw, "(raiz)");
  const chosen = field(root, "chosen", "");
  return {
    chosen:
      chosen === null
        ? null
        : (() => {
            const c = object(chosen, "chosen");
            return {
              path: text(field(c, "path", "chosen."), "chosen.path"),
              source: source(field(c, "source", "chosen.")),
              repoCount: count(field(c, "repo_count", "chosen."), "chosen.repo_count"),
            };
          })(),
    candidates: array(field(root, "candidates", ""), "candidates").map((item, i) => {
      const at = `candidates[${i}].`;
      const c = object(item, `candidates[${i}]`);
      return {
        path: text(field(c, "path", at), `${at}path`),
        repoCount: count(field(c, "repo_count", at), `${at}repo_count`),
      };
    }),
  };
}
