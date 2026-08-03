import { invoke } from "@tauri-apps/api/core";

/**
 * Typed bridge to the Rust backend.
 *
 * Every function propagates backend failures. Nothing here substitutes a
 * default, a demo value or an empty success: a page renders real data, an
 * explicit empty state, or an error state.
 *
 * Optional numeric fields are `number | null` on purpose. `null` means the
 * provider did not report the value, and the UI must not render a bar or a
 * percentage for it.
 */

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
  vendor: string;
  enabled: boolean;
  detected: boolean;
  source_type: string;
  usage_support: string;
  quota_support: string;
  auth_requirement: string;
  health_status: string;
  event_count: number;
  total_tokens: number;
  last_sync: string | null;
  last_error_code: string;
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

export interface AppEventRow {
  id: number;
  occurred_at: string;
  source: string;
  level: "info" | "warning" | "error";
  code: string;
  detail: string;
}

export async function fetchAppEvents(limit?: number): Promise<AppEventRow[]> {
  return invoke<AppEventRow[]>("get_app_events", { limit });
}

export interface QuotaWindowData {
  window_key: string;
  label: string;
  scope: "rolling" | "daily" | "weekly" | "monthly" | "session" | "other";
  kind: "requests" | "tokens" | "credits" | "parallel";
  used: number;
  /** Null when the provider reports no limit. */
  limit: number | null;
  remaining: number | null;
  used_percent: number | null;
  remaining_percent: number | null;
  reset_at: string | null;
  is_unlimited: boolean;
  confidence: "Low" | "Medium" | "High";
}

export type QuotaStatus =
  | "fresh"
  | "stale"
  | "unavailable"
  | "auth_expired"
  | "rate_limited"
  | "error";

