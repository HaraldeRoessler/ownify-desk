import { useEffect, useState, useCallback } from "react";
import { api, AgentDetail } from "../api";

// ---------- Tab definitions ----------
type TabId = "overview" | "config" | "sessions" | "skills" | "audit" | "metrics" | "usage" | "a2a" | "history" | "memory";

interface Tab { id: TabId; label: string; icon: string; description: string; }

const TABS: Tab[] = [
  { id: "overview",  label: "Overview",  icon: "📊", description: "Health, status, general info" },
  { id: "config",    label: "Config",    icon: "⚙️", description: "YAML configuration editor" },
  { id: "sessions",  label: "Sessions",  icon: "💬", description: "Active chat sessions" },
  { id: "history",   label: "History",   icon: "📜", description: "Chat history timeline" },
  { id: "skills",    label: "Skills",    icon: "🧠", description: "Enabled tool skills" },
  { id: "audit",     label: "Audit",     icon: "🔍", description: "Access audit log" },
  { id: "metrics",   label: "Metrics",   icon: "📈", description: "Performance metrics" },
  { id: "usage",     label: "Usage",     icon: "💰", description: "LLM usage & costs" },
  { id: "a2a",       label: "A2A",       icon: "🔄", description: "Agent-to-agent config" },
  { id: "memory",    label: "Memory",    icon: "🧩", description: "Memory observability" },
];

interface Props {
  slug: string;
  onBack: () => void;
  onRefresh: () => void;
}

export function AgentDetailPage({ slug, onBack, onRefresh }: Props) {
  const [detail, setDetail] = useState<AgentDetail | null>(null);
  const [error, setError] = useState("");
  const [activeTab, setActiveTab] = useState<TabId>("overview");
  const [agentRunning, setAgentRunning] = useState(false);

  const loadDetail = useCallback(async () => {
    try {
      const d = await api.getAgent(slug);
      setDetail(d);
      setAgentRunning(d.status === "running");
    } catch (e: any) {
      setError(e.message);
    }
  }, [slug]);

  useEffect(() => { loadDetail(); const t = setInterval(loadDetail, 5000); return () => clearInterval(t); }, [loadDetail]);

  const handleStartStop = async () => {
    try {
      if (agentRunning) await api.stopAgent(slug);
      else await api.startAgent(slug);
      setTimeout(loadDetail, 1000);
      onRefresh();
    } catch (e: any) { setError(e.message); }
  };

  const handleRestart = async () => {
    try {
      await api.restartAgent(slug);
      setTimeout(loadDetail, 2000);
      onRefresh();
    } catch (e: any) { setError(e.message); }
  };

  return (
    <div className="container">
      {/* Header */}
      <header className="header">
        <div style={{display:"flex",alignItems:"center",gap:12}}>
          <button className="btn btn-secondary btn-sm" onClick={onBack}>← Back</button>
          <div>
            <h1 style={{margin:0}}>{detail?.meta.display_name || slug}</h1>
            <span className="card-meta">Slug: {slug} · Port: {detail?.meta.port || "?"}</span>
          </div>
          <span className={`badge ${agentRunning ? "badge-running" : "badge-stopped"}`} style={{marginLeft:8}}>
            {agentRunning ? "Running" : "Stopped"}
          </span>
        </div>
        <nav style={{display:"flex",gap:8}}>
          <button className={`btn btn-sm ${agentRunning ? "btn-danger" : "btn-primary"}`} onClick={handleStartStop}>
            {agentRunning ? "Stop" : "Start"}
          </button>
          {agentRunning && <button className="btn btn-secondary btn-sm" onClick={handleRestart}>Restart</button>}
        </nav>
      </header>

      {error && <div className="error-message">{error} <button className="btn btn-secondary btn-sm" onClick={() => setError("")} style={{marginLeft:12}}>Dismiss</button></div>}

      {/* Tab bar */}
      <div className="tab-bar">
        {TABS.map(tab => (
          <button
            key={tab.id}
            className={`tab-btn ${activeTab === tab.id ? "tab-active" : ""}`}
            onClick={() => setActiveTab(tab.id)}
            title={tab.description}
          >
            {tab.icon} {tab.label}
          </button>
        ))}
      </div>

      {/* Tab content */}
      <div className="tab-content">
        {activeTab === "overview"  && <OverviewTab slug={slug} agentRunning={agentRunning} />}
        {activeTab === "config"    && <ConfigTab slug={slug} agentRunning={agentRunning} />}
        {activeTab === "sessions"  && <SessionsTab slug={slug} agentRunning={agentRunning} />}
        {activeTab === "history"   && <HistoryTab slug={slug} agentRunning={agentRunning} />}
        {activeTab === "skills"    && <SkillsTab slug={slug} agentRunning={agentRunning} />}
        {activeTab === "audit"     && <AuditTab slug={slug} agentRunning={agentRunning} />}
        {activeTab === "metrics"   && <MetricsTab slug={slug} agentRunning={agentRunning} />}
        {activeTab === "usage"     && <UsageTab slug={slug} agentRunning={agentRunning} />}
        {activeTab === "a2a"       && <A2ATab slug={slug} agentRunning={agentRunning} />}
        {activeTab === "memory"    && <MemoryTab slug={slug} agentRunning={agentRunning} />}
      </div>
    </div>
  );
}

