# Fase 1 — Estabilización funcional del motor de audio/red

- **Fecha:** 2026-06-05
- **Estado:** Aprobado (pendiente de revisión final del usuario)
- **Rama:** `phase-1-stabilization`
- **Autor:** Josue O.A (con Claude Code)

---

## 1. Contexto

`tcp-streamer` es una app de escritorio (Tauri 2 + Vue 3 + Pinia + Tailwind 4; backend Rust con
`cpal` + `socket2` + `ringbuf`) que captura audio del sistema y lo transmite por TCP a Snapcast u
otros sistemas con esa arquitectura. Funciona como **cliente** (se conecta a un Snapserver) o
**servidor** (escucha conexiones; además ofrece un stream HTTP `.wav` para navegadores).

La app **funciona hoy**, pero una auditoría completa reveló features anunciadas que no hacen nada,
bugs de tiempo real y de red, y un motor monolítico intestable. El objetivo de largo plazo es que
sea la opción más profesional, enterprise, segura, eficiente y multiplataforma de su categoría. Ese
objetivo se descompone en fases:

- **Fase 1 (este spec):** estabilización funcional + descomposición del motor.
- **Fase 2:** hardening enterprise (TLS + auth, multi-cliente, IPv6/hostnames).
- **Fase 3:** rediseño UI/UX completo (arquitectura de información, sistema de diseño, a11y, i18n).

### Estado verificado del working tree (baseline)

Hay una migración JS→TS + tooling (eslint/prettier/lefthook/vitest/tsconfig) + split inicial del
Rust **sin commitear** (~3570 inserciones). Se validó y está **verde**:

| Check               | Resultado         |
| ------------------- | ----------------- |
| Vitest (frontend)   | 17/17 ✅          |
| vue-tsc (typecheck) | limpio ✅         |
| ESLint              | sin errores ✅    |
| `cargo test`        | compila, 10/10 ✅ |

### Hallazgos de la auditoría que esta fase resuelve

1. **DSCP/QoS roto:** el UI envía `voip|lowdelay|throughput|besteffort` (default `voip`) pero el
   backend solo reconoce `EF|CS5|LowDelay` → el TOS siempre queda en `0x00`. Nunca se aplica QoS.
2. **Violación de tiempo real:** el productor del ring buffer se envuelve en `Arc<Mutex>` y se hace
   `lock()` dentro del callback de CPAL; además se aloca un `Vec` por callback en I16/U16. Riesgo de
   inversión de prioridad y dropouts.
3. **Adaptive buffer cosmético:** el código declara "display-only — ring buffer size is fixed at
   creation"; emite eventos pero nunca redimensiona nada.
4. **Controles muertos:** `buffer_size` se descarta (`let _ = buffer_size;`) pero ocupa lugar
   prominente en el UI; `max_buffer_ms` se pasa por toda la cadena y no se usa.
5. **Métricas engañosas:** "jitter" y "latency" se miden como variación del tiempo de `write()` al
   socket, no como métricas de red reales.
6. **Doble reconexión** que compite (hilo de red con backoff + manager recreando el stream).
7. **Handshake HTTP bloqueante:** `set_read_timeout(1500ms)` congela el hilo de red hasta 1.5 s por
   conexión.
8. **DSCP solo en cliente**, nunca en server.
9. **Motor monolítico:** `start_audio_stream` (~775 líneas) hace todo; intestable.

---

## 2. Decisiones tomadas

- **Orden:** estabilizar primero (Fase 1), antes de UI o enterprise.
- **Controles que no funcionan:** hacerlos **funcionales de verdad** (no quitarlos ni dejarlos
  cosméticos).
- **Motor:** **descomponer** el god-function en módulos enfocados con tests, y construir las
  features reales sobre esa base (en vez de parches in-place o reescritura total).

---

## 3. Alcance

### 3.1 En la Fase 1

- Fase 0: commitear la migración TS verificada como baseline; quitar el binario `tcp-streamer`
  (~10 MB) del control de versiones y `.DS_Store`; añadir a `.gitignore`.
