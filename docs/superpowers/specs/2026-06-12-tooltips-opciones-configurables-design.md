# Tooltips para opciones configurables

## Objetivo

Explicar en la interfaz para qué sirve cada opción configurable sin añadir texto
permanente que aumente la altura o el ruido visual de las pantallas.

## Alcance

Se añadirá ayuda contextual a todos los controles que modifican configuración en:

- Conexión.
- Audio.
- Ajustes.

Esto incluye campos, selectores, controles segmentados y switches. No incluye
navegación, botones de acción, filtros de registros ni métricas del panel.

## Diseño

Cada etiqueta configurable mostrará un botón informativo compacto junto a su
texto. Al pasar el cursor o enfocar ese botón con teclado se abrirá un tooltip
que describa:

- Qué cambia la opción.
- Cuándo conviene utilizarla.
- El efecto relevante de sus valores cuando pueda no ser evidente.

Los tooltips usarán el componente `Tooltip.vue` existente y los textos estarán
en los catálogos inglés y español. Los avisos operativos importantes, como la
advertencia de latencia ultra-baja con loopback, seguirán visibles.

## Componentes

`Field.vue` aceptará una propiedad opcional para el texto del tooltip y
renderizará el botón informativo junto a la etiqueta. Los switches, que no usan
`Field.vue`, compartirán un pequeño componente de etiqueta configurable para
mantener el mismo aspecto y comportamiento accesible.

El botón informativo tendrá un nombre accesible localizado, será alcanzable por
teclado y no modificará el valor del control asociado.

## Traducciones

Los catálogos `en.json` y `es.json` incluirán:

- Un nombre accesible común para los botones informativos.
- Una descripción específica para cada opción configurable.
- Traducciones de las etiquetas todavía escritas directamente en inglés cuando
  formen parte de este trabajo.

La prueba de paridad de catálogos seguirá garantizando que ambos idiomas tengan
las mismas claves.

## Pruebas

Se añadirán pruebas de componentes que comprueben:

- Que `Field.vue` no muestra el botón cuando no hay tooltip.
- Que lo muestra con nombre accesible y contenido correcto cuando sí lo hay.
- Que la etiqueta compartida para switches mantiene el mismo comportamiento.
- Que los catálogos inglés y español conservan paridad.

También se ejecutarán lint, formato, typecheck, pruebas frontend, pruebas Rust,
Clippy, formato Rust y un build local antes de publicar.

## Release

El cambio se publicará junto con el commit local de audio ya existente como
`v2.3.0`. Se enviará `main`, se creará y enviará el tag, y el workflow de release
generará artefactos para Linux, Windows y macOS.

Se vigilarán los workflows Build y Release en GitHub Actions. Cualquier fallo
reproducible relacionado con estos cambios se corregirá y se volverá a publicar
hasta que ambos workflows queden en verde.
