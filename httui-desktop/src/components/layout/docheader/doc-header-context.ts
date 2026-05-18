// / M3 — context that tells the deep DocHeader
// subcomponents (currently only the editable title input) which
// CM6 extension instance owns this render tree. The portal sets it
// when mounting so consumers can call into the registry exposed by
// `cm-doc-header.tsx` without prop-drilling.

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