- Callback de audio **real-time safe** (sin `Arc<Mutex>`, sin allocaciones por callback).
- **Adaptive buffer real** con modelo target-occupancy.
- **Buffer Size efectivo** vía `cpal::BufferSize::Fixed` con validación/fallback.
- **Métricas honestas:** underrun/overrun reales (primario) + RTT best-effort vía `TCP_INFO`
  (secundario); nuevo score de calidad.
- **DSCP arreglado:** mapeo correcto, función pura compartida, aplicado en cliente **y** server.
- **Reconexión consolidada** en un solo mecanismo.
- **Handshake HTTP no bloqueante.**
- **Descomposición del motor** en módulos + tests unitarios (objetivo ~30+ tests Rust).
- **Retoques mínimos de UI:** corregir etiquetas/semántica y claves DSCP (sin rediseño).

### 3.2 Diferido explícitamente

- **Fase 2:** TLS/cifrado, autenticación/allowlist para server, multi-cliente (fan-out),
  IPv6/hostnames/DNS, `trait Transport` _implementado_ para nuevos transportes.
- **Fase 3:** rediseño UI/UX, reestructura de tabs/IA, theming claro/oscuro, densidad, i18n.

### 3.3 Limitaciones conocidas

- El RTT vía `TCP_INFO` es **best-effort por plataforma** (Linux `tcp_info`, macOS
  `tcp_connection_info`, Windows `SIO_TCP_INFO`). Donde no esté disponible, el UI muestra "n/a". La
  señal de calidad primaria (underrun/overrun) sí es exacta y multiplataforma.

---

## 4. Arquitectura objetivo (`src-tauri/src/audio/`)

`(pure)` = lógica testeable sin I/O.

```
audio/
├── mod.rs                  # wiring + re-exports
├── commands.rs             # comandos Tauri (finos)            [keep]
├── manager.rs              # AudioState, loop de comandos, ciclo de vida
├── constants.rs · error.rs · stats.rs · wav_helper.rs · chunked.rs   [keep]
├── engine/
│   ├── mod.rs              # orquesta un stream activo (ex start_audio_stream)
│   ├── device.rs   (pure)  # enumeración + selección de dispositivo + negociación de formato
│   ├── capture.rs          # build cpal stream + callback RT-safe + conversión sin alocar
│   └── buffer.rs   (pure)  # ring buffer + controlador adaptive (target-occupancy)
├── transport/
│   ├── mod.rs              # trait Transport (write/peek/shutdown) → prepara Fase 2
│   ├── tcp_client.rs       # connect + opts de socket (keepalive, nodelay, DSCP)
│   ├── tcp_server.rs       # listener + accept + handshake HTTP no bloqueante
│   └── dscp.rs     (pure)  # estrategia DSCP → TOS  ← arregla el bug, compartido
└── metrics.rs      (pure)  # contadores underrun/overrun, RTT (cfg por SO), score de calidad
```

El `trait Transport` se **define** en Fase 1 (y `tcp_client`/`tcp_server` lo implementan) pero no se
añaden nuevos transportes hasta Fase 2.

---

## 5. Diseño técnico por componente

### 5.1 Captura real-time safe (`engine/capture.rs`)

- **Causa raíz:** las 3 ramas de `match selected_format` capturan un productor cada una aunque solo
  una se ejecuta; por eso se usó `Arc<Mutex>` + clones.
- **Fix:** construir **solo** el stream del formato seleccionado, moviendo el productor SPSC directo
  al closure. Sin `Mutex` ni `lock()` en el hilo de audio.
- **Conversión sin alocar:** I16/U16→F32 deja de hacer `Vec::new()` por callback; usa un buffer
  scratch reusable asignado una vez al construir el stream. El callback solo convierte in-place y
  hace `push_slice`.
- **Overrun:** si `push_slice` empuja menos de lo recibido, se incrementa el contador de overrun
  (samples perdidos) en `metrics`.

