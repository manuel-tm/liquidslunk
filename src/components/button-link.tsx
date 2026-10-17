import type { VariantProps } from "class-variance-authority"
import { Link, type LinkProps } from "react-router"

import { buttonVariants } from "@/components/ui/button"
import { cn } from "@/lib/utils"

/**
 * A link that looks like a button. It stays a link for assistive tech,
 * unlike a Button rendering a Link, which Base UI gives role="button".
 */
export function ButtonLink({
  className,
  variant,
  size,
  ...props
}: LinkProps & VariantProps<typeof buttonVariants>) {
  return (
    <Link
      data-slot="button"
      className={cn(buttonVariants({ variant, size }), className)}
      {...props}
    />
  )
}
