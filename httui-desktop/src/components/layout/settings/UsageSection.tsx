import { useState, useEffect, useCallback } from "react";
import { Box, Flex, HStack, VStack, Text, Button } from "@chakra-ui/react";
import { useSettingsStore } from "@/stores/settings";
import { Switch } from "@/components/ui/switch";
import {
  getFeatureUsage,
  clearFeatureUsage,
  type FeatureUsage,
} from "@/lib/tauri/telemetry";

/** Per-day counts pivoted across the tracked features, ready to render. */
export interface DayUsage {
  date: string;
  http: number;
  db: number;
}

/**
 * Collapse the flat `(date, feature, count)` rows into one entry per day
 * with a column per tracked feature. Pure — exported for unit tests.
 */
export function pivotFeatureUsageByDay(rows: FeatureUsage[]): DayUsage[] {
  const byDate = new Map<string, DayUsage>();
  for (const row of rows) {
    const day = byDate.get(row.date) ?? { date: row.date, http: 0, db: 0 };
    if (row.feature === "http_block_run") day.http += row.count;
    else if (row.feature === "db_block_run") day.db += row.count;
    byDate.set(row.date, day);
  }
  return [...byDate.values()].sort((a, b) => a.date.localeCompare(b.date));
}

function getDateRange(days: number): { from: string; to: string } {
  const to = new Date();
  const from = new Date();
  from.setDate(from.getDate() - days);
  return {
    from: from.toISOString().slice(0, 10),
    to: to.toISOString().slice(0, 10),
  };
}

export function UsageSection() {
  const telemetryEnabled = useSettingsStore((s) => s.telemetryEnabled);
  const setTelemetryEnabled = useSettingsStore((s) => s.setTelemetryEnabled);
  const [days, setDays] = useState<DayUsage[]>([]);

  const refresh = useCallback(async () => {
    try {
      const { from, to } = getDateRange(30);
      setDays(pivotFeatureUsageByDay(await getFeatureUsage(from, to)));
    } catch (e) {
      console.error("Failed to load feature usage:", e);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const handleClear = useCallback(async () => {
    try {
      await clearFeatureUsage();
      await refresh();
    } catch (e) {
      console.error("Failed to clear feature usage:", e);
    }
  }, [refresh]);

  const totalHttp = days.reduce((s, d) => s + d.http, 0);
  const totalDb = days.reduce((s, d) => s + d.db, 0);
  const maxRuns = Math.max(1, ...days.map((d) => d.http + d.db));

  return (
    <Flex direction="column" gap={4}>
      {/* Opt-in */}
      <Box>
        <Text fontWeight="semibold" fontSize="sm" mb={3}>
          Usage tracking
        </Text>
        <Flex align="center" justify="space-between" gap={4}>
          <Flex direction="column" gap={0} flex={1}>
            <Text fontSize="sm">Record feature usage locally</Text>
            <Text fontSize="xs" color="fg.muted">
              Counts how often you run HTTP and database blocks. Aggregated per
              day on this machine only — no payloads, never uploaded. Off by
              default.
            </Text>
          </Flex>
          <Switch
            aria-label="Record feature usage locally"
            checked={telemetryEnabled}
            onCheckedChange={(d) => setTelemetryEnabled(d.checked)}
            size="sm"
          />
        </Flex>
      </Box>

      {/* Summary */}
      <HStack gap={2}>
        <StatCard label="HTTP runs (30d)" value={totalHttp} color="blue.400" />
        <StatCard label="DB runs (30d)" value={totalDb} color="purple.400" />
      </HStack>

      {/* Bar chart */}
      <Box>
        <Text fontSize="xs" fontWeight="semibold" color="fg.muted" mb={2}>
          Block runs per day (last 30 days)
        </Text>
        {days.length === 0 ? (
          <Text fontSize="sm" color="fg.muted" textAlign="center" py={4}>
            {telemetryEnabled
              ? "No usage recorded yet"
              : "Tracking is off — enable it above to start recording"}
          </Text>
        ) : (
          <VStack gap={0.5} align="stretch">
            {days.map((day) => {
              const total = day.http + day.db;
              const pct = (total / maxRuns) * 100;
              const httpPct = total > 0 ? (day.http / total) * pct : 0;
              const dbPct = total > 0 ? (day.db / total) * pct : 0;
              return (
                <HStack key={day.date} gap={1.5} h="18px">
                  <Text
                    fontSize="2xs"
                    color="fg.muted"
                    w="45px"
                    flexShrink={0}
                    textAlign="right"
                  >
                    {day.date.slice(5)}
                  </Text>
                  <Flex
                    flex={1}
                    h="12px"
                    rounded="sm"
                    overflow="hidden"
                    bg="bg.subtle"
                  >
                    {httpPct > 0 && (
                      <Box
                        w={`${httpPct}%`}
                        bg="blue.400"
                        transition="width 0.2s"
                      />
                    )}
                    {dbPct > 0 && (
                      <Box
                        w={`${dbPct}%`}
                        bg="purple.400"
                        transition="width 0.2s"
                      />
                    )}
                  </Flex>
                  <Text fontSize="2xs" color="fg.muted" w="35px" flexShrink={0}>
                    {total}
                  </Text>
                </HStack>
              );
            })}
          </VStack>
        )}
      </Box>

      {/* Legend */}
      <HStack gap={3} justifyContent="center">
        <HStack gap={1}>
          <Box w="8px" h="8px" rounded="sm" bg="blue.400" />
          <Text fontSize="2xs" color="fg.muted">
            HTTP
          </Text>
        </HStack>
        <HStack gap={1}>
          <Box w="8px" h="8px" rounded="sm" bg="purple.400" />
          <Text fontSize="2xs" color="fg.muted">
            DB
          </Text>
        </HStack>
      </HStack>

      <Flex justify="flex-end">
        <Button
          size="xs"
          variant="outline"
          onClick={handleClear}
          disabled={totalHttp + totalDb === 0}
        >
          Clear usage data
        </Button>
      </Flex>
    </Flex>
  );
}

function StatCard({
  label,
  value,
  color,
}: {
  label: string;
  value: number;
  color: string;
}) {
  return (
    <Box
      flex={1}
      bg="bg.subtle"
      border="1px solid"
      borderColor="border"
      rounded="md"
      px={2}
      py={1.5}
      textAlign="center"
    >
      <Text fontSize="md" fontWeight="bold" color={color}>
        {value}
      </Text>
      <Text fontSize="2xs" color="fg.muted">
        {label}
      </Text>
    </Box>
  );
}
