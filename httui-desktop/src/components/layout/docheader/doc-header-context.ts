import { createContext } from "react";

export interface DocHeaderContextValue {
  /** Null when the shell renders outside the CM6 widget (e.g. in the
   *  static diff viewer or in unit tests that import DocHeaderCard
   *  directly). The editable input then no-ops on Enter / ArrowDown
   *  / Escape because there's no editor to dispatch into. */
  instanceId: string | null;
}

export const DocHeaderContext = createContext<DocHeaderContextValue>({
  instanceId: null,
});
