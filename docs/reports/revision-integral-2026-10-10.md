# Revisión integral de TCP Streamer

**Fecha:** 10 de octubre de 2026, Costa Rica.  
**Versión revisada:** 2.4.0.  
**Alcance:** frontend Vue/Pinia, componentes y estilos, IPC Tauri, motor Rust, captura/reproducción, TCP/HTTP, UDP, descubrimiento, cifrado, persistencia, pruebas, documentación y CI/release.

## Evaluación general

El proyecto tiene una base útil: separación entre captura y red mediante ring buffers, negociación de dispositivos, conversión de formato, perfiles de latencia, cifrado AEAD y una suite con pruebas del procesamiento de audio. Las oportunidades más importantes están en la coordinación entre capas: la UI representa estados que el backend no ha confirmado, varias opciones no producen el comportamiento anunciado y la recuperación de sesiones UDP queda incompleta.

Priorizaría **fiabilidad del ciclo de vida, seguridad del protocolo y coherencia de configuración** antes de ampliar funcionalidades. En el frontend, la mayor mejora sería que el usuario pueda distinguir con claridad “configurado”, “iniciando”, “esperando receptor”, “transmitiendo”, “reconectando” y “falló”. El aspecto visual por sí solo no resuelve esa incertidumbre.

Este reporte conserva el diagnóstico inicial, previo a las correcciones. La implementación posterior y su validación se documentan en [Implementación de la revisión integral](./implementacion-2026-10-10.md). Las pruebas temporales usadas para este diagnóstico inicial fueron retiradas del proyecto.

## Método y límites

- Revisión estática de las rutas de producción del frontend y backend, configuración de compilación y workflows.
- Ejecución de pruebas y controles locales en macOS.
- Inspección del frontend servido por Vite en el navegador: Dashboard, Conexión, Ajustes, controles, árbol de accesibilidad y consola. Se intentó navegar a Audio después de Conexión; el error de un selector vacío impidió esa transición. Audio y Logs también se revisaron por código.
- Cuatro pruebas temporales de Pinia reprodujeron comportamientos actuales: contaminación entre perfiles, exigencia de IP en UDP, estado activo tras un error del backend y métricas de la sesión anterior.
- Un ejecutable temporal, usando el módulo real de jitter, reprodujo el recorrido de un millón de secuencias faltantes al incorporarse tarde y el rechazo de una secuencia reiniciada.

La vista de navegador no contiene el runtime Tauri. Los errores de `invoke`/`listen` ausentes en esa vista son una limitación del entorno de inspección, no evidencia de que fallen en la aplicación nativa. El rechazo de `SelectItem` con valor vacío sí corresponde al componente real utilizado en producción.

No se midió audio físico, calidad perceptual, latencia extremo a extremo ni consumo sostenido. No se ejecutó la aplicación empaquetada ni pruebas en Windows/Linux. Los problemas de seguridad se identificaron por el flujo de código; no se realizaron ataques contra otros equipos. Donde el impacto depende del sistema operativo o del hardware se indica expresamente.

### Validación ejecutada

| Comprobación                                                                              | Resultado                                                |
| ----------------------------------------------------------------------------------------- | -------------------------------------------------------- |
| `pnpm test`                                                                               | 39 pruebas aprobadas en 8 archivos                       |
| `pnpm typecheck`                                                                          | Aprobado                                                 |
| `pnpm lint`                                                                               | Aprobado                                                 |
| `pnpm build`                                                                              | Aprobado; JS principal 306,19 kB, 100,35 kB gzip         |
| `cargo test --locked`                                                                     | 143 aprobadas, 1 omitida por requerir dispositivo real   |
| `cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` | Aprobado                                                 |
| `cargo fmt --check --manifest-path src-tauri/Cargo.toml`                                  | Aprobado                                                 |
| `pnpm format:check`                                                                       | Falló por 13 documentos locales de `.superpowers/sdd/`   |
| Diagnósticos temporales frontend                                                          | 4 comportamientos defectuosos reproducidos               |
| Diagnóstico temporal jitter                                                               | Incorporación tardía y secuencia reiniciada reproducidas |

El build emitió dos advertencias de anotaciones `PURE` en una dependencia VueUse; no impidieron compilar. Los 13 documentos que hacen fallar Prettier no están versionados según `git ls-files`; esto demuestra un fallo de la comprobación local, no necesariamente del checkout limpio de CI.

## Hallazgos prioritarios

**P1:** corregir antes de depender del proyecto para transmisiones importantes o presentar el cifrado como protección completa. **P2:** corregir en la siguiente iteración de fiabilidad/UX. **P3:** mejora menor. La prioridad expresa impacto; no todos los hallazgos tienen reproducción con hardware.

### H01 · P1 · La UI confirma un stream que todavía no arrancó

**Evidencia:** [commands.rs:76](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/commands.rs:76), [manager.rs:131](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/manager.rs:131), [stream.ts:163](/Users/jdavidoa91/Dev/tcp-streamer/src/stores/stream.ts:163).

`start_stream`/`start_sink` confirman únicamente que el comando fue enviado a la cola. El motor arranca después. El frontend cambia `isStreaming` a `true` al recibir esa confirmación y no consume un evento de estado real. Un dispositivo inexistente, un fallo de reproducción o una conexión fallida puede dejar el botón en “Detener” y el indicador verde. Un error de bind se registra desde el hilo de red, sin convertirse en fallo visible del arranque. Reproducido en Pinia con un evento de error posterior al inicio.

