// Input atom (design canvas §0).
//
// 24px tall, mono font, focus outline `--accent`. Wraps Chakra
// `<Input>` with the canvas spec sizes and tags itself with
// `data-atom="input"` for test/style hooks. Keeps the full Chakra
// prop surface so consumers (search bars, env switcher overrides,
// inline value cells) can use it as a drop-in.

import { Input as ChakraInput, type InputProps } from "@chakra-ui/react";
import { forwardRef } from "react";

export type InputAtomProps = InputProps;

export const Input = forwardRef<HTMLInputElement, InputAtomProps>(
  function Input({ ...rest }, ref) {
    return (
      <ChakraInput
        ref={ref}
        data-atom="input"
        h="24px"
        minH="24px"
        px="8px"
        py={0}
        fontFamily="mono"
        fontSize="12px"
        lineHeight={1}
        borderRadius="4px"
        borderWidth="1px"
        borderColor="border"
        bg="bg"
        color="fg"
        _placeholder={{ color: "fg.subtle" }}
        _focusVisible={{
          borderColor: "brand.fg",
          boxShadow: "0 0 0 1px var(--chakra-colors-accent)",
          outline: "none",
        }}
        {...rest}
      />
    );
  },
);
