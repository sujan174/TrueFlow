"use client"

import { createContext, useContext, useState, useEffect, useCallback, type ReactNode } from "react"
import { whoami, type WhoAmIResponse } from "@/lib/api"

type Role = "SuperAdmin" | "Admin" | "Member" | "ReadOnly"

interface PermissionsContextType {
  /** Current user's role */
  role: Role | null
  /** Current user's scopes (e.g., "tokens:read", "keys:manage") */
  scopes: string[]
  /** Whether permissions are currently loading */
  isLoading: boolean
  /** Whether user has at least the specified role */
  hasRole: (requiredRole: Role) => boolean
  /** Whether user has the specified scope */
  hasScope: (scope: string) => boolean
  /** Whether user can perform admin actions (Admin or SuperAdmin) */
  isAdmin: boolean
  /** Whether user is SuperAdmin */
  isSuperAdmin: boolean
  /** Refresh permissions from server */
  refresh: () => Promise<void>
}

const PermissionsContext = createContext<PermissionsContextType | null>(null)

const ROLE_HIERARCHY: Role[] = ["ReadOnly", "Member", "Admin", "SuperAdmin"]

function getRoleIndex(role: Role): number {
  return ROLE_HIERARCHY.indexOf(role)
}

interface PermissionsProviderProps {
  children: ReactNode
}

export function PermissionsProvider({ children }: PermissionsProviderProps) {
  const [permissions, setPermissions] = useState<WhoAmIResponse | null>(null)
  const [isLoading, setIsLoading] = useState(true)

  const fetchPermissions = useCallback(async () => {
    try {
      setIsLoading(true)
      const data = await whoami()
      setPermissions(data)
    } catch (err) {
      console.error("Failed to fetch permissions:", err)
      // Set default permissions on error
      setPermissions(null)
    } finally {
      setIsLoading(false)
    }
  }, [])

  useEffect(() => {
    fetchPermissions()
  }, [fetchPermissions])

  const role = (permissions?.role as Role) || null
  const scopes = permissions?.scopes || []

  const hasRole = useCallback(
    (requiredRole: Role): boolean => {
      if (!role) return false
      return getRoleIndex(role) >= getRoleIndex(requiredRole)
    },
    [role]
  )

  const hasScope = useCallback(
    (scope: string): boolean => {
      // Wildcard scope grants all permissions
      if (scopes.includes("*")) return true
      return scopes.includes(scope)
    },
    [scopes]
  )

  const isAdmin = role === "Admin" || role === "SuperAdmin"
  const isSuperAdmin = role === "SuperAdmin"

  return (
    <PermissionsContext.Provider
      value={{
        role,
        scopes,
        isLoading,
        hasRole,
        hasScope,
        isAdmin,
        isSuperAdmin,
        refresh: fetchPermissions,
      }}
    >
      {children}
    </PermissionsContext.Provider>
  )
}

export function usePermissions(): PermissionsContextType {
  const context = useContext(PermissionsContext)
  if (!context) {
    throw new Error("usePermissions must be used within a PermissionsProvider")
  }
  return context
}

/**
 * Hook to check if user can perform a specific action.
 * Returns loading state and permission check result.
 */
export function useCanPerform(requiredRole: Role): { canPerform: boolean; isLoading: boolean } {
  const { hasRole, isLoading } = usePermissions()
  return {
    canPerform: hasRole(requiredRole),
    isLoading,
  }
}