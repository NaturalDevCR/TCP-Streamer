# Fase 2A — Conectividad genérica + latencia configurable

- **Fecha:** 2026-06-06
- **Estado:** Aprobado (pendiente de revisión final del usuario)
- **Rama:** `phase-2a-connectivity`
- **Autor:** Josue O.A (con Claude Code)

---

## 1. Contexto

Con la Fase 1 cerrada (motor descompuesto, `Connection` trait cableado, métricas honestas, v2.1.0
en `main`), la Fase 2 es el "hardening" enterprise. Tras acotar el caso de uso real, queda así:

- **Despliegue:** LAN de confianza. Sin cifrado obligatorio.
- **Sin multi-room:** el server atiende **un cliente a la vez** (como hoy). No hay fan-out.
- **Objetivo transversal:** latencia **configurable** por el usuario (baja latencia ⇄ robustez).
- **Genérico:** la herramienta debe servir a **cualquier receptor de audio** (Sonium y otros). El
  nombre de un sistema concreto **no debe aparecer** en código, UI ni docs.

La Fase 2 se descompone en dos subproyectos, cada uno con su spec → plan:

- **2A (este spec):** conectividad genérica — IPv6/hostnames, allowlist con CIDR, perfil de latencia
  configurable, y genericización del nombre.
- **2B (futuro):** modo nativo de baja latencia tcp-streamer ↔ tcp-streamer (transporte optimizado,
  cifrado/auth opcional por PSK). Inspirado en Dante, sin pretender igualarlo.

## 2. Decisiones tomadas

- LAN de confianza → **sin TLS ni token-auth** en 2A (YAGNI; además no aplican a receptores de TCP
  plano). Cifrado opcional queda para el modo nativo de 2B.
- **Single-client** server: no se implementa fan-out.
- **Perfil de latencia** explícito que reemplaza los "Network Presets" y **elimina los mínimos de
  buffer hardcodeados**.
- **Allowlist con soporte de CIDR/rangos** (no solo IPs sueltas), vía crate `ipnet`.
- Término genérico: **"receptor de audio"** (audio receiver).

## 3. Alcance

### 3.1 En 2A
- Perfil Baja-latencia ⇄ Robustez (Ultra-baja / Balanceado / Robusto / Personalizado) que controla
  ring buffer, rango adaptive, prefill y chunk; quita los pisos fijos de `constants.rs`.
- IPv6 + hostnames/DNS en cliente (`ToSocketAddrs`) y bind dual-stack `[::]` en server.
- Allowlist opcional con IPs y CIDR (IPv4/IPv6); vacío = aceptar a todos.
- Genericización de nombres en backend (logs/comentarios), frontend (labels, config generada) y
  docs (README/CONTRIBUTING).

### 3.2 Diferido a 2B
- Modo nativo tcp-streamer ↔ tcp-streamer, transporte de baja latencia (UDP vs TCP afinado), cifrado
  y autenticación por PSK, y descubrimiento de peers.

### 3.3 Limitaciones conocidas
- **Loopback (WASAPI) + Ultra-baja latencia** puede causar cortes; cada perfil impone un piso más
  alto para loopback y el UI advierte al combinar ultra-baja con un dispositivo loopback.
- El bind dual-stack `[::]` depende del SO; si falla (p. ej. IPv6 deshabilitado) se cae a
  `0.0.0.0` (solo IPv4) con log.

---

## 4. Arquitectura objetivo

`(pure)` = lógica testeable sin I/O.

```
audio/
├── engine/
│   ├── latency.rs   (pure)  # perfil de latencia → parámetros de buffer
│   └── mod.rs               # usa latency::params; bind dual-stack; chequeo de allowlist en accept
├── transport/
│   ├── resolve.rs   (pure*) # parseo/resolución de destino (hostname/IPv4/IPv6) → SocketAddrs
│   ├── allowlist.rs (pure)  # parseo de reglas IP/CIDR + matching de peer
│   ├── tcp_client.rs        # connect usa resolve (hostnames/IPv6)
│   └── tcp_server.rs        # (helpers existentes)
└── constants.rs             # se quitan los mínimos de buffer fijos (pasan a latency.rs)
```

