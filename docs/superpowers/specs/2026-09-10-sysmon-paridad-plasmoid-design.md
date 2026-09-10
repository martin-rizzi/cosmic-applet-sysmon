# Paridad con el plasmoid `com.labatata.sysmonitor`

**Estado:** diseño aprobado por Martin el 2026-09-10.
**Objetivo:** que `cosmic-applet-sysmon` cubra toda la superficie funcional del plasmoid
de KDE — seis secciones, ocho opciones, popup por sección — leyendo `/proc` y `/sys`
directo, sin lanzar subprocesos por tick.

## 1. Punto de partida

Hoy el applet son 569 líneas de Rust (`main.rs`, `app.rs`, `draw.rs`, `proc.rs`) que
replican sólo la vista compacta de CPU y RAM. El plasmoid son 4177 líneas de QML.

| | Plasmoid | Applet hoy |
|---|---|---|
| Secciones | 6 (CPU, GPU, RAM, red, disco, temperaturas) | 2 (CPU, RAM) |
| Paneles de detalle | 6 | 1 popup fijo |
| Opciones de config | 8 | 0 |
| Gráficos de historial | 5 | 0 |

El popup actual (`app.rs:242`) muestra una vista fija: total de CPU, barra por núcleo,
resumen de RAM, caché y swap. No hay selección de sección ni configuración.

## 2. Comportamiento a replicar

### 2.1 Click por sección

Cada sección de la vista compacta es un botón independiente. Semántica tomada de
`main.qml:93-98`:

- click en una sección con el popup cerrado → abre el popup en esa sección;
- click en la **misma** sección con el popup abierto → cierra el popup;
- click en **otra** sección con el popup abierto → cambia de panel sin cerrar.

Índices del plasmoid, que conservamos: `0=CPU, 1=RAM, 2=Network, 3=Storage,
4=Temperatures, 5=GPU`.

### 2.2 Pie del popup

Dos botones centrados (`FullView.qml`, último `RowLayout`):

- **Monitor de sistema.** El plasmoid ejecuta `kstart plasma-systemmonitor`. No existe
  monitor nativo de COSMIC; se lanza el primero disponible de una lista ordenada
  (`plasma-systemmonitor`, `gnome-system-monitor`, `btop`, `htop`). Si no hay ninguno,
  el botón no se dibuja.
- **Engranaje.** Abre la página de settings dentro del mismo popup (COSMIC no tiene el
  diálogo de configuración de applets de Plasma).

### 2.3 Las ocho opciones

Nombres y defaults calcados de `contents/config/main.xml`:

| Clave | Tipo | Default | UI |
|---|---|---|---|
| `update_interval` | int, 500–10000, paso 500 | 2000 | spinner |
| `show_cpu` | bool | true | switch |
| `show_ram` | bool | true | switch |
| `show_network` | bool | true | switch |
| `show_storage` | bool | true | switch |
| `show_temps` | bool | true | switch |
| `show_gpu` | bool | true | switch |
| `section_order` | string CSV | `temps,network,storage,cpu,gpu,ram` | lista con ↑/↓ |

`section_order` se normaliza al leerse: se descartan claves desconocidas y duplicadas, y
se agregan al final las que falten (misma lógica que `normalizedSectionOrder` en
`ConfigGeneral.qml`). Persistencia con `cosmic-config` bajo el app id, que da recarga en
vivo sin reiniciar el applet.

### 2.4 Contenido de cada panel de detalle

Todos los paneles llevan encabezado (`DetailHeader`: título, ícono, valor, barra) salvo
donde se aclare.

- **CPU** — barra de total; filas `Total:`, `User:`, `System:`; gráfico de historial;
  barras por núcleo; top de procesos por CPU; uptime del sistema.
- **RAM** — filas `Total:`, `Used:`, `Free:`, `Cached:`; historial; top de procesos por
  memoria; bloque de swap con `Total:` y `Used:`.
- **Red** — filas `Upload:` / `Download:`; dos gráficos de historial separados; sin barra
  en el encabezado (`showBar: false`).
- **Disco** — lectura y escritura en MB/s con historial; lista de dispositivos con
  usado/total y porcentaje; mensaje "No storage devices found" si la lista queda vacía.
- **Temperaturas** — un renglón por sensor. El plasmoid muestra "Install lm-sensors for
  temperature data" cuando no hay datos; nosotros leemos hwmon directo, así que el texto
  equivalente es que no hay sensores expuestos.
- **GPU** — filas `Clock:`, `Memory:`, `Temperature:`, `Usage:`; historial; top de
  procesos por VRAM.

## 3. Arquitectura

```
src/
  main.rs
  config.rs          las 8 claves + cosmic-config
  metrics/
    mod.rs           agrega colectores y decide qué refrescar
    cpu.rs           /proc/stat + /proc/cpuinfo + /proc/uptime
    mem.rs           /proc/meminfo
    net.rs           /proc/net/dev
    disk.rs          /proc/diskstats + /proc/self/mounts + statvfs + /sys/block
    temp.rs          /sys/class/hwmon
    gpu.rs           /sys/class/drm/*/device + fdinfo
    procs.rs         /proc/N/stat + /proc/N/statm
    history.rs       buffer circular compartido
  ui/
    compact.rs       panel: N secciones clickeables
    settings.rs      página del engranaje
    detail/{cpu,gpu,ram,net,storage,temp}.rs
  draw.rs            medidores SVG + gráfico de historial
```

### 3.1 Muestreo en dos niveles

La decisión de diseño central. El panel sólo necesita datos baratos (un puñado de
lecturas de archivo). Lo caro corre **sólo mientras su panel de detalle está visible**:

