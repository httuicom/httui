import { useCallback, useState } from "react";
import { Box, Text } from "@chakra-ui/react";

import { useMigrationDetection } from "@/hooks/useMigrationDetection";
import { migrateVaultToV1 } from "@/lib/tauri/migration";
import { MigrationBanner } from "@/components/layout/empty-vault/MigrationBanner";

interface MigrationBannerHostProps {
  vaultPath: string;
}

type MigrateStatus =
  | { kind: "idle" }
  | { kind: "running" }
  | { kind: "error"; message: string }
  | { kind: "success"; summary: string };

export function MigrationBannerHost({ vaultPath }: MigrationBannerHostProps) {
  const { shouldShowBanner, dismiss, refresh } =
    useMigrationDetection(vaultPath);
  const [status, setStatus] = useState<MigrateStatus>({ kind: "idle" });

  const handleMigrate = useCallback(async () => {
    setStatus({ kind: "running" });
    try {
      const report = await migrateVaultToV1(vaultPath, false);
      const summary = `${report.connections_migrated} connection(s), ${report.environments_migrated} environment(s), ${report.variables_migrated} variable(s)`;
      setStatus({ kind: "success", summary });
      refresh();
    } catch (err) {
      setStatus({
        kind: "error",
        message: err instanceof Error ? err.message : String(err),
      });
    }
  }, [vaultPath, refresh]);

  // Keep this after the migrate status check so a success summary still
  // renders for one paint after refresh() clears the banner.
  if (!shouldShowBanner && status.kind !== "success") return null;

  return (
    <Box data-testid="migration-banner-host">
      {shouldShowBanner && (
        <MigrationBanner onMigrate={handleMigrate} onDismiss={dismiss} />
      )}
      {status.kind === "running" && (
        <Box
          data-testid="migration-running"
          px={4}
          py={2}
          bg="bg.muted"
          borderBottomWidth="1px"
          borderBottomColor="border"
        >
          <Text fontSize="12px" color="fg.muted">
            Migrating vault…
          </Text>
        </Box>
      )}
      {status.kind === "error" && (
        <Box
          data-testid="migration-error"
          px={4}
          py={2}
          bg="red.50"
          color="red.900"
          borderBottomWidth="1px"
          borderBottomColor="red.200"
        >
          <Text fontSize="12px" fontWeight={600}>
            Migration failed
          </Text>
          <Text fontSize="12px" mt={0.5}>
            {status.message}
          </Text>
        </Box>
      )}
      {status.kind === "success" && (
        <Box
          data-testid="migration-success"
          px={4}
          py={2}
          bg="green.50"
          color="green.900"
          borderBottomWidth="1px"
          borderBottomColor="green.200"
        >
          <Text fontSize="12px" fontWeight={600}>
            Migration complete — {status.summary}.
          </Text>
        </Box>
      )}
    </Box>
  );
}
