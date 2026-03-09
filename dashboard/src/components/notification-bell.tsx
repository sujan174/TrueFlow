"use client";

import { useEffect, useState } from "react";
import { Bell } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuSeparator,
    DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { listNotifications, countUnreadNotifications, markNotificationRead, markAllNotificationsRead, Notification } from "@/lib/api";
import { formatDistanceToNow } from "date-fns";
import { cn } from "@/lib/utils";
import { useRouter } from "next/navigation";
import { useProject } from "@/contexts/project-context";

export function NotificationBell() {
    const { selectedProjectId } = useProject();
    const [unreadCount, setUnreadCount] = useState(0);
    const [notifications, setNotifications] = useState<Notification[]>([]);
    const [open, setOpen] = useState(false);
    const router = useRouter();

    const fetchUnread = async () => {
        try {
            const { count } = await countUnreadNotifications();
            setUnreadCount(count);
        } catch (e) {
            console.error(e);
        }
    };

    const fetchList = async () => {
        try {
            const list = await listNotifications();
            setNotifications(list);
        } catch (e) {
            console.error(e);
        }
    };

    // Poll for unread count, re-run if project changes
    useEffect(() => {
        let active = true;
        if (!selectedProjectId) return; // Wait until project ID is validated by context
        if (active) {
            setTimeout(() => {
                if (active) fetchUnread();
            }, 0);
        }
        const interval = setInterval(() => {
            if (active) fetchUnread();
        }, 10000);
        return () => {
            active = false;
            clearInterval(interval);
        };
    }, [selectedProjectId]);

    // Fetch list when opening
    useEffect(() => {
        let active = true;
        if (open && selectedProjectId && active) {
            setTimeout(() => {
                if (active) fetchList();
            }, 0);
        }
        return () => {
            active = false;
        };
    }, [open, selectedProjectId]);

    const handleMarkAllRead = async (e: React.MouseEvent) => {
        e.stopPropagation();
        await markAllNotificationsRead();
        setUnreadCount(0);
        fetchList();
    };

    const handleItemClick = async (n: Notification) => {
        if (!n.is_read) {
            await markNotificationRead(n.id);
            setUnreadCount(prev => Math.max(0, prev - 1));
            setNotifications(prev => prev.map(item => item.id === n.id ? { ...item, is_read: true } : item));
        }

        // Navigate based on type
        if (n.type === 'approval_needed' && n.metadata?.approval_id) {
            router.push('/approvals');
        } else if (n.type === 'policy_violation') {
            router.push('/audit');
        } else if (n.type === 'approval_received' || n.type === 'approval_decision') {
            router.push('/approvals');
        }
        setOpen(false);
    };

    return (
        <DropdownMenu open={open} onOpenChange={setOpen}>
            <DropdownMenuTrigger asChild>
                <Button variant="ghost" size="icon" className="relative text-muted-foreground hover:text-foreground">
                    <Bell className="h-5 w-5" />
                    {unreadCount > 0 && (
                        <span className="absolute top-2 right-2 h-2 w-2 rounded-full bg-red-500 animate-pulse ring-2 ring-background" />
                    )}
                </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="w-80">
                <div className="flex items-center justify-between px-3 py-2">
                    <span className="text-sm font-semibold">Notifications</span>
                    {unreadCount > 0 && (
                        <Button variant="ghost" size="sm" className="h-auto px-1.5 text-xs text-muted-foreground hover:text-primary" onClick={handleMarkAllRead}>
                            Mark all read
                        </Button>
                    )}
                </div>
                <DropdownMenuSeparator />
                <div className="max-h-[350px] overflow-y-auto">
                    {notifications.length === 0 ? (
                        <div className="p-4 text-center text-xs text-muted-foreground">
                            No notifications
                        </div>
                    ) : (
                        notifications.map((n) => (
                            <DropdownMenuItem key={n.id} className="cursor-pointer flex flex-col items-start gap-1 p-3 focus:bg-muted/50" onClick={() => handleItemClick(n)}>
                                <div className="flex w-full items-start justify-between gap-2">
                                    <span className={cn("font-medium text-sm", !n.is_read ? "text-primary" : "text-foreground/80")}>
                                        {n.title}
                                    </span>
                                    <span className="text-[10px] text-muted-foreground whitespace-nowrap shrink-0">
                                        {formatDistanceToNow(new Date(n.created_at), { addSuffix: true })}
                                    </span>
                                </div>
                                {n.body && (
                                    <p className="text-xs text-muted-foreground line-clamp-2 w-full">
                                        {n.body}
                                    </p>
                                )}
                                {!n.is_read && (
                                    <div className="mt-1 flex items-center gap-2">
                                        <div className="h-1.5 w-1.5 rounded-full bg-primary" />
                                        <span className="text-[10px] text-primary font-medium">New</span>
                                    </div>
                                )}
                            </DropdownMenuItem>
                        ))
                    )}
                </div>
            </DropdownMenuContent>
        </DropdownMenu>
    );
}
