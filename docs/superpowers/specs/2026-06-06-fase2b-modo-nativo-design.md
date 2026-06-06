# Fase 2B — Modo nativo: Sink/reproducción + transporte UDP de baja latencia

- **Fecha:** 2026-06-06
- **Estado:** Aprobado (pendiente de revisión final del usuario)
- **Rama:** `phase-2b-native`
- **Autor:** Josue O.A (con Claude Code)

---

## 1. Contexto

Hasta ahora tcp-streamer es siempre un **emisor** (captura + manda); el receptor final es un
sistema externo. La Fase 2B convierte a tcp-streamer en un sistema **bidireccional**: puede ser
**Source** (lo de hoy) o **Sink** (recibe y **reproduce** en una salida local), y añade un
**transporte nativo UDP de baja latencia** entre dos instancias tcp-streamer, con cifrado opcional
y descubrimiento.

Inspiración: **Dante** — audio sobre UDP, baja latencia. No replicamos PTP (sincronía de reloj); en
su lugar usamos un **jitter buffer**. "Estilo Dante, sin la sincronía de reloj de Dante."

**Depende de:** Fase 2A (reusa `engine/latency.rs` para la profundidad del jitter buffer, la
terminología genérica, y `transport/`).

## 2. Decisiones tomadas

- **B reproduce localmente** (sink con salida cpal), no relé. tcp-streamer pasa de "solo fuente" a
  "fuente o destino".
- **2B completo en un spec** (sink + transporte UDP + cifrado + descubrimiento).
- **Transporte: solo UDP** (seq + timestamp, jitter buffer en B). Sin PTP. Sin TCP para el media.
- **Cifrado opcional: AEAD por paquete** ChaCha20-Poly1305 con clave derivada de PSK
  (`HKDF-SHA256`). Off por defecto (LAN de confianza).
- **Descubrimiento: manual + mDNS opcional.**
- **Single-sink** por source (coherente con "no multi-room").

## 3. Alcance

### 3.1 En 2B

- Rol **Sink**: enumerar dispositivos de salida, recibir stream, jitter buffer, reproducir vía cpal
  output (callback RT-safe).
- Source gana sub-modo **Native UDP** (emite por UDP, anuncia por mDNS, acepta un subscribe).
- Transporte UDP propio: framing de paquete, de-jitter por seq, concealment por silencio.
- Cifrado AEAD/PSK opcional por paquete + anti-replay.
- Handshake SUBSCRIBE/STREAM_INFO (negociación de formato) + heartbeat.
- Descubrimiento mDNS (anuncio + búsqueda) con fallback a entrada manual.
- Manejo simple de drift de reloj (sin PTP).
- Controles mínimos de UI (rol, source, output device, PSK). **No** rediseño (Fase 3).

### 3.2 Fuera de alcance

- Multi-sink (un source → varios sinks), PTP/clock-sync, resampling propio, relé (B no reenvía a
  otro sistema; reproduce), y TCP para el media nativo.

### 3.3 Limitaciones conocidas

- **Drift de reloj (sin PTP):** A captura con su reloj, B reproduce con el suyo (~ppm de
  diferencia); el jitter buffer deriva. Mitigación: al detectar drift sostenido, **descartar o
  duplicar un mini-chunk** ocasionalmente (un micro-glitch cada varios minutos), sin resampling.
- **Sample rate de salida:** si el device de B no soporta el rate de A, se confía en el resampler
  del SO (como en captura).
- **mDNS:** depende de multicast; puede no cruzar subredes/VLANs → fallback manual.
- **Cifrado:** PSK simétrica, sin forward secrecy (aceptable para LAN-opcional).

---

## 4. Arquitectura objetivo

### 4.1 Modelo de roles

- **Source** (existente): captura + emite. Sub-modos: TCP-cliente, TCP-server (2A), **Native UDP**.
- **Sink** (nuevo): recibe + reproduce. Sub-modo: **Native UDP** (subscribe a un source).

### 4.2 Módulos (`(pure)` = testeable sin I/O)

