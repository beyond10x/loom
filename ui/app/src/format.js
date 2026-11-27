export function number(value) {
  return value == null ? 'Unavailable' : new Intl.NumberFormat('en-US', {maximumFractionDigits:2}).format(value)
}
export function time(value) {
  if (!value) return 'Unknown'
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? 'Unknown' : date.toISOString().replace('T', ' ').slice(0,19) + ' UTC'
}
export function duration(ms) {
  if (!Number.isFinite(ms) || ms < 0) return 'Unavailable'
  return ms < 1000 ? number(ms) + ' ms' : number(ms / 1000) + ' s'
}

export function money(value, currency) {
  if (value == null || !currency) return 'Unavailable'
  return new Intl.NumberFormat('en-US', {style:'currency', currency, currencyDisplay:'code', maximumFractionDigits:6}).format(value)
}

export function itemLabel(id) {
  return id.length > 32 ? id.slice(0, 28) + '…' : id
}

export function agentKind(value) {
  return {agent_spec:'AgentSpec',session:'Session'}[value] || value || 'Unknown'
}
export function harnessName(value) {
  return {native:'Native',claude:'Claude',codex:'Codex'}[value] || value || 'Unknown'
}
