import { useEffect } from "react";
import { Box, Button, Flex, Portal, Text } from "@chakra-ui/react";

interface ConfirmRunDialogProps {
  reason: string;
  onCancel: () => void;
  onConfirm: () => void;
}

/**
 * Read-only / unscoped-write guard. Portal + Box (not Chakra Dialog) so
 * closing the dialog doesn't steal focus from CM6 when the user clicks
 * Cancel.
 */
export function ConfirmRunDialog({
  reason,
  onCancel,
  onConfirm,
}: ConfirmRunDialogProps) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onCancel();
      if (e.key === "Enter") onConfirm();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onCancel, onConfirm]);

  return (
    <Portal>
      {/* scrim */}
      <Box
        position="fixed"
        top={0}
        right={0}
        bottom={0}
        left={0}
        bg="blackAlpha.600"
        zIndex={2000}
        onClick={onCancel}
      />
      {/* card */}
      <Box
        position="fixed"
        top="50%"
        left="50%"
        transform="translate(-50%, -50%)"
        w="420px"
        maxW="calc(100vw - 32px)"
        bg="bg"
        borderWidth="1px"
        borderColor="border"
        borderRadius="md"
        boxShadow="xl"
        zIndex={2001}
        onMouseDown={(e) => e.stopPropagation()}
      >
        <Box px={5} py={4} borderBottomWidth="1px" borderColor="border">
          <Text fontWeight="semibold" fontSize="sm">
            Run this query?
          </Text>
        </Box>
        <Box px={5} py={4}>
          <Text fontSize="sm" color="fg.muted">
            {reason}
          </Text>
        </Box>
        <Flex
          px={5}
          py={3}
          borderTopWidth="1px"
          borderColor="border"
          justify="flex-end"
          gap={2}
        >
          <Button size="sm" variant="outline" onClick={onCancel}>
            Cancel
          </Button>
          <Button size="sm" colorPalette="orange" onClick={onConfirm}>
            Run anyway
          </Button>
        </Flex>
      </Box>
    </Portal>
  );
}