export interface ProviderQuotaCard {
  provider_id: string;
  display_name: string;
  status: QuotaStatus;
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

export async function fetchQuotaDashboard(): Promise<QuotaDashboardData> {
  return invoke<QuotaDashboardData>("get_quota_dashboard");
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
  status: QuotaStatus;
  error_code: string;
}

export interface RefreshCycle {
  usage: CollectionOutcome[];
  quota: QuotaCollectionOutcome[];
}

export async function refreshAll(): Promise<RefreshCycle> {
  return invoke<RefreshCycle>("refresh_all");
}

export async function refreshProvider(
  providerId: string,
): Promise<RefreshCycle> {
  return invoke<RefreshCycle>("refresh_provider", { providerId });
}

export type HistoryWindow = "last_24h" | "last_7d" | "last_30d" | "all";

export interface ModelUsageRow {
  model: string;
  provider_id: string;
  request_count: number;
  tokens_input: number;
  tokens_output: number;
  token_share_percent: number | null;
  first_seen_at: string | null;
  last_seen_at: string | null;
}

export interface DailyUsageRow {
  day: string;
  request_count: number;
  tokens_input: number;
  tokens_output: number;
}

export interface UsageHistoryData {
  window: HistoryWindow;
  generated_at: string;
  since: string | null;
  request_count: number;
  tokens_input: number;
  tokens_output: number;
  models: ModelUsageRow[];
  daily: DailyUsageRow[];
  providers: string[];
}

export async function fetchUsageHistory(
  window: HistoryWindow,
  providerId?: string,
): Promise<UsageHistoryData> {
  return invoke<UsageHistoryData>("get_usage_history", {
    window,
    providerId,
  });
}

export interface ModelCostRow {
  provider_id: string;
  model: string;
  request_count: number;
  tokens_input: number;
  tokens_output: number;
  /** Null when the model has no pricing entry. */
  cost: string | null;
  pricing_status: string;
}

export interface CostBreakdownData {
  window: HistoryWindow;
  generated_at: string;
  rows: ModelCostRow[];
  priced_total: string;
  priced_rows: number;
  unpriced_rows: number;
  unpriced_tokens: number;
}

export async function fetchCosts(
  window: HistoryWindow,
): Promise<CostBreakdownData> {
  return invoke<CostBreakdownData>("get_costs", { window });
}

export type BudgetPeriod = "daily" | "weekly" | "monthly";

export interface BudgetScope {
  kind: "global" | "provider";
  provider_id?: string;
}

export interface BudgetRowData {
  id: number;
  scope: BudgetScope;
  period: BudgetPeriod;
  cost_limit: string;
  token_limit: number | null;
  warn_percent: number;
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface BudgetProgressData {
  budget: BudgetRowData;
  period_start: string;
  request_count: number;
  tokens_used: number;
  cost_used: string;
  unpriced_tokens: number;
  cost_percent: number | null;
  token_percent: number | null;
  state: "under" | "warning" | "exceeded" | "unknown";
}

export interface BudgetOverviewData {
  generated_at: string;
  budgets: BudgetProgressData[];
}

export async function fetchBudgets(): Promise<BudgetOverviewData> {
  return invoke<BudgetOverviewData>("get_budgets");
}

export interface BudgetInput {
  scope: "global" | "provider";
  provider_id?: string;
  period: BudgetPeriod;
  cost_limit: string;
  token_limit?: number;
  warn_percent: number;
  enabled: boolean;
}

export async function saveBudget(budget: BudgetInput): Promise<number> {
  return invoke<number>("save_budget", { budget });
}

export async function deleteBudget(id: number): Promise<void> {
  return invoke<void>("delete_budget", { id });
}

export type AlertKind =
  | "quota_threshold"
  | "collector_error"
  | "auth_expired"
  | "rate_limited"
  | "budget_warning"
  | "budget_exceeded";

export interface AlertRowData {
  id: number;
  alert_key: string;
  kind: AlertKind;
  severity: "info" | "warning" | "critical";
  provider_id: string;
  title: string;
  detail: string;
  error_code: string;
  first_seen_at: string;
  last_seen_at: string;
  occurrences: number;
  acknowledged_at: string | null;
  resolved_at: string | null;
}

export interface AlertsViewData {
  generated_at: string;
  open: AlertRowData[];
  history: AlertRowData[];
  open_count: number;
  critical_count: number;
  unacknowledged_count: number;
}

export async function fetchAlerts(): Promise<AlertsViewData> {
  return invoke<AlertsViewData>("get_alerts");
}

export async function acknowledgeAlert(id: number): Promise<void> {
  return invoke<void>("acknowledge_alert", { id });
}

export interface AppSettingsData {
  launch_at_startup: boolean;
  theme: "dark" | "light" | "system";
  refresh_interval_seconds: number;
  auto_update_check: boolean;
  widget_opacity: number;
  widget_locked: boolean;
  widget_visible: boolean;
  retention_days: number;
}

export interface ProviderCredentialState {
  provider_id: string;
  display_name: string;
  state: "missing" | "configured" | "expired";
}

export interface SettingsViewData {
  settings: AppSettingsData;
  startup_supported: boolean;
  startup_registered: boolean;
  credential_store_supported: boolean;
  provider_credentials: ProviderCredentialState[];
  allowed_refresh_intervals: number[];
  allowed_themes: string[];
  allowed_retention_days: number[];
}

export async function fetchSettings(): Promise<SettingsViewData> {
  return invoke<SettingsViewData>("get_settings");
}

export async function saveSettings(
  settings: AppSettingsData,
): Promise<SettingsViewData> {
  return invoke<SettingsViewData>("save_settings", { settings });
}

export async function setProviderKey(
  providerId: string,
  apiKey: string,
): Promise<SettingsViewData> {
  return invoke<SettingsViewData>("set_provider_key", { providerId, apiKey });
}

export async function deleteProviderKey(
  providerId: string,
): Promise<SettingsViewData> {
  return invoke<SettingsViewData>("delete_provider_key", { providerId });
}

export interface WidgetSettingsData {
  opacity: number;
  locked: boolean;
  visible: boolean;
}

export async function fetchWidgetSettings(): Promise<WidgetSettingsData> {
  return invoke<WidgetSettingsData>("get_widget_settings");
}

export async function setWidgetOpacity(opacity: number): Promise<number> {
  return invoke<number>("set_widget_opacity", { opacity });
}

export async function setWidgetLocked(locked: boolean): Promise<boolean> {
  return invoke<boolean>("set_widget_locked", { locked });
}

export async function showWidgetWindow(): Promise<void> {
  return invoke<void>("show_widget");
}

export async function hideWidgetWindow(): Promise<void> {
  return invoke<void>("hide_widget");
}

export async function showMainWindow(): Promise<void> {
  return invoke<void>("show_main_window");
}

export interface UpdateCheckResult {
  available: boolean;
  current_version: string;
  version: string | null;
  notes: string | null;
  published_at: string | null;
}

/** Checks for an update. Never downloads or installs anything. */
export async function checkForUpdate(): Promise<UpdateCheckResult> {
  return invoke<UpdateCheckResult>("check_for_update");
}

/**
 * Downloads and installs the available update, then the backend restarts the
 * application. Rejects with a sanitized code when the download, the signature
 * check or the installer fails.
 */
export async function installUpdate(): Promise<string> {
  return invoke<string>("install_update");
}
