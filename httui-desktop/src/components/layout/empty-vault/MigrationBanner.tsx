import { Box, HStack, Text, chakra } from "@chakra-ui/react";
import { LuArrowRight, LuX } from "react-icons/lu";

import { Btn } from "@/components/atoms";

const DismissBtn = chakra("button");
const DocsLink = chakra("a");

export interface MigrationBannerProps {
  /** Click → run the v1 migration. */
  onMigrate: () => void;
  /** Click → hide the banner. Consumer should persist the dismissal. */
  onDismiss: () => void;
  /** Optional override of the docs link target. */
  docsHref?: string;
}

export function MigrationBanner({
  onMigrate,
  onDismiss,
  docsHref = "https://github.com/anthropics/httui-notes/blob/main/docs/MIGRATION.md",
}: MigrationBannerProps) {
  return (
    <HStack
      data-atom="migration-banner"
      data-testid="migration-banner"
      role="alert"
      gap={3}
      px={4}
      py={3}
      bg="brand.subtle"
      color="fg"
      borderBottomWidth="1px"
      borderBottomColor="border"
    >
      <Box flex={1} fontSize="13px" lineHeight={1.4}>
        <Text fontWeight={600}>
          MVP vault detected — run the v1 migration to unlock the new file
          layout.
        </Text>
        <Text fontSize="12px" color="fg.muted" mt={0.5}>
          See{" "}
          <DocsLink
            href={docsHref}
            target="_blank"
            rel="noreferrer"
            data-testid="migration-banner-docs"
            color="brand.fg"
            textDecoration="underline"
          >
            docs/MIGRATION.md
          </DocsLink>{" "}
          for what changes (vault data is backed up before any destructive
          write).
        </Text>
      </Box>
      <Btn
        variant="primary"
        data-testid="migration-banner-run"
        onClick={onMigrate}
        gap={2}
      >
        Run migration
        <LuArrowRight size={12} />
      </Btn>
      <DismissBtn
        type="button"
        data-testid="migration-banner-dismiss"
        aria-label="Dismiss migration banner"
        onClick={onDismiss}
        h="24px"
        w="24px"
        display="inline-flex"
        alignItems="center"
        justifyContent="center"
        bg="transparent"
        color="fg.subtle"
        cursor="pointer"
        borderRadius="4px"
        _hover={{ bg: "bg.muted", color: "fg.muted" }}
      >
        <LuX size={14} />
      </DismissBtn>
    </HStack>
  );
}
