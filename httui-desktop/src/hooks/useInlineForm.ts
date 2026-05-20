import { useState } from "react";

/**
 * Result shape every name-validator in the codebase already returns
 * (`validateVariableName`, `validateEnvName`, …). Structurally
 * identical to those modules' own union types, declared here so this
 * hook stays layer-clean (no `components/**` import).
 */
export type InlineValidation = { ok: true } | { ok: false; reason: string };

export interface UseInlineFormResult {
  /** The single validated text field. Bind to the input. */
  value: string;
  setValue: (next: string) => void;
  /** `touched && invalid` — drives both the error <Text> and the
   *  disabled state of the submit button (the forms used
   *  `touched && !validation.ok`, which is exactly this). */
  showError: boolean;
  /** The validator's rejection reason while invalid, else undefined. */
  error: string | undefined;
  /** The submit gate: marks the field touched, then returns whether
   *  it is valid. Usage mirrors the hand-rolled idiom verbatim —
   *  `if (!form.attemptSubmit()) return; …build payload…`. */
  attemptSubmit: () => boolean;
}

/**
 * The group-2 inline-form idiom, extracted (audit 05 Part B §B.2.1).
 *
 * The env/variable inline forms (`NewVariableForm`,
 * `NewEnvironmentForm`, `CloneEnvironmentForm`,
 * `RenameEnvironmentForm`) each re-implement the same shape: one
 * validated text field + a `touched` flag + a pure validator +
 * `showError = touched && !ok` + a `setTouched(true); if (!ok) return`
 * submit gate. Unvalidated extras (value, is_secret, the clone
 * checkboxes, the type pills) are genuinely form-specific and stay as
 * their own `useState` in the component — they are NOT part of the
 * shared idiom, so this hook deliberately owns only the *one validated
 * string*, not a generic `<T>` record. That keeps the API to exactly
 * what every consumer uses (no speculative per-field error map / no
 * `reset` — none of the 5 forms reset; they unmount).
 *
 * `validate` is passed already-curried with whatever the form needs
 * (e.g. `(n) => validateEnvName(n, existingFilenames)`), so the hook
 * never knows about duplicate lists or filtering.
 */
export function useInlineForm(
  initial: string,
  validate: (value: string) => InlineValidation,
): UseInlineFormResult {
  const [value, setValue] = useState(initial);
  const [touched, setTouched] = useState(false);

  const validation = validate(value);
  const showError = touched && !validation.ok;

  return {
    value,
    setValue,
    showError,
    error: validation.ok ? undefined : validation.reason,
    attemptSubmit: () => {
      setTouched(true);
      return validation.ok;
    },
  };
}
