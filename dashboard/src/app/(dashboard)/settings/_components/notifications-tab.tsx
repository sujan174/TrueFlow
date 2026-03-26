"use client"

import { useState, useEffect } from "react"
import { toast } from "sonner"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Loader2, Bell, BellOff, Check } from "lucide-react"
import {
  listNotifications,
  markNotificationRead,
  markAllNotificationsRead,
  type Notification,
} from "@/lib/api"
import { cn } from "@/lib/utils"

function formatDate(dateString: string): string {
  const date = new Date(dateString)
  const now = new Date()
  const diffMs = now.getTime() - date.getTime()
  const diffMins = Math.floor(diffMs / 60000)
  const diffHours = Math.floor(diffMs / 3600000)
  const diffDays = Math.floor(diffMs / 86400000)

  if (diffMins < 1) return "Just now"
  if (diffMins < 60) return `${diffMins}m ago`
  if (diffHours < 24) return `${diffHours}h ago`
  if (diffDays < 30) return `${diffDays}d ago`
  return date.toLocaleDateString()
}

const NOTIFICATION_TYPE_STYLES: Record<string, string> = {
  spend_cap_breach: "bg-destructive/10 text-destructive",
  policy_violation: "bg-warning/10 text-warning",
  anomaly_detected: "bg-info/10 text-info",
  system: "bg-muted text-muted-foreground",
}

const NOTIFICATION_TYPE_LABELS: Record<string, string> = {
  spend_cap_breach: "Spend Cap",
  policy_violation: "Policy",
  anomaly_detected: "Anomaly",
  system: "System",
}

