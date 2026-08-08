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

export type DashboardRange =
  | "day"
  | "week"
  | "month"
  | "year"
  | "total"
  | "custom";

export interface DashboardQuery {
  range: DashboardRange;
  /** Inclusive local calendar start, formatted as YYYY-MM-DD. */
  start?: string;
  /** Inclusive local calendar end, formatted as YYYY-MM-DD. */
  end?: string;
  /** Empty/undefined means all providers. */
  provider_id?: string;
}

export interface DashboardProviderUsage {
  provider_id: string;
  request_count: number;
  tokens_input: number;
  tokens_output: number;
  total_tokens: number;
}

export interface DashboardTrendPoint {
  bucket: string;
  request_count: number;
  tokens_input: number;
  tokens_output: number;
  total_tokens: number;
}

export interface DashboardHeatmapCell {
  day: string;
  request_count: number;
  total_tokens: number;
}

export interface DashboardSessionProvider {
  provider_id: string;
  request_count: number;
  tokens_input: number;
  tokens_output: number;
  total_tokens: number;
}

export interface DashboardSession {
  session_hash: string;
  display_name: string;
  request_count: number;
  tokens_input: number;
  tokens_output: number;
  total_tokens: number;
  first_seen_at: string | null;
  last_seen_at: string | null;
  providers: DashboardSessionProvider[];
}

export interface UsageDashboardData {
  range: DashboardRange;
  generated_at: string;
  start: string | null;
  end: string | null;
  duration_days: number;
  request_count: number;
  tokens_input: number;
  tokens_output: number;
  total_tokens: number;
  provider_count: number;
  session_count: number;
  providers: DashboardProviderUsage[];
  trend: DashboardTrendPoint[];
  heatmap: DashboardHeatmapCell[];
  sessions: DashboardSession[];
}