`resolve.rs` es `pure*`: la resolución DNS real hace I/O, pero el parseo de IPs literales y la
validación se testean de forma determinista.

Dependencia nueva: `ipnet = "2"` (matching CIDR IPv4/IPv6).

---

## 5. Diseño técnico por componente

### 5.1 Perfil de latencia (`engine/latency.rs`)

Función pura que mapea (perfil, is_loopback) → parámetros de buffer:

```
struct LatencyParams { ring_ms, adaptive_min_ms, adaptive_max_ms, chunk_size, prefill_ms }
fn params(profile: &str, is_loopback: bool) -> LatencyParams
```

Valores propuestos (no-loopback / loopback):

| Perfil | ring_ms | adaptive_min | adaptive_max | chunk | prefill_ms |
|---|---|---|---|---|---|
| ultra-low | 100 / 600 | 50 / 400 | 300 / 1500 | 256 | = ring |
| balanced  | 500 / 1500 | 300 / 1000 | 2000 / 4000 | 512 | = ring |
| robust    | 3000 / 5000 | 2000 / 4000 | 8000 / 12000 | 1024 | = ring |
| custom    | usa los campos manuales del usuario (como hoy) | | | | |

- `engine::run` usa estos parámetros **en vez de** `DEFAULT_MIN_BUFFER_MS` / `LOOPBACK_MIN_BUFFER_MS`
  / `ADAPTIVE_MIN_*`, que se eliminan de `constants.rs`.
- El frontend manda `latency_profile` (string); en "custom" manda los campos manuales y el backend
  los respeta.
- Tests: cada perfil devuelve los valores esperados; loopback ≥ no-loopback; custom no se toca.

### 5.2 IPv6 + hostnames (`transport/resolve.rs`, `tcp_client.rs`, `engine/mod.rs`)

- `resolve::resolve_target(host: &str, port: u16) -> io::Result<Vec<SocketAddr>>` usando
  `(host, port).to_socket_addrs()`. Devuelve candidatos (puede haber v4 y v6).
- `tcp_client::connect` itera los candidatos y se conecta al primero que funcione; el `Socket` se
  crea con el `Domain` correcto según la familia del candidato (v4/v6). Mantiene keepalive, nodelay,
  DSCP.
- Server: bind dual-stack. Crear `socket2::Socket` IPv6, `set_only_v6(false)`, bind a `[::]:port`,
  listen; si falla, fallback a `TcpListener::bind("0.0.0.0:port")`. Log claro de cuál quedó.
- Tests (resolve): `127.0.0.1`/`::1`/`localhost` resuelven a ≥1 addr; string inválido → Err. (El
  test de `localhost` depende de DNS local; se marca `#[ignore]` si el CI no resuelve.)

### 5.3 Allowlist con CIDR (`transport/allowlist.rs`)

```
fn parse_rules(s: &str) -> Vec<ipnet::IpNet>   // separa por coma/espacio; ignora inválidos
fn is_allowed(peer: IpAddr, rules: &[IpNet], allow_if_empty: bool) -> bool
```

- Reglas aceptan IP sola (`10.0.0.5`, `fe80::1`) — se normaliza a `/32`/`/128` — y CIDR
  (`192.168.1.0/24`, `fe80::/10`).
- `is_allowed`: si `rules` vacío → `allow_if_empty` (true). Si no, true sólo si algún `IpNet`
  contiene `peer`.
- En `engine::run` server, tras `accept()` se obtiene `addr.ip()` y se llama `is_allowed`; si no,
  se cierra la conexión con log `"Rejected connection from <ip> (not in allowlist)"` y se sigue
  escuchando.
