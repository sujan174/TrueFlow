"use client";

import { useEffect, useState } from "react";

export function CountUp({
    value,
    duration = 1000,
    decimals = 0,
    prefix = "",
    suffix = "",
}: {
    value: number;
    duration?: number;
    decimals?: number;
    prefix?: string;
    suffix?: string;
}) {
    const [count, setCount] = useState(0);

    useEffect(() => {
        let startTime: number | null = null;
        let animationFrame: number;

        const endValue = value;
        const startValue = count;

        const animate = (timestamp: number) => {
            if (!startTime) startTime = timestamp;
            const progress = timestamp - startTime;
            const percentage = Math.min(progress / duration, 1);

            // easeOutExpo
            const easeProgress = percentage === 1 ? 1 : 1 - Math.pow(2, -10 * percentage);

            const currentCount = startValue + (endValue - startValue) * easeProgress;
            setCount(currentCount);

            if (percentage < 1) {
                animationFrame = requestAnimationFrame(animate);
            } else {
                setCount(endValue);
            }
        };

        animationFrame = requestAnimationFrame(animate);

        return () => cancelAnimationFrame(animationFrame);
    }, [value, duration]);

    const formattedValue = count.toFixed(decimals);

    return (
        <span data-is-metric="true" className="font-mono tabular-nums tracking-tighter">
            {prefix}{formattedValue}{suffix}
        </span>
    );
}
