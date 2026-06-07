# Fase 3 — Rediseño UI/UX (enterprise dark + sidebar dashboard + i18n)

- **Fecha:** 2026-06-06
- **Estado:** Aprobado (pendiente de revisión final del usuario)
- **Rama:** `phase-3-ui-redesign`
- **Autor:** Josue O.A (con Claude Code)

---

## 1. Contexto

Con Fases 1 y 2 en `main` (v2.2.0), el backend es sólido: motor descompuesto, modos Source/Sink,
transporte TCP + nativo UDP cifrado, perfiles de latencia, allowlist, mDNS. Pero el **UI sigue
siendo el original** — estética "gamer" (glassmorphism, glows neón, emojis como iconos), 5 tabs
genéricos sobre-fragmentados, y la Fase 2 amontonó muchos controles condicionales en la pestaña de
Conexión. Esta fase rehace el UI/UX para que sea **profesional/enterprise**, que era la motivación
original del usuario.

**Esto es presentacional:** no se agregan features de producto (ya están). Se rehace look,
arquitectura de información, accesibilidad e idioma. Los **stores Pinia y los comandos Tauri se
mantienen**; el rediseño consume el mismo estado.

## 2. Decisiones tomadas

- **Dirección:** enterprise limpio/minimal (estilo Linear/Vercel/SaaS), **sin glows ni emojis**.
- **IA:** **dashboard con sidebar**, ventana **resizable** y más grande (no la angosta de 550px).
- **Tema:** **solo oscuro, refinado** (slate/zinc neutro, plano, sin gradientes radiales).
- **i18n:** **inglés + español** (`vue-i18n`), selector + default del SO.
- **Primitivos accesibles:** adoptar **Reka UI** (sucesor de Radix Vue) para Select/Switch/
  Dialog/Tooltip/Tabs; el resto a mano con Tailwind.
- **Un solo plan grande** (no decomposición), con milestones internos.

## 3. Alcance

### 3.1 En la Fase 3
- Sistema de diseño dark enterprise (tokens, tipografía, espaciado, iconos SVG, primitivos).
- Shell nuevo: ventana resizable, sidebar de navegación, header persistente con estado +
  Start/Stop + tema/idioma; se elimina el hack de tamaño de `main.rs` y el ancho fijo.
- Rediseño de las secciones: **Dashboard**, **Conexión** (por rol Source/Sink), **Audio**, **Logs**,
  **Ajustes**.
- Reescritura de los componentes UI (primitivos + tabs→secciones) sobre el nuevo sistema.
- i18n en/es de todo el *chrome* de UI.
- Accesibilidad (contraste AA, foco visible, teclado).

### 3.2 Fuera de alcance
- Features nuevas de producto, cambios de backend (salvo `main.rs`/`tauri.conf.json` para la
  ventana), tema claro, traducción del **contenido dinámico de logs** que emite el backend (queda
  en inglés técnico).

### 3.3 Limitaciones conocidas
- El contenido de logs lo formatea el backend en Rust; traducirlo en runtime no vale la pena → el
  *chrome* se traduce, los mensajes de log no.
- `<select>` nativo no se puede estilar de forma consistente entre SO → se reemplaza por el Select de
  Reka UI (esto quita el hack actual de `select option { background }`).

---

## 4. Sistema de diseño (dark enterprise)

- **Paleta:** base neutra slate/zinc; fondo casi-negro **plano** (sin `radial-gradient` ni glows).
  Superficies por capas (`surface-0/1/2`) diferenciadas por luminosidad + **borde sutil**, no por
  blur. Un único **acento** sobrio (indigo/azul ~`#4f6bff`) para acción primaria y estado activo;
  semánticos success/warning/danger contenidos.
- **Tipografía:** Inter (UI) + JetBrains Mono (datos/logs). Escala definida (xs 12 / sm 13 / base 14
  / lg 16 / xl 20 / 2xl 24) con pesos 400/500/600. Sin gradientes en texto.
- **Espaciado/forma:** grilla 4/8px; radios moderados (6–10px); elevación por borde + sombra muy
  sutil. Densidad cómoda pero compacta (es una utility).