### 5.2 Adaptive buffer real — target-occupancy (`engine/buffer.rs`)

- El ring buffer se asigna **una vez** a la capacidad de `max_buffer_ms` (nunca se realoca).
- Se mantiene un **target de latencia** dinámico en ms, acotado a `[min_buffer_ms, max_buffer_ms]`.
- El consumidor regula el drenado para mantener la ocupación cerca del target:
  - **Underrun reciente o buffer health bajo** → subir target en pasos (más colchón).
  - **Estable durante una ventana** → bajar target hacia el mínimo (menos latencia). Bajar implica
    drenar puntualmente un poco más rápido.
- El valor reportado en stats (`bufferMs`) es el **target actual**. `min`/`max`/adaptive pasan a ser
  reales y con efecto medible, sin gap de audio.
- **Controlador puro y testeable:** entrada = (eventos de underrun, ocupación, target actual,
  límites); salida = nuevo target. Sin I/O.

### 5.3 Buffer Size efectivo (`engine/capture.rs`)

- Se conecta a `cpal::BufferSize::Fixed(frames)`, **validado** contra `SupportedBufferSize::Range`
  del dispositivo (clamp a `[min, max]`; fallback a `Default` con log si no se soporta).
- Es la latencia/CPU de captura hardware — distinta del ring buffer (jitter de red) y del target
  adaptive. Las etiquetas del UI se corrigen para reflejar los tres conceptos (§5.9).

### 5.4 Métricas honestas (`metrics.rs`)

- **Primaria (exacta, multiplataforma):**
  - `underruns`: veces que el consumidor quedó hambriento (buffer < chunk tras prefill).
  - `overruns` / `dropped_samples`: samples perdidos cuando el productor no pudo empujar.
- **Secundaria (best-effort por SO):** RTT y varianza vía `getsockopt(TCP_INFO)`:
  - Linux: `libc::tcp_info` (`tcpi_rtt`, `tcpi_rttvar`, µs).
  - macOS: `tcp_connection_info` (`tcpi_srtt`, `tcpi_rttvar`).
  - Windows: `SIO_TCP_INFO` → `TCP_INFO_v0.RttUs` (vía `windows-sys`).
  - "n/a" si la plataforma/versión no lo expone.
- **Score de calidad** = función pura de (underruns, overruns, ocupación vs target, RTT). Reemplaza
  el score basado en tiempo de `write()`.
- **Eventos al frontend** se renombran: el `QualityEvent` pasa de `{score, jitter, avg_latency,
buffer_health, error_count}` a `{score, rtt_ms (Option), rtt_var_ms (Option), underruns, dropped,
buffer_health}`. Se actualizan los tipos en `src/types/events.ts` y el store.

### 5.5 DSCP arreglado (`transport/dscp.rs`)

- Función pura `dscp_to_tos(strategy: &str) -> u8`:
  - `voip`/`ef` → `0xB8`, `cs5` → `0xA0`, `lowdelay` → `0x10`, `throughput` → `0x08`,
    `besteffort` → `0x00`.
- El UI (`AdvancedTab.vue`) y el backend usan **las mismas claves**. Default del store pasa a una
  clave válida y con efecto.
- Se aplica en cliente **y** server. Test unitario por cada clave.

### 5.6 Reconexión consolidada (`manager.rs` + `engine`)

- Se elimina la reconexión redundante del manager. El hilo de red mantiene un único backoff con
  jitter; el manager solo arranca/para el stream. Se documenta el ciclo de vida resultante.

### 5.7 Handshake HTTP no bloqueante (`transport/tcp_server.rs`)

- Se quita `set_read_timeout(1500ms)`. Peek no bloqueante con ventana corta acotada (p. ej. hasta
  ~50 ms en poll no bloqueante); si no llega `GET ` se trata como TCP crudo. Elimina el stall.

### 5.8 Estrategia de tests