```
audio/
├── engine/
│   ├── playback.rs        # cpal OUTPUT stream RT-safe (sin lock/alloc en callback)
│   ├── decoder.rs (pure)  # PCM i16 LE → f32 (inverso de encoder.rs)
│   └── sink.rs            # orquestador sink: subscribe → recv → jitter → playback
├── transport/
│   ├── udp/
│   │   ├── packet.rs   (pure)  # header (magic,ver,flags,seq,ts) + handshake encode/decode
│   │   ├── crypto.rs   (pure)  # PSK→clave (HKDF), seal/open AEAD, ventana anti-replay
│   │   ├── jitter.rs   (pure)  # buffer de reordenamiento por seq (insertar/tirar en orden)
│   │   ├── source.rs           # emisor UDP (A): bind, acepta subscribe, stream + heartbeat
│   │   └── sink.rs             # receptor UDP (B): subscribe, recv, dejitter
│   └── discovery.rs            # mDNS: anuncia (source) / busca (sink)
```

Comandos nuevos: `get_output_devices`, `start_sink` / `stop_sink`, `list_sources` (mDNS); el source
gana la opción "native UDP" en su arranque.

Dependencias nuevas: `chacha20poly1305`, `hkdf`, `sha2` (RustCrypto); `mdns-sd` (mDNS Rust puro).

---

## 5. Diseño técnico por componente

### 5.1 Modo Sink + reproducción (`engine/playback.rs`, `engine/sink.rs`)

- `get_output_devices` enumera dispositivos de salida cpal; el usuario elige.
- El core de playback consume de un **`FrameSource` abstracto** (trait: `recv_frames(&mut [f32]) ->
usize`); el receptor UDP es la impl → testeable y deja la puerta abierta a otros transportes.
- `playback.rs` construye el cpal **output** stream con un callback **RT-safe** (igual disciplina
  que la captura: mueve el consumidor, sin mutex/alloc) que tira del jitter buffer; si está vacío →
  escribe silencio (concealment) y cuenta un underrun.
- La profundidad objetivo del jitter buffer usa `engine::latency::params` (reusa el perfil de 2A).

### 5.2 Decoder (`engine/decoder.rs`, pure)

- `decode_pcm_i16_le_to_f32(bytes: &[u8], out: &mut Vec<f32>)`: inverso exacto del `encoder.rs` de
  Fase 1. Tests: roundtrip con el encoder; bytes impares (incompletos) se truncan sin panic.

### 5.3 Transporte UDP — paquetes y jitter (`transport/udp/{packet,jitter}.rs`, pure)

- **Paquete de audio:** `magic(2) | version(1) | flags(1) | seq(u64) | timestamp_us(u64) |
payload`. `flags` indica encrypted. `packet::encode/decode` son puros y testeables (roundtrip;
  magic/versión inválidos → None).
- **Jitter/reorder (`jitter.rs`):** estructura que acepta `(seq, frame)` fuera de orden y entrega en
  orden al ritmo del playback; un `seq` que no llegó antes del deadline → hueco (concealment).
  Función pura: insertar seqs desordenados, `pop_next()` devuelve en orden o `None` si hay hueco
  vencido. Tests: orden, desorden, duplicados, hueco.
- **Sin estado de conexión:** B inicia (SUBSCRIBE); A emite a la dirección de origen de B
  (NAT-friendly). `source.rs`/`sink.rs` hacen el I/O de socket.

### 5.4 Cifrado AEAD/PSK (`transport/udp/crypto.rs`, pure)

- PSK = passphrase del usuario. **Clave de sesión** = `HKDF-SHA256(PSK, salt_A ‖ salt_B)` → 32 B.
- Por paquete: `seal(key, nonce, aad=header, plaintext=payload)` con ChaCha20-Poly1305; **nonce** =
  `salt(4) ‖ seq(8)` (96 bits); el header va como **AAD** (autenticado, no cifrado).
- B: `open(...)` o rechaza (forjado/manipulado). **Anti-replay:** ventana deslizante sobre `seq`
  (rechaza viejos/duplicados). Si cifrado off → `flags` sin el bit, payload en claro.
