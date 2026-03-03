"use client";

import { useState, useEffect } from "react";
import {
    Zap,
    Shield,
    BarChart3,
    Sparkles,
    ArrowRight,
    ArrowLeft,
    X,
    Rocket,
    Key,
    Network,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

const ONBOARDING_KEY = "ailink_onboarding_complete";

interface OnboardingStep {
    icon: React.ElementType;
    iconColor: string;
    title: string;
    description: string;
    features: string[];
}

const STEPS: OnboardingStep[] = [
    {
        icon: Rocket,
        iconColor: "text-blue-400",
        title: "Welcome to AILink",
        description:
            "Your unified AI gateway for routing, observability, and cost management. Let's take a quick tour of what you can do.",
        features: [
            "Route requests to 200+ LLM providers",
            "Real-time analytics and cost tracking",
            "Enterprise-grade security and RBAC",
        ],
    },
    {
        icon: Key,
        iconColor: "text-emerald-400",
        title: "Create Your First Agent Token",
        description:
            "Agent tokens (virtual keys) authenticate your applications with the gateway. Each token tracks usage, cost, and can be rate-limited independently.",
        features: [
            "Navigate to Agents in the sidebar",
            "Click \"Create Token\" and give it a name",
            "Use the token in your API requests as a Bearer token",
        ],
    },
    {
        icon: Network,
        iconColor: "text-violet-400",
        title: "Configure Upstreams & Policies",
        description:
            "Upstreams define which LLM providers to route to. Policies let you add guardrails, rate limits, A/B testing, and caching rules.",
        features: [
            "Set up model pricing in the Upstreams tab",
            "Create policies with conditions and actions",
            "Use the Playground to test your configuration",
        ],
    },
    {
        icon: BarChart3,
        iconColor: "text-amber-400",
        title: "Monitor & Optimize",
        description:
            "Track every request with full observability. Use analytics to find cost savings and the audit log for compliance.",
        features: [
            "Analytics dashboard for traffic & latency trends",
            "Per-session cost tracking and spend caps",
            "Audit log with full request/response details",
        ],
    },
];

function StepIndicator({
    currentStep,
    totalSteps,
}: {
    currentStep: number;
    totalSteps: number;
}) {
    return (
        <div className="flex items-center gap-1.5">
            {Array.from({ length: totalSteps }).map((_, i) => (
                <div
                    key={i}
                    className={cn(
                        "h-1.5 rounded-full transition-all duration-300",
                        i === currentStep
                            ? "w-6 bg-primary"
                            : i < currentStep
                                ? "w-1.5 bg-primary/50"
                                : "w-1.5 bg-muted-foreground/20"
                    )}
                />
            ))}
        </div>
    );
}

export function OnboardingModal() {
    const [open, setOpen] = useState(false);
    const [step, setStep] = useState(0);
    const [exiting, setExiting] = useState(false);

    useEffect(() => {
        // Only show for new users — check localStorage
        try {
            const done = localStorage.getItem(ONBOARDING_KEY);
            if (!done) {
                // Small delay so the dashboard paints first
                const timer = setTimeout(() => setOpen(true), 600);
                return () => clearTimeout(timer);
            }
        } catch {
            // localStorage unavailable (SSR, incognito) — skip
        }
    }, []);

    const handleComplete = () => {
        setExiting(true);
        try {
            localStorage.setItem(ONBOARDING_KEY, "1");
        } catch {
            // noop
        }
        setTimeout(() => setOpen(false), 250);
    };

    const handleSkip = () => {
        handleComplete();
    };

    const handleNext = () => {
        if (step < STEPS.length - 1) {
            setStep((s) => s + 1);
        } else {
            handleComplete();
        }
    };

    const handleBack = () => {
        if (step > 0) setStep((s) => s - 1);
    };

    if (!open) return null;

    const currentStep = STEPS[step];
    const Icon = currentStep.icon;
    const isLast = step === STEPS.length - 1;

    return (
        <div
            className={cn(
                "fixed inset-0 z-[100] flex items-center justify-center p-4",
                exiting ? "animate-fade-out" : "animate-fade-in"
            )}
        >
            {/* Backdrop */}
            <div
                className="absolute inset-0 bg-black/60 backdrop-blur-sm"
                onClick={handleSkip}
            />

            {/* Modal */}
            <div
                className={cn(
                    "relative w-full max-w-md rounded-xl border border-border/60 bg-card shadow-2xl overflow-hidden",
                    exiting ? "animate-scale-out" : "animate-scale-in"
                )}
            >
                {/* Close button */}
                <button
                    onClick={handleSkip}
                    className="absolute top-3 right-3 z-10 p-1.5 rounded-md text-muted-foreground hover:text-foreground hover:bg-muted/40 transition-colors"
                    aria-label="Skip onboarding"
                >
                    <X className="h-4 w-4" />
                </button>

                {/* Gradient header */}
                <div className="relative h-32 bg-gradient-to-br from-primary/20 via-violet-500/10 to-emerald-500/10 flex items-center justify-center">
                    <div
                        className={cn(
                            "p-4 rounded-2xl bg-card/80 backdrop-blur-sm border border-border/40 shadow-lg",
                            "transition-all duration-300"
                        )}
                    >
                        <Icon className={cn("h-8 w-8", currentStep.iconColor)} />
                    </div>
                    {/* Decorative dots */}
                    <div className="absolute top-4 left-4 w-2 h-2 rounded-full bg-primary/30" />
                    <div className="absolute bottom-6 right-8 w-1.5 h-1.5 rounded-full bg-violet-400/30" />
                    <div className="absolute top-8 right-16 w-1 h-1 rounded-full bg-emerald-400/40" />
                </div>

                {/* Content */}
                <div className="p-6 space-y-4">
                    <div className="space-y-2">
                        <div className="flex items-center justify-between">
                            <h2 className="text-lg font-semibold tracking-tight">
                                {currentStep.title}
                            </h2>
                            <span className="text-[10px] text-muted-foreground tabular-nums">
                                {step + 1}/{STEPS.length}
                            </span>
                        </div>
                        <p className="text-sm text-muted-foreground leading-relaxed">
                            {currentStep.description}
                        </p>
                    </div>

                    {/* Feature list */}
                    <ul className="space-y-2.5">
                        {currentStep.features.map((feature, i) => (
                            <li key={i} className="flex items-start gap-2.5">
                                <div className="mt-0.5 h-5 w-5 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
                                    <Sparkles className="h-3 w-3 text-primary" />
                                </div>
                                <span className="text-sm text-foreground/90">
                                    {feature}
                                </span>
                            </li>
                        ))}
                    </ul>

                    {/* Step indicator */}
                    <div className="flex items-center justify-center pt-1">
                        <StepIndicator
                            currentStep={step}
                            totalSteps={STEPS.length}
                        />
                    </div>
                </div>

                {/* Footer */}
                <div className="px-6 pb-5 flex items-center justify-between">
                    <div>
                        {step > 0 ? (
                            <Button
                                variant="ghost"
                                size="sm"
                                onClick={handleBack}
                                className="text-xs"
                            >
                                <ArrowLeft className="h-3.5 w-3.5 mr-1" />
                                Back
                            </Button>
                        ) : (
                            <Button
                                variant="ghost"
                                size="sm"
                                onClick={handleSkip}
                                className="text-xs text-muted-foreground"
                            >
                                Skip tour
                            </Button>
                        )}
                    </div>
                    <Button size="sm" onClick={handleNext} className="text-xs">
                        {isLast ? (
                            <>
                                Get Started
                                <Zap className="h-3.5 w-3.5 ml-1" />
                            </>
                        ) : (
                            <>
                                Next
                                <ArrowRight className="h-3.5 w-3.5 ml-1" />
                            </>
                        )}
                    </Button>
                </div>
            </div>
        </div>
    );
}
