import { describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { useRunAllPreflightGate } from "@/hooks/useRunAllPreflightGate";
import type { PreflightPillItem } from "@/components/blocks/preflight/PreflightPills";
import { renderWithProviders, screen } from "@/test/render";

function pass(idx = 0): PreflightPillItem {
  return {
    id: `${idx}-pass`,
    label: `pass-${idx}`,
    result: { outcome: "pass" },
  };
}

function fail(idx = 0): PreflightPillItem {
  return {
    id: `${idx}-fail`,
    label: `fail-${idx}`,
    result: { outcome: "fail", reason: "boom" },
  };
}

function skip(idx = 0): PreflightPillItem {
  return {
    id: `${idx}-skip`,
    label: `skip-${idx}`,
    result: { outcome: "skip", reason: "n/a" },
  };
}

interface HostProps {
  items: PreflightPillItem[];
  onRunAll: (
    ...args: Parameters<
      Parameters<typeof useRunAllPreflightGate>[0]["onRunAll"]
    >
  ) => void;
  initialOverride?: boolean;
}

function HostWithButton({ items, onRunAll, initialOverride }: HostProps) {
  const { trigger, dialog } = useRunAllPreflightGate({ items, onRunAll });
  return (
    <>
      <button
        data-testid="host-trigger"
        onClick={() => trigger(initialOverride)}
      >
        run-all
      </button>
      {dialog}
    </>
  );
}

describe("useRunAllPreflightGate", () => {
  it("runs immediately when there are no preflight items", async () => {
    const user = userEvent.setup();
    const onRunAll = vi.fn();
    renderWithProviders(<HostWithButton items={[]} onRunAll={onRunAll} />);
    await user.click(screen.getByTestId("host-trigger"));
    expect(onRunAll).toHaveBeenCalledTimes(1);
    expect(onRunAll.mock.calls[0]![0]).toMatchObject({
      block: false,
      failedCount: 0,
      skippedCount: 0,
    });
    expect(
      screen.queryByTestId("preflight-run-all-confirm"),
    ).not.toBeInTheDocument();
  });

  it("runs immediately when every check passes", async () => {
    const user = userEvent.setup();
    const onRunAll = vi.fn();
    renderWithProviders(
      <HostWithButton items={[pass(0), pass(1)]} onRunAll={onRunAll} />,
    );
    await user.click(screen.getByTestId("host-trigger"));
    expect(onRunAll).toHaveBeenCalledTimes(1);
    expect(onRunAll.mock.calls[0]![0].block).toBe(false);
  });

  it("opens the confirmation dialog when at least one check fails", async () => {
    const user = userEvent.setup();
    const onRunAll = vi.fn();
    renderWithProviders(
      <HostWithButton items={[pass(0), fail(1)]} onRunAll={onRunAll} />,
    );
    await user.click(screen.getByTestId("host-trigger"));
    expect(onRunAll).not.toHaveBeenCalled();
    const dialog = screen.getByTestId("preflight-run-all-confirm");
    expect(dialog).toBeInTheDocument();
    expect(dialog.textContent).toMatch(/1 pre-flight check failed/);
  });

  it("Cancel closes the dialog without running", async () => {
    const user = userEvent.setup();
    const onRunAll = vi.fn();
    renderWithProviders(
      <HostWithButton items={[fail(0)]} onRunAll={onRunAll} />,
    );
    await user.click(screen.getByTestId("host-trigger"));
    await user.click(screen.getByTestId("preflight-run-all-confirm-cancel"));
    expect(
      screen.queryByTestId("preflight-run-all-confirm"),
    ).not.toBeInTheDocument();
    expect(onRunAll).not.toHaveBeenCalled();
  });

  it("Run anyway closes the dialog and fires onRunAll with override audit note", async () => {
    const user = userEvent.setup();
    const onRunAll = vi.fn();
    renderWithProviders(
      <HostWithButton items={[fail(0), fail(1)]} onRunAll={onRunAll} />,
    );
    await user.click(screen.getByTestId("host-trigger"));
    await user.click(
      screen.getByTestId("preflight-run-all-confirm-run-anyway"),
    );
    expect(onRunAll).toHaveBeenCalledTimes(1);
    const decision = onRunAll.mock.calls[0]![0];
    expect(decision.block).toBe(false);
    expect(decision.failedCount).toBe(2);
    expect(decision.auditNote).toMatch(/ran anyway via shift/);
    expect(
      screen.queryByTestId("preflight-run-all-confirm"),
    ).not.toBeInTheDocument();
  });

  it("trigger(true) bypasses the gate entirely", async () => {
    const user = userEvent.setup();
    const onRunAll = vi.fn();
    renderWithProviders(
      <HostWithButton items={[fail(0)]} onRunAll={onRunAll} initialOverride />,
    );
    await user.click(screen.getByTestId("host-trigger"));
    expect(
      screen.queryByTestId("preflight-run-all-confirm"),
    ).not.toBeInTheDocument();
    expect(onRunAll).toHaveBeenCalledTimes(1);
    expect(onRunAll.mock.calls[0]![0].auditNote).toMatch(/ran anyway/);
  });

  it("the skipped-count copy appears when there are skipped checks alongside failures", async () => {
    const user = userEvent.setup();
    const onRunAll = vi.fn();
    renderWithProviders(
      <HostWithButton
        items={[fail(0), skip(1), skip(2)]}
        onRunAll={onRunAll}
      />,
    );
    await user.click(screen.getByTestId("host-trigger"));
    const note = screen.getByTestId("preflight-run-all-confirm-skipped");
    expect(note.textContent).toMatch(/2 pre-flight checks skipped/);
  });

  it("clicking the overlay cancels", async () => {
    const user = userEvent.setup();
    const onRunAll = vi.fn();
    renderWithProviders(
      <HostWithButton items={[fail(0)]} onRunAll={onRunAll} />,
    );
    await user.click(screen.getByTestId("host-trigger"));
    await user.click(screen.getByTestId("preflight-run-all-confirm-overlay"));
    expect(
      screen.queryByTestId("preflight-run-all-confirm"),
    ).not.toBeInTheDocument();
    expect(onRunAll).not.toHaveBeenCalled();
  });
});
