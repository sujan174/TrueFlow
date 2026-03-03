import { TooltipProps } from "recharts";
import { NameType, ValueType } from "recharts/types/component/DefaultTooltipContent";

interface CustomTooltipProps extends Omit<TooltipProps<ValueType, NameType>, "labelFormatter"> {
    valueFormatter?: (value: number) => string;
    labelFormatter?: (label: string) => string;
}

export function CustomTooltip({
    active,
    payload,
    label,
    valueFormatter,
    labelFormatter
}: any) {
    if (!active || !payload || payload.length === 0) {
        return null;
    }

    return (
        <div className="rounded-md border border-border/50 bg-background/95 p-3 shadow-xl backdrop-blur-sm">
            <p className="mb-2 text-xs font-medium text-muted-foreground uppercase tracking-wider">
                {labelFormatter ? labelFormatter(label as string) : label}
            </p>
            <div className="flex flex-col gap-1.5">
                {payload.map((entry: any, index: number) => (
                    <div key={index} className="flex items-center justify-between gap-4">
                        <div className="flex items-center gap-2">
                            <div
                                className="h-2.5 w-2.5 rounded-[2px]"
                                style={{ backgroundColor: entry.color }}
                            />
                            <span className="text-sm font-medium text-foreground">
                                {entry.name}
                            </span>
                        </div>
                        <span className="text-sm font-semibold font-mono text-foreground">
                            {valueFormatter && typeof entry.value === 'number'
                                ? valueFormatter(entry.value)
                                : entry.value}
                        </span>
                    </div>
                ))}
            </div>
        </div>
    );
}

// Chart axis defaults
export const CHART_AXIS_PROPS = {
    stroke: "#5e503f",
    fontSize: 11,
    tickLine: false,
    axisLine: false,
    tickMargin: 10,
};
