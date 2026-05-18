import { describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { SharePopover } from "@/components/layout/share/SharePopover";
import { renderWithProviders, screen } from "@/test/render";

const ORIGIN = {
  name: "origin",
  url: "git@github.com:owner/repo.git",
};
const UPSTREAM = {
  name: "upstream",
  url: "https://github.com/upstream/repo.git",
};

describe("SharePopover", () => {
  it("renders the empty-state when remotes is empty", () => {
    renderWithProviders(<SharePopover remotes={[]} onCopy={() => {}} />);
    const root = screen.getByTestId("share-popover");
    expect(root.getAttribute("data-state")).toBe("empty");
    expect(root.textContent).toMatch(/No remote configured/);
  });

  it("renders the configure-remote link in empty state when callback supplied", async () => {
    const onOpenWorkspaceSettings = vi.fn();
    renderWithProviders(
      <SharePopover
        remotes={[]}
        onCopy={() => {}}
        onOpenWorkspaceSettings={onOpenWorkspaceSettings}
      />,
    );
    const link = screen.getByTestId("share-popover-configure");
    await userEvent.setup().click(link);
    expect(onOpenWorkspaceSettings).toHaveBeenCalledTimes(1);
  });

  it("hides the configure-remote link when no callback supplied", () => {
    renderWithProviders(<SharePopover remotes={[]} onCopy={() => {}} />);
    expect(
      screen.queryByTestId("share-popover-configure"),
    ).not.toBeInTheDocument();
  });

  it("renders the URL of the only remote when there's exactly one", () => {
    renderWithProviders(<SharePopover remotes={[ORIGIN]} onCopy={() => {}} />);
    const url = screen.getByTestId("share-popover-url");
    expect(url.textContent).toBe(ORIGIN.url);
    // No remote picker shown for a single remote.
    expect(
      screen.queryByTestId(`share-popover-remote-${ORIGIN.name}`),
    ).not.toBeInTheDocument();
  });

  it("fires onCopy with the active URL on Copy click", async () => {
    const onCopy = vi.fn();
    renderWithProviders(<SharePopover remotes={[ORIGIN]} onCopy={onCopy} />);
    await userEvent.setup().click(screen.getByTestId("share-popover-copy"));
    expect(onCopy).toHaveBeenCalledWith(ORIGIN.url);
  });

  it("disables the Copy button + flips its label while copying", () => {
    renderWithProviders(
      <SharePopover remotes={[ORIGIN]} onCopy={() => {}} copying />,
    );
    const btn = screen.getByTestId("share-popover-copy") as HTMLButtonElement;
    expect(btn.disabled).toBe(true);
    expect(btn.textContent).toBe("Copying…");
  });

  it("renders the remote picker for multi-remote setups, defaulting to origin", () => {
    renderWithProviders(
      <SharePopover remotes={[UPSTREAM, ORIGIN]} onCopy={() => {}} />,
    );
    expect(
      screen
        .getByTestId(`share-popover-remote-${ORIGIN.name}`)
        .getAttribute("data-active"),
    ).toBe("true");
    // URL panel shows origin's URL by default.
    expect(screen.getByTestId("share-popover-url").textContent).toBe(
      ORIGIN.url,
    );
  });

  it("falls back to the first remote when there's no `origin`", () => {
    const FORK = { name: "fork", url: "git@github.com:me/repo.git" };
    renderWithProviders(
      <SharePopover remotes={[FORK, UPSTREAM]} onCopy={() => {}} />,
    );
    expect(
      screen
        .getByTestId(`share-popover-remote-${FORK.name}`)
        .getAttribute("data-active"),
    ).toBe("true");
  });

  it("switches the active remote on picker click", async () => {
    const onCopy = vi.fn();
    renderWithProviders(
      <SharePopover remotes={[ORIGIN, UPSTREAM]} onCopy={onCopy} />,
    );
    const user = userEvent.setup();
    await user.click(
      screen.getByTestId(`share-popover-remote-${UPSTREAM.name}`),
    );
    expect(screen.getByTestId("share-popover-url").textContent).toBe(
      UPSTREAM.url,
    );
    await user.click(screen.getByTestId("share-popover-copy"));
    expect(onCopy).toHaveBeenCalledWith(UPSTREAM.url);
  });

  it("encodes remote count via data-remote-count", () => {
    renderWithProviders(
      <SharePopover remotes={[ORIGIN, UPSTREAM]} onCopy={() => {}} />,
    );
    expect(
      screen.getByTestId("share-popover").getAttribute("data-remote-count"),
    ).toBe("2");
  });

  describe("open action", () => {
    const HTTPS = {
      name: "HTTPS",
      url: "https://github.com/a/b.git",
      openable: false,
    };
    const WEB = {
      name: "Web",
      url: "https://github.com/a/b",
      openable: true,
    };

    it("shows Open only for the active openable option", async () => {
      const onOpen = vi.fn();
      const user = userEvent.setup();
      renderWithProviders(
        <SharePopover
          remotes={[HTTPS, WEB]}
          onCopy={() => {}}
          onOpen={onOpen}
        />,
      );
      // Defaults to first (HTTPS, not openable) — no Open button.
      expect(
        screen.queryByTestId("share-popover-open"),
      ).not.toBeInTheDocument();
      await user.click(screen.getByTestId("share-popover-remote-Web"));
      await user.click(screen.getByTestId("share-popover-open"));
      expect(onOpen).toHaveBeenCalledWith(WEB.url);
    });

    it("never shows Open when onOpen is absent", () => {
      renderWithProviders(<SharePopover remotes={[WEB]} onCopy={() => {}} />);
      expect(
        screen.queryByTestId("share-popover-open"),
      ).not.toBeInTheDocument();
    });
  });
});