| Barato — siempre | Caro — sólo con el detalle abierto |
|---|---|
| `/proc/stat`, `/proc/meminfo` | top de procesos (recorre ~300 `/proc/N`) |
| `/proc/net/dev`, `/proc/diskstats` | VRAM por proceso (`fdinfo` de cada PID) |
| `hwmon/temp*_input` | `statvfs` por punto de montaje |
| `gpu_busy_percent` | inventario de `/sys/block` |

El plasmoid recolecta todo siempre. Ésta es la diferencia real de costo, más que sacar
los forks.

### 3.2 Sin `df`, sin `sensors`, sin `lsblk`

- `statvfs` es una syscall vía `rustix`, ya presente en el árbol de dependencias
  (igual que `libc`): no suma nada externo al build. Los puntos de montaje salen de
  `/proc/self/mounts`, filtrados a dispositivos de bloque reales — equivalente al
  `df -h ... | grep '^/dev'` del plasmoid.
- Las temperaturas salen de `/sys/class/hwmon/*/name`, `temp*_input` y `temp*_label`,
  que es exactamente lo que lee `lm_sensors`.
- El inventario de discos sale de `/sys/block`, no de `lsblk -J`.

### 3.3 Métricas de tasa

Red, I/O de disco y CPU se calculan por delta. Cada colector guarda su muestra previa
más el timestamp y expone `refresh(now)`. La primera lectura sólo fija la línea base.

### 3.4 Historial

Buffer circular de **60 puntos** por métrica (cpu, ram, red, disco, gpu), como
`main.qml`. El gráfico se dibuja en `draw.rs` con la misma estética del Canvas del
plasmoid: fondo tenue, líneas de grilla horizontales, área rellena bajo la curva.

**Desvío deliberado:** el plasmoid rotula el eje con textos fijos ("5 mins ago" en
`CpuDetail`, "2 mins ago" en `RamDetail`) que no concuerdan entre sí ni con el buffer
real — 60 puntos × 2000 ms son 2 minutos. Nosotros calculamos el rótulo a partir del
intervalo configurado, así que sigue siendo correcto si se cambia.

### 3.5 GPU: la única excepción al "sin forks"

- **AMD:** `gpu_busy_percent`, `gt_cur_freq_mhz`/`pp_dpm_sclk`, `mem_info_vram_*` y
  `hwmon/temp1_input` bajo `/sys/class/drm/card*/device`. Sin forks.
- **NVIDIA:** no hay fuente sysfs equivalente; la única es `nvidia-smi`. Se conserva ese
  fork **condicionado a que el panel de GPU esté abierto y `nvidia-smi` exista**.
- **Intel (el caso de `casa`, UHD 630):** no expone `gpu_busy_percent` y la vía i915 PMU
  exige capacidades de `perf`. La sección se auto-oculta, como hace el plasmoid cuando no
  hay datos, y no se ejecuta nada.

### 3.6 Config y ciclo de vida

`update_interval` cambia el período de la `Subscription`. Cambiar `show_*` o
`section_order` redibuja la vista compacta. Si la sección abierta en el popup se apaga
por config, el popup cae a la primera sección visible.

### 3.7 Top de procesos: cinco, y una decisión de semántica

Las tres listas (CPU, RAM, VRAM) muestran **cinco** procesos, como el plasmoid
(`count == 5` en los dos comandos `ps`, `head -5` en el de VRAM).

El plasmoid filtra de la lista los nombres `ps|awk|sh|bash|dash|zsh`. Ese filtro existe
sólo porque *él mismo* los crea al hacer `ps -eo ... | awk ...`; leyendo `/proc` no
generamos esos procesos, así que el filtro no se replica.

**Desvío deliberado, y el más visible de todos:** `ps -eo pcpu` devuelve el promedio de
CPU sobre *toda la vida del proceso*, no el uso instantáneo. Un proceso que quemó CPU
hace una hora y ahora duerme sigue apareciendo alto. Calculando por delta de
`/proc/N/stat` entre dos muestras obtenemos el uso **instantáneo**, que es lo que la
lista aparenta mostrar y lo que coincide con el total de CPU del mismo panel. Los números
no van a coincidir con los del plasmoid, y ésa es la intención; si se prefiere el promedio
de vida, se cambia acá y en ningún otro lado.

## 4. Tests

Los colectores son parsers puros sobre texto: se capturan `/proc/stat`, `/proc/meminfo`,
`/proc/net/dev`, `/proc/diskstats` y un árbol hwmon reales a `tests/fixtures/`, y se
afirman valores parseados y deltas entre dos muestras consecutivas. Cubre también la
normalización de `section_order` y el cálculo instantáneo de CPU por proceso descrito en
3.7. La UI no se testea unitariamente: se verifica
corriendo el applet en el panel.

## 5. Orden de trabajo

1. `config.rs` + página del engranaje + mostrar/ocultar/reordenar, sobre las secciones
   CPU y RAM que ya existen.
2. Click por sección + ruteo del panel de detalle + pie del popup.
3. `history.rs` + gráfico en `draw.rs`, aplicado a CPU y RAM.
4. Colectores nuevos, uno por vez: temperaturas → red → disco → GPU.
5. Paneles de detalle de cada sección nueva.
6. Top de procesos (CPU, RAM, VRAM), con la recolección condicionada del punto 3.1.

## 6. Fuera de alcance

- Traducciones: el applet queda en español, como está hoy.
- Métricas que el plasmoid no tiene (batería, ventiladores, frecuencia por núcleo).
- Soporte de i915 PMU para uso de GPU en Intel.
