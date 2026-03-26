// Prompt Management Types
// Matches backend API response types from gateway/src/api/prompt_handlers.rs

export interface PromptRow {
  id: string;
  project_id: string;
  name: string;
  slug: string;
  description: string;
  folder: string;
  tags: Record<string, unknown>;
  created_at: string;
  updated_at: string;
  created_by: string;
  is_active: boolean;
}

export interface PromptVersionRow {
  id: string;
  prompt_id: string;
  version: number;
  model: string;
  messages: Message[];
  temperature: number | null;
  max_tokens: number | null;
  top_p: number | null;
  tools: Tool[] | null;
  commit_message: string;
  created_at: string;
  created_by: string;
  labels: string[];
}

export interface Message {
  role: "system" | "user" | "assistant" | "tool";
  content: string;
  name?: string;
  tool_call_id?: string;
  tool_calls?: ToolCall[];
}

export interface ToolCall {
  id: string;
  type: "function";
  function: {
    name: string;
    arguments: string;
  };
}

export interface Tool {
  type: "function";
  function: {
    name: string;
    description: string;
    parameters: Record<string, unknown>;
  };
}

// Request types
export interface CreatePromptRequest {
  name: string;
  slug?: string;
  description?: string;
  folder?: string;
  tags?: Record<string, unknown>;
}

export interface UpdatePromptRequest {
  name: string;
  description?: string;
  folder?: string;
  tags?: Record<string, unknown>;
}

export interface CreateVersionRequest {
  model: string;
  messages: Message[];
  temperature?: number;
  max_tokens?: number;
  top_p?: number;
  tools?: Tool[];
  commit_message?: string;
}

export interface DeployRequest {
  version: number;
  label: string;
}

export interface RenderRequest {
  variables?: Record<string, string | number | boolean>;
  label?: string;
  version?: number;
}

// Response types
export interface PromptListResponse {
  id: string;
  name: string;
  slug: string;
  description: string;
  folder: string;
  tags: Record<string, unknown>;
  created_at: string;
  updated_at: string;
  version_count: number;
  latest_version: number | null;
  latest_model: string | null;
  labels: string[];
}

export interface PromptDetailResponse {
  prompt: PromptRow;
  versions: PromptVersionRow[];
  version_count: number;
}

export interface CreatePromptResponse {
  id: string;
  name: string;
  slug: string;
  folder: string;
  message: string;
}

export interface CreateVersionResponse {
  id: string;
  version: number;
  model: string;
  message: string;
}

export interface RenderResponse {
  model: string;
  messages: Message[];
  temperature?: number;
  max_tokens?: number;
  top_p?: number;
  tools?: Tool[];
  version: number;
  label?: string;
  prompt_id: string;
  prompt_slug: string;
}

// Deployment labels (predefined)
export const DEPLOYMENT_LABELS = ["production", "staging", "development"] as const;
export type DeploymentLabel = (typeof DEPLOYMENT_LABELS)[number];

// Label colors for UI
export const LABEL_COLORS: Record<DeploymentLabel, { bg: string; text: string; border: string }> = {
  production: {
    bg: "bg-green-500/10",
    text: "text-green-600 dark:text-green-400",
    border: "border-green-500/30",
  },
  staging: {
    bg: "bg-yellow-500/10",
    text: "text-yellow-600 dark:text-yellow-400",
    border: "border-yellow-500/30",
  },
  development: {
    bg: "bg-blue-500/10",
    text: "text-blue-600 dark:text-blue-400",
    border: "border-blue-500/30",
  },
};