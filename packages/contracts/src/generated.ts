// GENERATED FILE — do not edit manually.
// Source: schemas/domain/*.json (generated from crates/domain Rust types)

export type Confidence = "Low" | "Medium" | "High";

export interface UsageEvent {
  id: string;
  timestamp: string;
  provider_id: string;
  model: string;
  tokens_input: number;
  tokens_output: number;
  confidence: Confidence;
  data_source: string;
  cost: string;
  session_hash: string | null;
  project_hash: string | null;
  account_fingerprint: string | null;
}

export interface UsageBatch {
  batch_id: string;
  events: UsageEvent[];
}

export interface QuotaSnapshot {
  provider_id: string;
  quota_limit: number;
  quota_used: number;
  recorded_at: string;
}

export interface ProviderDescriptor {
  id: string;
  name: string;
  adapter_id: string;
  enabled: boolean;
}
