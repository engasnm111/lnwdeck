import { invoke } from "@tauri-apps/api/core";

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
  return invoke<OverviewData>("get_overview");
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
  return invoke<AnalyticsResult>("get_analytics", { filter });
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
  return invoke<DetailedProviderInfo[]>("get_providers");
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
  return invoke<PipelineDiagnostics>("get_pipeline_diagnostics");
}

export async function refreshAll(): Promise<CollectorRunRow[]> {
  return invoke<CollectorRunRow[]>("refresh_all");
}
