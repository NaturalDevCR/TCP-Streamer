# Implementación de la revisión integral

El reporte `revision-integral-2026-10-10.md`, aprobado mediante «procede con todo», guía esta implementación. Los cambios corresponden a la preparación de la versión 3.0.0, autorizada para commit y publicación. Este documento distingue las correcciones realizadas, la evidencia de validación y las ampliaciones de producto todavía pendientes.

## Resultado implementado

### Frontend y experiencia de uso

- Start espera el resultado nativo y muestra estados por sesión: iniciando, esperando, ejecutando, reconectando, detenido y error. Stop puede cancelar un inicio pendiente; respuestas y eventos de sesiones antiguas no reactivan la interfaz.
- La fuente UDP no exige un destino TCP ni abre un listener TCP por una configuración anterior. Los controles corresponden al rol y transporte y se bloquean durante una sesión activa.
- Los selectores vacíos no crean opciones inválidas de Reka. Sus etiquetas seleccionadas se actualizan al cambiar de idioma.
- Los perfiles restauran snapshots completos, incluidos campos vacíos y dispositivos propios. Crear un perfil usa el formulario actual; los cambios pendientes y la eliminación tienen confirmaciones accesibles. Los fallos al guardar se muestran.
- Las salidas se enumeran al iniciar y la preferencia de loopback se restaura antes de enumerar entradas. El descubrimiento tiene progreso, error, deduplicación y rechazo de resultados tardíos.
- Campos y descripciones están asociados mediante IDs; los grupos de opciones permiten navegación con teclado. Ctrl/Cmd+S guarda y Ctrl/Cmd+Enter inicia o detiene.
- El Header se adapta a ventanas pequeñas y distingue estados mediante colores. El idioma persiste; se traducen estados, métricas, copias y diálogos.
- Los registros tienen fecha y nivel, seguimiento basado en identidad, pausa y exportación sanitizada. Los errores críticos no se pierden por el límite de mensajes informativos.
- El Dashboard separa configuración solicitada, configuración negociada, ocupación actual y objetivo. Los contadores indican eventos o frames según su origen.
- El secreto UDP permanece en memoria por defecto. Guardarlo en preferencias exige una opción explícita que avisa que el almacenamiento local es texto plano. Export/import de perfiles y copia de logs excluyen secretos y direcciones conocidas.
- El enlace de soporte usa el repositorio real y el plugin opener, restringido a ese destino.

### Backend, audio y red

- El supervisor confirma la inicialización real, identifica sesiones monotonamente y expone estado por comando/evento. Los errores nativos se serializan como texto legible.
- La captura/reproducción se construye antes de lanzar sus trabajadores. Sus fallos detienen el transporte; la liberación cancela y reúne los hilos. El cierre pide confirmación de limpieza con un plazo de dos segundos.
- UDP v2 tiene negociación HMAC de tres pasos, salts del sistema, audio ligado a sesión, confirmación y desconexión autenticadas. Las negociaciones fallidas y la construcción fallida de salida liberan el lease del receptor.
- Con PSK, el receptor rechaza plaintext. La autenticación ocurre antes de actualizar anti-replay. Heartbeats llevan contador y un SUBSCRIBE repetido no mantiene indefinidamente una sesión muerta.
- Un segundo receptor no sustituye al receptor activo. El trabajo de control está acotado y el jitter conserva como máximo 256 paquetes; entrar tarde o saltar secuencias no recorre millones de huecos.
- La recuperación de una fuente reiniciada renueva claves, secuencias, jitter y conversión de frecuencia/canales. El formato visible también se actualiza. Tras 30 segundos sin audio auténtico, el supervisor comunica el fallo.
- Las posiciones/cantidades de frames permiten conservar la duración de pérdidas de paquetes mediante silencio, con límite de dos segundos por discontinuidad.
- UDP acepta dual-stack con fallback IPv4. Discovery identifica v2, deduplica endpoints y representa IPv6 con corchetes; omite link-local sin scope utilizable y apaga su daemon al liberar recursos.
- TCP utiliza un plazo total de escritura no bloqueante de 200 ms. Una escritura incompleta cierra la conexión, evitando continuar PCM o chunks HTTP desalineados. Reintentos son cancelables; la resolución DNS usa un único trabajador con cola acotada y espera cancelable de dos segundos.
- HTTP consume headers fragmentados con límites de tiempo/tamaño. HEAD responde headers y cierra sin enviar audio.
- La allowlist rechaza cualquier token inválido y normaliza peers IPv4 mapeados. Dispositivos se buscan en todos los hosts que ofrece la enumeración.
- El buffer de captura solicitado se aplica dentro del rango soportado. Configuración, perfiles, endpoints, formatos y límites de memoria se validan antes de arrancar.
- Broadcast conserva el objetivo en el buffer de la fuente; su detector de inanición observa audio disponible por encima de ese objetivo. Fuera de Broadcast, adaptación limita backlog y no garantiza un mínimo de retardo de fuente.
- El receptor publica bytes, bitrate por intervalo real, ocupación, objetivo, pérdidas, rechazos de autenticación y drift. Una sesión esperando no anuncia mala calidad por ausencia de mediciones.

