/**
 * Shared types for TrueFlow API requests and responses.
 *
 * Every interface mirrors the gateway's JSON schema and the Python SDK's
 * Pydantic models 1:1, with TypeScript naming conventions (camelCase).
 *
 * @module
 */

// ────────────────────────────────────────────────────────────────────────────
// Upstream
// ────────────────────────────────────────────────────────────────────────────

/** A single upstream target with weight and priority for load balancing. */
export interface Upstream {
    /** The upstream API base URL. */
    url: string;
    /** Traffic weight (0–100). Higher weight = more traffic. */
    weight?: number;
    /** Priority tier (1 = primary, 2 = fallback). */
    priority?: number;
    /** Optional credential ID for this specific upstream. */
    credentialId?: string;
}

// ────────────────────────────────────────────────────────────────────────────
// Token
// ────────────────────────────────────────────────────────────────────────────

/** A virtual token that maps an agent to a credential and upstream endpoint. */
export interface Token {
    id: string;
    name: string;
    credentialId?: string;
    upstreamUrl: string;
    projectId?: string;
    policyIds: string[];
    scopes: string[];
    isActive: boolean;
    createdAt?: string;
}

/** Response from creating a new token. */
export interface TokenCreateResponse {
    /** The `tf_v1_...` key — only returned once at creation time. */
    tokenId?: string;
    /** Internal UUID. */
    id?: string;
    name?: string;
    upstreamUrl?: string;
    credentialId?: string;
    projectId?: string;
}

/** Options for creating a token. */
export interface TokenCreateOptions {
    name: string;
    upstreamUrl: string;
    credentialId?: string;
    projectId?: string;
    policyIds?: string[];
    circuitBreaker?: Record<string, unknown>;
    fallbackUrl?: string;
    upstreams?: Upstream[];
    logLevel?: "metadata" | "redacted" | "full";
    expiresAt?: string;
}

// ────────────────────────────────────────────────────────────────────────────
// Credential
// ────────────────────────────────────────────────────────────────────────────

/** An encrypted credential (API key) for an upstream provider. */
export interface Credential {
    id: string;
    name: string;
    provider: string;
    createdAt?: string;
}

/** Response from creating a credential. */
export interface CredentialCreateResponse {
    id?: string;
    name?: string;
    provider?: string;
}

// ────────────────────────────────────────────────────────────────────────────
// Policy
// ────────────────────────────────────────────────────────────────────────────

/** A security policy applied to token requests. */
export interface Policy {
    id: string;
    name: string;
    mode: string;
    phase: string;
    rules: PolicyRule[];
}

/** A single rule within a policy. */
export interface PolicyRule {
    when: Record<string, unknown>;
    then: Record<string, unknown>;
}

/** Response from creating a policy. */
export interface PolicyCreateResponse {
    id?: string;
    name?: string;
    mode?: string;
    phase?: string;
}

// ────────────────────────────────────────────────────────────────────────────
// Audit
// ────────────────────────────────────────────────────────────────────────────

/** A single audit log entry for a proxied request. */
export interface AuditLog {
    id: string;
    createdAt: string;
    method: string;
    path: string;
    upstreamStatus?: number;
    responseLatencyMs?: number;
    agentName?: string;
    policyResult?: string;
    hitlRequired: boolean;
    hitlDecision?: string;
    hitlLatencyMs?: number;
    fieldsRedacted?: string[];
    shadowViolations?: string[];
    model?: string;
    promptTokens?: number;
    completionTokens?: number;
    finishReason?: string;
    isStreaming?: boolean;
    cacheHit?: boolean;
}

// ────────────────────────────────────────────────────────────────────────────
// HITL / Approvals
// ────────────────────────────────────────────────────────────────────────────

/** Summary of the original request, embedded in approval requests. */
export interface RequestSummary {
    method: string;
    path: string;
    agent?: string;
    upstream?: string;
}

/** A HITL approval request pending admin review. */
export interface ApprovalRequest {
    id: string;
    tokenId: string;
    status: "pending" | "approved" | "rejected" | "expired" | "timeout";
    requestSummary: RequestSummary;
    expiresAt?: string;
    updated?: boolean;
}

/** The result of an admin approval decision. */
export interface ApprovalDecision {
    id: string;
    status: string;
    updated: boolean;
}

// ────────────────────────────────────────────────────────────────────────────
// Service
// ────────────────────────────────────────────────────────────────────────────

/** A registered external service for the Action Gateway. */
export interface Service {
    id: string;
    name: string;
    description: string;
    baseUrl: string;
    serviceType: string;
    credentialId?: string;
    isActive: boolean;
    createdAt?: string;
    updatedAt?: string;
}

// ────────────────────────────────────────────────────────────────────────────
// Pagination & generic
// ────────────────────────────────────────────────────────────────────────────

/** Pagination options for list endpoints. */
export interface PaginationOptions {
    limit?: number;
    offset?: number;
}

/** Options for OpenAI-style cursor pagination. */
export interface CursorPaginationOptions {
    limit?: number;
    after?: string;
}

/** Generic key-value response returned by many endpoints. */
export type JsonObject = Record<string, unknown>;
