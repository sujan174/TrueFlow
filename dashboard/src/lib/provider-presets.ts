/**
 * Provider presets for easy upstream configuration.
 * These are synced with the model catalog in docs/model-catalog.md
 *
 * Last Updated: 2026-03-27
 */

import { UpstreamTarget } from './types/token'

export interface ProviderPreset {
  name: string
  url: string
  allowed_models: string[]
  description: string
}

/**
 * Predefined provider configurations.
 * Use these to quickly set up upstreams for common providers.
 */
export const PROVIDER_PRESETS: ProviderPreset[] = [
  {
    name: 'OpenAI',
    url: 'https://api.openai.com/v1',
    allowed_models: ['gpt-*', 'o1-*', 'o3-*', 'text-*', 'tts-*', 'dall-e-*', 'whisper-*'],
    description: 'GPT-4o, GPT-4, GPT-3.5, O1, O3 models',
  },
  {
    name: 'Anthropic',
    url: 'https://api.anthropic.com/v1',
    allowed_models: ['claude-*'],
    description: 'Claude 4, Claude 3.5, Claude 3 models',
  },
  {
    name: 'Google Gemini',
    url: 'https://generativelanguage.googleapis.com/v1beta',
    allowed_models: ['gemini-*'],
    description: 'Gemini 3, Gemini 2.5, Gemini 1.5 models',
  },
  {
    name: 'Groq',
    url: 'https://api.groq.com/openai/v1',
    allowed_models: ['*'],
    description: 'Fast inference for Llama, Mixtral, Gemma, etc.',
  },
  {
    name: 'Mistral',
    url: 'https://api.mistral.ai/v1',
    allowed_models: ['mistral-*', 'mixtral-*', 'codestral-*', 'devstral-*'],
    description: 'Mistral Large, Medium, Small, Codestral',
  },
  {
    name: 'Cohere',
    url: 'https://api.cohere.ai/v1',
    allowed_models: ['command-*', 'embed-*', 'rerank-*'],
    description: 'Command R, Aya, Embed, Rerank models',
  },
  {
    name: 'Together AI',
    url: 'https://api.together.xyz/v1',
    allowed_models: ['*'],
    description: 'Hosted open-source models (Llama, Qwen, etc.)',
  },
  {
    name: 'OpenRouter',
    url: 'https://openrouter.ai/api/v1',
    allowed_models: ['*'],
    description: 'Unified API for many providers',
  },
  {
    name: 'Azure OpenAI',
    url: '', // URL varies by deployment
    allowed_models: ['gpt-*', 'o1-*'],
    description: 'Azure-hosted OpenAI models (URL varies)',
  },
  {
    name: 'AWS Bedrock',
    url: '', // URL is regional
    allowed_models: ['*'],
    description: 'AWS-hosted foundation models (URL varies)',
  },
  {
    name: 'Ollama',
    url: 'http://localhost:11434/v1',
    allowed_models: ['*'],
    description: 'Local LLM inference',
  },
  {
    name: 'Custom',
    url: '',
    allowed_models: ['*'],
    description: 'Custom upstream endpoint',
  },
]

/**
 * Get a preset by name.
 */
export function getProviderPreset(name: string): ProviderPreset | undefined {
  return PROVIDER_PRESETS.find(p => p.name === name)
}

/**
 * Convert a preset to an UpstreamTarget.
 */
export function presetToUpstream(preset: ProviderPreset, overrides?: Partial<UpstreamTarget>): UpstreamTarget {
  return {
    url: preset.url,
    weight: 100,
    priority: 1,
    allowed_models: preset.allowed_models,
    ...overrides,
  }
}

/**
 * Detect provider from model name.
 */
export function detectProviderFromModel(model: string): string | null {
  if (model.startsWith('gpt-') || model.startsWith('o1-') || model.startsWith('o3-')) {
    return 'OpenAI'
  }
  if (model.startsWith('claude-')) {
    return 'Anthropic'
  }
  if (model.startsWith('gemini-')) {
    return 'Google Gemini'
  }
  if (model.startsWith('mistral-') || model.startsWith('codestral-') || model.startsWith('devstral-')) {
    return 'Mistral'
  }
  if (model.startsWith('command-') || model.startsWith('embed-') || model.startsWith('rerank-')) {
    return 'Cohere'
  }
  return null
}

/**
 * Model patterns by provider for filtering.
 * Used to suggest which models work with which upstream.
 */
export const MODEL_PATTERNS: Record<string, string[]> = {
  OpenAI: ['gpt-*', 'o1-*', 'o3-*'],
  Anthropic: ['claude-*'],
  'Google Gemini': ['gemini-*'],
  Groq: ['llama-*', 'mixtral-*', 'gemma*', 'deepseek-*', 'qwen-*'],
  Mistral: ['mistral-*', 'codestral-*', 'devstral-*'],
  Cohere: ['command-*', 'embed-*', 'rerank-*'],
}

/**
 * Check if a model matches a pattern.
 * Supports glob-style patterns with * and ?.
 */
export function modelMatchesPattern(model: string, pattern: string): boolean {
  // Convert glob pattern to regex
  const regex = pattern
    .replace(/[.+^${}()|[\]\\]/g, '\\$&') // Escape special regex chars except * and ?
    .replace(/\*/g, '.*') // * matches any sequence
    .replace(/\?/g, '.') // ? matches any single char

  return new RegExp(`^${regex}$`, 'i').test(model)
}

/**
 * Check if a model is compatible with an upstream's allowed_models.
 */
export function isModelCompatible(model: string, allowedModels: string[] | null | undefined): boolean {
  if (!allowedModels || allowedModels.length === 0) {
    return true // No restrictions
  }

  return allowedModels.some(pattern => modelMatchesPattern(model, pattern))
}