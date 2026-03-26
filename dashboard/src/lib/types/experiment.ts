// Experiment types matching gateway models

// Variant configuration for an experiment
export interface ExperimentVariant {
  name: string
  weight: number
  model?: string
  set_body_fields?: Record<string, unknown>
}

// Experiment as stored in the database (via policy wrapper)
export interface Experiment {
  id: string
  name: string
  status: 'running' | 'stopped'
  created_at: string
  variants: ExperimentVariant[]
  condition?: unknown
  rules?: unknown[]
}

// Per-variant metrics from audit log aggregation
export interface ExperimentResult {
  variant: string
  total_requests: number
  avg_latency_ms: number
  total_cost_usd: number
  avg_tokens: number
  error_count: number
  error_rate: number
}

// Full experiment with results
export interface ExperimentWithResults {
  id: string
  name: string
  status: 'running' | 'stopped'
  created_at: string
  rules?: unknown[]
  results: ExperimentResult[]
}

// Request to create a new experiment
export interface CreateExperimentRequest {
  name: string
  variants: ExperimentVariant[]
  condition?: unknown
}

// Request to update experiment variants
export interface UpdateExperimentRequest {
  variants: ExperimentVariant[]
}

// Timeseries point for experiment charts
export interface ExperimentTimeseriesPoint {
  bucket: string
  variant_name: string
  request_count: number
  avg_latency_ms: number
  total_cost_usd: number
}

// Statistical significance result
export interface StatisticalResult {
  metric: string
  controlValue: number
  treatmentValue: number
  delta: number
  deltaPercent: number
  pValue: number
  confidenceInterval: {
    lower: number
    upper: number
  }
  isSignificant: boolean
  confidenceLevel: number
  sampleSize: number
}

// Winner recommendation
export interface WinnerRecommendation {
  variant: string
  confidence: 'high' | 'medium' | 'low'
  metrics: string[]
  reason: string
}