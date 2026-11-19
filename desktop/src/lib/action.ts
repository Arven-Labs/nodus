// O resultado de `run_action` visto pela GUI (ADR 0027). Formato: `Outcome` em `src-tauri/src/actions.rs`. Diferente das leituras,
// o `invoke` já devolve um objeto (não JSON em texto). Parsing estrito: campo ausente ou de tipo errado falha alto, com o nome do campo.

import { bool, field, object, ParseError, text } from "./parse.ts";

export interface ActionOutcome {
  /** `true`: o terminal foi aberto. `false`: não há terminal conhecido, e `command` é para o dono colar num terminal. */
  launched: boolean;
  /** O comando exato, só para mostrar (com aspas quando preciso). Nunca é executado pela GUI. */
  command: string;
}

export class ActionError extends ParseError {
  name = "ActionError";
}

export function parseActionOutcome(raw: unknown): ActionOutcome {
  try {
    const root = object(raw, "(raiz)");
    return {
      launched: bool(field(root, "launched", ""), "launched"),
      command: text(field(root, "command", ""), "command"),
    };
  } catch (error) {
    if (error instanceof ParseError && !(error instanceof ActionError)) throw new ActionError(error.message);
    throw error;
  }
}
