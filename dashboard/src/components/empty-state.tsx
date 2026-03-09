import { LucideIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

interface EmptyStateProps {
    icon: LucideIcon;
    title: string;
    description: string;
    actionLabel?: string;
    onAction?: () => void;
    className?: string;
}

export function EmptyState({
    icon: Icon,
    title,
    description,
    actionLabel,
    onAction,
    className,
}: EmptyStateProps) {
    return (
        <div className={cn(
            "flex min-h-[400px] flex-col items-center justify-center rounded-md border border-dashed border-white/10 p-4 text-center animate-in fade-in-50 bg-black",
            className
        )}>
            <div className="flex h-16 w-16 items-center justify-center rounded-full bg-white/[0.02] border border-white/[0.05]">
                <Icon className="h-6 w-6 text-zinc-500" />
            </div>
            <h3 className="mt-4 text-[13px] font-medium text-white tracking-wide uppercase">{title}</h3>
            <p className="mb-4 mt-2 max-w-sm text-[13px] text-zinc-500 text-balance">
                {description}
            </p>
            {actionLabel && onAction && (
                <Button onClick={onAction} size="sm" className="mt-2 text-xs">
                    {actionLabel}
                </Button>
            )}
        </div>
    );
}
