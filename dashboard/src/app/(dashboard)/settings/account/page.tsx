"use client"

import { useState, useEffect } from "react"
import { useRouter } from "next/navigation"
import { useTheme } from "next-themes"
import { createClient } from "@/lib/supabase/client"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar"
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog"
import {
  User,
  Mail,
  Palette,
  LogOut,
  Trash2,
  Loader2,
  Check,
  Sun,
  Moon,
  Monitor,
} from "lucide-react"
import { toast } from "sonner"
import { cn } from "@/lib/utils"
import type { User as SupabaseUser } from "@supabase/supabase-js"
import { SettingsSidebar } from "../_components/settings-sidebar"

export default function AccountPage() {
  const router = useRouter()
  const supabase = createClient()
  const { theme, setTheme } = useTheme()
  const [user, setUser] = useState<SupabaseUser | null>(null)
  const [loading, setLoading] = useState(true)
  const [name, setName] = useState("")
  const [isSaving, setIsSaving] = useState(false)
  const [isDeleting, setIsDeleting] = useState(false)

  useEffect(() => {
    async function loadUser() {
      const { data: { user } } = await supabase.auth.getUser()
      setUser(user)
      setName(user?.user_metadata?.name || "")
      setLoading(false)
    }
    loadUser()
  }, [supabase.auth])

  async function handleUpdateProfile() {
    if (!user) return

    setIsSaving(true)
    try {
      const { error } = await supabase.auth.updateUser({
        data: { name }
      })

      if (error) throw error

      toast.success("Profile updated successfully")
    } catch (error) {
      toast.error("Failed to update profile")
      console.error(error)
    } finally {
      setIsSaving(false)
    }
  }

  async function handleSignOut() {
    localStorage.removeItem("trueflow_project_id")
    await supabase.auth.signOut()
    router.push("/login")
    router.refresh()
  }

  async function handleDeleteAccount() {
    if (!user) return

    setIsDeleting(true)
    try {
      // Call server-side API route for account deletion
      // This handles API key revocation and initiates 30-day deletion grace period
      const response = await fetch("/api/auth/delete-account", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
      })

      if (!response.ok) {
        const error = await response.json()
        throw new Error(error.error || "Failed to delete account")
      }

      toast.success("Account deletion initiated. You have 30 days to recover by logging back in.")
      localStorage.removeItem("trueflow_project_id")
      router.push("/login")
      router.refresh()
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Failed to delete account")
      console.error(error)
    } finally {
      setIsDeleting(false)
    }
  }

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    )
  }

  const themeOptions = [
    { value: "light", label: "Light", icon: Sun },
    { value: "dark", label: "Dark", icon: Moon },
    { value: "system", label: "System", icon: Monitor },
  ]

  return (
    <div className="flex-1 flex min-w-0">
      <SettingsSidebar />

      {/* Main Content */}
      <div className="flex-1 flex flex-col min-w-0 overflow-auto">
        <div className="flex-1 p-6 lg:p-8">
          {/* Header */}
          <header className="mb-8">
            <h1 className="text-xl font-semibold tracking-tight">Account</h1>
            <p className="text-sm text-muted-foreground mt-1">
              Manage your personal settings and preferences
            </p>
          </header>

          <div className="max-w-2xl space-y-8">
            {/* Profile Section */}
            <section>
              <h3 className="text-sm font-medium mb-4 flex items-center gap-2">
                <User className="h-4 w-4 text-muted-foreground" />
                Profile
              </h3>

              <div className="border rounded-lg p-6 space-y-6">
                {/* Avatar */}
                <div className="flex items-center gap-4">
                  <Avatar className="h-16 w-16">
                    <AvatarImage
                      src={user?.user_metadata?.avatar_url}
                      alt={user?.email || ""}
                    />
                    <AvatarFallback className="bg-primary/10 text-primary text-lg font-medium">
                      {user?.email?.charAt(0).toUpperCase() || "U"}
                    </AvatarFallback>
                  </Avatar>
                  <div>
                    <p className="text-sm font-medium">{name || "User"}</p>
                    <p className="text-xs text-muted-foreground">{user?.email}</p>
                  </div>
                </div>

                {/* Name Field */}
                <div className="space-y-2 max-w-md">
                  <Label htmlFor="name" className="text-xs">Display Name</Label>
                  <Input
                    id="name"
                    value={name}
                    onChange={(e) => setName(e.target.value)}
                    placeholder="Your name"
                    className="h-9"
                  />
                </div>

                {/* Email (read-only) */}
                <div className="space-y-2 max-w-md">
                  <Label htmlFor="email" className="text-xs flex items-center gap-1.5">
                    <Mail className="h-3 w-3" />
                    Email
                  </Label>
                  <Input
                    id="email"
                    value={user?.email || ""}
                    disabled
                    className="h-9 bg-muted"
                  />
                  <p className="text-xs text-muted-foreground">
                    Contact support to change your email address
                  </p>
                </div>

                {/* Save Button */}
                <div className="flex justify-end pt-2">
                  <Button
                    size="sm"
                    onClick={handleUpdateProfile}
                    disabled={isSaving}
                    className="gap-2"
                  >
                    {isSaving ? (
                      <>
                        <Loader2 className="h-4 w-4 animate-spin" />
                        Saving...
                      </>
                    ) : (
                      <>
                        <Check className="h-4 w-4" />
                        Save Changes
                      </>
                    )}
                  </Button>
                </div>
              </div>
            </section>

            {/* Appearance Section */}
            <section>
              <h3 className="text-sm font-medium mb-4 flex items-center gap-2">
                <Palette className="h-4 w-4 text-muted-foreground" />
                Appearance
              </h3>

              <div className="border rounded-lg p-6">
                <div className="space-y-2 mb-4">
                  <Label className="text-xs">Theme</Label>
                  <p className="text-xs text-muted-foreground">
                    Select your preferred color theme
                  </p>
                </div>

                <div className="flex gap-2">
                  {themeOptions.map((option) => (
                    <button
                      key={option.value}
                      onClick={() => setTheme(option.value)}
                      className={cn(
                        "flex items-center gap-2 px-4 py-2.5 rounded-lg border text-sm font-medium transition-colors",
                        theme === option.value
                          ? "border-primary bg-primary/5 text-primary"
                          : "border-border hover:bg-accent text-muted-foreground hover:text-foreground"
                      )}
                    >
                      <option.icon className="h-4 w-4" />
                      {option.label}
                    </button>
                  ))}
                </div>
              </div>
            </section>

            {/* Danger Zone */}
            <section className="pt-4">
              <h3 className="text-sm font-medium mb-4 text-destructive">Danger Zone</h3>

              <div className="border border-destructive/20 rounded-lg p-6 space-y-4">
                <div className="flex items-start justify-between gap-4">
                  <div>
                    <p className="text-sm font-medium">Sign Out</p>
                    <p className="text-xs text-muted-foreground">
                      Sign out of your account on this device
                    </p>
                  </div>
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={handleSignOut}
                    className="gap-2"
                  >
                    <LogOut className="h-4 w-4" />
                    Sign Out
                  </Button>
                </div>

                <div className="border-t pt-4 flex items-start justify-between gap-4">
                  <div>
                    <p className="text-sm font-medium">Delete Account</p>
                    <p className="text-xs text-muted-foreground">
                      Permanently delete your account and all associated data
                    </p>
                  </div>
                  <AlertDialog>
                    <AlertDialogTrigger render={
                      <Button variant="destructive" size="sm" className="gap-2">
                        <Trash2 className="h-4 w-4" />
                        Delete Account
                      </Button>
                    } />
                    <AlertDialogContent>
                      <AlertDialogHeader>
                        <AlertDialogTitle>Delete Account</AlertDialogTitle>
                        <AlertDialogDescription>
                          Are you sure you want to delete your account? This action cannot be undone.
                          All your projects, tokens, and data will be permanently removed.
                        </AlertDialogDescription>
                      </AlertDialogHeader>
                      <AlertDialogFooter>
                        <AlertDialogCancel>Cancel</AlertDialogCancel>
                        <AlertDialogAction
                          onClick={handleDeleteAccount}
                          disabled={isDeleting}
                          className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
                        >
                          {isDeleting ? "Deleting..." : "Delete Account"}
                        </AlertDialogAction>
                      </AlertDialogFooter>
                    </AlertDialogContent>
                  </AlertDialog>
                </div>
              </div>
            </section>
          </div>
        </div>
      </div>
    </div>
  )
}