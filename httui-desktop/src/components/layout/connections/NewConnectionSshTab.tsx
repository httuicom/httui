// Canvas §5 — "SSH tunnel" tab for the Nova Conexão modal
// (Epic 42 Story 06 — Phase 3).
//
// SSH tunnelling itself is v1.x scope per `out-of-scope.md`. For v1
// the tab just shows a "Coming soon" surface that explains the
// workaround (paste a parsed URL with the host/port behind the
// tunnel). Pure presentational; no state.

import { Box, Flex, Text } from "@chakra-ui/react";

export function NewConnectionSshTab() {
  return (
    <Flex
      data-testid="new-connection-ssh-tab"
      direction="column"
      gap={3}
    >
      <Box
        data-testid="new-connection-ssh-coming-soon"
        bg="bg.muted"
        borderWidth="1px"
        borderColor="border"
        borderRadius="8px"
        px={4}
        py={3}
      >
        <Text fontFamily="serif" fontSize="14px" fontWeight={500} color="fg">
          SSH tunnel — em breve
        </Text>
        <Text fontSize="12px" color="fg.muted" mt={1}>
          O assistente nativo (host, porta, jump-host, key file) entra
          numa próxima versão. No v1, conecte-se via tunnel local
          (<Mono>ssh -L</Mono>) e aponte a connection string para{" "}
          <Mono>localhost:&lt;porta-local&gt;</Mono>.
        </Text>
      </Box>

      <Box
        data-testid="new-connection-ssh-example"
        bg="bg.emphasized"
        borderWidth="1px"
        borderColor="border"
        borderRadius="6px"
        px={3}
        py={2}
        fontFamily="mono"
        fontSize="11px"
        color="fg.muted"
      >
        # cria o túnel local antes de salvar a conexão
        <br />
        ssh -L 6432:db.prod.internal:5432 bastion.example.com -N
      </Box>

      <Text fontSize="11px" color="fg.subtle">
        Quando o SSH nativo entrar, suas conexões existentes não vão
        precisar de migração — o túnel passa a ser apenas um detalhe da
        conexão, e o resto do formulário continua igual.
      </Text>
    </Flex>
  );
}

const Mono = ({ children }: { children: React.ReactNode }) => (
  <Text as="span" fontFamily="mono">
    {children}
  </Text>
);
