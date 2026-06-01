import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

const badgeVariants = cva(
  "inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium transition-colors",
  {
    variants: {
      variant: {
        default:
          "bg-primary/15 text-primary border border-primary/20",
        secondary:
          "bg-surface-raised text-muted-foreground border border-border",
        accent:
          "bg-accent/15 text-accent border border-accent/20",
        destructive:
          "bg-destructive/15 text-destructive border border-destructive/20",
        success:
          "bg-success/15 text-success border border-success/20",
        warning:
          "bg-warning/15 text-warning border border-warning/20",
        outline:
          "border border-border text-foreground",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  }
);

export interface BadgeProps
  extends React.HTMLAttributes<HTMLSpanElement>,
    VariantProps<typeof badgeVariants> {}

function Badge({ className, variant, ...props }: BadgeProps) {
  return <span className={cn(badgeVariants({ variant, className }))} {...props} />;
}

export { Badge, badgeVariants };