- **Iconos:** un set SVG consistente (estilo lucide) en `components/icons.ts`; **fuera emojis**.
- **Tokens:** redefinir `@theme` en `style.css` con la paleta dark neutra; eliminar
  `--color-accent-glow`, `.glow-btn`, los keyframes de glow/pulse y el `body` con gradientes.
- **Accesibilidad:** contraste AA en texto/controles; anillo de foco visible; targets táctiles
  ≥32px; navegación completa por teclado (Reka UI lo aporta en los primitivos).

### 4.1 Primitivos (`components/ui/`)
- **Hechos a mano (Tailwind):** `Button` (variantes primary/secondary/ghost/danger + tamaños),
  `Input`, `Textarea`, `Card`/`Panel` (título + contenido), `Badge`/`StatusDot`, `Field` (label +
  ayuda + error), `Stat` (label + valor mono), `CopyField`.
- **Sobre Reka UI:** `Select`, `Switch` (reemplaza CheckboxField donde sea toggle), `Tooltip`,
  `Dialog` (para "nuevo perfil", confirmaciones), `Tabs` (si hace falta dentro de una sección),
  `SegmentedControl` (para el rol Source/Sink).

---

## 5. Shell & arquitectura de información

- **Ventana:** `tauri.conf.json` pasa a default ~960×640, min ~760×560, `resizable: true`; se
  **elimina** el bloque de `set_size`/`center` por monitor en `main.rs` (el hack de altura). Tray y
  single-instance se mantienen.
- **Layout (3 zonas):**
  - **Sidebar** (izq, ~200px, colapsable a iconos en ventanas chicas): logo + nav de secciones +
    versión al pie.
  - **Header** (arriba): título de sección · **chip de estado** (Ready/Streaming/Listening/Playing)
    · **botón Start/Stop** primario · acciones (tema —aunque sea dark-only, dejamos el control de
    idioma— selector de idioma).
  - **Content:** la sección activa.
- **Secciones del sidebar:**
  1. **Dashboard** — estado grande, stat-cards (uptime, transferido, bitrate, RTT, underruns,
     calidad), resumen de la config activa, peek de logs recientes, y el control Start/Stop también
     accesible aquí.
  2. **Conexión** — `SegmentedControl` Source/Sink y, debajo, **solo lo que aplica** (§6.2).
  3. **Audio** — dispositivo(s) de entrada/salida, sample rate, formato, **perfil de latencia**,
     buffers (modo custom).
  4. **Logs** — lista virtualizada con filtro por nivel, búsqueda, copiar/limpiar, autoscroll.
  5. **Ajustes** — perfiles (guardar/cargar/crear/borrar vía `Dialog`), automatización
     (autostart/autostream/reconnect), **idioma**, info/acerca de.

---

## 6. Secciones — diseño

### 6.1 Dashboard
Estado prominente + `Start/Stop`. Stat-cards en grilla (se ocultan/segregan métricas que no aplican
al rol). Resumen de config (rol, modo, dispositivo, destino/origen, cifrado on/off, perfil de
latencia). Peek de los últimos ~5 logs con enlace a la sección Logs.

### 6.2 Conexión (por rol)
- **Source:** `Select` de modo (TCP Client · TCP Server · Native UDP). Campos condicionales:
  - *Client:* dirección destino (host:puerto, acepta hostname/IPv6).
  - *Server:* puerto de escucha + **Allowlist** (IP/CIDR) + card de **info de conexión** (URLs
    `tcp://`/`http://…/stream.wav`) cuando corre.
  - *Native UDP:* puerto + **PSK** (`Input` password, vacío = sin cifrar) + toggle "anunciar por
    mDNS".
- **Sink:** **Output device** (`Select`), **Source address** + botón **Scan** que abre la lista
  mDNS (`list_sources`) para elegir, y **PSK**.
- Solo se muestran los campos del rol/modo activo; el resto se oculta (no se deshabilita en gris).

### 6.3 Audio
Dispositivo (entrada para source, ya integrado con loopback en Windows; salida para sink), sample
rate, formato (server), **perfil de latencia** (Ultra-low/Balanced/Robust/Custom) con los campos de
buffer manuales visibles solo en Custom, y aviso cuando Ultra-low + loopback.