**Corrección:** definir un estado de sesión autoritativo en Rust, con `sessionId`, transiciones y códigos de error; emitir cambios y permitir consultar el estado actual. La UI debe esperar confirmación del motor y de la disponibilidad del transporte. Añadir estado pendiente y bloquear inicios duplicados.

**Validación:** dispositivo ausente, puerto ocupado, error de `play`, fallo TCP y Stop durante Start deben dejar la UI y el motor en el mismo estado. Eventos tardíos de una sesión anterior deben ignorarse.

### H02 · P1 · La fuente UDP exige una dirección TCP oculta

**Evidencia:** [stream.ts:127](/Users/jdavidoa91/Dev/tcp-streamer/src/stores/stream.ts:127), [ConnectionSection.vue:82](/Users/jdavidoa91/Dev/tcp-streamer/src/components/sections/ConnectionSection.vue:82).

En la configuración inicial, `mode` es `client` e `ip` está vacía. Al elegir Native UDP se oculta el destino TCP, pero `startStream` sigue exigiendo `ip` si `isServer` es falso. Una fuente UDP nueva no puede iniciar desde su propia pantalla. Además, conservar `mode=server` puede crear un listener TCP innecesario, porque el motor evalúa `is_server` antes de seleccionar la rama UDP.

**Corrección:** validar y construir la configuración por combinación de rol/transporte. Una fuente UDP requiere dispositivo y puerto; el destino corresponde al sink. Limitar el listener TCP, las URLs TCP/HTTP y los textos de estado a TCP.

**Validación:** iniciar UDP con IP vacía desde ambos valores previos de `mode`; comprobar que solo se abre UDP y que las URLs/textos corresponden al transporte.

### H03 · P1 · Los selectores vacíos pueden romper el renderizado

**Evidencia:** [ConnectionSection.vue:24](/Users/jdavidoa91/Dev/tcp-streamer/src/components/sections/ConnectionSection.vue:24), [ConnectionSection.vue:190](/Users/jdavidoa91/Dev/tcp-streamer/src/components/sections/ConnectionSection.vue:190), [AudioSection.vue:18](/Users/jdavidoa91/Dev/tcp-streamer/src/components/sections/AudioSection.vue:18), [SelectOption.vue:3](/Users/jdavidoa91/Dev/tcp-streamer/src/components/ui/SelectOption.vue:3).

Las opciones de carga, ausencia de dispositivos y “elegir fuente” usan `value=""`. El `SelectItem` instalado lanza una excepción para ese valor. Se observó el error en consola al abrir Conexión sin dispositivos; después, navegar a Audio produjo otro error de desmontaje. La opción vacía de descubrimiento también se renderiza cuando hay resultados, por lo que el riesgo no se limita a equipos sin audio.