### Diagnóstico previo y distribución

- La comprobación previa devuelve etapas de configuración, dispositivo/formato, red y alcance. Comprueba resolución y disponibilidad temporal de puertos sin capturar audio ni contactar el receptor. Sus resultados se marcan desactualizados al editar configuración.
- Build/Release comparten gates frontend y Rust para macOS, Windows y Linux, con instalaciones reproducibles. Release manual valida un tag existente contra las tres versiones y notas y prepara un borrador.
- CSP y capacidades se reducen a scripts/IPC y al enlace externo necesario. Segunda instancia muestra, restaura y enfoca la ventana existente.
- README, guías y changelog documentan los comportamientos, límites y compatibilidad real.

## Cobertura de los hallazgos

| Hallazgo | Estado y evidencia principal                                                                                      |
| -------- | ----------------------------------------------------------------------------------------------------------------- |
| H01      | Corregido: acuse nativo, estado numerado, cancelación pendiente y error legible.                                  |
| H02      | Corregido: UDP sin IP TCP oculta; bind TCP solo en servidor TCP.                                                  |
| H03      | Corregido: selector vacío probado con componentes Reka reales.                                                    |
| H04      | Corregido: perfiles completos, vacíos y dispositivos aislados probados.                                           |
| H05–H07  | Corregidos: pruebas de plaintext, autenticación/replay y protección de receptor activo.                           |
| H08      | Corregido: arranque ordenado, rollback de lease y propiedad/join de trabajadores.                                 |
| H09      | Corregido: plazo total probado con receptor TCP que no lee; cierre ante escritura parcial.                        |
| H10      | Corregido: rechazo estricto de allowlist y peer IPv4 mapeado.                                                     |
| H11      | Corregido: secuencias acotadas y renegociación real de clave/frecuencia/canales.                                  |
| H12      | Corregido: petición de buffer conectada a configuración CPAL y rango soportado.                                   |
| H13      | Corregido: controles por rol y adaptación visible aplicada; configuración efectiva publicada.                     |
| H14      | Corregido en telemetría y unidades; la latencia física total todavía requiere medición.                           |
| H15      | Corregido para retención de buffer Broadcast; no se promete un retardo físico universal.                          |
| H16      | Corregido para pérdidas de paquetes mediante posiciones de frames y silencio exacto acotado.                      |
| H17      | Corregido el guardado silencioso; mitigado PSK por memoria/opt-in. Falta almacén nativo seguro para persistencia. |
| H18      | Corregido: validación central y metadata versionada/autenticada con límites.                                      |

## Verificación

