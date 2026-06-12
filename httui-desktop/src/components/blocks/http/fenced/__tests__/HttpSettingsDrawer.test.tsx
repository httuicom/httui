import { describe, it, expect, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/render";
import userEvent from "@testing-library/user-event";
import { HttpSettingsDrawer } from "@/components/blocks/http/fenced/HttpSettingsDrawer";
import type {
  BlockExample,
  HistoryEntry,
  HttpBlockSettings,
} from "@/lib/tauri/commands";
import type { HttpBlockMetadata } from "@/lib/blocks/http-message";

const mkMetadata = (
  over: Partial<HttpBlockMetadata> = {},
): HttpBlockMetadata => ({
  alias: "createUser",
  displayMode: "input",
  timeoutMs: 30000,
  ...over,
});

const mkHistory = (over: Partial<HistoryEntry> = {}): HistoryEntry => ({
  id: 1,
  file_path: "/v/note.md",
  block_alias: "createUser",
  method: "POST",
  url_canonical: "https://api/x",
  status: 201,
  request_size: 100,
  response_size: 250,
  elapsed_ms: 47,
  outcome: "success",
  ran_at: new Date(Date.now() - 30_000).toISOString(),
  ...over,
});

const mkExample = (over: Partial<BlockExample> = {}): BlockExample => ({
  id: 7,
  file_path: "/v/note.md",
  block_alias: "createUser",
  name: "happy path 200",
  response_json: "{}",
  saved_at: new Date(Date.now() - 60_000).toISOString(),
  ...over,
});

const baseProps = {
  metadata: mkMetadata(),
  history: [] as HistoryEntry[],
  examples: [] as BlockExample[],
  settings: {} as HttpBlockSettings,
  canSaveExample: false,
  onClose: vi.fn(),
  onUpdateMetadata: vi.fn(),
  onUpdateSettings: vi.fn(),
  onDelete: vi.fn(),
  onPurgeHistory: vi.fn(),
  onSaveExample: vi.fn(),
  onRestoreExample: vi.fn(),
  onDeleteExample: vi.fn(),
};

describe("HttpSettingsDrawer", () => {
  describe("identity section", () => {
    it("renders the alias input pre-filled", () => {
      renderWithProviders(<HttpSettingsDrawer {...baseProps} />);
      expect(screen.getByDisplayValue("createUser")).toBeInTheDocument();
    });

    it("editing alias calls onUpdateMetadata with new alias", async () => {
      const user = userEvent.setup();
      const onUpdateMetadata = vi.fn();
      renderWithProviders(
        <HttpSettingsDrawer
          {...baseProps}
          metadata={mkMetadata({ alias: undefined })}
          onUpdateMetadata={onUpdateMetadata}
        />,
      );

      const aliasInput = screen.getByPlaceholderText(/createUser/);
      await user.type(aliasInput, "x");
      expect(onUpdateMetadata).toHaveBeenCalledWith({ alias: "x" });
    });

    it("clearing alias passes undefined (not empty string)", async () => {
      const user = userEvent.setup();
      const onUpdateMetadata = vi.fn();
      renderWithProviders(
        <HttpSettingsDrawer
          {...baseProps}
          onUpdateMetadata={onUpdateMetadata}
        />,
      );

      await user.clear(screen.getByDisplayValue("createUser"));
      expect(onUpdateMetadata).toHaveBeenCalledWith({ alias: undefined });
    });

    it("changing display mode calls onUpdateMetadata", async () => {
      const user = userEvent.setup();
      const onUpdateMetadata = vi.fn();
      renderWithProviders(
        <HttpSettingsDrawer
          {...baseProps}
          onUpdateMetadata={onUpdateMetadata}
        />,
      );

      await user.selectOptions(screen.getByRole("combobox"), "split");
      expect(onUpdateMetadata).toHaveBeenCalledWith({ displayMode: "split" });
    });
  });

  describe("timeout input", () => {
    it("typing a number commits the truncated value", async () => {
      const onUpdateMetadata = vi.fn();
      renderWithProviders(
        <HttpSettingsDrawer
          {...baseProps}
          metadata={mkMetadata({ timeoutMs: undefined })}
          onUpdateMetadata={onUpdateMetadata}
        />,
      );

      // Controlled input — drive via fireEvent.change for atomic value set
      const input = screen.getByPlaceholderText("30000") as HTMLInputElement;
      const evt = new Event("input", { bubbles: true });
      Object.defineProperty(evt, "target", {
        value: { value: "12345" },
        writable: false,
      });
      // Simpler: dispatch change via userEvent's key events with fake input
      const setter = Object.getOwnPropertyDescriptor(
        window.HTMLInputElement.prototype,
        "value",
      )?.set;
      setter?.call(input, "12345");
      input.dispatchEvent(new Event("input", { bubbles: true }));

      expect(onUpdateMetadata).toHaveBeenCalledWith({ timeoutMs: 12345 });
    });

    it("a non-numeric value is ignored (no onUpdate call)", async () => {
      const onUpdateMetadata = vi.fn();
      renderWithProviders(
        <HttpSettingsDrawer
          {...baseProps}
          metadata={mkMetadata({ timeoutMs: undefined })}
          onUpdateMetadata={onUpdateMetadata}
        />,
      );

      const input = screen.getByPlaceholderText("30000") as HTMLInputElement;
      const setter = Object.getOwnPropertyDescriptor(
        window.HTMLInputElement.prototype,
        "value",
      )?.set;
      setter?.call(input, "not-a-number");
      input.dispatchEvent(new Event("input", { bubbles: true }));

      // Number("not-a-number") = NaN → onUpdateMetadata NOT called for the
      // number branch (only the empty branch passes undefined).
      // The function in the component returns early on NaN.
      expect(onUpdateMetadata).not.toHaveBeenCalledWith(
        expect.objectContaining({ timeoutMs: expect.any(Number) }),
      );
    });

    it("clearing the timeout input passes undefined", async () => {
      const user = userEvent.setup();
      const onUpdateMetadata = vi.fn();
      renderWithProviders(
        <HttpSettingsDrawer
          {...baseProps}
          onUpdateMetadata={onUpdateMetadata}
        />,
      );

      await user.clear(screen.getByDisplayValue("30000"));
      expect(onUpdateMetadata).toHaveBeenCalledWith({ timeoutMs: undefined });
    });
  });

  describe("per-block flags", () => {
    it("renders all five flags", () => {
      renderWithProviders(<HttpSettingsDrawer {...baseProps} />);
      expect(screen.getByText("Follow redirects")).toBeInTheDocument();
      expect(screen.getByText("Verify SSL")).toBeInTheDocument();
      expect(screen.getByText("Encode URL")).toBeInTheDocument();
      expect(screen.getByText("Trim whitespace")).toBeInTheDocument();
      expect(screen.getByText("Disable history")).toBeInTheDocument();
    });

    it("toggling a switch calls onUpdateSettings with the patched key", async () => {
      const user = userEvent.setup();
      const onUpdateSettings = vi.fn();
      renderWithProviders(
        <HttpSettingsDrawer
          {...baseProps}
          onUpdateSettings={onUpdateSettings}
        />,
      );

      // Click the first switch (Follow redirects, defaults ON → toggle = false)
      const followSwitch = screen.getByLabelText("Follow redirects");
      await user.click(followSwitch);
      expect(onUpdateSettings).toHaveBeenCalledWith({ followRedirects: false });
    });
  });

  describe("history list", () => {
    it("shows alias gate when alias is undefined", () => {
      renderWithProviders(
        <HttpSettingsDrawer
          {...baseProps}
          metadata={mkMetadata({ alias: undefined })}
        />,
      );
      expect(
        screen.getByText(/Set an alias to start tracking run history/i),
      ).toBeInTheDocument();
    });

    it("shows empty state when alias set but history is empty", () => {
      renderWithProviders(<HttpSettingsDrawer {...baseProps} />);
      expect(screen.getByText(/^No runs yet\./)).toBeInTheDocument();
    });

    it("renders history rows + Clear button", async () => {
      const user = userEvent.setup();
      const onPurgeHistory = vi.fn();
      renderWithProviders(
        <HttpSettingsDrawer
          {...baseProps}
          history={[mkHistory(), mkHistory({ id: 2, status: 500 })]}
          onPurgeHistory={onPurgeHistory}
        />,
      );

      expect(screen.getAllByText("POST")).toHaveLength(2);
      expect(screen.getByText("201")).toBeInTheDocument();
      expect(screen.getByText("500")).toBeInTheDocument();

      await user.click(screen.getByRole("button", { name: /clear history/i }));
      expect(onPurgeHistory).toHaveBeenCalled();
    });
  });

  describe("examples list", () => {
    it("shows alias gate when alias is undefined", () => {
      renderWithProviders(
        <HttpSettingsDrawer
          {...baseProps}
          metadata={mkMetadata({ alias: undefined })}
        />,
      );
      expect(
        screen.getByText(/Set an alias to pin response examples/i),
      ).toBeInTheDocument();
    });

    it("save button is disabled when canSaveExample=false and shows hint", () => {
      renderWithProviders(<HttpSettingsDrawer {...baseProps} />);
      const btn = screen.getByRole("button", { name: /pin current response/i });
      expect(btn).toBeDisabled();
      expect(screen.getByText(/run the request first/i)).toBeInTheDocument();
    });

    it("save button is enabled when canSaveExample=true (no hint)", () => {
      renderWithProviders(
        <HttpSettingsDrawer {...baseProps} canSaveExample={true} />,
      );
      const btn = screen.getByRole("button", { name: /pin current response/i });
      expect(btn).not.toBeDisabled();
      expect(
        screen.queryByText(/run the request first/i),
      ).not.toBeInTheDocument();
    });

    it("save button calls onSaveExample with prompted name", async () => {
      const user = userEvent.setup();
      const onSaveExample = vi.fn();
      const promptSpy = vi
        .spyOn(window, "prompt")
        .mockReturnValue("my snapshot");

      renderWithProviders(
        <HttpSettingsDrawer
          {...baseProps}
          canSaveExample={true}
          onSaveExample={onSaveExample}
        />,
      );

      await user.click(
        screen.getByRole("button", { name: /pin current response/i }),
      );

      expect(promptSpy).toHaveBeenCalled();
      expect(onSaveExample).toHaveBeenCalledWith("my snapshot");
      promptSpy.mockRestore();
    });

    it("save button is no-op when prompt returns null", async () => {
      const user = userEvent.setup();
      const onSaveExample = vi.fn();
      const promptSpy = vi.spyOn(window, "prompt").mockReturnValue(null);

      renderWithProviders(
        <HttpSettingsDrawer
          {...baseProps}
          canSaveExample={true}
          onSaveExample={onSaveExample}
        />,
      );

      await user.click(
        screen.getByRole("button", { name: /pin current response/i }),
      );
      expect(onSaveExample).not.toHaveBeenCalled();
      promptSpy.mockRestore();
    });

    it("clicking an example row calls onRestoreExample", async () => {
      const user = userEvent.setup();
      const onRestoreExample = vi.fn();
      const ex = mkExample();
      renderWithProviders(
        <HttpSettingsDrawer
          {...baseProps}
          examples={[ex]}
          onRestoreExample={onRestoreExample}
        />,
      );

      await user.click(screen.getByText("happy path 200"));
      expect(onRestoreExample).toHaveBeenCalledWith(ex);
    });

    it("delete button on example row calls onDeleteExample with id", async () => {
      const user = userEvent.setup();
      const onDeleteExample = vi.fn();
      renderWithProviders(
        <HttpSettingsDrawer
          {...baseProps}
          examples={[mkExample({ id: 42 })]}
          onDeleteExample={onDeleteExample}
        />,
      );

      await user.click(screen.getByRole("button", { name: /delete example/i }));
      expect(onDeleteExample).toHaveBeenCalledWith(42);
    });
  });

  describe("close + delete", () => {
    it("close button calls onClose", async () => {
      const user = userEvent.setup();
      const onClose = vi.fn();
      renderWithProviders(
        <HttpSettingsDrawer {...baseProps} onClose={onClose} />,
      );

      await user.click(screen.getByRole("button", { name: /close settings/i }));
      expect(onClose).toHaveBeenCalledTimes(1);
    });

    it("Delete block button calls onDelete", async () => {
      const user = userEvent.setup();
      const onDelete = vi.fn();
      renderWithProviders(
        <HttpSettingsDrawer {...baseProps} onDelete={onDelete} />,
      );

      await user.click(screen.getByRole("button", { name: /delete block/i }));
      expect(onDelete).toHaveBeenCalledTimes(1);
    });
  });
});