- El frontend manda `allowlist` (string) en los settings.
- Tests: IP exacta dentro/fuera; CIDR v4 contiene/no; CIDR v6; lista vacía respeta `allow_if_empty`;
  reglas inválidas se ignoran.

### 5.4 Genericización del nombre

- **Backend:** logs y comentarios que dicen "Snapserver/Snapcast" → "audio receiver"/genérico
  (p. ej. el comentario de resampling en `engine/mod.rs`, mensajes de connect/accept).
- **Frontend:**
  - `stream.ts`: la cadena de config generada deja de ser específica de un sistema; se exponen
    `tcpUrl` (`tcp://host:port`) y `httpUrl` (`http://host:port/stream.wav`) genéricas, más un hint
    neutro ("Usá esta dirección como fuente TCP en tu sistema de audio").
  - `ConnectionTab.vue` / `AudioTab.vue`: labels "Snapcast Server Address" / "Snapserver Config",
    "Connects to a Snapserver", etc. → genéricos ("Receptor de audio", "Dirección del servidor",
    "Usar como fuente TCP en tu sistema de audio").
- **Docs:** README/CONTRIBUTING: reemplazar el sistema nombrado por "audio receivers such as Sonium"
  y ejemplos genéricos. Mantener exactitud técnica (puerto de ejemplo, formato PCM/WAV).
- No se rediseña el layout (eso es Fase 3); sólo cambian textos.

---

## 6. Estrategia de tests

- **Rust unit (nuevos):** `latency::params` (todos los perfiles + loopback), `allowlist`
  (exacto/CIDR/v6/vacío/inválido), `resolve` (literales v4/v6 + inválido). Meta: +~15 tests.
- **Frontend:** extender store tests para el nuevo `latency_profile` y `allowlist` (se mandan en el
  comando; el contrato de claves queda fijado).
- **Disciplina:** TDD en las funciones puras.
- Mantener verde: `cargo test`/`clippy`, `pnpm test`/`typecheck`/`lint`/`format:check`.

## 7. Riesgos y mitigaciones

- **Quitar los pisos de buffer** podría dejar a un usuario con cortes si elige Ultra-baja en mala
  red. Mitigación: Balanceado es el default; el perfil es explícito y la UI advierte (loopback).
- **Dual-stack** difiere por SO. Mitigación: fallback a IPv4 con log; no se asume `[::]` siempre.
- **`to_socket_addrs` bloquea** durante la resolución DNS. Ocurre en el hilo de red antes de
  conectar (no en el callback de audio); el timeout de connect lo acota. Aceptable.
- **Genericización amplia** (muchos strings). Mitigación: cambio mecánico, cubierto por que la app
  siga compilando/tests verdes; revisión final con grep del término viejo.

## 8. Criterios de aceptación

- [ ] Elegir un perfil de latencia cambia de verdad los buffers (Ultra-baja << Robusto), verificable
      en logs; ya no hay pisos de 5 s/8 s forzados.
- [ ] El cliente conecta por **hostname** y por **IPv6**, no solo IP v4 literal.
- [ ] El server acepta conexiones v4 y v6 (dual-stack) o cae a v4 con log.
- [ ] Con allowlist configurada (IP o CIDR), una IP fuera de rango es rechazada; vacía acepta a
      todos. Verificado por tests unitarios.
- [ ] No queda el nombre del sistema concreto en código, UI ni docs (`grep -ri` del término viejo
      sin resultados funcionales).
- [ ] `cargo test`/`clippy`, `pnpm test`/`typecheck`/`lint`/`format:check` en verde; +~15 tests Rust.
- [ ] La app sigue transmitiendo a un receptor (cliente y server) sin regresiones.

## 9. Fuera de alcance (recordatorio)

Multi-cliente/fan-out, TLS, token-auth y el **modo nativo de baja latencia** (2B) no se abordan
aquí. El rediseño UI/UX es Fase 3.
