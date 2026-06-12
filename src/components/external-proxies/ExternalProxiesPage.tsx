import { useEffect, useMemo, useRef, useState } from "react";
import {
  FolderOpen,
  HeartPulse,
  ListChecks,
  Loader2,
  Pencil,
  Play,
  Plus,
  Power,
  RefreshCw,
  RotateCcw,
  Save,
  SendHorizontal,
  Trash2,
  Terminal,
} from "lucide-react";
import { toast } from "sonner";
import {
  backendsApi,
  settingsApi,
  type BackendRequest,
  type ManagedBackend,
} from "@/lib/api";
import { extractErrorMessage } from "@/utils/errorUtils";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { ConfirmDialog } from "@/components/ConfirmDialog";

interface ProxyFormState {
  name: string;
  startCommand: string;
  workingDir: string;
  port: string;
  healthPath: string;
  apiKey: string;
  startupTimeoutMs: string;
  envJson: string;
}

const emptyForm: ProxyFormState = {
  name: "",
  startCommand: "npm run dev",
  workingDir: "",
  port: "",
  healthPath: "",
  apiKey: "",
  startupTimeoutMs: "30000",
  envJson: "{\n}",
};

function toForm(profile: ManagedBackend): ProxyFormState {
  return {
    name: profile.name,
    startCommand: profile.start_command,
    workingDir: profile.working_dir ?? "",
    port: profile.port ? String(profile.port) : "",
    healthPath: profile.health_path ?? "",
    apiKey: profile.api_key ?? "",
    startupTimeoutMs: String(profile.startup_timeout_ms || 30000),
    envJson: JSON.stringify(profile.env_json ?? {}, null, 2),
  };
}

function buildRequest(form: ProxyFormState): BackendRequest {
  let env: Record<string, string> | null = null;
  const trimmedEnv = form.envJson.trim();
  if (trimmedEnv) {
    const parsed = JSON.parse(trimmedEnv) as Record<string, unknown>;
    env = Object.fromEntries(
      Object.entries(parsed).map(([key, value]) => [key, String(value)]),
    );
  }

  const parsedPort = Number(form.port);
  const parsedTimeout = Number(form.startupTimeoutMs);
  const healthPath = form.healthPath.trim();

  return {
    name: form.name.trim(),
    kind: "custom",
    start_command: form.startCommand.trim(),
    working_dir: form.workingDir.trim(),
    host: "127.0.0.1",
    port: form.port.trim() && Number.isFinite(parsedPort) ? parsedPort : 0,
    health_path: healthPath
      ? healthPath.startsWith("/")
        ? healthPath
        : `/${healthPath}`
      : "",
    api_key: form.apiKey.trim(),
    env_json: env,
    auto_restart: false,
    startup_timeout_ms:
      form.startupTimeoutMs.trim() && Number.isFinite(parsedTimeout)
        ? parsedTimeout
        : 30000,
  };
}

function isProcessAlive(profile: ManagedBackend) {
  return Boolean(profile.pid) || profile.status === "running" || profile.status === "starting";
}

function statusLabel(profile: ManagedBackend) {
  if (profile.pid && profile.status === "error") {
    return "Running";
  }

  const status = profile.status;
  switch (status) {
    case "running":
      return "Running";
    case "starting":
      return "Starting";
    case "stopping":
      return "Stopping";
    case "error":
      return "Error";
    default:
      return "Stopped";
  }
}

function statusClass(profile: ManagedBackend) {
  if (profile.pid && profile.status === "error") {
    return "bg-amber-500/10 text-amber-600 border-amber-500/20";
  }

  const status = profile.status;
  switch (status) {
    case "running":
      return "bg-emerald-500/10 text-emerald-600 border-emerald-500/20";
    case "starting":
    case "stopping":
      return "bg-blue-500/10 text-blue-600 border-blue-500/20";
    case "error":
      return "bg-red-500/10 text-red-600 border-red-500/20";
    default:
      return "bg-muted text-muted-foreground border-border";
  }
}

function quoteEnvValue(value: string) {
  if (!value || /^[A-Za-z0-9_./:@-]+$/.test(value)) {
    return value;
  }
  return JSON.stringify(value);
}

