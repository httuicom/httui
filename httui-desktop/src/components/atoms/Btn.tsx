import { Button, type ButtonProps } from "@chakra-ui/react";
import { forwardRef } from "react";

export type BtnVariant = "primary" | "ghost";

export type BtnProps = Omit<ButtonProps, "variant" | "size"> & {
  variant?: BtnVariant;
};

export const Btn = forwardRef<HTMLButtonElement, BtnProps>(function Btn(
  { variant = "primary", children, ...rest }: BtnProps,
  ref,
) {
  const palette =
    variant === "primary"
      ? {
          bg: "brand.fg",
          color: "brand.contrast",
          _hover: {
            bg: "brand.fg",
            opacity: 0.9,
          },
          _active: {
            bg: "brand.fg",
            opacity: 0.8,
          },
          fontWeight: 600,
          borderColor: "transparent",
        }
      : {
          bg: "transparent",
          color: "fg",
          _hover: { bg: "bg.muted" },
          _active: { bg: "bg.emphasized" },
          fontWeight: 500,
          borderColor: "transparent",
        };

  return (
    <Button
      ref={ref}
      data-atom="btn"
      data-variant={variant}
      h="24px"
      minH="24px"
      px="10px"
      py={0}
      borderRadius="4px"
      fontSize="12px"
      lineHeight={1}
      borderWidth="1px"
      {...palette}
      {...rest}
    >
      {children}
    </Button>
  );
});
