// dashboard/src/lib/field-registry.ts

import { Globe, Brain, Wrench, Clock, Code, Send, Shield, User, Activity } from "lucide-react"
import type { ConditionOperator } from "./types/policy"

export interface FieldDefinition {
  path: string
  label: string
  description: string
  type: "string" | "number" | "array" | "boolean"
  operators: ConditionOperator[]
  examples?: string[]
  placeholder?: string
  validationHint?: string
  enum?: string[]  // For fields with fixed values
  range?: [number, number]  // For number fields
}

export interface FieldCategory {
  id: string
  label: string
  icon: React.ComponentType<{ className?: string }>
  description: string
  color: string  // Tailwind color class prefix
  fields: FieldDefinition[]
}

export const FIELD_CATEGORIES: FieldCategory[] = [
  {
    id: "ip_network",
    label: "IP / Network",
    icon: Globe,
    description: "Restrict by client IP address or network",
    color: "blue",
    fields: [
      {
        path: "context.ip",
        label: "Client IP",
        description: "The IP address of the client making the request",
        type: "string",
        operators: ["eq", "neq", "in", "glob"],
        examples: ["192.168.1.1", "10.0.0.1", "172.16.0.0/12"],
        placeholder: "Enter IP address or CIDR",
        validationHint: "Supports single IPs or CIDR notation (e.g., 10.0.0.0/8)"
      }
    ]
  },
  {
    id: "model",
    label: "Model",
    icon: Brain,
    description: "Restrict by AI model name",
    color: "purple",
    fields: [
      {
        path: "request.body.model",
        label: "Model Name",
        description: "The model specified in the request body",
        type: "string",
        operators: ["eq", "neq", "in", "contains", "starts_with", "glob", "regex"],
        examples: ["gpt-4", "claude-3-opus", "gemini-pro"],
        placeholder: "Enter model name",
      }
    ]
  },
  {
    id: "tools",
    label: "Tools / Functions",
    icon: Wrench,
    description: "Restrict by MCP tool or function names",
    color: "amber",
    fields: [
      {
        path: "request.body.tools[*].function.name",
        label: "Tool Name",
        description: "Name of the tool/function being called",
        type: "array",
        operators: ["contains", "in"],
        examples: ["web_search", "read_file", "execute_code"],
        placeholder: "Enter tool name",
        validationHint: "Matches against any tool in the request"
      }
    ]
  },
  {
    id: "time",
    label: "Time",
    icon: Clock,
    description: "Time-based conditions for business hours",
    color: "green",
    fields: [
      {
        path: "context.time.hour",
        label: "Hour of Day",
        description: "Current hour (0-23, UTC)",
        type: "number",
        operators: ["eq", "neq", "gt", "gte", "lt", "lte", "in"],
        range: [0, 23],
        placeholder: "0-23",
        validationHint: "Hours are in UTC (0-23)"
      },
      {
        path: "context.time.weekday",
        label: "Day of Week",
        description: "Current day of the week",
        type: "string",
        operators: ["eq", "neq", "in"],
        enum: ["mon", "tue", "wed", "thu", "fri", "sat", "sun"],
        examples: ["mon", "fri"],
        placeholder: "Select day"
      }
    ]
  },
  {
    id: "request",
    label: "Request",
    icon: Send,
    description: "HTTP request properties",
    color: "slate",
    fields: [
      {
        path: "request.method",
        label: "HTTP Method",
        description: "The HTTP method (GET, POST, etc.)",
        type: "string",
        operators: ["eq", "neq", "in"],
        enum: ["GET", "POST", "PUT", "DELETE", "PATCH"],
        examples: ["POST", "GET"]
      },
      {
        path: "request.path",
        label: "Request Path",
        description: "The URL path of the request",
        type: "string",
        operators: ["eq", "neq", "contains", "starts_with", "ends_with", "glob", "regex", "in"],
        examples: ["/v1/chat/completions", "/v1/embeddings"],
        placeholder: "/v1/chat/completions"
      },
      {
        path: "request.body_size",
        label: "Body Size (bytes)",
        description: "Size of the request body in bytes",
        type: "number",
        operators: ["eq", "neq", "gt", "gte", "lt", "lte"],
        placeholder: "1024"
      }
    ]
  },
  {
    id: "token",
    label: "Token",
    icon: Shield,
    description: "Token properties",
    color: "cyan",
    fields: [
      {
        path: "token.id",
        label: "Token ID",
        description: "The virtual token ID",
        type: "string",
        operators: ["eq", "neq", "in", "contains"],
        placeholder: "tf_v1_..."
      },
      {
        path: "token.name",
        label: "Token Name",
        description: "The display name of the token",
        type: "string",
        operators: ["eq", "neq", "in", "contains", "starts_with"]
      },
      {
        path: "token.purpose",
        label: "Token Purpose",
        description: "Purpose of the token",
        type: "string",
        operators: ["eq", "neq", "in"],
        enum: ["llm", "tool", "both"]
      }
    ]
  },
  {
    id: "agent",
    label: "Agent",
    icon: User,
    description: "Agent identity properties",
    color: "indigo",
    fields: [
      {
        path: "agent.name",
        label: "Agent Name",
        description: "The name of the agent making requests",
        type: "string",
        operators: ["eq", "neq", "in", "contains", "starts_with"],
        examples: ["claude", "gpt-4-agent"],
        placeholder: "Enter agent name"
      }
    ]
  },
  {
    id: "response",
    label: "Response",
    icon: Activity,
    description: "Response properties (post-flight phase only)",
    color: "rose",
    fields: [
      {
        path: "response.status",
        label: "Response Status",
        description: "HTTP response status code",
        type: "number",
        operators: ["eq", "neq", "gt", "gte", "lt", "lte", "in"],
        examples: ["200", "429", "500"],
        placeholder: "200"
      }
    ]
  },
  {
    id: "custom",
    label: "Custom Path",
    icon: Code,
    description: "Enter any JSON path for advanced conditions",
    color: "gray",
    fields: [] // Custom fields are user-defined
  }
]

// Helper functions
export function getCategoryById(id: string): FieldCategory | undefined {
  return FIELD_CATEGORIES.find(c => c.id === id)
}

export function getFieldByPath(path: string): FieldDefinition | undefined {
  for (const category of FIELD_CATEGORIES) {
    const field = category.fields.find(f => f.path === path)
    if (field) return field
  }
  return undefined
}

export function getOperatorsForField(field: FieldDefinition): ConditionOperator[] {
  return field.operators
}

export function getExamplesForField(field: FieldDefinition): string[] {
  return field.examples || []
}

export function validateFieldValue(field: FieldDefinition, value: unknown): { valid: boolean; error?: string } {
  if (field.enum && typeof value === 'string' && !field.enum.includes(value)) {
    return { valid: false, error: `Must be one of: ${field.enum.join(', ')}` }
  }
  if (field.range && typeof value === 'number') {
    const [min, max] = field.range
    if (value < min || value > max) {
      return { valid: false, error: `Must be between ${min} and ${max}` }
    }
  }
  return { valid: true }
}