// ---------- Reusable helpers ----------

function StatusBadge({ ok, label }: { ok: boolean; label?: string }) {
  return <span className={`badge ${ok ? "badge-running" : "badge-stopped"}`}>{label || (ok ? "OK" : "Offline")}</span>;
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="card" style={{marginBottom:16}}>
      <div className="card-header"><span className="card-title">{title}</span></div>
      <div style={{padding:"8px 0"}}>{children}</div>
    </div>
  );
}

function JsonBlock({ data }: { data: any }) {
  return <pre className="code-block">{JSON.stringify(data, null, 2)}</pre>;
}

async function withProxy<T>(slug: string, fn: () => Promise<T | null>, setter: (v: T | null) => void, setError?: (e: string) => void) {
  try {
    const result = await fn();
    setter(result);
  } catch (e: any) {
    setter(null);
    if (setError) setError(e.message);
  }
}

// ---------- Tab components ----------

function OverviewTab({ slug, agentRunning }: { slug: string; agentRunning: boolean }) {
  const [health, setHealth] = useState<any>(null);
  const [auth, setAuth] = useState<any>(null);
  const [selfCheck, setSelfCheck] = useState<any>(null);
  const [configContent, setConfigContent] = useState<string>("");
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    setLoading(true);
    Promise.all([
      api.proxy.health(slug),
      api.proxy.authStatus(slug),
      api.proxy.configSelfCheck(slug),
      api.getConfig(slug).then(c => c.content).catch(() => ""),
    ]).then(([h, a, s, c]) => {
      setHealth(h); setAuth(a); setSelfCheck(s); setConfigContent(c || "");
    }).catch(() => {}).finally(() => setLoading(false));
  }, [slug, agentRunning]);

  if (!agentRunning) return <div className="card"><p style={{color:"#8b949e"}}>Agent is stopped. Start it to see health and status.</p></div>;

  // Parse key values from config
  const llmProvider = configContent.match(/llm_provider:\s*"?(\w+)"?/)?.[1] || "?";
  const model = configContent.match(/model:\s*"?([\w.-]+)"?/)?.[1] || "?";

  return (
    <>
      <Section title="Health Status">
        {loading ? <p style={{color:"#8b949e"}}>Loading...</p> : (
          <div className="overview-grid">
            {health && <div className="overview-item"><span className="overview-label">API Status</span><StatusBadge ok={true} label="Healthy" /></div>}
            {auth && <div className="overview-item"><span className="overview-label">Auth</span><span>{auth.auth_enabled ? "Password protected" : "Open (no password)"}</span></div>}
            {selfCheck && <div className="overview-item"><span className="overview-label">Self Check</span><StatusBadge ok={selfCheck.ok !== false} /></div>}
          </div>
        )}
      </Section>

      <Section title="Agent Configuration">
        <div className="overview-grid">
          <div className="overview-item"><span className="overview-label">LLM Provider</span><span>{llmProvider}</span></div>
          <div className="overview-item"><span className="overview-label">Model</span><span>{model}</span></div>
          <div className="overview-item"><span className="overview-label">Direct URL</span><a href={`http://127.0.0.1:${health?.port || "?"}`} target="_blank" rel="noreferrer">http://127.0.0.1:{health?.port || "?"}</a></div>
        </div>
      </Section>

      {health && <Section title="Health Response"><JsonBlock data={health} /></Section>}
      {auth && <Section title="Auth Status"><JsonBlock data={auth} /></Section>}
    </>
  );
}

