const API = "http://localhost:9090";

async function get<T>(path: string): Promise<T> {
  const res = await fetch(`${API}${path}`);
  const json = await res.json();
  if (!json.ok) throw new Error(json.error);
  return json.data as T;
}

async function post<T>(path: string, body?: any): Promise<T> {
  const res = await fetch(`${API}${path}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: body ? JSON.stringify(body) : undefined,
  });
  const json = await res.json();
  if (!json.ok) throw new Error(json.error);
  return json.data as T;
}

async function put<T>(path: string, body?: any): Promise<T> {
  const res = await fetch(`${API}${path}`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: body ? JSON.stringify(body) : undefined,
  });
  const json = await res.json();
  if (!json.ok) throw new Error(json.error);
  return json.data as T;
}

async function del(path: string): Promise<void> {
  const res = await fetch(`${API}${path}`, { method: "DELETE" });
  const json = await res.json();
  if (!json.ok) throw new Error(json.error);
}

export interface AgentListItem {
  slug: string;
  display_name: string;
  description: string;
  port: number;
  status: "running" | "stopped" | "crashed" | "starting";
  enabled: boolean;
  auto_start: boolean;
}

export interface AgentDetail {
  meta: {
    slug: string;
    display_name: string;
    description: string;
    port: number;
    auto_start: boolean;
    enabled: boolean;
    created_at: string;
    updated_at: string;
  };
  status: "running" | "stopped" | "crashed" | "starting";
  pid: number | null;
}

export interface DashboardData {
  agents: AgentListItem[];
  total: number;
  running: number;
  stopped: number;
}

export interface StatusData {
  version: string;
  binary: string | null;
  dashboard_url: string;
  agents: number;
  running: number;
}

export const api = {
  listAgents: () => get<AgentListItem[]>("/api/agents"),
  getAgent: (slug: string) => get<AgentDetail>(`/api/agents/${slug}`),
  createAgent: (data: { slug: string; display_name: string; description?: string; auto_start?: boolean; enabled?: boolean }) =>
    post<AgentDetail["meta"]>("/api/agents", data),
  updateAgent: (slug: string, data: any) => put<AgentDetail["meta"]>(`/api/agents/${slug}`, data),
  deleteAgent: (slug: string) => del(`/api/agents/${slug}`),
  startAgent: (slug: string) => post<any>(`/api/agents/${slug}/start`),
  stopAgent: (slug: string) => post<any>(`/api/agents/${slug}/stop`),
  restartAgent: (slug: string) => post<any>(`/api/agents/${slug}/restart`),
  getConfig: (slug: string) => get<{ slug: string; content: string }>(`/api/agents/${slug}/config`),
  saveConfig: (slug: string, content: string) => put<string>(`/api/agents/${slug}/config`, { content }),
  getLogs: (slug: string) => get<{ slug: string; lines: string }>(`/api/agents/${slug}/logs`),
  getDashboard: () => get<DashboardData>("/api/dashboard"),
  getStatus: () => get<StatusData>("/api/status"),
};
