import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { Provider as ChakraProvider } from "@/components/ui/provider";
import { AboutSection } from "@/components/layout/settings/AboutSection";

// matchMedia mock is set up globally in src/test/setup.ts so Chakra v3's
// next-themes dep can load.

function renderAboutSection() {
  return render(
    <ChakraProvider>
      <AboutSection />
    </ChakraProvider>,
  );
}

describe("AboutSection", () => {
  it("renders the app identity card with name + version badge", () => {
    renderAboutSection();
    expect(screen.getByText("Notes")).toBeInTheDocument();
    // Version uses the literal `v{APP_VERSION}` template.
    expect(screen.getByText(/^v\d/)).toBeInTheDocument();
    expect(
      screen.getByText("Desktop markdown editor with executable blocks"),
    ).toBeInTheDocument();
  });

  it("renders the About paragraph", () => {
    renderAboutSection();
    expect(
      screen.getByText(/Notes is a desktop markdown editor/i),
    ).toBeInTheDocument();
  });

  it("lists each tech-stack entry with its label and value", () => {
    renderAboutSection();
    // CodeMirror 6 — kept current after the TipTap migration.
    expect(screen.getByText("Editor")).toBeInTheDocument();
    expect(screen.getByText("CodeMirror 6")).toBeInTheDocument();
    expect(screen.getByText("Backend")).toBeInTheDocument();
    expect(screen.getByText("Tauri v2 (Rust)")).toBeInTheDocument();
    expect(screen.getByText("Database")).toBeInTheDocument();
    expect(screen.getByText("SQLite (sqlx)")).toBeInTheDocument();
    expect(screen.getByText("AI")).toBeInTheDocument();
    expect(screen.getByText("Claude (Anthropic SDK)")).toBeInTheDocument();
    expect(screen.getByText("Frontend")).toBeInTheDocument();
    expect(
      screen.getByText("React + TypeScript + Chakra UI v3"),
    ).toBeInTheDocument();
  });

  it("surfaces the four security-summary badges", () => {
    renderAboutSection();
    expect(screen.getByText("Keychain")).toBeInTheDocument();
    expect(screen.getByText("Read-only")).toBeInTheDocument();
    expect(screen.getByText("Sandboxed")).toBeInTheDocument();
    expect(screen.getByText("Signed")).toBeInTheDocument();
  });

  it("describes data storage (notes.db + .md files)", () => {
    renderAboutSection();
    expect(screen.getByText("notes.db")).toBeInTheDocument();
    expect(screen.getByText(".md")).toBeInTheDocument();
    expect(screen.getByText(/in your vault directory/i)).toBeInTheDocument();
  });
});