- **Rust unit (nuevos):**
  - `device`: selección dada una lista simulada de dispositivos + negociación de formato (prioridad
    stereo > mono, F32 > I16 > U16, rate soportado).
  - `dscp`: todas las claves → TOS esperado.
  - `encoder`: f32→PCM i16 (clamp a [-1,1], escala 32767, little-endian).
  - `buffer`: controlador adaptive (sube ante underrun, baja tras estabilidad, respeta límites).
  - `metrics`: scoring determinista para casos límite.
- **Frontend:** extender `settings.test.ts`/`stream.test.ts` para claves DSCP válidas y semántica de
  buffers/métricas nuevas.
- **Disciplina:** TDD en las funciones puras (test rojo → implementación → verde).
- **Meta:** de 10 a ~30+ tests Rust; mantener todo verde (`cargo test`, `vitest`, `typecheck`,
  `clippy`, `lint`).

### 5.9 Retoques mínimos de UI (sin rediseño — Fase 3)

- Alinear los `value` del dropdown DSCP con las claves del backend.
- Renombrar métricas en `StatsBar`/store: `jitter`→`RTT`, exponer `Underruns`/`Dropped`.
- Etiquetas que distingan **Capture Buffer** (hardware) vs **Ring/Jitter Buffer** vs **Latency
  Target** (adaptive).
- Corregir el default de puerto confuso (1704 es del snapclient): documentar 4953 para source TCP de
  Snapserver y/o quitar el placeholder engañoso.
- No se toca layout, tabs ni estética.

---

## 6. Fase 0 — baseline e higiene de repo

1. Confirmar verde (hecho) y commitear la migración TS + tooling + split Rust como baseline.
2. `git rm --cached tcp-streamer` (binario de 10 MB) y añadir a `.gitignore`; quitar `.DS_Store`.
3. Verificar que la app arranca (`npm run tauri dev`) tras el commit baseline.

---

## 7. Riesgos y mitigaciones

- **`TCP_INFO` por plataforma:** API distinta en cada SO y posible ausencia. Mitigación: feature
  best-effort con `Option`, "n/a" en UI, y la métrica primaria (underrun/overrun) no depende de él.
- **`BufferSize::Fixed` no soportado por algunos dispositivos:** validar contra el rango y fallback a
  `Default`.
- **Bajar el target adaptive sin glitch:** drenar de más puede causar un micro-salto; hacerlo en
  pasos pequeños y poco frecuentes, y nunca por debajo de `min`.
- **Regresiones por el refactor:** mitigado por la batería de tests nueva y por mantener el
  comportamiento observable (mismo contrato de comandos/eventos salvo los renombres documentados).

---

## 8. Criterios de aceptación

- [ ] Working tree baseline commiteado; binario/artefactos fuera del control de versiones.
- [ ] No hay `Mutex`/`lock()` ni allocaciones en el callback de audio.
- [ ] Seleccionar una estrategia DSCP produce el TOS correcto en el socket (cliente y server),
      verificado por test unitario.
- [ ] El adaptive buffer cambia el target real y se observa en logs/stats; `min`/`max` lo acotan.
- [ ] "Buffer Size" cambia el `BufferSize` de cpal (o hace fallback con log) — ya no se descarta.
- [ ] Las métricas muestran underruns/dropped reales y RTT (o "n/a"); no hay "jitter/latency"
      derivados del tiempo de `write()`.
- [ ] Un solo mecanismo de reconexión; sin condiciones de carrera en logs.
- [ ] El handshake HTTP ya no bloquea el hilo de red.
- [ ] El motor está descompuesto en los módulos de §4 y cada función pura tiene tests.
- [ ] `cargo test`, `cargo clippy`, `vitest`, `vue-tsc`, `eslint` en verde.
- [ ] La app arranca y transmite a Snapcast en modo cliente y server como antes (sin regresiones).

---

## 9. Fuera de alcance (recordatorio)

TLS, autenticación, multi-cliente, IPv6/hostnames y el rediseño UI/UX **no** se abordan aquí; tienen
sus propios specs en Fase 2 y Fase 3.
