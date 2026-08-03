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

/** Fetches real backend data. Throws on failure so the caller renders an
 * explicit error state; no fabricated values are ever returned. */
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

export interface QuotaWindowData {
  window_key: string;
  label: string;
  scope: "rolling" | "daily" | "weekly" | "monthly" | "session" | "other";
  kind: "requests" | "tokens" | "credits" | "parallel";
  used: number;
  limit: number;
  remaining: number;
  used_percent: number;
  remaining_percent: number;
  reset_at: string | null;
  is_unlimited: boolean;
  confidence: "Low" | "Medium" | "High";
}

export interface ProviderQuotaCard {
  provider_id: string;
  display_name: string;
  status:
    | "fresh"
    | "stale"
    | "unavailable"
    | "auth_expired"
    | "rate_limited"
    | "error";
  plan: string | null;
  source: string;
  collected_at: string;
  stale_at: string;
  error_code: string | null;
  windows: QuotaWindowData[];
}

export interface QuotaDashboardData {
  generated_at: string;
  providers: ProviderQuotaCard[];
}

export interface CollectionOutcome {
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

export interface QuotaCollectionOutcome {
  provider_id: string;
  collector_mode: string;
  started_at: string;
  finished_at: string;
  duration_ms: number;
  windows_collected: number;
  status:
    | QuotaWindowData["confidence"]
    | "fresh"
    | "stale"
    | "unavailable"
    | "auth_expired"
    | "rate_limited"
    | "error";
  error_code: string;
}

export interface RefreshCycle {
  usage: CollectionOutcome[];
  quota: QuotaCollectionOutcome[];
}

/** Fetches the normalized quota dashboard. Throws on failure; the caller
 * renders loading/error states instead of fabricated data. */
export async function fetchQuotaDashboard(): Promise<QuotaDashboardData> {
  return invoke<QuotaDashboardData>("get_quota_dashboard");
}

export async function refreshAll(): Promise<RefreshCycle> {
  return invoke<RefreshCycle>("refresh_all");
}

/** Refreshes a single provider (detection + usage + quota channels). */
export async function refreshProvider(
  providerId: string,
): Promise<RefreshCycle> {
  return invoke<RefreshCycle>("refresh_provider", { providerId });
}

/** Hides the floating widget window. No-op-safe outside Tauri. */
export async function hideWidgetWindow(): Promise<void> {
  try {
    await invoke("hide_widget");
  } catch {
    // outside a Tauri runtime the widget is a normal page
  }
}

/** Brings the main dashboard window to the front. */
export async function showMainWindow(): Promise<void> {
  try {
    await invoke("show_main_window");
  } catch {
    // no-op outside a Tauri runtime
  }
}

export async function checkForUpdate(): Promise<string> {
  return invoke<string>("check_for_update");
}
