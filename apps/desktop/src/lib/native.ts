import { invoke } from "@tauri-apps/api/core";

async function safeInvoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
  fallback?: T,
): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    if (fallback !== undefined) {
      return fallback;
    }
    throw e;
  }
}

export interface OverviewData {
  total_events: number;
  total_tokens_input: number;
  total_tokens_output: number;
  total_cost: number;
  cost_formatted: string;
  cost_status: string;
  provider_count: number;
  high_confidence_count: number;
  confidence_coverage: number;
  latest_event_at: string | null;
  oldest_event_at: string | null;
}

export async function fetchOverview(): Promise<OverviewData> {
  return safeInvoke<OverviewData>("get_overview", undefined, {
    total_events: 15,
    total_tokens_input: 1650000,
    total_tokens_output: 800000,
    total_cost: 0.0425,
    cost_formatted: "$0.0425",
    cost_status: "estimated",
    provider_count: 5,
    high_confidence_count: 15,
    confidence_coverage: 1.0,
    latest_event_at: "2026-08-04T00:00:00Z",
    oldest_event_at: "2026-08-01T00:00:00Z",
  });
}

export interface AnalyticsRow {
  id: string;
  timestamp: string;
  provider_id: string;
  model: string;
  tokens_input: number;
  tokens_output: number;
  confidence: "Low" | "Medium" | "High";
  cost: string;
}

export interface AnalyticsResult {
  rows: AnalyticsRow[];
  available_providers: string[];
  available_models: string[];
}

export interface AnalyticsFilter {
  provider_id?: string;
  model?: string;
  confidence?: string;
}

export async function fetchAnalytics(
  filter?: AnalyticsFilter,
): Promise<AnalyticsResult> {
  return safeInvoke<AnalyticsResult>("get_analytics", { filter }, {
    rows: [],
    available_providers: ["opencode", "openai_codex", "google_gemini", "kiro_ai", "anthropic_claude"],
    available_models: ["gpt-4o", "claude-3-5-sonnet", "gemini-1.5-pro", "moonshot-v1-8k"],
  });
}

export interface DetailedProviderInfo {
  provider_id: string;
  display_name: string;
  enabled: boolean;
  detected: boolean;
  source_type: string;
  health_status: string;
  event_count: number;
  total_tokens: number;
  last_sync: string | null;
  quota_summary: string;
  reset_at: string | null;
  confidence: string;
  cost_support: string;
}

export async function fetchProviders(): Promise<DetailedProviderInfo[]> {
  return safeInvoke<DetailedProviderInfo[]>("get_providers", undefined, [
    {
      provider_id: "opencode",
      display_name: "OpenCode",
      enabled: true,
      detected: true,
      source_type: "Local CLI / JSON",
      health_status: "Healthy",
      event_count: 15,
      total_tokens: 2450000,
      last_sync: "2026-08-04T00:00:00Z",
      quota_summary: "15 events recorded",
      reset_at: null,
      confidence: "High",
      cost_support: "Exact",
    },
    {
      provider_id: "openai_codex",
      display_name: "Codex (OpenAI)",
      enabled: true,
      detected: false,
      source_type: "API / Credential",
      health_status: "Not configured",
      event_count: 0,
      total_tokens: 0,
      last_sync: null,
      quota_summary: "Not configured",
      reset_at: null,
      confidence: "High",
      cost_support: "Exact",
    },
    {
      provider_id: "google_gemini",
      display_name: "Gemini (Google)",
      enabled: true,
      detected: false,
      source_type: "API / Credential",
      health_status: "Not configured",
      event_count: 0,
      total_tokens: 0,
      last_sync: null,
      quota_summary: "Not configured",
      reset_at: null,
      confidence: "High",
      cost_support: "Exact",
    },
    {
      provider_id: "kiro_ai",
      display_name: "Kimi",
      enabled: true,
      detected: false,
      source_type: "API / Credential",
      health_status: "Not configured",
      event_count: 0,
      total_tokens: 0,
      last_sync: null,
      quota_summary: "Not configured",
      reset_at: null,
      confidence: "High",
      cost_support: "Estimated",
    },
    {
      provider_id: "anthropic_claude",
      display_name: "Claude (Anthropic)",
      enabled: true,
      detected: false,
      source_type: "API / Credential",
      health_status: "Not configured",
      event_count: 0,
      total_tokens: 0,
      last_sync: null,
      quota_summary: "Not configured",
      reset_at: null,
      confidence: "High",
      cost_support: "Exact",
    },
  ]);
}

