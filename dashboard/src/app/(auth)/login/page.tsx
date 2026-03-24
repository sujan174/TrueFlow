"use client";

import { useState, useEffect } from "react";
import { createClient } from "@/lib/supabase/client";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useRouter } from "next/navigation";
import { cn } from "@/lib/utils";
import { toast } from "sonner";

export default function LoginPage() {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [mounted, setMounted] = useState(false);
  const router = useRouter();
  const supabase = createClient();

  useEffect(() => {
    setMounted(true);
  }, []);

  const handleEmailSignIn = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setError(null);

    const { error } = await supabase.auth.signInWithPassword({
      email,
      password,
    });

    if (error) {
      setError(error.message);
      setLoading(false);
      return;
    }

    const {
      data: { user },
    } = await supabase.auth.getUser();
    if (user) {
      await fetch("/api/auth/sync-user", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          supabase_id: user.id,
          email: user.email,
          name: user.user_metadata?.name,
          picture: user.user_metadata?.avatar_url,
        }),
      });
    }

    router.push("/");
    router.refresh();
  };

  const handleGoogleSignIn = async () => {
    setLoading(true);
    setError(null);

    const { error } = await supabase.auth.signInWithOAuth({
      provider: "google",
      options: {
        redirectTo: `${window.location.origin}/auth/callback`,
      },
    });

    if (error) {
      setError(error.message);
      setLoading(false);
    }
  };

  const handleForgotPassword = async () => {
    if (!email) {
      setError("Please enter your email address first");
      return;
    }

    setLoading(true);
    const { error } = await supabase.auth.resetPasswordForEmail(email, {
      redirectTo: `${window.location.origin}/auth/reset-password`,
    });

    if (error) {
      setError(error.message);
    } else {
      setError(null);
      toast.success("Password reset email sent! Check your inbox.");
    }
    setLoading(false);
  };

  // Bar chart data for the preview card
  const bars = [
    { height: 24 },
    { height: 32 },
    { height: 40 },
    { height: 52 },
    { height: 60 },
    { height: 48 },
    { height: 36 },
    { height: 28 },
    { height: 20 },
    { height: 26 },
  ];

  return (
    <div className="flex min-h-screen w-full">
      {/* LEFT PANE - Dark with gradient mesh */}
      <div className="hidden lg:flex lg:w-1/2 flex-col bg-[#0A0A0A] relative overflow-hidden">
        {/* Gradient mesh background */}
        <div className="absolute inset-0 bg-gradient-mesh opacity-60" />

        {/* Animated gradient orbs */}
        <div className="absolute top-1/4 left-1/4 w-96 h-96 bg-primary/20 rounded-full blur-3xl animate-pulse" />
        <div className="absolute bottom-1/4 right-1/4 w-80 h-80 bg-info/10 rounded-full blur-3xl animate-pulse delay-1000" />

        {/* Logo */}
        <div
          className={cn(
            "absolute top-[50px] left-[80px] flex items-center gap-2 transition-all duration-700",
            mounted ? "opacity-100 translate-y-0" : "opacity-0 -translate-y-4"
          )}
        >
          <div className="w-[6px] h-[6px] rounded-full bg-primary" />
          <span className="font-semibold text-white text-base">TrueFlow</span>
        </div>

        {/* Main Card */}
        <div
          className={cn(
            "mx-10 mt-[90px] mb-[20px] rounded-3xl p-10 flex-1 relative z-10 border border-white/5 transition-all duration-700 delay-150",
            mounted ? "opacity-100 translate-y-0" : "opacity-0 translate-y-8"
          )}
          style={{
            background: "linear-gradient(135deg, rgba(26,26,26,0.8) 0%, rgba(10,10,10,0.9) 100%)",
            backdropFilter: "blur(10px)",
          }}
        >
          {/* Eyebrow */}
          <div className="flex items-center gap-2 mb-8">
            <div className="w-[5px] h-[5px] rounded-full bg-primary" />
            <span
              className="font-semibold text-primary"
              style={{ fontSize: "11px", letterSpacing: "2px" }}
            >
              AI GATEWAY · POLICY-FIRST
            </span>
          </div>

          {/* Headline */}
          <h1
            className="font-bold text-white mb-8 leading-tight"
            style={{ fontSize: "48px", letterSpacing: "-1px" }}
          >
            Ship AI with confidence.
            <br />
            Govern every request.
          </h1>

          {/* Subheadline */}
          <p
            className="mb-16 text-white/60"
            style={{ fontSize: "16px", lineHeight: 1.6 }}
          >
            Policies, keys, audits, and spend —
            <br />
            unified in one calm console.
          </p>

          {/* Key Benefits */}
          <div className="mb-12">
            <p
              className="font-semibold mb-3 text-white/40"
              style={{ fontSize: "10px", letterSpacing: "1.5px" }}
            >
              KEY BENEFITS
            </p>
            <div className="flex flex-wrap gap-3">
              {["Policy Engine", "Audit Trail", "Spend Caps"].map((benefit, i) => (
                <div
                  key={benefit}
                  className={cn(
                    "px-4 py-2 rounded-full border border-white/10 bg-white/5 transition-all duration-500",
                    mounted ? "opacity-100 translate-y-0" : "opacity-0 translate-y-4"
                  )}
                  style={{ transitionDelay: `${300 + i * 100}ms` }}
                >
                  <span className="font-medium text-white text-xs">{benefit}</span>
                </div>
              ))}
            </div>
          </div>

          {/* Preview Card */}
          <div
            className="p-5 rounded-xl mb-8 border border-white/5"
            style={{ background: "rgba(0,0,0,0.4)" }}
          >
            <p
              className="font-semibold mb-2 text-white/40"
              style={{ fontSize: "10px", letterSpacing: "1.5px" }}
            >
              USAGE SNAPSHOT
            </p>
            <p className="font-bold text-white text-2xl">14.2k</p>
            <p className="font-medium text-success text-sm">+12% vs last week</p>

            {/* Bar Chart */}
            <div className="flex items-end gap-1.5 mt-4 h-[70px]">
              {bars.map((bar, i) => (
                <div
                  key={i}
                  className={cn(
                    "w-2 rounded-sm bg-gradient-to-t from-primary/30 to-primary/60 transition-all duration-500",
                    mounted ? "opacity-100" : "opacity-0"
                  )}
                  style={{
                    height: `${bar.height}px`,
                    transitionDelay: `${400 + i * 50}ms`,
                  }}
                />
              ))}
            </div>
          </div>

          {/* Trust Bar */}
          <div className="pt-4 border-t border-white/5">
            <p className="mb-4 text-white/40 text-xs">
              Trusted by engineering teams shipping AI at scale.
            </p>
            <div className="flex gap-3">
              {["Stripe", "Linear", "Vercel", "Notion"].map((company, i) => (
                <div
                  key={company}
                  className={cn(
                    "px-3 flex items-center justify-center border border-white/5 bg-white/5 rounded-md transition-all duration-500",
                    mounted ? "opacity-100 translate-y-0" : "opacity-0 translate-y-2"
                  )}
                  style={{
                    height: "24px",
                    transitionDelay: `${600 + i * 100}ms`,
                  }}
                >
                  <span className="font-medium text-white/50 text-[10px]">{company}</span>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>

      {/* RIGHT PANE - Light with subtle gradient */}
      <div className="flex-1 flex items-center justify-center p-8 bg-gradient-to-br from-background via-background to-muted/20">
        <div
          className={cn(
            "w-full max-w-[420px] p-10 rounded-2xl border border-border/50 bg-card/50 backdrop-blur-sm shadow-xl transition-all duration-700",
            mounted ? "opacity-100 translate-y-0" : "opacity-0 translate-y-8"
          )}
          style={{ transitionDelay: "300ms" }}
        >
          <h2 className="font-bold text-foreground text-3xl mb-2">Sign in</h2>
          <p className="text-muted-foreground text-sm mb-8">
            Your AI infrastructure awaits.
          </p>

          {/* Google SSO Button */}
          <Button
            type="button"
            variant="outline"
            className="w-full h-12 mb-4"
            onClick={handleGoogleSignIn}
            disabled={loading}
          >
            <svg className="w-5 h-5 mr-2" viewBox="0 0 24 24">
              <path
                fill="#4285F4"
                d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z"
              />
              <path
                fill="#34A853"
                d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"
              />
              <path
                fill="#FBBC05"
                d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z"
              />
              <path
                fill="#EA4335"
                d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"
              />
            </svg>
            <span className="text-sm">Continue with Google</span>
          </Button>

          {/* OR Divider */}
          <div className="flex items-center gap-4 my-6">
            <div className="flex-1 h-px bg-border" />
            <span className="text-xs text-muted-foreground">or</span>
            <div className="flex-1 h-px bg-border" />
          </div>

          {/* Email/Password Form */}
          <form onSubmit={handleEmailSignIn}>
            <div className="space-y-4">
              <div>
                <Label
                  htmlFor="email"
                  className="mb-2 block text-[10px] text-muted-foreground tracking-widest"
                >
                  EMAIL
                </Label>
                <Input
                  id="email"
                  type="email"
                  placeholder="you@company.com"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  required
                  className="h-11"
                />
              </div>

              <div>
                <Label
                  htmlFor="password"
                  className="mb-2 block text-[10px] text-muted-foreground tracking-widest"
                >
                  PASSWORD
                </Label>
                <Input
                  id="password"
                  type="password"
                  placeholder="••••••••••••"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  required
                  className="h-11"
                />
              </div>

              <div className="text-left">
                <button
                  type="button"
                  onClick={handleForgotPassword}
                  className="text-xs text-primary hover:underline bg-transparent border-none cursor-pointer p-0"
                >
                  Forgot password?
                </button>
              </div>

              {error && (
                <p className="text-destructive text-sm text-center">{error}</p>
              )}

              <Button
                type="submit"
                className="w-full h-12"
                loading={loading}
                loadingText="Signing in..."
              >
                <span className="font-semibold">Continue →</span>
              </Button>
            </div>
          </form>

          {/* Terms */}
          <p className="mt-6 text-center text-xs text-muted-foreground">
            By continuing, you agree to our Terms of Service
            <br />
            and Privacy Policy.
          </p>
        </div>
      </div>
    </div>
  );
}