- Todo (`derive_key`, `seal`, `open`, `replay_window`) es **puro y testeable**: seal→open roundtrip;
  tampering del ciphertext o del AAD → open falla; replay rechaza seq repetido/viejo; KDF
  determinista para mismas entradas.

### 5.5 Handshake SUBSCRIBE / STREAM_INFO (`transport/udp/packet.rs` + source/sink)

- **SUBSCRIBE** (B→A): versión, `salt_B`, y (si cifrado) un tag de auth derivado de la PSK sobre el
  mensaje (prueba conocimiento de PSK).
- **STREAM_INFO** (A→B): `sample_rate`, `channels`, `bits=16`, `salt_A`, `flags(encrypted)`. Tras
  esto A empieza a emitir audio. Negocia el formato que B usará para abrir su output stream.
- **Heartbeat:** B reenvía SUBSCRIBE cada N s (mantiene vivo + refresca NAT); A deja de emitir si no
  hay heartbeat en M s. Encode/decode del handshake: **puro y testeable** (roundtrip; auth tag
  inválido → rechazo).

### 5.6 Descubrimiento mDNS (`transport/discovery.rs`)

- Crate `mdns-sd`. Servicio `_tcp-streamer._udp.local.`; el source anuncia nombre + puerto + TXT
  (versión, formato, `encrypted`). El sink **busca** y arma la lista para `list_sources` (nombre,
  dirección, encrypted). Entrada manual siempre disponible; mDNS best-effort (si falla, no rompe).

### 5.7 Drift de reloj (sin PTP)

- El jitter buffer monitorea su nivel de ocupación promedio. Si crece sostenidamente (B más lento
  que A) → **descarta** un mini-chunk; si decrece (B más rápido) → **duplica/inserta** un mini-chunk
  de silencio. Frecuencia baja (umbral con histéresis) para que sea un micro-glitch ocasional, no
  audible en lo normal. Lógica de decisión **pura y testeable**.

---

## 6. Estrategia de tests

- **Pure Rust:** `decoder` (roundtrip con encoder, bytes impares), `packet` (audio + handshake
  roundtrip, basura → None), `crypto` (seal/open, tampering, replay, KDF determinista), `jitter`
  (orden/desorden/dup/hueco), drift (sube→descarta, baja→duplica, histéresis).
- **Integración loopback:** `udp::source` ↔ `udp::sink` en localhost — SUBSCRIBE, recibir N
  paquetes, decodificar; variante cifrada con PSK.
- **Frontend:** store/settings del rol Sink (output device, source, PSK).
- Meta: +~25 tests. Mantener verde todo el gate (pnpm + cargo + clippy).

## 7. Riesgos y mitigaciones

- **Drift sin PTP:** §5.7 (descarte/duplicación). Limitación honesta documentada.
- **Output device no soporta el rate:** confiar en resampler del SO; loguear el config real.
- **mDNS/multicast:** fallback manual siempre.
- **Callback RT-safe del output:** misma disciplina que captura (sin mutex/alloc) — cubierto por
  revisión + ausencia de allocs en el hot path.
- **UDP bloqueado por firewall:** el enlace nativo no funciona; se documenta (en LAN poco común).

## 8. Criterios de aceptación

- [ ] tcp-streamer puede correr como **Sink**: elegir output device, suscribirse a un source nativo
      y reproducir audio.
- [ ] Dos instancias (A source nativo, B sink) reproducen audio end-to-end por UDP en LAN.
- [ ] Con cifrado activado (misma PSK en A y B), el audio fluye; con PSK distinta, B rechaza los
      paquetes (sin reproducir). Verificado por tests de `crypto`.
- [ ] Pérdida/reordenamiento de paquetes → micro-glitch, nunca un stall (jitter buffer).
- [ ] mDNS: el sink lista sources disponibles; la entrada manual también funciona.
- [ ] Callback de output sin mutex/alloc.
- [ ] Gate verde (pnpm test/typecheck/lint/format, cargo test/clippy); +~25 tests.

## 9. Fuera de alcance (recordatorio)

Multi-sink, PTP, resampling propio, relé, TCP para media nativo, y el rediseño UI/UX (Fase 3).