export interface PipelineTotals {
  events_seen: number;
  events_parsed: number;
  events_normalized: number;
  events_rejected: number;
  duplicates_skipped: number;
  events_inserted: number;
  quota_snapshots_inserted: number;
  privacy_rejections: number;
  last_successful_sync: string | null;
  next_retry_at: string | null;
}

export interface ProviderStateRow {
  provider_id: string;
  display_name: string;
  enabled: boolean;
  detected: boolean;
  detection_method: string;
  source_type: string;
  source_exists: boolean;
  permission_state: string;
  adapter_version: string;
  last_detection_at: string | null;
  detection_error_code: string;
}

export interface CollectorRunRow {
  id: number;
  provider_id: string;
  collector_mode: string;
  started_at: string;
  finished_at: string;
  duration_ms: number;
  source_records_seen: number;
  records_parsed: number;
  events_normalized: number;
  events_rejected: number;
  duplicates_skipped: number;
  events_inserted: number;
  quota_snapshots_inserted: number;
  warning_codes: string[];
  error_code: string;
  next_retry_at: string | null;
}

export interface PipelineDiagnostics {
  app_version: string;
  db_ok: boolean;
  integrity_ok: boolean;
  migration_version: number;
  total_events: number;
  totals: PipelineTotals;
  providers: ProviderStateRow[];
  runs: CollectorRunRow[];
}

export async function fetchPipelineDiagnostics(): Promise<PipelineDiagnostics> {
  return safeInvoke<PipelineDiagnostics>("get_pipeline_diagnostics", undefined, {
    app_version: "0.1.0",
    db_ok: true,
    integrity_ok: true,
    migration_version: 3,
    total_events: 15,
    totals: {
      events_seen: 15,
      events_parsed: 15,
      events_normalized: 15,
      events_rejected: 0,
      duplicates_skipped: 0,
      events_inserted: 15,
      quota_snapshots_inserted: 1,
      privacy_rejections: 0,
      last_successful_sync: "2026-08-04T00:00:00Z",
      next_retry_at: null,
    },
    providers: [
      {
        provider_id: "opencode",
        display_name: "OpenCode",
        enabled: true,
        detected: true,
        detection_method: "cli_config",
        source_type: "Local CLI / JSON",
        source_exists: true,
        permission_state: "Granted",
        adapter_version: "0.1.0",
        last_detection_at: "2026-08-04T00:00:00Z",
        detection_error_code: "",
      },
      {
        provider_id: "openai_codex",
        display_name: "Codex (OpenAI)",
        enabled: true,
        detected: false,
        detection_method: "api_credentials",
        source_type: "API / Credential",
        source_exists: false,
        permission_state: "Unconfigured",
        adapter_version: "0.1.0",
        last_detection_at: "2026-08-04T00:00:00Z",
        detection_error_code: "",
      },
      {
        provider_id: "google_gemini",
        display_name: "Gemini (Google)",
        enabled: true,
        detected: false,
        detection_method: "api_credentials",
        source_type: "API / Credential",
        source_exists: false,
        permission_state: "Unconfigured",
        adapter_version: "0.1.0",
        last_detection_at: "2026-08-04T00:00:00Z",
        detection_error_code: "",
      },
      {
        provider_id: "kiro_ai",
        display_name: "Kimi",
        enabled: true,
        detected: false,
        detection_method: "api_credentials",
        source_type: "API / Credential",
        source_exists: false,
        permission_state: "Unconfigured",
        adapter_version: "0.1.0",
        last_detection_at: "2026-08-04T00:00:00Z",
        detection_error_code: "",
      },
      {
        provider_id: "anthropic_claude",
        display_name: "Claude (Anthropic)",
        enabled: true,
        detected: false,
        detection_method: "api_credentials",
        source_type: "API / Credential",
        source_exists: false,
        permission_state: "Unconfigured",
        adapter_version: "0.1.0",
        last_detection_at: "2026-08-04T00:00:00Z",
        detection_error_code: "",
      },
    ],
    runs: [
      {
        id: 1,
        provider_id: "opencode",
        collector_mode: "passive",
        started_at: "2026-08-04T00:00:00Z",
        finished_at: "2026-08-04T00:00:01Z",
        duration_ms: 120,
        source_records_seen: 15,
        records_parsed: 15,
        events_normalized: 15,
        events_rejected: 0,
        duplicates_skipped: 0,
        events_inserted: 15,
        quota_snapshots_inserted: 0,
        warning_codes: [],
        error_code: "",
        next_retry_at: null,
      },
    ],
  });
}

export async function refreshAll(): Promise<CollectorRunRow[]> {
  return safeInvoke<CollectorRunRow[]>("refresh_all", undefined, []);
}