- Frontend: **59 pruebas aprobadas en 12 archivos**; `typecheck`, `lint`, `format:check` y build aprobados.
- Rust: **170 pruebas aprobadas; 1 prueba de captura física ignorada**. `cargo fmt --check` y Clippy para todos los targets, con warnings como errores, aprobados.
- `pnpm tauri build --debug --bundles app`: paquete macOS construido. Hubo advertencias de anotaciones PURE de VueUse; no bloquearon el build.
- Se corrigió una incompatibilidad real de versiones minor entre opener JS/Rust que inicialmente impedía empaquetar; ahora ambos usan la serie 2.5.
- No se ejecutó release firmado, notarización, CI remoto ni medición física de audio/video.

Pruebas de interfaz nativa ejecutadas en macOS: carga inicial de entrada/salida, dropdowns y teclado, diagnóstico de destino ausente, rechazo legible de un hostname inválido sin abrir audio, diagnóstico de fuente UDP sin IP TCP, inicio pendiente ante un socket local sin respuesta y cancelación a estado Listo. No se capturó audio ni se reprodujo sonido durante esas pruebas. Se restauraron rol, transporte, dirección temporal e idioma al finalizar.

Se inspeccionó la ventana nativa de 960 × 640; la comprobación de 760 × 560 en navegador no mostró overflow horizontal. No equivale a probar todas las combinaciones de ventana nativa y plataforma.

[Captura de la comprobación previa aprobada en la aplicación nativa](./assets/diagnostico-nativo-2026-10-10.png).

La revisión independiente encontró escritura TCP parcial, cancelación pendiente, rollback de suscripción, supervisión de errores CPAL e inanición del buffer Broadcast. Se corrigieron y añadieron pruebas de regresión pertinentes.

## Compatibilidad y ampliaciones pendientes

**Ambos extremos deben actualizarse para usar UDP v2.** No hay compatibilidad de negociación con v1. TCP/HTTP mantienen audio sin cifrar y un receptor activo. El changelog de esta implementación corresponde a la versión 3.0.0. La publicación y los instaladores se verifican por separado en GitHub.

Las siguientes propuestas del reporte siguen como ampliaciones de producto, no se presentan como implementadas:

| Capacidad                                  | Situación actual / siguiente alcance                                                                                                           |
| ------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| Credenciales seguras persistidas           | Memoria por defecto; opt-in local sin cifrar. Integrar un almacén del sistema por plataforma.                                                  |
| Recuperación de dispositivo/suspend/red    | Fallo supervisado y recuperación UDP de fuente dentro de su plazo. Falta selección estable por ID y política explícita de fallback/reapertura. |
| Medidores RMS/peak/clipping                | Telemetría de transporte/audio buffering implementada; faltan medidores de contenido en callbacks sin asignaciones.                            |
| Descubrimiento continuo e interfaz de bind | Escaneo cancelable y dual-stack implementados; falta servicio continuo de altas/bajas y selección de interfaz.                                 |
| Varios receptores                          | Lease único protegido. Requiere colas independientes, claves/sesiones por receptor y políticas de backpressure.                                |
| Ganancia, mute y ruteo                     | Conversión actual normaliza canales, conservando el par frontal en captura multicanal; faltan rampas y selección configurable.                 |
| Codec/WAN                                  | PCM para LAN. Evaluar Opus y recuperación frente a pérdida antes de prometer operación por Internet.                                           |
| TLS y autenticación TCP                    | TCP sigue sin cifrado; diseñar identidad/verificación de certificados y configuración interoperable.                                           |
| Tray con Start/Stop y locale               | Se corrigieron cierre/segunda instancia; falta ampliar acciones/estado e idioma del menú nativo.                                               |
| Firma, notarización y updater              | Necesita identidad/credenciales y feed de publicación del propietario; no hay updater configurado ni release firmado.                          |
| Calidad del resampling y RT estricto       | Mantiene el conversor existente; falta comparar aliasing y hacer mediciones de asignaciones/buffer fuera de la capacidad habitual.             |

Las pruebas prolongadas, mediciones de audio/video y validación real en Windows/Linux requieren sus dispositivos/entornos. Los gates están configurados para ejecutarlas en CI; no se afirma que hayan corrido en GitHub desde este checkout local.
