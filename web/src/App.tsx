import { useEffect, useState } from "react";
import { api, AgentListItem, DashboardData } from "./api";

export default function App() {
  const [dashboard, setDashboard] = useState<DashboardData | null>(null);
  const [error, setError] = useState("");
  const [showCreate, setShowCreate] = useState(false);

  const load = () => api.getDashboard().then(setDashboard).catch(e => setError(e.message));

  useEffect(() => { load(); const t = setInterval(load, 5000); return () => clearInterval(t); }, []);

  const onCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    const form = e.target as HTMLFormElement;
    const slug = (form.elements.namedItem("slug") as HTMLInputElement).value;
    const name = (form.elements.namedItem("name") as HTMLInputElement).value;
    try {
      await api.createAgent({ slug, display_name: name });
      setShowCreate(false);
      load();
    } catch (e: any) { setError(e.message); }
  };

  const onDelete = async (slug: string) => {
    if (!confirm(`Delete agent "${slug}" and all its data?`)) return;
    try { await api.deleteAgent(slug); load(); } catch (e: any) { setError(e.message); }
  };

  const onToggle = async (a: AgentListItem) => {
    try {
      if (a.status === "running") await api.stopAgent(a.slug);
      else await api.startAgent(a.slug);
      load();
    } catch (e: any) { setError(e.message); }
  };

  return (
    <div className="container">
      <header className="header">
        <div>
          <h1>ownify-desk</h1>
          <span className="card-meta">Local agent management</span>
        </div>
        <nav>
          <a href="#agents" onClick={() => load()}>Refresh</a>
        </nav>
      </header>

      {error && <div className="error-message">{error} <button className="btn btn-secondary btn-sm" onClick={() => setError("")} style={{marginLeft:12}}>Dismiss</button></div>}

      {dashboard && (
        <div className="stats">
          <div className="stat"><div className="stat-value">{dashboard.total}</div><div className="stat-label">Total Agents</div></div>
          <div className="stat"><div className="stat-value" style={{color:"#3fb950"}}>{dashboard.running}</div><div className="stat-label">Running</div></div>
          <div className="stat"><div className="stat-value" style={{color:"#f85149"}}>{dashboard.stopped}</div><div className="stat-label">Stopped</div></div>
          <button className="btn btn-primary" style={{marginLeft:"auto",alignSelf:"center"}} onClick={() => setShowCreate(true)}>+ New Agent</button>
        </div>
      )}

      {showCreate && (
        <div className="card" style={{marginBottom:24}}>
          <div className="card-header">
            <span className="card-title">Create New Agent</span>
            <button className="btn btn-secondary btn-sm" onClick={() => setShowCreate(false)}>Cancel</button>
          </div>
          <form onSubmit={onCreate}>
            <div className="form-group">
              <label className="form-label">Slug</label>
              <input name="slug" className="form-input" placeholder="my-bot" required pattern="[a-z0-9-]+" maxLength={32} />
              <span className="card-meta">Lowercase letters, numbers, hyphens. Used in file paths and URLs.</span>
            </div>
            <div className="form-group">
              <label className="form-label">Display Name</label>
              <input name="name" className="form-input" placeholder="My Bot" required />
            </div>
            <button type="submit" className="btn btn-primary">Create Agent</button>
          </form>
        </div>
      )}

      {dashboard?.agents.length === 0 ? (
        <div className="empty-state">
          <h2>No agents yet</h2>
          <p>Create your first agent to get started</p>
          <button className="btn btn-primary" onClick={() => setShowCreate(true)}>+ New Agent</button>
        </div>
      ) : (
        <div className="grid">
          {dashboard?.agents.map(a => (
            <AgentCard key={a.slug} agent={a} onToggle={onToggle} onDelete={onDelete} />
          ))}
        </div>
      )}
    </div>
  );
}

function AgentCard({ agent, onToggle, onDelete }: { agent: AgentListItem; onToggle: (a: AgentListItem) => void; onDelete: (slug: string) => void }) {
  const statusClass = agent.status === "running" ? "badge-running" : agent.status === "starting" ? "badge-starting" : "badge-stopped";
  return (
    <div className="card">
      <div className="card-header">
        <span className="card-title">{agent.display_name}</span>
        <span className={`badge ${statusClass}`}>{agent.status}</span>
      </div>
      <div className="card-meta" style={{marginBottom:8}}>Slug: {agent.slug} · Port: {agent.port}</div>
      {agent.description && <p style={{fontSize:13,color:"#8b949e",marginBottom:12}}>{agent.description}</p>}
      <div style={{display:"flex",gap:8}}>
        <button className={`btn btn-sm ${agent.status === "running" ? "btn-danger" : "btn-primary"}`} onClick={() => onToggle(agent)}>
          {agent.status === "running" ? "Stop" : "Start"}
        </button>
        <button className="btn btn-secondary btn-sm" onClick={() => window.location.hash = `#/agents/${agent.slug}`}>Config & Logs</button>
        <button className="btn btn-secondary btn-sm" onClick={() => onDelete(agent.slug)} style={{marginLeft:"auto",color:"#f85149"}}>Delete</button>
      </div>
    </div>
  );
}
