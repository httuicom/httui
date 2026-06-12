import { describe, it, expect, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/render";
import userEvent from "@testing-library/user-event";
import {
  HttpFormMode,
  type InlineCMProps,
} from "@/components/blocks/http/fenced/HttpFormMode";
import type {
  HttpKVRow,
  HttpMessageParsed,
  HttpMethod,
} from "@/lib/blocks/http-message";

// Replace the CM6 inline editor with a plain controlled input that calls
// `onCommit` on blur — keeps tests free of CodeMirror in jsdom.
function PlainInlineCM({ placeholder, value, onCommit }: InlineCMProps) {
  return (
    <input
      aria-label={placeholder}
      defaultValue={value}
      onBlur={(e) => onCommit(e.target.value)}
      onChange={() => {}}
    />
  );
}

const mkParsed = (
  over: Partial<HttpMessageParsed> = {},
): HttpMessageParsed => ({
  method: "GET" as HttpMethod,
  url: "https://api.test/x",
  params: [],
  headers: [],
  body: "",
  ...over,
});

const mkRow = (key: string, value: string, enabled = true): HttpKVRow => ({
  key,
  value,
  enabled,
});

describe("HttpFormMode", () => {
  describe("tabs + counts", () => {
    it("renders tabs with counts including pending rows", async () => {
      const user = userEvent.setup();
      renderWithProviders(
        <HttpFormMode
          parsed={mkParsed({
            params: [mkRow("p1", "v1")],
            headers: [],
          })}
          bodyMode="json"
          onChange={vi.fn()}
          onPickFile={vi.fn(async () => null)}
          InlineCM={PlainInlineCM}
          renderBodyTab={() => <div data-testid="body-tab">BODY</div>}
        />,
      );

      expect(
        screen.getByRole("tab", { name: "Params (1)" }),
      ).toBeInTheDocument();
      expect(
        screen.getByRole("tab", { name: "Headers (0)" }),
      ).toBeInTheDocument();
      expect(screen.getByRole("tab", { name: "Body" })).toBeInTheDocument();

      // Switch to Headers, add a pending row → count goes to (1)
      await user.click(screen.getByRole("tab", { name: "Headers (0)" }));
      await user.click(screen.getByRole("button", { name: /\+ add header/ }));
      expect(
        screen.getByRole("tab", { name: "Headers (1)" }),
      ).toBeInTheDocument();
    });
  });

  describe("KVRow rendering", () => {
    it("renders one row per existing param with key/value/description fields", () => {
      renderWithProviders(
        <HttpFormMode
          parsed={mkParsed({
            params: [
              { key: "page", value: "1", enabled: true, description: "pg" },
            ],
          })}
          bodyMode="none"
          onChange={vi.fn()}
          onPickFile={vi.fn(async () => null)}
          InlineCM={PlainInlineCM}
          renderBodyTab={() => null}
        />,
      );

      const keyInput = screen.getByLabelText("key") as HTMLInputElement;
      const valueInput = screen.getByLabelText("value") as HTMLInputElement;
      const descInput = screen.getByLabelText(
        "description",
      ) as HTMLInputElement;
      expect(keyInput.value).toBe("page");
      expect(valueInput.value).toBe("1");
      expect(descInput.value).toBe("pg");
    });

    it("toggling enabled checkbox calls onChange with patched row", async () => {
      const user = userEvent.setup();
      const onChange = vi.fn();
      renderWithProviders(
        <HttpFormMode
          parsed={mkParsed({ params: [mkRow("k", "v", true)] })}
          bodyMode="none"
          onChange={onChange}
          onPickFile={vi.fn(async () => null)}
          InlineCM={PlainInlineCM}
          renderBodyTab={() => null}
        />,
      );

      await user.click(
        screen.getByRole("checkbox", { name: /toggle params row 0/i }),
      );
      const last = onChange.mock.calls.at(-1)?.[0] as HttpMessageParsed;
      expect(last.params[0].enabled).toBe(false);
    });

    it("clicking delete button on existing row calls onChange with row removed", async () => {
      const user = userEvent.setup();
      const onChange = vi.fn();
      renderWithProviders(
        <HttpFormMode
          parsed={mkParsed({
            params: [mkRow("a", "1"), mkRow("b", "2")],
          })}
          bodyMode="none"
          onChange={onChange}
          onPickFile={vi.fn(async () => null)}
          InlineCM={PlainInlineCM}
          renderBodyTab={() => null}
        />,
      );

      await user.click(
        screen.getByRole("button", { name: /delete params row 0/i }),
      );
      const last = onChange.mock.calls.at(-1)?.[0] as HttpMessageParsed;
      expect(last.params).toEqual([mkRow("b", "2")]);
    });
  });

  describe("pending row lifecycle", () => {
    it("clicking '+ add param' adds a pending row (count bumps but onChange NOT called yet)", async () => {
      const user = userEvent.setup();
      const onChange = vi.fn();
      renderWithProviders(
        <HttpFormMode
          parsed={mkParsed()}
          bodyMode="none"
          onChange={onChange}
          onPickFile={vi.fn(async () => null)}
          InlineCM={PlainInlineCM}
          renderBodyTab={() => null}
        />,
      );

      await user.click(screen.getByRole("button", { name: /\+ add param/ }));

      expect(
        screen.getByRole("tab", { name: "Params (1)" }),
      ).toBeInTheDocument();
      expect(onChange).not.toHaveBeenCalled();
    });

    it("typing a key in pending row promotes it to committed via onChange", async () => {
      const user = userEvent.setup();
      const onChange = vi.fn();
      renderWithProviders(
        <HttpFormMode
          parsed={mkParsed()}
          bodyMode="none"
          onChange={onChange}
          onPickFile={vi.fn(async () => null)}
          InlineCM={PlainInlineCM}
          renderBodyTab={() => null}
        />,
      );

      await user.click(screen.getByRole("button", { name: /\+ add param/ }));
      // Now there's a single key input — type and blur to commit
      const keyInput = screen.getByLabelText("key");
      await user.type(keyInput, "TOKEN");
      await user.tab(); // blur

      const last = onChange.mock.calls.at(-1)?.[0] as HttpMessageParsed;
      expect(last.params).toEqual([{ key: "TOKEN", value: "", enabled: true }]);
    });

    it("deleting a pending row drops it without onChange", async () => {
      const user = userEvent.setup();
      const onChange = vi.fn();
      renderWithProviders(
        <HttpFormMode
          parsed={mkParsed()}
          bodyMode="none"
          onChange={onChange}
          onPickFile={vi.fn(async () => null)}
          InlineCM={PlainInlineCM}
          renderBodyTab={() => null}
        />,
      );

      await user.click(screen.getByRole("button", { name: /\+ add param/ }));
      // Pending row index = 0 (no committed rows)
      await user.click(
        screen.getByRole("button", { name: /delete params row 0/i }),
      );
      expect(
        screen.getByRole("tab", { name: "Params (0)" }),
      ).toBeInTheDocument();
      expect(onChange).not.toHaveBeenCalled();
    });
  });

  describe("body tab", () => {
    it("invokes renderBodyTab with parsed/onCommit/onPickFile when Body tab is active", async () => {
      const user = userEvent.setup();
      const renderBodyTab = vi.fn(() => <div data-testid="body-tab">BD</div>);
      renderWithProviders(
        <HttpFormMode
          parsed={mkParsed()}
          bodyMode="json"
          onChange={vi.fn()}
          onPickFile={vi.fn(async () => null)}
          InlineCM={PlainInlineCM}
          renderBodyTab={renderBodyTab}
        />,
      );

      await user.click(screen.getByRole("tab", { name: "Body" }));
      expect(renderBodyTab).toHaveBeenCalled();
      expect(screen.getByTestId("body-tab")).toBeInTheDocument();
    });
  });

  describe("empty state", () => {
    it("shows '(no params)' when params + pending are empty", () => {
      renderWithProviders(
        <HttpFormMode
          parsed={mkParsed()}
          bodyMode="none"
          onChange={vi.fn()}
          onPickFile={vi.fn(async () => null)}
          InlineCM={PlainInlineCM}
          renderBodyTab={() => null}
        />,
      );
      expect(screen.getByText("(no params)")).toBeInTheDocument();
    });

    it("shows '(no headers)' for empty headers tab", () => {
      renderWithProviders(
        <HttpFormMode
          parsed={mkParsed()}
          bodyMode="none"
          onChange={vi.fn()}
          onPickFile={vi.fn(async () => null)}
          InlineCM={PlainInlineCM}
          renderBodyTab={() => null}
        />,
      );
      // Both panels are mounted (Chakra tabs keep panels mounted with closed state)
      expect(screen.getByText("(no headers)")).toBeInTheDocument();
    });
  });
});
