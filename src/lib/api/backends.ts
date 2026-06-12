import { invoke } from "@tauri-apps/api/core";

export type BackendStatus =
  | "stopped"
  | "starting"
  | "running"
  | "stopping"
  | "error";

export interface ManagedBackend {
  id: string;
  name: string;
  kind: string;
  enabled: boolean;
  managed: boolean;
  start_command: string;
  start_args?: string[] | null;
  working_dir?: string | null;
  host: string;
  port: number;
  health_path: string;
  api_key?: string | null;
  env_json?: Record<string, string> | null;
  auto_restart: boolean;
  startup_timeout_ms: number;
  status: BackendStatus;
  pid?: number | null;
  last_error?: string | null;
  created_at: number;
  updated_at: number;
}

export interface BackendRequest {
  name: string;
  kind: string;
  start_command: string;
  start_args?: string[] | null;
  working_dir?: string | null;
  host?: string | null;
  port?: number | null;
  health_path?: string | null;
  api_key?: string | null;
  env_json?: Record<string, string> | null;
  auto_restart?: boolean | null;
  startup_timeout_ms?: number | null;
}

export interface BackendHealthResult {
  ok: boolean;
  url: string;
  status?: number | null;
  latency_ms: number;
  message: string;
  backend: ManagedBackend;
}

export interface BackendModelsResult {
  url: string;
  models: string[];
}

export interface BackendEnvFile {
  path: string;
  exists: boolean;
  content: string;
}

export const backendsApi = {
  list(): Promise<ManagedBackend[]> {
    return invoke("list_backends");
  },

  create(req: BackendRequest): Promise<ManagedBackend> {
    return invoke("create_backend", { req });
  },

  update(id: string, req: Partial<BackendRequest>): Promise<ManagedBackend> {
    return invoke("update_backend", { id, req });
  },

  delete(id: string): Promise<void> {
    return invoke("delete_backend", { id });
  },

  start(id: string): Promise<ManagedBackend> {
    return invoke("start_backend", { id });
  },

  stop(id: string): Promise<ManagedBackend> {
    return invoke("stop_backend", { id });
  },

  restart(id: string): Promise<ManagedBackend> {
    return invoke("restart_backend", { id });
  },

  logs(id: string): Promise<string[]> {
    return invoke("get_backend_logs", { id });
  },

  sendInput(id: string, input: string): Promise<ManagedBackend> {
    return invoke("send_backend_input", { id, input });
  },

  checkHealth(id: string): Promise<BackendHealthResult> {
    return invoke("check_backend_health", { id });
  },

  listModels(id: string): Promise<BackendModelsResult> {
    return invoke("list_backend_models", { id });
  },

  readEnvFile(workingDir: string): Promise<BackendEnvFile> {
    return invoke("read_backend_env_file", { workingDir });
  },

  writeEnvFile(workingDir: string, content: string): Promise<BackendEnvFile> {
    return invoke("write_backend_env_file", { workingDir, content });
  },
};