function setEnvLine(content: string, key: string, value: string) {
  const line = `${key}=${quoteEnvValue(value)}`;
  const lines = content.replace(/\r\n/g, "\n").split("\n");
  const index = lines.findIndex((item) =>
    new RegExp(`^\\s*${key}\\s*=`).test(item),
  );

  if (index >= 0) {
    lines[index] = line;
    return lines.join("\n");
  }

  const next = content.trimEnd();
  return next ? `${next}\n${line}\n` : `${line}\n`;
}

export function ExternalProxiesPage() {
  const [profiles, setProfiles] = useState<ManagedBackend[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [logs, setLogs] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editing, setEditing] = useState<ManagedBackend | null>(null);
  const [form, setForm] = useState<ProxyFormState>(emptyForm);
  const [deleteTarget, setDeleteTarget] = useState<ManagedBackend | null>(null);
  const [terminalInput, setTerminalInput] = useState("");
  const [models, setModels] = useState<string[]>([]);
  const [modelsUrl, setModelsUrl] = useState("");
  const [envFileContent, setEnvFileContent] = useState("");
  const [envFilePath, setEnvFilePath] = useState("");
  const [envBusy, setEnvBusy] = useState(false);
  const logScrollRef = useRef<HTMLDivElement | null>(null);
  const shouldFollowLogsRef = useRef(true);

  const selected = useMemo(
    () => profiles.find((profile) => profile.id === selectedId) ?? null,
    [profiles, selectedId],
  );

  const refresh = async (quiet = false) => {
    try {
      if (!quiet) {
        setLoading(true);
      }
      const next = await backendsApi.list();
      setProfiles(next);
      setSelectedId((current) => current ?? next[0]?.id ?? null);
    } catch (error) {
      toast.error("Failed to load external proxies", {
        description: extractErrorMessage(error) || undefined,
      });
    } finally {
      setLoading(false);
    }
  };

  const refreshLogs = async (id: string | null) => {
    if (!id) {
      setLogs([]);
      return;
    }
    try {
      setLogs(await backendsApi.logs(id));
    } catch (error) {
      setLogs([extractErrorMessage(error) || "Failed to load logs"]);
    }
  };

  useEffect(() => {
    void refresh();
  }, []);

  useEffect(() => {
    void refreshLogs(selectedId);
    const timer = window.setInterval(() => {
      void refresh(true);
      void refreshLogs(selectedId);
    }, 2000);
    return () => window.clearInterval(timer);
  }, [selectedId]);

  useEffect(() => {
    const el = logScrollRef.current;
    if (el && shouldFollowLogsRef.current) {
      el.scrollTop = el.scrollHeight;
    }
  }, [logs]);

  useEffect(() => {
    shouldFollowLogsRef.current = true;
    setModels([]);
    setModelsUrl("");
  }, [selectedId]);

  const openCreate = () => {
    setEditing(null);
    setForm(emptyForm);
    setEnvFileContent("");
    setEnvFilePath("");
    setDialogOpen(true);
  };

  const openEdit = (profile: ManagedBackend) => {
    setEditing(profile);
    setForm(toForm(profile));
    setEnvFileContent("");
    setEnvFilePath("");
    setDialogOpen(true);
  };

  const save = async () => {
    try {
      const req = buildRequest(form);
      if (!req.name || !req.start_command) {
        toast.error("Name and command are required");
        return;
      }

      const saved = editing
        ? await backendsApi.update(editing.id, req)
        : await backendsApi.create(req);
      setDialogOpen(false);
      setSelectedId(saved.id);
      await refresh(true);
      toast.success(editing ? "Proxy updated" : "Proxy added");
    } catch (error) {
      toast.error("Could not save proxy", {
        description: extractErrorMessage(error) || undefined,
      });
    }
  };

  const runAction = async (
    profile: ManagedBackend,
    action: "start" | "stop" | "restart",
  ) => {
    try {
      setBusyId(profile.id);
      const updated =
        action === "start"
          ? await backendsApi.start(profile.id)
          : action === "stop"
            ? await backendsApi.stop(profile.id)
            : await backendsApi.restart(profile.id);
      setSelectedId(updated.id);
      await refresh(true);
      await refreshLogs(updated.id);
    } catch (error) {
      toast.error(`Failed to ${action} proxy`, {
        description: extractErrorMessage(error) || undefined,
      });
    } finally {
      setBusyId(null);
    }
  };

  const sendTerminalInput = async () => {
    if (!selected) return;

    try {
      setBusyId(selected.id);
      const updated = await backendsApi.sendInput(selected.id, terminalInput);
      setTerminalInput("");
      setSelectedId(updated.id);
      await refresh(true);
      await refreshLogs(updated.id);
    } catch (error) {
      toast.error("Could not send command", {
        description: extractErrorMessage(error) || undefined,
      });
    } finally {
      setBusyId(null);
    }
  };

  const checkHealth = async () => {
    if (!selected) return;

    try {
      setBusyId(selected.id);
      const result = await backendsApi.checkHealth(selected.id);
      setProfiles((current) =>
        current.map((profile) =>
          profile.id === result.backend.id ? result.backend : profile,
        ),
      );
      const options = { description: `${result.url} - ${result.latency_ms}ms` };
      if (result.ok) {
        toast.success(result.message, options);
      } else {
        toast.error(result.message, options);
      }
    } catch (error) {
      toast.error("Health check failed", {
        description: extractErrorMessage(error) || undefined,
      });
    } finally {
      setBusyId(null);
    }
  };

  const listModels = async () => {
    if (!selected) return;

    try {
      setBusyId(selected.id);
      const result = await backendsApi.listModels(selected.id);
      setModels(result.models);
      setModelsUrl(result.url);
      toast.success(`Found ${result.models.length} models`);
    } catch (error) {
      setModels([]);
      setModelsUrl("");
      toast.error("Could not list models", {
        description: extractErrorMessage(error) || undefined,
      });
    } finally {
      setBusyId(null);
    }
  };

  const confirmDelete = async () => {
    if (!deleteTarget) return;
    try {
      await backendsApi.delete(deleteTarget.id);
      setDeleteTarget(null);
      setSelectedId(null);
      await refresh(true);
      toast.success("Proxy removed");
    } catch (error) {
      toast.error("Could not remove proxy", {
        description: extractErrorMessage(error) || undefined,
      });
    }
  };

  const pickWorkingDir = async () => {
    try {
      const dir = await settingsApi.pickDirectory();
      if (dir) {
        setForm((current) => ({ ...current, workingDir: dir }));
      }
    } catch (error) {
      toast.error("Could not select folder", {
        description: extractErrorMessage(error) || undefined,
      });
    }
  };

  const loadEnvFile = async () => {
    if (!form.workingDir.trim()) {
      toast.error("Working directory is required");
      return;
    }

    try {
      setEnvBusy(true);
      const envFile = await backendsApi.readEnvFile(form.workingDir);
      setEnvFileContent(envFile.content);
      setEnvFilePath(envFile.path);
      toast.success(envFile.exists ? ".env loaded" : ".env not found", {
        description: envFile.path,
      });
    } catch (error) {
      toast.error("Could not load .env", {
        description: extractErrorMessage(error) || undefined,
      });
    } finally {
      setEnvBusy(false);
    }
  };

  const saveEnvFile = async () => {
    if (!form.workingDir.trim()) {
      toast.error("Working directory is required");
      return;
    }

    try {
      setEnvBusy(true);
      const envFile = await backendsApi.writeEnvFile(
        form.workingDir,
        envFileContent,
      );
      setEnvFileContent(envFile.content);
      setEnvFilePath(envFile.path);
      toast.success(".env saved", { description: envFile.path });
    } catch (error) {
      toast.error("Could not save .env", {
        description: extractErrorMessage(error) || undefined,
      });
    } finally {
      setEnvBusy(false);
    }
  };

  const applyFieldsToEnv = () => {
    let next = envFileContent;
    if (form.port.trim()) {
      next = setEnvLine(next, "PORT", form.port.trim());
    }
    next = setEnvLine(next, "HOST", "127.0.0.1");
    if (form.apiKey.trim()) {
      next = setEnvLine(next, "API_KEY", form.apiKey.trim());
      next = setEnvLine(next, "OPENAI_API_KEY", form.apiKey.trim());
      next = setEnvLine(next, "ANTHROPIC_API_KEY", form.apiKey.trim());
    }
    setEnvFileContent(next);
  };

  return (
    <div className="flex h-full min-h-0 flex-col px-6 pb-8">
      <div className="flex items-center justify-between gap-3 py-4">
        <div>
          <h2 className="text-xl font-semibold">External Proxies</h2>
          <p className="text-sm text-muted-foreground">
            Start, stop, and monitor your local proxy processes from one place.
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="outline" onClick={() => void refresh()}>
            <RefreshCw className="h-4 w-4" />
            Refresh
          </Button>
          <Button onClick={openCreate}>
            <Plus className="h-4 w-4" />
            Add Proxy
          </Button>
        </div>
      </div>

      <div className="grid min-h-0 flex-1 grid-cols-[minmax(280px,1fr)_minmax(520px,2fr)] gap-4">
        <div className="min-h-0 overflow-y-auto pr-1">
          {loading ? (
            <div className="flex h-48 items-center justify-center text-muted-foreground">
              <Loader2 className="mr-2 h-4 w-4 animate-spin" />
              Loading proxies
            </div>
          ) : profiles.length === 0 ? (
            <div className="flex h-64 flex-col items-center justify-center rounded-lg border border-dashed text-center">
              <Terminal className="mb-3 h-8 w-8 text-muted-foreground" />
              <p className="text-sm font-medium">No external proxies yet</p>
              <p className="mt-1 max-w-sm text-sm text-muted-foreground">
                Add a command like npm run dev and CC Switch will run it here.
              </p>
              <Button className="mt-4" onClick={openCreate}>
                <Plus className="h-4 w-4" />
                Add Proxy
              </Button>
            </div>
          ) : (
            <div className="space-y-3">
              {profiles.map((profile) => {
                const running = isProcessAlive(profile);
                const busy = busyId === profile.id;
                return (
                  <Card
                    key={profile.id}
                    className={cn(
                      "cursor-pointer transition-colors",
                      selectedId === profile.id
                        ? "border-blue-500/60 bg-blue-500/5"
                        : "hover:bg-muted/40",
                    )}
                    onClick={() => setSelectedId(profile.id)}
                  >
                    <CardHeader className="space-y-3 p-4">
                      <div className="flex items-start justify-between gap-3">
                        <div className="min-w-0">
                          <CardTitle className="truncate text-base">
                            {profile.name}
                          </CardTitle>
                          <CardDescription className="mt-1 truncate font-mono text-xs">
                            {profile.start_command}
                          </CardDescription>
                        </div>
                        <Badge
                          variant="outline"
                          className={statusClass(profile)}
                        >
                          {statusLabel(profile)}
                        </Badge>
                      </div>
                      <div className="flex flex-wrap items-center gap-2">
                        <Button
                          size="sm"
                          variant={running ? "secondary" : "default"}
                          disabled={busy}
                          onClick={(event) => {
                            event.stopPropagation();
                            void runAction(
                              profile,
                              running ? "stop" : "start",
                            );
                          }}
                        >
                          {busy ? (
                            <Loader2 className="h-4 w-4 animate-spin" />
                          ) : running ? (
                            <Power className="h-4 w-4" />
                          ) : (
                            <Play className="h-4 w-4" />
                          )}
                          {running ? "Stop" : "Start"}
                        </Button>
                        <Button
                          size="sm"
                          variant="outline"
                          disabled={busy}
                          onClick={(event) => {
                            event.stopPropagation();
                            void runAction(profile, "restart");
                          }}
                        >
                          <RotateCcw className="h-4 w-4" />
                          Restart
                        </Button>
                        <Button
                          size="sm"
                          variant="ghost"
                          onClick={(event) => {
                            event.stopPropagation();
                            openEdit(profile);
                          }}
                        >
                          <Pencil className="h-4 w-4" />
                        </Button>
                        <Button
                          size="sm"
                          variant="ghost"
                          className="text-red-500 hover:text-red-600"
                          onClick={(event) => {
                            event.stopPropagation();
                            setDeleteTarget(profile);
                          }}
                        >
                          <Trash2 className="h-4 w-4" />
                        </Button>
                      </div>
                    </CardHeader>
                  </Card>
                );
              })}
            </div>
          )}
        </div>

        <Card className="flex min-h-0 flex-col">
          <CardHeader className="p-4">
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <CardTitle className="text-base">
                  {selected?.name ?? "Logs"}
                </CardTitle>
                <CardDescription className="truncate font-mono text-xs">
                  {selected?.working_dir || selected?.start_command || "Select a proxy"}
                </CardDescription>
              </div>
              {selected?.pid && (
                <Badge variant="secondary">PID {selected.pid}</Badge>
              )}
            </div>
            {selected?.pid && selected.status === "error" && selected.last_error && (
              <p className="mt-2 text-xs text-amber-500">{selected.last_error}</p>
            )}
            <div className="mt-3 flex flex-wrap items-center gap-2">
              <Button
                size="sm"
                variant="outline"
                disabled={!selected || busyId === selected.id || selected.port === 0}
                onClick={() => void checkHealth()}
              >
                <HeartPulse className="h-4 w-4" />
                Health
              </Button>
              <Button
                size="sm"
                variant="outline"
                disabled={!selected || !isProcessAlive(selected) || busyId === selected.id || selected.port === 0}
                onClick={() => void listModels()}
              >
                <ListChecks className="h-4 w-4" />
                Models
              </Button>
            </div>
          </CardHeader>
          <CardContent className="flex min-h-0 flex-1 flex-col gap-3 p-4 pt-0">
            {models.length > 0 && (
              <div className="rounded-md border bg-muted/30 p-3">
                <div className="mb-2 flex items-center justify-between gap-3">
                  <p className="text-sm font-medium">Available models</p>
                  <p className="truncate font-mono text-xs text-muted-foreground">
                    {modelsUrl}
                  </p>
                </div>
                <div className="max-h-28 overflow-auto font-mono text-xs text-muted-foreground">
                  {models.map((model) => (
                    <div key={model} className="truncate py-0.5">
                      {model}
                    </div>
                  ))}
                </div>
              </div>
            )}
            <div
              ref={logScrollRef}
              className="min-h-[360px] flex-1 overflow-auto rounded-md border bg-black p-3"
              onScroll={(event) => {
                const target = event.currentTarget;
                const distanceFromBottom =
                  target.scrollHeight - target.scrollTop - target.clientHeight;
                shouldFollowLogsRef.current = distanceFromBottom < 24;
              }}
            >
              <pre className="whitespace-pre-wrap break-words text-xs leading-relaxed text-green-100">
                {logs.length > 0
                  ? logs.join("\n")
                  : selected
                    ? "No logs yet."
                    : "Select a proxy to see logs."}
              </pre>
            </div>
            <div className="flex gap-2">
              <Input
                value={terminalInput}
                disabled={!selected || !isProcessAlive(selected)}
                placeholder="Send text to the running process"
                className="font-mono text-xs"
                onChange={(event) => setTerminalInput(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" && !event.shiftKey) {
                    event.preventDefault();
                    void sendTerminalInput();
                  }
                }}
              />
              <Button
                disabled={!selected || !isProcessAlive(selected)}
                onClick={() => void sendTerminalInput()}
              >
                <SendHorizontal className="h-4 w-4" />
                Send
              </Button>
            </div>
          </CardContent>
        </Card>
      </div>

      <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle>
              {editing ? "Edit External Proxy" : "Add External Proxy"}
            </DialogTitle>
            <DialogDescription>
              Use a shell command exactly as you would run it in a terminal.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4 overflow-y-auto px-6 py-5">
            <div className="space-y-2">
              <Label>Name</Label>
              <Input
                value={form.name}
                placeholder="Local NewAPI"
                onChange={(event) =>
                  setForm((current) => ({
                    ...current,
                    name: event.target.value,
                  }))
                }
              />
            </div>
            <div className="space-y-2">
              <Label>Command</Label>
              <Input
                value={form.startCommand}
                placeholder="npm run dev"
                onChange={(event) =>
                  setForm((current) => ({
                    ...current,
                    startCommand: event.target.value,
                  }))
                }
              />
            </div>
            <div className="space-y-2">
              <Label>Working directory</Label>
              <div className="flex gap-2">
                <Input
                  value={form.workingDir}
                  placeholder="C:\\path\\to\\proxy"
                  onChange={(event) =>
                    setForm((current) => ({
                      ...current,
                      workingDir: event.target.value,
                    }))
                  }
                />
                <Button variant="outline" type="button" onClick={pickWorkingDir}>
                  <FolderOpen className="h-4 w-4" />
                </Button>
              </div>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-2">
                <Label>Port</Label>
                <Input
                  value={form.port}
                  placeholder="3000"
                  inputMode="numeric"
                  onChange={(event) =>
                    setForm((current) => ({
                      ...current,
                      port: event.target.value,
                    }))
                  }
                />
              </div>
              <div className="space-y-2">
                <Label>Health path</Label>
                <Input
                  value={form.healthPath}
                  placeholder="/health"
                  onChange={(event) =>
                    setForm((current) => ({
                      ...current,
                      healthPath: event.target.value,
                    }))
                  }
                />
              </div>
            </div>
            <div className="space-y-2">
              <Label>API Key</Label>
              <Input
                value={form.apiKey}
                type="password"
                placeholder="Optional proxy API key"
                onChange={(event) =>
                  setForm((current) => ({
                    ...current,
                    apiKey: event.target.value,
                  }))
                }
              />
            </div>
            <div className="space-y-2">
              <Label>Startup timeout ms</Label>
              <Input
                value={form.startupTimeoutMs}
                placeholder="30000"
                inputMode="numeric"
                onChange={(event) =>
                  setForm((current) => ({
                    ...current,
                    startupTimeoutMs: event.target.value,
                  }))
                }
              />
            </div>
            <div className="space-y-2">
              <Label>Environment variables JSON</Label>
              <Textarea
                value={form.envJson}
                className="min-h-32 font-mono text-xs"
                onChange={(event) =>
                  setForm((current) => ({
                    ...current,
                    envJson: event.target.value,
                  }))
                }
              />
            </div>
            <div className="space-y-2">
              <div className="flex items-center justify-between gap-3">
                <Label>.env file</Label>
                {envFilePath && (
                  <span className="truncate font-mono text-xs text-muted-foreground">
                    {envFilePath}
                  </span>
                )}
              </div>
              <div className="flex flex-wrap gap-2">
                <Button
                  variant="outline"
                  type="button"
                  disabled={envBusy}
                  onClick={() => void loadEnvFile()}
                >
                  <FolderOpen className="h-4 w-4" />
                  Load .env
                </Button>
                <Button
                  variant="outline"
                  type="button"
                  disabled={envBusy}
                  onClick={applyFieldsToEnv}
                >
                  <RefreshCw className="h-4 w-4" />
                  Apply fields
                </Button>
                <Button
                  variant="outline"
                  type="button"
                  disabled={envBusy}
                  onClick={() => void saveEnvFile()}
                >
                  <Save className="h-4 w-4" />
                  Save .env
                </Button>
              </div>
              <Textarea
                value={envFileContent}
                placeholder="Load or create C:\\path\\to\\proxy\\.env"
                className="min-h-44 font-mono text-xs"
                onChange={(event) => setEnvFileContent(event.target.value)}
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDialogOpen(false)}>
              Cancel
            </Button>
            <Button onClick={() => void save()}>Save</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <ConfirmDialog
        isOpen={Boolean(deleteTarget)}
        title="Remove proxy"
        message={
          deleteTarget
            ? `Remove ${deleteTarget.name}? Running processes will be stopped first.`
            : ""
        }
        variant="destructive"
        onConfirm={() => void confirmDelete()}
        onCancel={() => setDeleteTarget(null)}
      />
    </div>
  );
}