function ConfigTab({ slug, agentRunning }: { slug: string; agentRunning: boolean }) {
  const [config, setConfig] = useState<string>("");
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState("");

  useEffect(() => {
    api.getConfig(slug).then(c => setConfig(c.content)).catch(() => setConfig("# Could not load config"));
  }, [slug]);

  const handleSave = async () => {
    setSaving(true); setMessage("");
    try {
      await api.saveConfig(slug, config);
      setDirty(false);
      setMessage("✓ Config saved");
    } catch (e: any) {
      setMessage(`✗ ${e.message}`);
    }
    setSaving(false);
  };

  return (
    <Section title="microclaw.config.yaml">
      <p className="card-meta" style={{marginBottom:8}}>
        Direct configuration file for this agent. Changes require a restart to take effect.
        {!agentRunning && <span style={{color:"#d29922",marginLeft:8}}>Agent is stopped — start it after saving.</span>}
      </p>
      {message && <p style={{color: message.startsWith("✓") ? "#3fb950" : "#f85149", marginBottom:8, fontSize:13}}>{message}</p>}
      <textarea
        className="code-editor"
        value={config}
        onChange={e => { setConfig(e.target.value); setDirty(true); }}
        rows={30}
        spellCheck={false}
      />
      <div style={{marginTop:8,display:"flex",gap:8,alignItems:"center"}}>
        <button className="btn btn-primary btn-sm" onClick={handleSave} disabled={saving || !dirty}>
          {saving ? "Saving..." : "Save Config"}
        </button>
        {dirty && <span className="card-meta" style={{color:"#d29922"}}>Unsaved changes</span>}
      </div>
    </Section>
  );
}

