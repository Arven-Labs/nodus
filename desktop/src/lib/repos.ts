// O contrato do `nodusctl repos --json` visto pela GUI (ADR 0026). Formato: `crates/nodus-core/src/repos.rs`.
//
// A raiz é um array de repositórios. A GUI nunca pede `--fetch`, então `fetch` só pode ser `null`: qualquer outro valor quer
// dizer que o contrato mudou, e falhar alto é mais seguro que mostrar o que a tela não sabe interpretar.
// Nomes, branches e URLs vêm do disco: dado externo, tratado só como texto.

import { array, bool, field, nullableCount, nullableText, object, ParseError, text } from "./parse.ts";

export interface Repo {
  name: string;
  path: string;
  /** `null` com HEAD solto ou repositório sem commits. */
  branch: string | null;
  commit: string | null;
  upstream: string | null;
  /** `null` sem upstream (não é o mesmo que 0). */
  ahead: number | null;
  behind: number | null;
  upstreamGone: boolean;
  originUrl: string | null;
  /** Falha ao inspecionar este repositório; os outros continuam. */
  error: string | null;
}

export class ReposError extends ParseError {
  name = "ReposError";
}

export function parseRepos(jsonText: string): Repo[] {
  try {
    return parseReposFields(jsonText);
  } catch (error) {
    if (error instanceof ParseError && !(error instanceof ReposError)) throw new ReposError(error.message);
    throw error;
  }
}

function parseReposFields(jsonText: string): Repo[] {
  let raw: unknown;
  try {
    raw = JSON.parse(jsonText);
  } catch {
    throw new ReposError("a saída do nodusctl não é JSON");
  }
  return array(raw, "(raiz)").map((item, i) => {
    const at = `repos[${i}].`;
    const r = object(item, `repos[${i}]`);
    if (field(r, "fetch", at) !== null) {
      throw new ReposError(`campo \`${at}fetch\`: esperava null (a GUI não pede --fetch)`);
    }
    return {
      name: text(field(r, "name", at), `${at}name`),
      path: text(field(r, "path", at), `${at}path`),
      branch: nullableText(field(r, "branch", at), `${at}branch`),
      commit: nullableText(field(r, "commit", at), `${at}commit`),
      upstream: nullableText(field(r, "upstream", at), `${at}upstream`),
      ahead: nullableCount(field(r, "ahead", at), `${at}ahead`),
      behind: nullableCount(field(r, "behind", at), `${at}behind`),
      upstreamGone: bool(field(r, "upstream_gone", at), `${at}upstream_gone`),
      originUrl: nullableText(field(r, "origin_url", at), `${at}origin_url`),
      error: nullableText(field(r, "error", at), `${at}error`),
    };
  });
}
