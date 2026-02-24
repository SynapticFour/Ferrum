/** DRS object (minimal for listing) */
export interface DrsObject {
  id: string;
  name?: string;
  mime_type?: string;
  size?: number;
  created_time?: string;
  updated_time?: string;
  checksums?: { type: string; checksum: string }[];
  access_methods?: { type: string; access_id?: string; access_url?: { url: string } }[];
  aliases?: string[];
  is_bundle?: boolean;
  description?: string;
}

/** WES run state */
export type WesState =
  | 'UNKNOWN'
  | 'QUEUED'
  | 'INITIALIZING'
  | 'RUNNING'
  | 'PAUSED'
  | 'COMPLETE'
  | 'EXECUTOR_ERROR'
  | 'SYSTEM_ERROR'
  | 'CANCELED'
  | 'CANCELING';

export interface WesRun {
  run_id: string;
  state: WesState;
  run_log?: {
    name?: string;
    cmd?: string[];
    start_time?: string;
    end_time?: string;
    exit_code?: number;
  };
  task_logs?: Array<{
    task_id?: string;
    name?: string;
    cmd?: string[];
    start_time?: string;
    end_time?: string;
    exit_code?: number;
  }>;
  start_time?: string;
  end_time?: string;
}

/** TRS tool */
export interface TrsTool {
  id: string;
  name?: string;
  description?: string;
  organization?: string;
  toolclass?: string;
  meta_version?: string;
}

export interface TrsToolVersion {
  id: string;
  name: string;
  tool_id: string;
}

/** Beacon variant query response */
export interface BeaconVariantResponse {
  meta?: Record<string, unknown>;
  response?: {
    exists?: boolean;
    count?: number;
    variants?: unknown[];
  };
}

/** Health status */
export interface HealthStatus {
  status?: string;
  services?: Record<string, { status: string }>;
}
