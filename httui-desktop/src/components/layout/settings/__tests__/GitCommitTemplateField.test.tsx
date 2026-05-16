import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { fireEvent, renderWithProviders, screen } from "@/test/render";
import { clearTauriMocks } from "@/test/mocks/tauri";
import { useSettingsStore } from "@/stores/settings";
import { GitCommitTemplateField } from "@/components/layout/settings/GitCommitTemplateField";

beforeEach(() => {
  clearTauriMocks();
  useSettingsStore.setState({ gitCommitTemplate: "" });
});

afterEach(() => {
  clearTauriMocks();
  useSettingsStore.setState({ gitCommitTemplate: "" });
});

describe("GitCommitTemplateField", () => {
  it("shows the persisted template value", () => {
    useSettingsStore.setState({ gitCommitTemplate: "docs: {{notes}}" });
    renderWithProviders(<GitCommitTemplateField />);
    expect(screen.getByTestId("git-commit-template-input")).toHaveValue(
      "docs: {{notes}}",
    );
  });

  it("renders the placeholder hint and an empty field by default", () => {
    renderWithProviders(<GitCommitTemplateField />);
    const input = screen.getByTestId("git-commit-template-input");
    expect(input).toHaveValue("");
    expect(input).toHaveAttribute("placeholder", "Update {{notes}}");
    expect(screen.getByText(/smart default/i)).toBeInTheDocument();
  });

  it("writes edits through to the settings store", () => {
    renderWithProviders(<GitCommitTemplateField />);
    fireEvent.change(screen.getByTestId("git-commit-template-input"), {
      target: { value: "chore: {{count}} files" },
    });
    expect(useSettingsStore.getState().gitCommitTemplate).toBe(
      "chore: {{count}} files",
    );
  });
});
