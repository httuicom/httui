import { Box, Text } from "@chakra-ui/react";
import {
  BlocksPreview,
  GitDiffPreview,
  SchemaPreview,
} from "./marketing/previews";
import { Hero } from "./sections/Hero";
import { HeroPreview } from "./sections/HeroPreview";
import { OssStrip } from "./sections/OssStrip";
import { FeatureRow } from "./sections/FeatureRow";
import { InstallSection } from "./sections/InstallSection";
import { CtaSection } from "./sections/CtaSection";
import { Footer } from "./sections/Footer";

function InlineCode({ children }: { children: React.ReactNode }) {
  return (
    <Text as="code" fontFamily="mono" px={1} bg="bg.elevated" rounded="sm">
      {children}
    </Text>
  );
}

function Bold({ children }: { children: React.ReactNode }) {
  return (
    <Text as="b" color="fg">
      {children}
    </Text>
  );
}

const blocksPoints = [
  <>
    <Bold>Chained captures</Bold> — reuse a value from one response as{" "}
    <InlineCode>{"{{login.body.token}}"}</InlineCode> in the next block.
  </>,
  <>
    <Bold>Assertions</Bold> — an <InlineCode># expect:</InlineCode> section
    with lines like <InlineCode>time {"<"} 500ms</InlineCode> fails the
    runbook on regression.
  </>,
  <>
    <Bold>Variables &amp; secrets</Bold> referenced by key. The value never
    touches git.
  </>,
];

const schemaPoints = [
  <>
    <Bold>Multi-database</Bold> in a single runbook — query PostgreSQL, then
    MySQL, without switching windows.
  </>,
  <>
    <Bold>Mutation guard</Bold> — destructive SQL (UPDATE / DELETE) warns
    before it runs.
  </>,
  <>
    <Bold>EXPLAIN ANALYZE</Bold> in tree form, with slow sequential scans
    highlighted.
  </>,
];

const gitPoints = [
  <>
    <Bold>PR review</Bold> for runbooks on GitHub or GitLab.
  </>,
  <>
    <Bold>Diff between runs</Bold> — compare today's execution with yesterday's
    in two clicks.
  </>,
  <>
    <Bold>Share the repo URL</Bold> — HTTPS, SSH, or web — for any runbook in
    one click.
  </>,
];

export default function App() {
  return (
    <Box bg="bg" color="fg" fontFamily="body">
      <Hero />
      <HeroPreview />
      <OssStrip />

      <FeatureRow
        kicker="One file · many blocks"
        title="Markdown that runs."
        body="Each block is executable: HTTP requests and SQL queries (PostgreSQL, MySQL, SQLite). Captures from one block become variables for the next, chaining the entire flow inside a single .md."
        points={blocksPoints}
        preview={<BlocksPreview />}
      />

      <FeatureRow
        reverse
        kicker="Database-native"
        title="Schema explorer next to the editor."
        body="Connect PostgreSQL, MySQL, and SQLite. Browse tables with foreign keys, indexes, and row counts. EXPLAIN ANALYZE in tree form shows where your query spends time."
        points={schemaPoints}
        preview={<SchemaPreview />}
      />

      <FeatureRow
        kicker="Git-native · diffable"
        title="Versioned. Reviewable. Sharable."
        body="Runbooks are .md files in your repo. Pull request review like any other code. Diff between runs shows what changed in the response across executions."
        points={gitPoints}
        preview={<GitDiffPreview />}
      />

      <InstallSection />
      <CtaSection />
      <Footer />
    </Box>
  );
}
