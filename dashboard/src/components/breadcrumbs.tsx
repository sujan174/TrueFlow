"use client";

import { usePathname } from "next/navigation";
import Link from "next/link";
import { ChevronRight } from "lucide-react";
import { Fragment } from "react";

const ROUTE_LABELS: Record<string, string> = {
    "audit": "Audit Logs",
    "tokens": "Tokens",
    "virtual-keys": "Virtual Keys",
    "credentials": "The Vault",
    "policies": "Policies",
    "approvals": "Approvals",
    "analytics": "Analytics",
    "guardrails": "Guardrails",
    "cache": "Cache",
    "playground": "Playground",
    "sessions": "Sessions",
    "settings": "Settings",
    "experiments": "Experiments",
};

export function Breadcrumbs() {
    const pathname = usePathname();
    const segments = pathname.split("/").filter(Boolean);

    if (segments.length === 0) return null;

    return (
        <nav className="flex items-center text-[13px] font-medium text-zinc-500">
            <Link href="/" className="hover:text-white transition-colors">
                TrueFlow
            </Link>
            {segments.map((segment, index) => {
                const isLast = index === segments.length - 1;
                const path = `/${segments.slice(0, index + 1).join("/")}`;
                const label = ROUTE_LABELS[segment] || segment.charAt(0).toUpperCase() + segment.slice(1);

                return (
                    <Fragment key={path}>
                        <span className="text-zinc-700 mx-2 select-none">/</span>
                        {isLast ? (
                            <span className="text-white truncate max-w-[400px]">
                                {label}
                            </span>
                        ) : (
                            <Link href={path} className="hover:text-white transition-colors truncate max-w-[150px]">
                                {label}
                            </Link>
                        )}
                    </Fragment>
                );
            })}
        </nav>
    );
}
