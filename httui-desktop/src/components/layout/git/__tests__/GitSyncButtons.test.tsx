import { describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { GitSyncButtons } from "@/components/layout/git/GitSyncButtons";
import { renderWithProviders, screen } from "@/test/render";

describe("GitSyncButtons", () => {
  it("renders nothing inside the row when no callbacks are provided", () => {
    renderWithProviders(<GitSyncButtons />);
    expect(screen.getByTestId("git-sync-buttons")).toBeInTheDocument();
    expect(screen.queryByTestId("git-sync-fetch")).not.toBeInTheDocument();
    expect(screen.queryByTestId("git-sync-pull")).not.toBeInTheDocument();
    expect(screen.queryByTestId("git-sync-push")).not.toBeInTheDocument();
  });

  it("renders each button when its handler is supplied", () => {
    renderWithProviders(
      <GitSyncButtons onFetch={() => {}} onPull={() => {}} onPush={() => {}} />,
    );
    expect(screen.getByTestId("git-sync-fetch")).toBeInTheDocument();
    expect(screen.getByTestId("git-sync-pull")).toBeInTheDocument();
    expect(screen.getByTestId("git-sync-push")).toBeInTheDocument();
  });

  it("fires onFetch / onPull / onPush on click", async () => {
    const onFetch = vi.fn();
    const onPull = vi.fn();
    const onPush = vi.fn();
    renderWithProviders(
      <GitSyncButtons onFetch={onFetch} onPull={onPull} onPush={onPush} />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("git-sync-fetch"));
    await user.click(screen.getByTestId("git-sync-pull"));
    await user.click(screen.getByTestId("git-sync-push"));
    expect(onFetch).toHaveBeenCalledTimes(1);
    expect(onPull).toHaveBeenCalledTimes(1);
    expect(onPush).toHaveBeenCalledTimes(1);
  });

  it("disables every button while an op is in flight", () => {
    renderWithProviders(
      <GitSyncButtons
        inFlight="fetch"
        onFetch={() => {}}
        onPull={() => {}}
        onPush={() => {}}
      />,
    );
    expect(
      (screen.getByTestId("git-sync-fetch") as HTMLButtonElement).disabled,
    ).toBe(true);
    expect(
      (screen.getByTestId("git-sync-pull") as HTMLButtonElement).disabled,
    ).toBe(true);
    expect(
      (screen.getByTestId("git-sync-push") as HTMLButtonElement).disabled,
    ).toBe(true);
  });

  it("flips the in-flight button label and adds data-in-flight", () => {
    renderWithProviders(
      <GitSyncButtons
        inFlight="pull"
        onFetch={() => {}}
        onPull={() => {}}
        onPush={() => {}}
      />,
    );
    const pullBtn = screen.getByTestId("git-sync-pull");
    expect(pullBtn.textContent).toBe("Pulling…");
    expect(pullBtn.getAttribute("data-in-flight")).toBe("true");
    // Other buttons keep their idle label.
    expect(screen.getByTestId("git-sync-fetch").textContent).toBe("Fetch");
    expect(screen.getByTestId("git-sync-push").textContent).toBe("Push");
  });

  it("disables Push and shows the no-remote hint when hasRemote is false", () => {
    renderWithProviders(<GitSyncButtons hasRemote={false} onPush={() => {}} />);
    const pushBtn = screen.getByTestId("git-sync-push") as HTMLButtonElement;
    expect(pushBtn.disabled).toBe(true);
    expect(screen.getByTestId("git-sync-no-remote-hint")).toBeInTheDocument();
    expect(
      screen.getByTestId("git-sync-buttons").getAttribute("data-no-remote"),
    ).toBe("true");
  });

  it("disables fetch + pull + push together when no remote (cenário 8)", () => {
    renderWithProviders(
      <GitSyncButtons
        hasRemote={false}
        onFetch={() => {}}
        onPull={() => {}}
        onPush={() => {}}
      />,
    );
    expect(
      (screen.getByTestId("git-sync-fetch") as HTMLButtonElement).disabled,
    ).toBe(true);
    expect(
      (screen.getByTestId("git-sync-pull") as HTMLButtonElement).disabled,
    ).toBe(true);
    expect(
      (screen.getByTestId("git-sync-push") as HTMLButtonElement).disabled,
    ).toBe(true);
  });

  it("hides the no-remote hint when hasRemote is true", () => {
    renderWithProviders(<GitSyncButtons onPush={() => {}} />);
    expect(
      screen.queryByTestId("git-sync-no-remote-hint"),
    ).not.toBeInTheDocument();
  });

  it("renders the configure-remote link when callback is provided", async () => {
    const onConfigureRemote = vi.fn();
    renderWithProviders(
      <GitSyncButtons
        hasRemote={false}
        onPush={() => {}}
        onConfigureRemote={onConfigureRemote}
      />,
    );
    const link = screen.getByTestId("git-sync-configure-remote");
    await userEvent.setup().click(link);
    expect(onConfigureRemote).toHaveBeenCalledTimes(1);
  });

  it("hides the no-remote hint entirely when onPush is missing", () => {
    // No Push button → no hint to render either, even with hasRemote false.
    renderWithProviders(<GitSyncButtons hasRemote={false} />);
    expect(
      screen.queryByTestId("git-sync-no-remote-hint"),
    ).not.toBeInTheDocument();
  });

  it("encodes the in-flight op via root data-in-flight attribute", () => {
    renderWithProviders(<GitSyncButtons inFlight="push" onPush={() => {}} />);
    expect(
      screen.getByTestId("git-sync-buttons").getAttribute("data-in-flight"),
    ).toBe("push");
  });
});
