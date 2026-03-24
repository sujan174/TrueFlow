import { Badge } from "@/components/ui/badge"
import { CheckCircle, XCircle, Clock } from "lucide-react"

export function StatusBadge({ status }: { status: string }) {
  const variants: Record<string, "success" | "destructive" | "secondary" | "outline"> = {
    Connected: "success",
    Disconnected: "secondary",
    Error: "destructive",
    pending: "outline",
    Pending: "outline",
  }

  const icons: Record<string, React.ReactNode> = {
    Connected: <CheckCircle className="h-3 w-3 mr-1" />,
    Disconnected: <XCircle className="h-3 w-3 mr-1" />,
    Error: <XCircle className="h-3 w-3 mr-1" />,
    pending: <Clock className="h-3 w-3 mr-1" />,
    Pending: <Clock className="h-3 w-3 mr-1" />,
  }

  return (
    <Badge variant={variants[status] || "outline"} className="text-[10px] gap-1">
      {icons[status]}
      {status}
    </Badge>
  )
}

export function AuthTypeBadge({ authType }: { authType: string }) {
  const variants: Record<string, "default" | "secondary" | "outline"> = {
    none: "outline",
    bearer: "default",
    oauth2: "secondary",
  }

  const labels: Record<string, string> = {
    none: "No Auth",
    bearer: "Bearer Token",
    oauth2: "OAuth 2.0",
  }

  return (
    <Badge variant={variants[authType] || "outline"} className="text-[10px]">
      {labels[authType] || authType}
    </Badge>
  )
}