export async function fetchUsageDashboard(
  query: DashboardQuery,
): Promise<UsageDashboardData> {
  return invoke<UsageDashboardData>("get_usage_dashboard", { query });
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

/** Writes a sanitized diagnostics snapshot to Downloads and returns its path. */
export async function exportDiagnostics(): Promise<string> {
  return invoke<string>("export_diagnostics");
}

/** Opens the file explorer with the given file selected. */
export async function revealInExplorer(path: string): Promise<void> {
  return invoke<void>("reveal_in_explorer", { path });
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

export interface RefreshStartResult {
  started: boolean;
  already_running: boolean;
}

export type RefreshProgressPhase =
  | "started"
  | "progress"
  | "completed"
  | "partial"
  | "failed";

export interface RefreshProgressEvent {
  phase: RefreshProgressPhase;
  completed: number;
  total: number;
  provider_id: string | null;
  error_code: string | null;
}

export async function refreshAll(): Promise<RefreshCycle> {
  return invoke<RefreshCycle>("refresh_all");
}

/** Starts the shared non-blocking refresh job used by the app, widget and tray. */
export async function startRefresh(): Promise<RefreshStartResult> {
  return invoke<RefreshStartResult>("start_refresh");
}

/** Requests a cooperative stop between provider refreshes. */
export async function cancelRefresh(): Promise<void> {
  return invoke<void>("cancel_refresh");
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
  /** Decimal cost string; unknown models carry a labeled estimate. */
  cost: string;
  /** `priced`, `estimated` or `no catalog entry`. */
  pricing_status: string;
}

export interface CostBreakdownData {
  window: HistoryWindow;
  generated_at: string;
  rows: ModelCostRow[];
  priced_total: string;
  priced_rows: number;
  /** Rows charged at the generic estimate rate. */
  estimated_rows: number;
  unpriced_rows: number;
  unpriced_tokens: number;
}

export async function fetchCosts(
  window: HistoryWindow,
): Promise<CostBreakdownData> {
  return invoke<CostBreakdownData>("get_costs", { window });
}

// ── Sessions ─────────────────────────────────────────────────────────────

export interface SessionUsageRow {
  session_hash: string;
  /** User-entered name, or a generated label such as `Session 01`. */
  display_name: string;
  provider_id: string;
  request_count: number;
  tokens_input: number;
  tokens_output: number;
  cost: string;
  first_seen_at: string | null;
  last_seen_at: string | null;
}

export interface ProjectUsage {
  /** Keyed hash of the folder identity; `""` groups unassigned events. */
  project_hash: string;
  /** User-entered name, or a generated label such as `Project 01`. */
  display_name: string;
  request_count: number;
  tokens_input: number;
  tokens_output: number;
  cost: string;
  first_seen_at: string | null;
  last_seen_at: string | null;
  sessions: SessionUsageRow[];
}

export interface SessionsOverview {
  window: HistoryWindow;
  generated_at: string;
  since: string | null;
  request_count: number;
  tokens_input: number;
  tokens_output: number;
  cost: string;
  projects: ProjectUsage[];
  providers: string[];
}

export async function fetchSessions(
  window: HistoryWindow,
  providerId?: string,
): Promise<SessionsOverview> {
  return invoke<SessionsOverview>("get_sessions", {
    window,
    providerId,
  });
}

/** Stores a user-entered display name for a session (metadata only). */
export async function renameSession(
  sessionHash: string,
  displayName: string,
): Promise<void> {
  return invoke<void>("rename_session", { sessionHash, displayName });
}

/** Stores a user-entered display name for a project (metadata only). */
export async function renameProject(
  projectHash: string,
  displayName: string,
): Promise<void> {
  return invoke<void>("rename_project", { projectHash, displayName });
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

export async function markAllAlertsRead(): Promise<number> {
  return invoke<number>("acknowledge_all_alerts");
}

export interface AppSettingsData {
  launch_at_startup: boolean;
  theme: "dark" | "light" | "system";
  refresh_interval_seconds: number;
  auto_update_check: boolean;
  widget_opacity: number;
  widget_locked: boolean;
  widget_visible: boolean;
  widget_size: string;
  retention_days: number;
  pet_visible: boolean;
  pet_character: string;
  pet_speed: string;
  pet_opacity: number;
  pet_auto_sleep: boolean;
  pet_size: string;
  pet_stay_in_place: boolean;
  pet_pose_wave: boolean;
  pet_pose_jump: boolean;
  pet_pose_look_left: boolean;
  pet_pose_look_right: boolean;
  pet_pose_waiting: boolean;
  pet_pose_review: boolean;
  language: string;
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

/** Widget layout: horizontal bars, compact rings, or the animated pet. */
export type WidgetView = "bars" | "rings" | "pet";

/** Fixed widget window sizes: chosen in Settings, never user-resized. */
export type WidgetSizePreset = "small" | "medium" | "large";

/** A validated community pet from codex-pets.net, installed locally. */
export interface PetManifest {
  id: string;
  displayName: string;
  description: string;
  spritesheetPath: string;
  /** 1 or 2; v2 spritesheets carry the look-direction rows. */
  spriteVersionNumber: number;
  kind?: string;
}

export interface WidgetSettingsData {
  opacity: number;
  locked: boolean;
  visible: boolean;
  /** Pinned provider ids. Empty means every reporting provider is shown. */
  selected_providers: string[];
  view: WidgetView;
  /** Community pet id for the pet layout. Empty means the built-in robot. */
  pet_id: string;
  /** Fixed window size preset: "small", "medium" or "large". */
  size_preset: WidgetSizePreset;
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

/** Switches the widget layout and returns the stored layout. */
export async function setWidgetView(view: WidgetView): Promise<WidgetView> {
  return invoke<WidgetView>("set_widget_view", { view });
}

/**
 * Switches the widget's fixed size preset and returns the stored preset.
 * The backend resizes the window; the widget is never user-resized.
 */
export async function setWidgetSizePreset(
  preset: WidgetSizePreset,
): Promise<WidgetSizePreset> {
  return invoke<WidgetSizePreset>("set_widget_size_preset", { preset });
}

/**
 * Pins the widget to a set of providers and returns the stored selection.
 * An empty list restores every reporting provider.
 */
export async function setWidgetProviders(
  providers: string[],
): Promise<string[]> {
  return invoke<string[]>("set_widget_providers", { providers });
}

/** Imports a community pet from an official codex-pets.net URL. */
export async function importWidgetPet(input: string): Promise<PetManifest> {
  return invoke<PetManifest>("import_widget_pet", { input });
}

/** Imports a community pet from a local `.codex-pet.zip` file path. */
export async function importWidgetPetFile(path: string): Promise<PetManifest> {
  return invoke<PetManifest>("import_widget_pet_file", { path });
}

/** Installed community pets, sorted by display name. */
export async function listWidgetPets(): Promise<PetManifest[]> {
  return invoke<PetManifest[]>("list_widget_pets");
}

/** The community pet selected for the widget, when one is selected. */
export async function getWidgetPet(): Promise<PetManifest | null> {
  return invoke<PetManifest | null>("get_widget_pet");
}

/**
 * Selects the widget's community pet and returns the stored id.
 * An empty id restores the built-in robot.
 */
export async function setWidgetPet(petId: string): Promise<string> {
  return invoke<string>("set_widget_pet", { petId });
}

/** Removes an installed pet; the widget falls back to the built-in robot. */
export async function removeWidgetPet(petId: string): Promise<void> {
  return invoke<void>("remove_widget_pet", { petId });
}

// ── Desktop pet window ───────────────────────────────────────────────────

export interface PetWindowSettingsData {
  visible: boolean;
  character: string;
  speed: string;
  opacity: number;
  /** Serialized camelCase by the backend (PetWindowSettings). */
  autoSleep: boolean;
  /** Serialized camelCase by the backend (PetWindowSettings). */
  sizePreset: PetSizePreset;
  /** Whether the pet stays in place instead of walking. */
  stayInPlace: boolean;
  poseWave: boolean;
  poseJump: boolean;
  poseLookLeft: boolean;
  poseLookRight: boolean;
  poseWaiting: boolean;
  poseReview: boolean;
}

/** Ambient pose keys, matching the backend setting keys. */
export type PetPoseKey =
  | "pet_pose_wave"
  | "pet_pose_jump"
  | "pet_pose_look_left"
  | "pet_pose_look_right"
  | "pet_pose_waiting"
  | "pet_pose_review";

/** Fixed pet window sizes: chosen in Settings, never user-resized. */
export type PetSizePreset = "small" | "medium" | "large";

export async function fetchPetWindowSettings(): Promise<PetWindowSettingsData> {
  return invoke<PetWindowSettingsData>("get_pet_window_settings");
}

export async function showPetWindow(): Promise<void> {
  return invoke<void>("show_pet_window");
}

export async function hidePetWindow(): Promise<void> {
  return invoke<void>("hide_pet_window");
}

/** Moves the pet window so it follows the pet as it walks (screen coords). */
export async function movePetWindow(x: number, y: number): Promise<void> {
  return invoke<void>("move_pet_window", { x, y });
}

/**
 * Tells the backend which screen rectangle (logical px) the pet sprite and
 * its tooltip occupy; the pet window is click-through everywhere else.
 */
export async function setPetHitRect(rect: [number, number, number, number] | null): Promise<void> {
  return invoke<void>("set_pet_hit_rect", { rect });
}

/**
 * Applies the click-through state computed by the backend's cursor poller.
 * Called periodically from the pet window so window APIs stay on the UI thread.
 */
export async function applyPetClickThrough(): Promise<void> {
  return invoke<void>("apply_pet_click_through");
}

const spritesheetCache = new Map<string, string>();

/**
 * The installed pet's spritesheet as an object URL.
 *
 * Loaded over IPC (raw bytes -> Blob) instead of a custom URI scheme, which
 * WebView2 blocks on http dev origins. Cached per pet id.
 */
export async function fetchPetSpritesheetUrl(petId: string): Promise<string> {
  const cached = spritesheetCache.get(petId);
  if (cached) return cached;
  const buffer = await invoke<ArrayBuffer>("read_pet_spritesheet", { id: petId });
  const url = URL.createObjectURL(
    new Blob([buffer], { type: "image/webp" }),
  );
  spritesheetCache.set(petId, url);
  return url;
}

export async function setPetCharacter(character: string): Promise<string> {
  return invoke<string>("set_pet_character", { character });
}

export async function setPetSpeed(speed: string): Promise<string> {
  return invoke<string>("set_pet_speed", { speed });
}

export async function setPetOpacity(opacity: number): Promise<number> {
  return invoke<number>("set_pet_opacity", { opacity });
}

export async function setPetAutoSleep(autoSleep: boolean): Promise<boolean> {
  return invoke<boolean>("set_pet_auto_sleep", { autoSleep });
}

/**
 * Switches the pet's fixed size preset and returns the stored preset.
 * The backend resizes the window; the sprite scales with it.
 */
export async function setPetSizePreset(
  preset: PetSizePreset,
): Promise<PetSizePreset> {
  return invoke<PetSizePreset>("set_pet_size_preset", { preset });
}

/** Sets whether the pet stays in place instead of walking. */
export async function setPetStayInPlace(value: boolean): Promise<boolean> {
  return invoke<boolean>("set_pet_stay_in_place", { value });
}

/** Enables or disables one ambient pose by its backend key. */
export async function setPetPose(
  key: PetPoseKey,
  enabled: boolean,
): Promise<boolean> {
  return invoke<boolean>("set_pet_pose", { key, enabled });
}

/** Sets the UI language and returns what was stored. */
export async function setLanguage(language: string): Promise<string> {
  return invoke<string>("set_language", { language });
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
