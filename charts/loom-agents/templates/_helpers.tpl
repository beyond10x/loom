{{- define "loom-agents.name" -}}
{{ .Release.Name }}-operator
{{- end }}

{{- define "loom-agents.selector" -}}
app.kubernetes.io/name: loom-operator
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{- define "loom-agents.labels" -}}
{{ include "loom-agents.selector" . }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
helm.sh/chart: {{ printf "%s-%s" .Chart.Name .Chart.Version }}
{{- end }}
