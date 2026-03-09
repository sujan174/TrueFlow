import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { cn } from "@/lib/utils"

const badgeVariants = cva(
    "inline-flex items-center rounded-sm border px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-widest transition-colors focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2",
    {
        variants: {
            variant: {
                default:
                    "border-white/10 bg-white/5 text-zinc-300 hover:bg-white/10",
                secondary:
                    "border-white/5 bg-transparent text-zinc-500 hover:bg-white/5",
                destructive:
                    "border-rose-500/20 bg-rose-500/10 text-rose-400 hover:bg-rose-500/20",
                outline: "text-zinc-400 border-white/10",
                success:
                    "border-white/20 bg-white/10 text-white hover:bg-white/20",
                warning:
                    "border-amber-500/20 bg-amber-500/10 text-amber-400 hover:bg-amber-500/20",
            },
        },
        defaultVariants: {
            variant: "default",
        },
    }
)

export type BadgeProps = React.HTMLAttributes<HTMLDivElement> &
    VariantProps<typeof badgeVariants> & {
        dot?: boolean
    }

function Badge({ className, variant, dot, children, ...props }: BadgeProps) {
    return (
        <div className={cn(badgeVariants({ variant }), className)} {...props}>
            {dot && <span className={cn("mr-1.5 h-1.5 w-1.5 rounded-full",
                variant === 'success' ? 'bg-emerald-500' :
                    variant === 'warning' ? 'bg-amber-500' :
                        variant === 'destructive' ? 'bg-rose-500' :
                            'bg-current'
            )} />}
            {children}
        </div>
    )
}

export { Badge, badgeVariants }