### 6.4 Logs
Lista monoespaciada con colores por nivel (contenidos, sin neón), filtro por nivel (`Select`),
**búsqueda**, **copiar** y **limpiar**, autoscroll con "pausar al hacer scroll arriba".

### 6.5 Ajustes
Perfiles con un `Dialog` para crear (reemplaza el input inline + emojis), automatización con
`Switch`es, selector de **idioma**, y un bloque "Acerca de" (versión, links) sobrio (el CTA de
PayPal se mantiene pero discreto).

---

## 7. i18n

- `vue-i18n` (composition API). Catálogos `src/i18n/{en,es}.json` con todas las claves del chrome.
- Todos los textos de UI vía `t('clave')`; nada hardcodeado en componentes.
- Selector de idioma en Ajustes; default = idioma del SO (fallback `en`). Persistido en el store.
- Los **mensajes de log** del backend (emitidos en Rust) **no** se traducen.

## 8. Técnica

- **Deps nuevas:** `reka-ui` (primitivos accesibles), `vue-i18n`. Tailwind 4 ya está.
- **Stores Pinia:** se mantienen `settings`/`stream`; se agrega estado de `locale` (y `theme` aunque
  hoy sea solo dark, para no recablear si se suma claro luego). Los comandos Tauri no cambian.
- **Estructura de componentes (objetivo):**
  ```
  components/
  ├── AppShell.vue (sidebar + header + content)
  ├── Sidebar.vue · Header.vue
  ├── sections/ (Dashboard, Connection, Audio, Logs, Settings)
  ├── ui/ (Button, Input, Select, Switch, Field, Card, Stat, Badge, Dialog, Tooltip, SegmentedControl, CopyField, Textarea)
  └── icons.ts
  ```
  Se eliminan/absorben `tabs/*`, `TabNav`, `StreamButton`, `StatsBar`, `AppHeader`, `AppFooter`,
  `CheckboxField`/`SelectField`/`InputField` (reemplazados por los nuevos primitivos).

## 9. Estrategia de tests
- **Vitest (stores):** mantener verdes los tests de `settings`/`stream`; agregar tests del estado
  `locale`/`theme` y de que `t()` resuelve claves base.
- **Componentes:** smoke tests ligeros de los primitivos clave (Button, Select, Switch) con
  `@vue/test-utils`.
- **A11y/visual:** verificación manual (contraste, foco, teclado) — sin tooling automático nuevo.
- Mantener verde todo el gate (pnpm test/typecheck/lint/format + cargo si toca `main.rs`/conf).

## 10. Riesgos y mitigaciones
- **Reescritura amplia de componentes** → riesgo de regresión funcional. Mitigación: los stores
  (lógica) no cambian; los componentes solo cambian presentación + bindings; gate verde + smoke
  manual del flujo Start/Stop por cada rol.
- **Reka UI / vue-i18n** son deps nuevas → confirmar versiones e integración con Vue 3.5 + Vite 7 al
  iniciar el plan.
- **Ventana resizable** → asegurar que el layout no rompe en min-size; sidebar colapsable.

## 11. Criterios de aceptación
- [ ] Sin glassmorphism/glows/emojis; paleta dark neutra; tokens nuevos en `style.css`.
- [ ] Shell con sidebar + header persistente; ventana resizable; sin el hack de `main.rs`.
- [ ] Conexión muestra solo los controles del rol/modo activo (Source/Sink).
- [ ] Start/Stop y estado visibles siempre (header); funciona en los tres modos source + sink.
- [ ] i18n en/es con selector; cambiar idioma actualiza todo el chrome; sin strings hardcodeados.
- [ ] Primitivos accesibles (foco visible, teclado, contraste AA).
- [ ] Gate verde (pnpm test/typecheck/lint/format; cargo si se tocó backend); stores tests intactos.
- [ ] La app sigue haciendo todo lo de Fases 1-2 sin regresiones (smoke por rol).

## 12. Fuera de alcance (recordatorio)
Features nuevas, tema claro, traducción de logs dinámicos, cambios de backend más allá de la ventana.
La Fase 3 cierra el ciclo del rediseño; el resto del proyecto ya está en `main`.
