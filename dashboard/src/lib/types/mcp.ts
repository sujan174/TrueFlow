// MCP (Model Context Protocol) Types

export interface McpServerInfo {
  id: string
  name: string
  endpoint: string
  status: "Connected" | "Disconnected" | "Error" | "pending"
  auth_type: "none" | "bearer" | "oauth2"
  tool_count: number
  tools: string[]
  last_refreshed_secs_ago: number
  server_info?: {
    name: string
    version: string
  }
}

export interface McpToolDef {
  name: string
  description?: string
  input_schema: Record<string, unknown>
  output_schema?: Record<string, unknown>
}

export interface DiscoveryResult {
  endpoint: string
  requires_auth: boolean
  auth_type: string
  token_endpoint?: string
  scopes_supported?: string[]
  server_info?: {
    name: string
    version: string
  }
  tools: McpToolDef[]
  tool_count: number
}

export interface RegisterMcpServerRequest {
  name?: string
  endpoint: string
  api_key?: string
  client_id?: string
  client_secret?: string
  auto_discover?: boolean
}

export interface RegisterMcpServerResponse {
  id: string
  name: string
  auth_type: string
  tool_count: number
  tools: string[]
}

export interface TestMcpServerResponse {
  connected: boolean
  tool_count: number
  tools: McpToolDef[]
  error?: string
}

export interface ReauthResponse {
  success: boolean
  error?: string
}