**Corrección:** representar los estados vacíos con el `placeholder` del selector y texto de estado externo; no convertirlos en opciones seleccionables de valor vacío. Añadir una prueba de navegador con los componentes reales. Referencia del API: [Select de Reka UI](https://www.reka-ui.com/docs/components/select).

**Validación:** sin dispositivos, cargando dispositivos y con fuentes descubiertas, la pantalla debe renderizar y permitir cambiar de sección sin errores.

### H04 · P1 · Los perfiles mezclan destinos, secretos y dispositivos

**Evidencia:** [settings.ts:202](/Users/jdavidoa91/Dev/tcp-streamer/src/stores/settings.ts:202), [settings.ts:225](/Users/jdavidoa91/Dev/tcp-streamer/src/stores/settings.ts:225), [settings.ts:297](/Users/jdavidoa91/Dev/tcp-streamer/src/stores/settings.ts:297).

La carga usa comprobaciones de truthiness como `if (s.psk)`. Un perfil con IP, PSK o allowlist vacía conserva el valor anterior. Los campos ausentes tampoco recuperan valores predeterminados. `s.device` se guarda, pero no se restaura por perfil: la selección de entrada se recupera desde una clave global. Reproducido pasando de un perfil con valores a uno con cadenas vacías y otro dispositivo.

Crear un perfil copia la última configuración persistida, no el formulario actual. Cambiar/eliminar perfiles está permitido durante el streaming, de modo que la configuración visible puede diferir de la sesión que sigue activa.

**Corrección:** construir un snapshot completo desde defaults más datos validados, asignar todos los campos —incluidas cadenas vacías— y restaurar el dispositivo por perfil. Separar configuración editada, guardada y activa; mostrar cambios pendientes. La creación debe clonar el snapshot elegido explícitamente.

**Validación:** A→B→A debe restaurar todos los campos sin herencia accidental; probar perfiles antiguos, vacíos, incompletos y dispositivos desconectados.

### H05 · P1 · Un sink con PSK acepta paquetes de audio sin cifrar

**Evidencia:** [udp/sink.rs:136](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/transport/udp/sink.rs:136), [udp/sink.rs:166](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/transport/udp/sink.rs:166), [engine/sink.rs:47](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/engine/sink.rs:47).

La necesidad de descifrar se decide a partir del flag del paquete. Si el flag no está activado, el payload se acepta como PCM aunque exista una clave configurada. La protección depende de una elección del remitente, en vez de una política obligatoria de sesión. `STREAM_INFO.flags` tampoco se utiliza para rechazar una incompatibilidad de cifrado al iniciar.

El socket está conectado al origen, por lo que no basta con que cualquier máquina envíe un datagrama desde cualquier dirección. El riesgo corresponde a un origen malicioso/comprometido o a un actor capaz de suplantar/interceptar ese flujo. No se demostró una explotación de red.

**Corrección:** negociar y autenticar el modo de sesión; con cifrado requerido, rechazar cualquier paquete plano. Validar la correspondencia entre PSK, metadata y flags antes de reproducir. La autenticación debe producir plaintext verificado o un fallo, de acuerdo con [RFC 5116](https://www.rfc-editor.org/rfc/rfc5116).

**Validación:** PSK configurada más paquete plano debe rechazarse; claves distintas y metadata alterada deben mostrar un error de autenticación, sin anunciar reproducción exitosa.

### H06 · P1 · Un paquete inválido puede modificar la ventana anti-replay

**Evidencia:** [udp/sink.rs:140](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/transport/udp/sink.rs:140), [crypto.rs:85](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/transport/udp/crypto.rs:85).

`check_and_update` registra la secuencia antes de comprobar AEAD. Un paquete con secuencia futura y tag inválido puede avanzar la ventana y hacer que audio auténtico posterior se descarte como antiguo. Requiere que el paquete llegue como parte del flujo aceptado del origen; el código demuestra el orden defectuoso, no un ataque ejecutado.

**Corrección:** comprobar elegibilidad sin mutar, autenticar y después registrar la secuencia. Limitar también saltos de secuencia para evitar trabajo desproporcionado en jitter.

**Validación:** un paquete falsificado con secuencia alta no debe modificar el estado ni impedir aceptar el siguiente paquete auténtico.

### H07 · P1 · SUBSCRIBE puede sustituir al receptor sin autenticación

**Evidencia:** [udp/source.rs:57](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/transport/udp/source.rs:57).

Todo SUBSCRIBE válido en estructura actualiza el peer y, con PSK, la clave derivada. No exige demostrar conocimiento del secreto. Un segundo equipo que alcance el puerto puede desplazar al receptor activo sin conocer la PSK. Con dos sinks legítimos, sus heartbeats también pueden hacer alternar el destino. El cifrado protege el payload, pero no esta decisión de control.

**Corrección:** autenticar el handshake, confirmar la sesión antes de sustituir el peer y establecer una política explícita de receptor único. Añadir expiración, límites de frecuencia y una acción de desconexión. Usar identificadores de sesión y salts obtenidos de un generador criptográfico del sistema, en lugar de depender del reloj para unicidad.

**Validación:** suscripción sin credenciales, segundo receptor y tráfico repetitivo no deben interrumpir una sesión autenticada activa.

### H08 · P1 · Fugas de hilo y arranques incompletos

**Evidencia:** [engine/mod.rs:352](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/engine/mod.rs:352), [engine/mod.rs:935](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/engine/mod.rs:935), [manager.rs:162](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/manager.rs:162).

La fuente lanza el hilo de red antes de construir la captura. Si `build_input_stream` falla, retorna sin limpiar `is_running`; no llega a crear el `StreamStats` cuyo Drop apagaría el hilo. Puede quedar actividad de red y un puerto ocupado después de un inicio fallido. El resultado de `spawn` se ignora, permitiendo el caso inverso: captura activa sin hilo de red. El sink sí contiene una limpieza explícita para el fallo de construcción del output; conviene aplicar ese patrón de forma uniforme.

Start/Stop tampoco conservan handles para esperar que termine la sesión anterior. Esto deja una carrera al reiniciar un puerto rápidamente. El cierre envía Stop sin esperar una confirmación de liberación.

**Corrección:** tratar el arranque como una operación que se completa o revierte; conservar recursos e hilos en una sesión con cancelación y cierre acotado. Construir y validar recursos antes de emitir “listo”.

**Validación:** repetir fallos de captura y Start/Stop rápido sin acumular hilos ni listeners; reiniciar en el mismo puerto tras Stop.

### H09 · P1 · Un receptor TCP lento puede bloquear el hilo indefinidamente

**Evidencia:** [tcp_client.rs:15](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/transport/tcp_client.rs:15), [engine/mod.rs:714](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/engine/mod.rs:714), [engine/mod.rs:871](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/engine/mod.rs:871).

Existe timeout al conectar, pero no timeout de escritura. Los sockets pasan a modo bloqueante y `write_all` puede quedar detenido si el receptor deja de leer. Cambiar el flag de ejecución no interrumpe esa llamada; Stop puede aparentar finalizar mientras el hilo y sus recursos siguen retenidos. Las esperas de reconexión de hasta 60 segundos tampoco son cancelables durante el sleep.

**Corrección:** escrituras con plazo y cancelación, cierre del socket al detener y esperas de reconexión interrumpibles. Si se usa modo no bloqueante, conservar el offset de los datos pendientes para no perder audio tras escrituras parciales.

**Validación:** un cliente que conecta y deja de leer no debe impedir Stop ni el siguiente Start; fijar un objetivo de parada inferior a un segundo y medirlo.

### H10 · P1 · Una allowlist inválida abre el acceso

**Evidencia:** [allowlist.rs:11](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/transport/allowlist.rs:11), [engine/mod.rs:687](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/engine/mod.rs:687).

El parser descarta tokens inválidos. Si todos son inválidos, devuelve una lista vacía; el motor permite cualquier peer cuando la lista está vacía. Un error tipográfico en una configuración que el operador cree restrictiva termina permitiendo acceso general.

**Corrección:** distinguir “sin restricciones” de “restricciones inválidas”; devolver errores con los tokens problemáticos y rechazar el arranque. En dual-stack, normalizar IPv4 mapeada en IPv6 antes de comprobar redes IPv4; falta una prueba del camino real de aceptación para esa combinación.

**Validación:** una lista no vacía pero inválida nunca debe equivaler a acceso abierto; cubrir IPv4, IPv6, direcciones mapeadas y listas mixtas.

### H11 · P1 · La recuperación UDP no reconoce nuevas sesiones

**Evidencia:** [udp/jitter.rs:29](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/transport/udp/jitter.rs:29), [udp/sink.rs:134](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/transport/udp/sink.rs:134), [udp/source.rs:39](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/transport/udp/source.rs:39).

El jitter empieza en secuencia cero aunque el source siga emitiendo con una secuencia avanzada. Incorporarse en secuencia 1.000.000 obligó al módulo real a recorrer un millón de `Gap` antes del primer frame. Los saltos no tienen un límite de trabajo por iteración.

Al reiniciar la fuente, cambian salt y secuencia. El receive loop solo procesa AUDIO: ignora nuevos STREAM_INFO, conserva la clave antigua, la ventana replay y el jitter. Con cifrado deja de autenticar; sin cifrado rechaza las secuencias bajas hasta alcanzar la secuencia previa. En errores de recepción sale sin notificar a la UI; con timeouts puede seguir emitiendo heartbeats y reproduciendo silencio.

**Corrección:** identificador de sesión, secuencia inicial negociada o sincronización con el primer frame autenticado, límites de salto/trabajo, detector de pérdida de origen y resuscripción completa con backoff cancelable. Reinicializar formato, replay, jitter y conversión al cambiar de sesión.

**Validación:** incorporarse después de una hora de emisión, reiniciar source, cambiar clave, perder red y recuperar conectividad sin reiniciar manualmente el sink.

### H12 · P2 · El tamaño de captura elegido no se aplica

**Evidencia:** [engine/mod.rs:247](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/engine/mod.rs:247), [capture.rs:143](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/engine/capture.rs:143), [AudioSection.vue:48](/Users/jdavidoa91/Dev/tcp-streamer/src/components/sections/AudioSection.vue:48).

La UI permite 256/512/1024/2048 frames, pero el motor siempre construye `BufferSize::Default`. `resolve_buffer_size` tiene pruebas y no se usa en la ruta de producción. README y guía Broadcast describen ese control como efectivo.

**Corrección:** integrar la resolución contra el rango del dispositivo, con fallback explícito y visualización del valor efectivo; o eliminar la promesa y mostrar “automático” si esa es la política deseada.

**Validación:** comprobar el config enviado a CPAL y ejecutar una prueba de hardware en cada plataforma; una prueba aislada del helper no basta.

### H13 · P2 · Opciones de audio que no corresponden al modo

**Evidencia:** [AudioSection.vue:30](/Users/jdavidoa91/Dev/tcp-streamer/src/components/sections/AudioSection.vue:30), [AudioSection.vue:57](/Users/jdavidoa91/Dev/tcp-streamer/src/components/sections/AudioSection.vue:57), [engine/sink.rs:127](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/engine/sink.rs:127), [engine/mod.rs:325](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/engine/mod.rs:325).

El sink muestra sample rate y buffer de captura, aunque negocia su salida desde el formato recibido y usa buffer del driver. El formato WAV depende solo de `isServer`, incluso si quedó ese valor al cambiar a UDP/sink. El sink recibe overrides de chunk size pero no usa ese tamaño para recibir paquetes. El control Adaptive Buffer del formulario no se pasa a `start_sink`. En fuentes, la adaptación de perfiles predefinidos depende de un toggle que solo es visible en Custom y vale false inicialmente.

**Corrección:** matriz explícita de controles por rol/transporte y resumen de configuración efectiva del backend. Diferenciar adaptación de latencia, corrección de drift y capacidad del ring. Bloquear o marcar los cambios que requieren reiniciar; hoy min/max y loopback siguen editables durante la sesión.

**Validación:** cada control visible debe tener una consecuencia verificable o indicar que es informativo; revisar todas las combinaciones, no solo Source/TCP.

### H14 · P2 · Telemetría incompleta y métricas con significado ambiguo

**Evidencia:** [engine/sink.rs:182](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/engine/sink.rs:182), [engine/mod.rs:604](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/engine/mod.rs:604), [metrics.rs:24](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/metrics.rs:24), [stream.ts:165](/Users/jdavidoa91/Dev/tcp-streamer/src/stores/stream.ts:165).

El sink no emite stats/quality periódicos. Su dashboard muestra ceros o datos heredados de una fuente anterior. Start no reinicia contadores, calidad ni RTT; se reprodujo con Pinia. El bitrate de fuente es promedio desde el inicio, no velocidad actual. UDP cuenta payload enviado aunque `send_to` falle, y no dispone de información de pérdidas del receptor. Su detección de starvation usa `current_stream` TCP, por lo que no observa de igual modo una sesión UDP.

La calidad suma underruns expresados como eventos con overruns/drops expresados como muestras; una sola pérdida de varias muestras puede consumir toda la penalización. `buffer_health = 1 - ocupación/capacidad` también favorece un buffer vacío, que puede significar falta de audio. `bufferMs` empieza como capacidad del ring y luego se actualiza como objetivo de latencia: cambia de significado.

**Corrección:** esquema común de métricas por sesión y rol, unidades explícitas, bitrate en ventana, timestamps y disponibilidad. Mostrar “sin datos” cuando corresponda. Separar eventos de glitch, frames/muestras perdidas, ocupación, capacidad, objetivo y latencia medida. Añadir feedback del sink para evaluar UDP.

**Validación:** iniciar una sesión nueva, cambiar Source→Sink, simular silencio y pérdidas, desconectar receptor y comprobar que ninguna métrica sugiere actividad o calidad que no ha sido observada.

### H15 · P2 · El objetivo de latencia no garantiza latencia fija en la fuente

**Evidencia:** [engine/mod.rs:555](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/engine/mod.rs:555), [engine/mod.rs:811](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/engine/mod.rs:811), [latency.rs:126](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/engine/latency.rs:126).

La fuente limita backlog por arriba, pero después vacía lo disponible sin mantener una ocupación mínima. TCP tiene un prefill inicial y luego puede drenarlo; UDP no ejecuta ese prefill. Para Broadcast con valores mayores de 400 ms, el prefill TCP se limita a 400 ms. Por ello, el “objetivo” no es evidencia de una demora fija de ese valor en la fuente. El sink sí intenta mantener ocupación mediante prefill y drift.

**Corrección:** definir exactamente qué representa el valor Broadcast —buffer del sink, retardo de fuente o extremo a extremo— y alinear UI, guía y algoritmo. Si se exige demora de fuente constante, implementar pacing que conserve esa ocupación. Añadir medición con impulso y recuperación tras stalls.

**Validación:** medir 250/500/2000 ms y su estabilidad en source/sink, TCP/UDP y tras reconexión. No derivar una garantía audiovisual solo de parámetros de buffer.

### H16 · P2 · Pérdida de paquetes sin conservación explícita de duración

**Evidencia:** [udp/sink.rs:198](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/transport/udp/sink.rs:198), [udp/packet.rs:14](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/transport/udp/packet.rs:14).

`Pop::Gap` no inserta audio ni silencio; simplemente avanza. Si quedan frames posteriores y el ring contiene suficiente audio, el callback no necesariamente entra en underrun: la duración del frame perdido desaparece y se une audio posterior. El timestamp no se usa para reconstruir ese tiempo. Como los datagramas pueden tener tamaños distintos, no basta con asumir duración fija por secuencia.

**Corrección:** incluir posición/número de frames y conservar la línea temporal; comenzar con silencio exacto o repetición suavizada antes de una concealment más avanzada. Contabilizar pérdidas y duplicados de forma independiente.

**Validación:** eliminar paquetes de diferentes tamaños con ring prellenado y verificar duración, continuidad de canales y artefactos audibles.

### H17 · P2 · Persistencia silenciosa y almacenamiento de PSK sin protección específica

**Evidencia:** [useTauri.ts:28](/Users/jdavidoa91/Dev/tcp-streamer/src/composables/useTauri.ts:28), [settings.ts:261](/Users/jdavidoa91/Dev/tcp-streamer/src/stores/settings.ts:261).

Si falla la apertura de Store, se sustituye permanentemente por un store no-op. Guardar puede mostrar éxito aunque nada quede persistido. El nombre `store.bin` no añade cifrado: la PSK se entrega directamente al mismo store de preferencias, sin una capa específica de protección. Esto aumenta la exposición ante acceso al archivo de configuración o copias de respaldo; no implica que cualquier usuario remoto pueda leerlo.

**Corrección:** estado de persistencia observable, reintento y errores de guardado visibles. Guardar secretos mediante almacén de credenciales del sistema o una solución con gestión de claves explícita; almacenar en perfiles solo la referencia. Exportar configuración sin secretos por defecto.

**Validación:** archivo corrupto, permiso denegado y reapertura deben detectar pérdida de persistencia; inspeccionar que un export de soporte no contenga PSK.

### H18 · P2 · Falta validación integral de configuración y metadata

**Evidencia:** [commands.rs:30](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/commands.rs:30), [latency.rs:97](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/engine/latency.rs:97), [udp/packet.rs:85](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/transport/udp/packet.rs:85).

Los tipos numéricos rechazan algunas entradas durante IPC, pero faltan invariantes de aplicación. Custom permite mínimos/máximos incoherentes, tamaños extremos y chunk cero desde IPC o persistencia alterada. Los strings de rol/transporte/perfil no tienen validación central. STREAM_INFO valida estructura, pero no un rango razonable de frecuencia/canales ni una combinación permitida de flags.

**Corrección:** `StreamConfig`/`SinkConfig` tipados y validados con enums, límites de memoria, `0 < min ≤ max ≤ capacidad efectiva` y valores de chunk aceptados. Validar metadata antes de negociar dispositivos o reservar buffers. Mostrar errores por campo y no corregir silenciosamente todo.

**Validación:** pruebas de entradas inválidas, límites y fuzzing de packets/config; ningún caso debe provocar panic, asignaciones desmesuradas ni loop ocupado.

## Mejoras de UI/UX

La estructura de cinco secciones y la consistencia visual son una base aprovechable. Las siguientes propuestas completan el flujo de uso; las de accesibilidad y registros contienen además defectos concretos.

| Prioridad | Mejora                                   | Motivo y propuesta concreta                                                                                                                                                                                                                                                                                                    |
| --------- | ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| P1        | Estado y errores accionables             | Resolver H01/H03 primero. Mostrar el paso que falla y una acción: seleccionar dispositivo, revisar puerto, corregir PSK o reintentar. El color de Header actualmente solo distingue activo/inactivo, incluso ante error.                                                                                                       |
| P2        | Primer uso guiado                        | El dashboard inicial ofrece Start sin contexto. Mostrar un checklist de rol, dispositivo, destino/puerto y preparación. Llevar directamente al campo pendiente; ofrecer ejemplos Source TCP, Source UDP y Sink UDP.                                                                                                            |
| P2        | Resumen permanente de sesión             | Mostrar rol, transporte, perfil activo, dispositivo, endpoint, formato negociado, cifrado y receptor. Distinguir claramente el perfil editado de la configuración realmente ejecutada.                                                                                                                                         |
| P2        | Configuración básica y avanzada          | Mantener dispositivo, endpoint y perfil en la vista principal; agrupar ring, chunks, QoS y prioridad en opciones avanzadas. Evitar duplicar la selección de entrada en Conexión y Audio sin explicar el flujo.                                                                                                                 |
| P2        | Accesibilidad de formularios             | `Field` acepta `id` y lo expone al slot, pero las secciones no conectan ese id con sus controles. Los switches aparecen sin nombre en el árbol de accesibilidad. Añadir ids estables, `for`, `aria-labelledby`, descripciones y errores asociados.                                                                             |
| P2        | Semántica de controles                   | `SegmentedControl` declara radiogroup, pero sus botones no exponen radio/checked ni navegación con flechas. Close de Dialog y CopyField son acciones solo con icono sin nombre. Toasters carecen de región live. Añadir semántica y verificar uso completo con teclado/lector.                                                 |
| P2        | Descubrimiento comprensible              | Añadir estado “buscando”, cancelar/reintentar y separar “sin fuentes” de error. Mostrar nombre amigable, host, cifrado y compatibilidad. Deduplicar resultados; deshabilitar escaneos repetidos mientras uno está pendiente.                                                                                                   |
| P2        | Dispositivos y permisos                  | Botón de refrescar, estado de salida equivalente al de entrada y explicación de permiso de micrófono. Cargar outputs durante init para que auto-stream del sink no dependa de visitar Conexión; hoy `loadOutputDevices` está en el mounted de esa sección. Restaurar loopback antes de enumerar entradas.                      |
| P2        | Guardado comprensible                    | Indicador de cambios pendientes, Guardar disponible cerca de los campos y decisión explícita al cambiar perfil. Confirmar eliminación de perfil. Autostart debe revertir el switch y explicar el error si falla; hoy solo escribe en consola.                                                                                  |
| P2        | Registros operativos                     | Al llegar a 150 entradas la longitud deja de cambiar, pero Logs observa solamente `filteredLogs.length`: puede dejar de seguir entradas nuevas. Observar identidad/último id, incluir nivel y fecha al copiar, pausar seguimiento y ofrecer export sanitizado.                                                                 |
| P2        | Errores que sobreviven al límite de logs | `emit_log` limita globalmente a cinco eventos por segundo sin priorizar errores: los mensajes de inicio pueden consumir el cupo y ocultar el fallo posterior. Reservar errores/estado crítico y agregar repeticiones de debug.                                                                                                 |
| P2        | Idioma completo y persistente            | Traducir `statusText`, calidad, textos de copia, `found`, `encrypted`, nombres de buffer y diálogo de perfil. Persistir locale. Probar cambio de idioma con sesión activa y estados de error; equivalencia entre catálogos no detecta strings fuera del catálogo.                                                              |
| P2        | Ventana pequeña y mensajes largos        | Header tiene navegación, estado completo, Start y lengua en una sola fila. Puede desbordar con mensajes largos, español o ventana mínima de 760 px. Añadir diseño compacto, truncado con detalle accesible y validación a 760×560 y 960×640. Esto requiere verificación nativa; la vista de navegador no sustituye esa prueba. |
| P2        | Atajos coherentes                        | Ctrl/Cmd+S alterna streaming, una acción inesperada mientras se edita configuración. Reservarlo para guardar; utilizar un atajo distinto para Start/Stop y mostrarlo. Evitar que el atajo opere dentro de inputs si puede interrumpir una emisión.                                                                             |
| P3        | Cierre y bandeja consistentes            | Explicar que cerrar la ventana mantiene la aplicación activa. Segunda instancia debe mostrar una ventana previamente oculta; hoy solo intenta focus. Añadir estado/Start/Stop al tray e idioma coherente con la UI.                                                                                                            |
| P3        | Botón de apoyo funcional                 | En Settings se pasa `as="a"` y `href`, pero Button siempre renderiza `<button>`; además el destino contiene `YOUR_ID`. Usar enlace real compatible con Tauri y el destino correcto.                                                                                                                                            |

Referencias de implementación: [Header](/Users/jdavidoa91/Dev/tcp-streamer/src/components/Header.vue:1), [Field](/Users/jdavidoa91/Dev/tcp-streamer/src/components/ui/Field.vue:1), [Switch](/Users/jdavidoa91/Dev/tcp-streamer/src/components/ui/Switch.vue:1), [SegmentedControl](/Users/jdavidoa91/Dev/tcp-streamer/src/components/ui/SegmentedControl.vue:1), [Logs](/Users/jdavidoa91/Dev/tcp-streamer/src/components/sections/LogsSection.vue:90), [App](/Users/jdavidoa91/Dev/tcp-streamer/src/App.vue:31), [Settings](/Users/jdavidoa91/Dev/tcp-streamer/src/components/sections/SettingsSection.vue:112), [tray](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/lib.rs:22).

## Funcionalidades recomendadas para el backend

Estas son propuestas de producto, no fallos por no existir. Se ordenan por utilidad para una aplicación de audio en vivo.

| Orden | Funcionalidad                                    | Alcance recomendado                                                                                                                                                                                                       |
| ----- | ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1     | Diagnóstico previo al inicio                     | Validar configuración, dispositivo, formato, puerto y resolución. Separar comprobación de conectividad de prueba de audio; devolver resultados por etapa a la UI.                                                         |
| 2     | Reconexión del sink y supervisión de dispositivo | Recuperar pérdida/reinicio del origen; detectar dispositivo desconectado, suspend/resume y cambio de red. Permitir políticas de fallo y fallback explícitas.                                                              |
| 3     | Telemetría del sink y nivel de audio             | Frames recibidos, pérdidas, descartes, ocupación real, drift, tiempo sin audio, RMS/peak y clipping. Los medidores ayudan a distinguir “red activa” de “audio presente”.                                                  |
| 4     | Sesiones, capacidades y metadata versionada      | Negociar versión, formato, modo de cifrado, identidad y política de receptor; rechazar incompatibilidades antes de arrancar reproducción.                                                                                 |
| 5     | Export/import y soporte sanitizado               | Perfiles con versión de esquema y migraciones; diagnóstico exportable con SO, versiones, formato efectivo, métricas y logs, excluyendo secretos y direcciones sensibles cuando se solicite.                               |
| 6     | IPv6 y selección de interfaz en UDP              | TCP ya resuelve IPv6, pero UDP source/sink se vinculan a IPv4 y discovery utiliza direcciones v4. Unificar resolución, elegir interfaz de bind y presentar endpoints válidos.                                             |
| 7     | Descubrimiento continuo                          | Mantener una tarea de browse con altas/bajas, deduplicación y cancelación; nombres basados en identidad de equipo/sesión. Hoy distintas máquinas usan `tcp-streamer-{port}` como instancia y hostname.                    |
| 8     | Varios receptores                                | Arquitectura con cola y política de backpressure por receptor. TCP usa una conexión activa; UDP un peer mutable. Un cliente lento no debe detener a los demás. Implementar después de resolver sesión/cancelación.        |
| 9     | Ganancia, mute y ruteo de canales                | Ganancia con rampas, mute local y selección/downmix configurable. Actualmente multicanal conserva FL/FR; exponer la limitación y permitir elegir canales si el caso de uso lo requiere.                                   |
| 10    | Transporte eficiente para WAN                    | Evaluar codec como Opus y recuperación de pérdidas cuando el producto quiera operar por Internet. PCM estéreo 48 kHz/16 bit requiere 1.536 kbps de payload, antes de overhead; no es imprescindible para una LAN estable. |
| 11    | Protección TCP                                   | Para redes fuera de confianza, evaluar TLS y autenticación; documentar que TCP actual y HTTP no cifran audio. Evitar presentar AEAD de UDP como protección de todos los modos.                                            |
| 12    | Actualización de la aplicación                   | Distribución firmada y actualización verificable, con rollback o recuperación y política de actualización que no interrumpa sesiones activas.                                                                             |

## Calidad de audio, arquitectura y pruebas

### Audio y rendimiento

- **Resampling:** Catmull-Rom mantiene estado entre chunks, pero no incluye un filtro antialias explícito para downsampling. Añadir pruebas espectrales a frecuencias cercanas a Nyquist y tasas más alejadas; decidir después si adoptar un resampler con filtrado. Las pruebas actuales de tonos verifican pitch y canales, no toda la respuesta en frecuencia. [convert.rs:115](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/engine/convert.rs:115).
- **Callbacks:** captura usa `reserve/push` y reproducción de enteros `resize` con capacidad preasignada. Si el driver entrega bloques mayores a la reserva, pueden asignar memoria durante el callback. La reserva de 16.384 muestras ayuda, pero no demuestra ausencia absoluta de asignaciones. Dimensionar contra límites conocidos o procesar por bloques fijos sin crecimiento; medir en hardware multicanal. [capture.rs:30](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/engine/capture.rs:30), [playback.rs:28](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/engine/playback.rs:28).
- **Errores CPAL:** se registran en logger sin transición de estado o recuperación. Convertirlos en eventos del supervisor fuera del callback y distinguir pérdida de dispositivo de error recuperable.
- **UDP:** reutilizar buffers y contexto de cifrado donde sea razonable; actualmente se crean varios vectores por datagrama cifrado y se deriva la clave otra vez en heartbeats. Optimizar después de medir CPU/asignaciones. Conservar el límite de payload de 1.200 bytes, que es una protección útil frente a fragmentación.
- **Formatos efectivos:** enumeración recorre todos los hosts CPAL, pero source/sink abren solo `default_host`. Con hosts alternativos habilitados puede ofrecer un dispositivo imposible de abrir. Representar dispositivos mediante host + identificador, no solo nombre deduplicado.

### Arquitectura

El archivo `engine/mod.rs` tiene 1.071 líneas y mezcla apertura, conexión, HTTP, reintentos, pacing, cifrado indirecto y telemetría. Separarlo por responsabilidades facilita probar precisamente los fallos encontrados. La abstracción `Connection` cubre TCP, mientras UDP tiene una rama específica dentro del motor: conviene unificar el contrato de sesión/transporte o hacer orquestadores separados con un supervisor común.

Propuesta de separación: configuración validada → sesión/supervisor → captura o playback → transporte → métricas/eventos. Mantener conversión, jitter y control de drift como módulos puros. Generar o comprobar automáticamente tipos IPC/eventos compartidos para evitar divergencia entre TypeScript y Rust. Los errores deberían tener códigos tipados más contexto, dejando a la UI la traducción y la acción sugerida.

El API HTTP detecta GET/HEAD mediante peek de 50 ms y no implementa un parser completo ni diferencia HEAD para omitir cuerpo. Pedidos fragmentados/lentos pueden interpretarse como PCM y cualquier ruta GET recibe streaming. Definir compatibilidad HTTP limitada explícitamente o implementar parsing acotado, rutas, HEAD y errores adecuados. [engine/mod.rs:706](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/src/audio/engine/mod.rs:706).

### Seguridad de la aplicación de escritorio

Revisar CSP: permite script inline y conexiones HTTP/HTTPS generales aunque la red de audio está en Rust. Reducir el permiso a lo que usa el frontend y conservar la compatibilidad IPC. La existencia de esa política no demuestra por sí sola una vulnerabilidad XSS. Revisar también permisos de plugins que no se usan y rutas de apertura externa. [tauri.conf.json:31](/Users/jdavidoa91/Dev/tcp-streamer/src-tauri/tauri.conf.json:31).

No se considera que `capabilities: []` desactive automáticamente el archivo default: la configuración de Tauri contempla cargar capabilities del directorio cuando la lista está vacía. Cualquier endurecimiento debe contrastarse con la versión instalada y probarse en el runtime nativo. Referencia: [configuración de seguridad Tauri](https://v2.tauri.app/reference/config/#securityconfig).

### Pruebas que faltan

La suite es más fuerte en funciones puras que en coordinación y UX real. No calcula cobertura porcentual; el número de pruebas aprobadas no demuestra cobertura de los flujos enumerados.

1. Start/Stop real con ack, cancelación, arranque fallido y eventos tardíos.
2. Perfiles completos, vacíos, corruptos y migraciones; dispositivos por perfil y errores de persistencia.
3. Navegador con Select/Dialog reales: ausencia de dispositivos, discovery, teclado, labels y cambio de secciones. El test actual de Select comprueba placeholder cerrado y excluye abrir el menú.
4. UDP end-to-end: origen reiniciado, join tardío, pérdida/reorden/duplicado, PSK incorrecta, downgrade, replay inválido y segundo subscriber.
5. TCP con consumidor lento, cierre a mitad de escritura, HTTP fragmentado y puerto ocupado.
6. Audio físico por plataforma: micrófono, loopback Windows, salida, mono/multicanal, suspensión y desconexión USB.
7. Prueba prolongada de varias horas: latencia medida, memoria, CPU, crecimiento de hilos, pérdida de paquetes y recuperación.

## CI, distribución y documentación

- **Paridad de gates:** Build comprueba typecheck y formatos; Release no los exige. Compartir un workflow de validación evita publicar un tag que salta controles del flujo principal. Añadir `--locked` a checks Rust y alinear versiones Node de build/release. [build.yml](/Users/jdavidoa91/Dev/tcp-streamer/.github/workflows/build.yml:1), [release.yml](/Users/jdavidoa91/Dev/tcp-streamer/.github/workflows/release.yml:1).
- **Pruebas por plataforma:** los tests/clippy del workflow se ejecutan en Ubuntu; la matriz de tres SO solo compila. Ejecutar tests relevantes en los tres sistemas y reservar hardware smoke para un runner apropiado.
- **Dependencias:** mantener lockfiles —ambos están versionados— y añadir revisión periódica/advisories. No se ejecutó un escáner de vulnerabilidades, así que este reporte no certifica ausencia de CVE.
- **Release manual:** el extractor de notas deriva versión de `github.ref_name`; al ejecutar workflow_dispatch sobre una rama buscará una sección del changelog con el nombre de esa rama. Añadir input de tag/versión o restringir claramente el uso manual. Comprobar tag contra package.json, Cargo.toml y tauri.conf.json.
- **Firma:** no se aprecia configuración explícita de firma/notarización en el workflow inspeccionado. Verificar la distribución real antes de concluir su estado; para usuarios finales conviene una ruta firmada que reduzca bloqueos y pasos manuales.
- **Documentación:** corregir las afirmaciones de buffer de captura efectivo, alcance IPv6, adaptación y latencia fija conforme al comportamiento validado. Explicar receptor único, PCM estéreo y diferencia entre compatibilidad TCP/WAV y protocolos de terceros.
- **Identidad:** completar metadata de Cargo (`authors = ["you"]`, descripción solo TCP) y mantener una lista pública de capacidades soportadas por rol, transporte y SO.
- **Higiene local:** excluir o formatear consistentemente `.superpowers/` en Prettier según la política del proyecto. No ocultar errores de código fuente mediante exclusiones generales.

## Plan de mejora sugerido

| Etapa                        | Trabajo                                                                             | Criterio de salida                                                                                                                                    |
| ---------------------------- | ----------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1 · Uso básico fiable        | H01–H04, H08–H09; carga inicial de outputs/loopback                                 | Start/Stop y perfiles coherentes; fuente UDP funciona desde configuración limpia; cero errores de render ante estados vacíos; no se acumulan recursos |
| 2 · Protocolo y recuperación | H05–H07, H10–H11, H18                                                               | Cifrado requerido no admite plaintext; paquetes inválidos no alteran estado; allowlist inválida rechazada; recuperación ante reinicio de source       |
| 3 · Audio y observabilidad   | H12–H16; telemetría y pruebas prolongadas                                           | Cada opción visible se aplica; métricas disponibles por rol; pérdidas conservan duración; latencia medida y documentada                               |
| 4 · Experiencia de operación | Accesibilidad, onboarding, perfiles editados/activos, i18n, logs y diagnósticos     | Flujo completo con teclado; errores accionables; configuración y sesión distinguibles; reportes sanitizados                                           |
| 5 · Expansión y distribución | Multi-receptor, codecs/WAN si se necesitan, IPv6 UDP, firma/updater y gates comunes | Capacidades nuevas respaldadas por pruebas, límites y documentación; release reproducible                                                             |

No propondría una reescritura completa. La mayor parte del valor se obtiene al conectar correctamente los módulos existentes y comprobar los límites entre UI, sesión, audio y transporte. La primera entrega debería centrarse en que iniciar, detener, cambiar perfil y recuperarse de un fallo sean operaciones observables y predecibles.
