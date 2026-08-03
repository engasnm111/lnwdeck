import { invoke } from "@tauri-apps/api/core";

export interface OverviewData {
  total_events: number;
  total_tokens_input: number;
  total_tokens_output: number;
  provider_count: number;
  high_confidence_count: number;
  confidence_coverage: number;
  latest_event_at: string | null;
  oldest_event_at: string | null;
}

export async function fetchOverview(): Promise<OverviewData> {
  return invoke<OverviewData>("get_overview");
}
