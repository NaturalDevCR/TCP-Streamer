/** Log event from the Rust backend */
export interface LogEvent {
  timestamp: string;
  level: string;
  message: string;
}

/** Streaming quality metrics */
export interface QualityEvent {
  score: number;
  jitter: number;
  avg_latency: number;
  buffer_health: number;
  error_count: number;
}

/** Periodic streaming statistics */
export interface StatsEvent {
  uptime_seconds: number;
  bytes_sent: number;
  bitrate_kbps: number;
}

/** Buffer resize notification */
export interface BufferResizeEvent {
  new_size_ms: number;
  reason: string;
}

/** Toast notification */
export interface Toast {
  id: number;
  type: "success" | "error" | "warning" | "info";
  message: string;
}

/** Log entry for display */
export interface LogEntry {
  timestamp: string;
  level: string;
  message: string;
  visible: boolean;
}

/** Audio device configuration */
export interface StreamConfig {
  device_name: string;
  ip: string;
  port: number;
  sample_rate: number;
  buffer_size: number;
  ring_buffer_duration_ms: number;
  auto_reconnect: boolean;
  high_priority: boolean;
  dscp_strategy: string;
  format: string;
  chunk_size: number;
  is_loopback: boolean;
  is_server: boolean;
  enable_adaptive_buffer: boolean;
  min_buffer_ms: number;
  max_buffer_ms: number;
}

/** Network preset configuration */
export interface NetworkPreset {
  name: string;
  chunk_size: number;
  ring_buffer_duration_ms: number;
  min_buffer_ms: number;
  max_buffer_ms: number;
}

/** Profile management */
export interface Profile {
  name: string;
  settings: Record<string, unknown>;
}

/** Settings store state shape */
export interface SettingsState {
  devices: string[];
  selectedDevice: string;
  osType: string;
  localIp: string;
  isLoopback: boolean;
  isServer: boolean;
  targetIp: string;
  targetPort: number;
  sampleRate: number;
  bufferSize: number;
  ringBufferDurationMs: number;
  audioFormat: string;
  adaptiveBuffer: boolean;
  adaptiveBufferMinMs: number;
  adaptiveBufferMaxMs: number;
  autoStart: boolean;
  autoStreamOnLoad: boolean;
  autoReconnect: boolean;
  highPriority: boolean;
  dscpStrategy: string;
  chunkSize: number;
  currentProfile: string;
  profiles: Profile[];
  newProfileName: string;
  networkPresets: NetworkPreset[];
}

/** Streaming state shape */
export interface StreamState {
  isStreaming: boolean;
  uptime: string;
  uptimeSeconds: number;
  bytesSent: number;
  bitrate: string;
  score: number;
  jitter: number;
  latency: number;
  bufferHealth: number;
  errorCount: number;
  bufferSizeMs: number;
  logs: LogEntry[];
  toasts: Toast[];
  serverUrlTcp: string;
  serverUrlHttp: string;
  snapcastConfig: string;
  logLevelFilter: string;
}