function SessionsTab({ slug, agentRunning }: { slug: string; agentRunning: boolean }) {
  const [sessions, setSessions] = useState<any[] | null>(null);

  useEffect(() => {
    if (agentRunning) api.proxy.sessions(slug).then(setSessions);
  }, [slug, agentRunning]);

  if (!agentRunning) return <OfflineNotice />;
  if (sessions === null) return <LoadingNotice />;

  return (
    <Section title="Chat Sessions">
      {sessions && sessions.length > 0 ? (
        <table className="data-table">
          <thead><tr><th>ID</th><th>Channel</th><th>Type</th><th>Messages</th><th>Created</th></tr></thead>
          <tbody>
            {sessions.map((s: any, i: number) => (
              <tr key={i}>
                <td style={{fontFamily:"monospace",fontSize:12}}>{(s.id || s.session_id || "").slice(0,16)}…</td>
                <td>{s.channel || s.chat_type || "web"}</td>
                <td>{s.conversation_kind || s.kind || "—"}</td>
                <td>{s.message_count || s.messages?.length || "?"}</td>
                <td>{s.created_at ? new Date(s.created_at).toLocaleDateString() : "—"}</td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : <p style={{color:"#8b949e"}}>No active sessions.</p>}
    </Section>
  );
}

function HistoryTab({ slug, agentRunning }: { slug: string; agentRunning: boolean }) {
  const [history, setHistory] = useState<any[] | null>(null);

  useEffect(() => {
    if (agentRunning) api.proxy.history(slug).then(setHistory);
  }, [slug, agentRunning]);

  if (!agentRunning) return <OfflineNotice />;
  if (history === null) return <LoadingNotice />;

  return (
    <Section title="Chat History">
      {history && history.length > 0 ? (
        <table className="data-table">
          <thead><tr><th>Time</th><th>Channel</th><th>Preview</th><th>Messages</th></tr></thead>
          <tbody>
            {history.map((h: any, i: number) => (
              <tr key={i}>
                <td style={{whiteSpace:"nowrap"}}>{h.timestamp ? new Date(h.timestamp).toLocaleString() : "—"}</td>
                <td>{h.channel || h.chat_type || "web"}</td>
                <td style={{maxWidth:400,overflow:"hidden",textOverflow:"ellipsis",whiteSpace:"nowrap"}}>{(h.preview || h.summary || "").slice(0,80)}</td>
                <td>{h.message_count || "?"}</td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : <p style={{color:"#8b949e"}}>No chat history yet.</p>}
    </Section>
  );
}

function SkillsTab({ slug, agentRunning }: { slug: string; agentRunning: boolean }) {
  const [skills, setSkills] = useState<any[] | null>(null);

  useEffect(() => {
    if (agentRunning) api.proxy.skills(slug).then(setSkills);
  }, [slug, agentRunning]);

  if (!agentRunning) return <OfflineNotice />;
  if (skills === null) return <LoadingNotice />;

  const skillList = Array.isArray(skills) ? skills : (skills as any)?.skills || [];

  return (
    <Section title="Skills & Tools">
      {skillList.length > 0 ? (
        <table className="data-table">
          <thead><tr><th>Name</th><th>Enabled</th><th>Description</th></tr></thead>
          <tbody>
            {skillList.map((s: any, i: number) => (
              <tr key={i}>
                <td style={{fontFamily:"monospace"}}>{s.name || s.skill_name || "?"}</td>
                <td><StatusBadge ok={s.enabled !== false} label={s.enabled !== false ? "Enabled" : "Disabled"} /></td>
                <td style={{fontSize:12,color:"#8b949e"}}>{s.description || s.summary || "—"}</td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : <p style={{color:"#8b949e"}}>No skills configured. Configure them in the agent's config file.</p>}
    </Section>
  );
}

function AuditTab({ slug, agentRunning }: { slug: string; agentRunning: boolean }) {
  const [audit, setAudit] = useState<any[] | null>(null);

  useEffect(() => {
    if (agentRunning) api.proxy.audit(slug).then(setAudit);
  }, [slug, agentRunning]);

  if (!agentRunning) return <OfflineNotice />;
  if (audit === null) return <LoadingNotice />;

  const auditList = Array.isArray(audit) ? audit : (audit as any)?.entries || [];

  return (
    <Section title="Access Audit Log">
      {auditList.length > 0 ? (
        <table className="data-table">
          <thead><tr><th>Time</th><th>Action</th><th>Caller</th><th>Target</th></tr></thead>
          <tbody>
            {auditList.map((a: any, i: number) => (
              <tr key={i}>
                <td style={{whiteSpace:"nowrap",fontSize:12}}>{a.timestamp ? new Date(a.timestamp).toLocaleString() : "—"}</td>
                <td><code>{a.action || a.operation || "?"}</code></td>
                <td style={{fontSize:12}}>{a.caller || a.caller_id || "—"}</td>
                <td style={{fontSize:12}}>{a.target || a.path || "—"}</td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : <p style={{color:"#8b949e"}}>No audit entries yet.</p>}
    </Section>
  );
}

function MetricsTab({ slug, agentRunning }: { slug: string; agentRunning: boolean }) {
  const [metrics, setMetrics] = useState<any | null>(null);
  const [history, setHistory] = useState<any[] | null>(null);

  useEffect(() => {
    if (agentRunning) {
      api.proxy.metrics(slug).then(setMetrics);
      api.proxy.metricsHistory(slug).then(h => setHistory(Array.isArray(h) ? h : [])).catch(() => {});
    }
  }, [slug, agentRunning]);

  if (!agentRunning) return <OfflineNotice />;
  if (metrics === null) return <LoadingNotice />;

  return (
    <>
      <Section title="Performance Metrics">
        <div className="overview-grid">
          <div className="overview-item"><span className="overview-label">HTTP Requests</span><span>{metrics.http_requests ?? "?"}</span></div>
          <div className="overview-item"><span className="overview-label">OK</span><span style={{color:"#3fb950"}}>{metrics.request_ok ?? "?"}</span></div>
          <div className="overview-item"><span className="overview-label">Errors</span><span style={{color: (metrics.request_error || 0) > 0 ? "#f85149" : undefined}}>{metrics.request_error ?? "?"}</span></div>
          <div className="overview-item"><span className="overview-label">LLM Completions</span><span>{metrics.llm_completions ?? "?"}</span></div>
          <div className="overview-item"><span className="overview-label">Input Tokens</span><span>{metrics.llm_input_tokens ?? "?"}</span></div>
          <div className="overview-item"><span className="overview-label">Output Tokens</span><span>{metrics.llm_output_tokens ?? "?"}</span></div>
          <div className="overview-item"><span className="overview-label">Tool Executions</span><span>{metrics.tool_executions ?? "?"}</span></div>
        </div>
      </Section>
      {metrics && <Section title="Full Metrics"><JsonBlock data={metrics} /></Section>}
      {history && history.length > 0 && (
        <Section title="Metric History (recent points)">
          <table className="data-table">
            <thead><tr><th>Time</th><th>LLM Calls</th><th>Tokens In</th><th>Tokens Out</th></tr></thead>
            <tbody>
              {history.slice(-20).map((h: any, i: number) => (
                <tr key={i}>
                  <td style={{fontSize:12,whiteSpace:"nowrap"}}>{h.timestamp ? new Date(h.timestamp).toLocaleString() : "—"}</td>
                  <td>{h.llm_completions ?? "?"}</td>
                  <td>{h.llm_input_tokens ?? "?"}</td>
                  <td>{h.llm_output_tokens ?? "?"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </Section>
      )}
    </>
  );
}

function UsageTab({ slug, agentRunning }: { slug: string; agentRunning: boolean }) {
  const [usage, setUsage] = useState<any | null>(null);

  useEffect(() => {
    if (agentRunning) api.proxy.usage(slug).then(setUsage);
  }, [slug, agentRunning]);

  if (!agentRunning) return <OfflineNotice />;
  if (usage === null) return <LoadingNotice />;

  return (
    <Section title="LLM Usage Report">
      {usage && (
        <div className="overview-grid">
          <div className="overview-item"><span className="overview-label">Total Completions</span><span>{usage.total_completions ?? usage.completions ?? "?"}</span></div>
          <div className="overview-item"><span className="overview-label">Total Input Tokens</span><span>{usage.total_input_tokens ?? usage.input_tokens ?? "?"}</span></div>
          <div className="overview-item"><span className="overview-label">Total Output Tokens</span><span>{usage.total_output_tokens ?? usage.output_tokens ?? "?"}</span></div>
          <div className="overview-item"><span className="overview-label">Total Cost</span><span>{usage.total_cost ? `$${usage.total_cost}` : "—"}</span></div>
        </div>
      )}
      {usage && <JsonBlock data={usage} />}
    </Section>
  );
}

function A2ATab({ slug, agentRunning }: { slug: string; agentRunning: boolean }) {
  const [agentCard, setAgentCard] = useState<any | null>(null);

  useEffect(() => {
    if (agentRunning) api.proxy.a2aAgentCard(slug).then(setAgentCard);
  }, [slug, agentRunning]);

  if (!agentRunning) return <OfflineNotice />;
  if (agentCard === null) return <LoadingNotice />;

  return (
    <>
      <Section title="Agent-to-Agent (A2A)">
        {agentCard ? (
          <>
            <div className="overview-grid">
              <div className="overview-item"><span className="overview-label">Name</span><span>{agentCard.name || agentCard.identifier || slug}</span></div>
              <div className="overview-item"><span className="overview-label">Version</span><span>{agentCard.version || "—"}</span></div>
              <div className="overview-item"><span className="overview-label">Description</span><span>{agentCard.description || "—"}</span></div>
            </div>
            <JsonBlock data={agentCard} />
          </>
        ) : <p style={{color:"#8b949e"}}>No A2A agent card available.</p>}
      </Section>
      <Section title="A2A Endpoints">
        <table className="data-table">
          <thead><tr><th>Endpoint</th><th>URL</th></tr></thead>
          <tbody>
            <tr><td>Agent Card</td><td><code>/api/a2a/agent-card</code></td></tr>
            <tr><td>Message</td><td><code>/api/a2a/message</code></td></tr>
          </tbody>
        </table>
      </Section>
    </>
  );
}

function MemoryTab({ slug, agentRunning }: { slug: string; agentRunning: boolean }) {
  const [memory, setMemory] = useState<any | null>(null);

  useEffect(() => {
    if (agentRunning) api.proxy.memoryObservability(slug).then(setMemory);
  }, [slug, agentRunning]);

  if (!agentRunning) return <OfflineNotice />;
  if (memory === null) return <LoadingNotice />;

  return (
    <Section title="Memory Observability">
      {memory ? (
        <>
          <div className="overview-grid">
            <div className="overview-item"><span className="overview-label">Total Memories</span><span>{memory.total_memories ?? memory.count ?? "?"}</span></div>
            <div className="overview-item"><span className="overview-label">Categories</span><span>{memory.categories?.length || memory.category_count || "?"}</span></div>
          </div>
          <JsonBlock data={memory} />
        </>
      ) : <p style={{color:"#8b949e"}}>No memory data available.</p>}
    </Section>
  );
}

function OfflineNotice() {
  return <div className="card"><p style={{color:"#8b949e"}}>Agent is stopped. Start it to view this data.</p></div>;
}

function LoadingNotice() {
  return <div className="card"><p style={{color:"#8b949e"}}>Loading...</p></div>;
}