export function NotificationsTab() {
  const [notifications, setNotifications] = useState<Notification[]>([])
  const [isLoading, setIsLoading] = useState(true)
  const [isMarkingAll, setIsMarkingAll] = useState(false)
  const [selectedNotification, setSelectedNotification] = useState<Notification | null>(null)

  useEffect(() => {
    loadNotifications()
  }, [])

  async function loadNotifications() {
    try {
      const data = await listNotifications()
      setNotifications(data)
    } catch (error) {
      toast.error("Failed to load notifications")
      console.error(error)
    } finally {
      setIsLoading(false)
    }
  }

  async function handleMarkRead(id: string) {
    try {
      await markNotificationRead(id)
      setNotifications((prev) =>
        prev.map((n) => (n.id === id ? { ...n, is_read: true } : n))
      )
    } catch (error) {
      toast.error("Failed to mark notification as read")
      console.error(error)
    }
  }

  async function handleMarkAllRead() {
    setIsMarkingAll(true)
    try {
      await markAllNotificationsRead()
      setNotifications((prev) => prev.map((n) => ({ ...n, is_read: true })))
      toast.success("All notifications marked as read")
    } catch (error) {
      toast.error("Failed to mark all notifications as read")
      console.error(error)
    } finally {
      setIsMarkingAll(false)
    }
  }

  const unreadCount = notifications.filter((n) => !n.is_read).length

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-16">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    )
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-sm font-medium">
            Notifications
            {unreadCount > 0 && (
              <span className="ml-2 inline-flex items-center px-2 py-0.5 text-xs font-medium rounded-full bg-primary/10 text-primary">
                {unreadCount} unread
              </span>
            )}
          </h3>
          <p className="text-xs text-muted-foreground mt-0.5">
            System alerts and notifications
          </p>
        </div>
        {unreadCount > 0 && (
          <Button
            variant="outline"
            size="sm"
            onClick={handleMarkAllRead}
            disabled={isMarkingAll}
            className="gap-2"
          >
            {isMarkingAll ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <Check className="h-4 w-4" />
            )}
            Mark All Read
          </Button>
        )}
      </div>

      {/* Notifications List */}
      <div className="border rounded-lg">
        {notifications.length === 0 ? (
          <div className="p-12 text-center">
            <div className="w-12 h-12 rounded-full bg-muted flex items-center justify-center mx-auto mb-4">
              <BellOff className="h-6 w-6 text-muted-foreground" />
            </div>
            <p className="text-sm font-medium mb-1">No notifications</p>
            <p className="text-xs text-muted-foreground">
              System alerts will appear here
            </p>
          </div>
        ) : (
          <div className="divide-y">
            {notifications.map((notification) => (
              <div
                key={notification.id}
                className={cn(
                  "p-4 hover:bg-muted/30 transition-colors cursor-pointer",
                  !notification.is_read && "bg-primary/[0.02]"
                )}
                onClick={() => setSelectedNotification(notification)}
              >
                <div className="flex items-start gap-3">
                  {/* Unread indicator */}
                  <div className="mt-1.5">
                    <div className={cn(
                      "w-2 h-2 rounded-full",
                      notification.is_read ? "bg-muted" : "bg-primary"
                    )} />
                  </div>

                  {/* Content */}
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 mb-1">
                      <span className={cn(
                        "inline-flex px-2 py-0.5 text-xs font-medium rounded",
                        NOTIFICATION_TYPE_STYLES[notification.type] || NOTIFICATION_TYPE_STYLES.system
                      )}>
                        {NOTIFICATION_TYPE_LABELS[notification.type] || notification.type}
                      </span>
                      <span className="text-xs text-muted-foreground">
                        {formatDate(notification.created_at)}
                      </span>
                    </div>
                    <p className={cn("text-sm", !notification.is_read && "font-medium")}>
                      {notification.title}
                    </p>
                    {notification.body && (
                      <p className="text-xs text-muted-foreground mt-1 line-clamp-2">
                        {notification.body}
                      </p>
                    )}
                  </div>

                  {/* Mark read button */}
                  {!notification.is_read && (
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      onClick={(e) => {
                        e.stopPropagation()
                        handleMarkRead(notification.id)
                      }}
                      title="Mark as read"
                    >
                      <Check className="h-4 w-4" />
                    </Button>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Notification Detail Dialog */}
      <Dialog
        open={!!selectedNotification}
        onOpenChange={() => setSelectedNotification(null)}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2 text-base">
              {selectedNotification && (
                <span className={cn(
                  "inline-flex px-2 py-0.5 text-xs font-medium rounded",
                  NOTIFICATION_TYPE_STYLES[selectedNotification.type] || NOTIFICATION_TYPE_STYLES.system
                )}>
                  {NOTIFICATION_TYPE_LABELS[selectedNotification.type] || selectedNotification.type}
                </span>
              )}
            </DialogTitle>
            <DialogDescription className="text-sm font-medium text-foreground pt-2">
              {selectedNotification?.title}
            </DialogDescription>
          </DialogHeader>
          {selectedNotification && (
            <div className="py-4 space-y-4">
              {selectedNotification.body && (
                <p className="text-sm text-muted-foreground">
                  {selectedNotification.body}
                </p>
              )}

              {/* Metadata */}
              {selectedNotification.metadata && (
                <div className="bg-muted p-3 rounded-lg">
                  <p className="text-xs font-medium text-muted-foreground mb-2 uppercase">
                    Details
                  </p>
                  <pre className="text-xs font-mono overflow-x-auto">
                    {JSON.stringify(selectedNotification.metadata, null, 2)}
                  </pre>
                </div>
              )}

              <div className="text-xs text-muted-foreground">
                {new Date(selectedNotification.created_at).toLocaleString()}
              </div>

              {!selectedNotification.is_read && (
                <Button
                  size="sm"
                  onClick={() => {
                    handleMarkRead(selectedNotification.id)
                    setSelectedNotification(null)
                  }}
                  className="gap-2"
                >
                  <Check className="h-4 w-4" />
                  Mark as Read
                </Button>
              )}
            </div>
          )}
        </DialogContent>
      </Dialog>
    </div>
  )
}