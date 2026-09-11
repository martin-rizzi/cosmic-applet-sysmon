# cosmic-applet-sysmon

Applet de panel para el escritorio COSMIC que muestra **CPU por núcleo**, **RAM**,
**red**, **disco**, **temperaturas** y **GPU**, cada sección con su popup de detalle.

Es una réplica del plasmoid de KDE
[`com.labatata.sysmonitor`](https://github.com/LaBatata101/sysmonitor-kde-plasmoid)
para quienes migran de Plasma a COSMIC y quieren el mismo indicador en la barra:
al momento de escribir esto ningún applet de COSMIC dibuja una barra por núcleo
con un color distinto cada una.

- Muestreo cada 2 s (configurable) desde `/proc` y `/sys`, con las mismas fórmulas
  que el plasmoid (ver `src/metrics/`): la CPU se calcula por delta contra la
  muestra anterior con `iowait` contando como tiempo ocioso, y la RAM usada es
  `MemTotal - (MemFree + Buffers + Cached + SReclaimable - Shmem)`.
- Geometría, colores y umbrales calcados de `CompactView.qml` y
  `VerticalUsageMeter.qml` (ver `src/draw.rs`): barras de 4 px por núcleo a
  tamaño de ícono 16, borde al 35 % del color de texto, y para la RAM azul hasta
  el 70 %, ámbar hasta el 85 % y rojo por encima.
- Cada sección del panel es su propio botón: abre el popup en esa sección, lo cierra
  si ya la mostraba o cambia de sección sin cerrarlo. El engranaje del popup abre
  los ajustes (qué secciones mostrar, su orden y el intervalo).
- Se adapta al tamaño y a la orientación del panel, y al tema claro/oscuro.

## Secciones

- **CPU**: una barra por núcleo en el panel; en el popup, usuario/sistema, historial, núcleos,
  top de procesos (uso instantáneo) y tiempo encendido.
- **RAM**: medidor con color por umbral; en el popup, libre/en caché, top de procesos y swap.
- **Red**: mini gráfico de subida y bajada con las tasas; en el popup, el gráfico completo.
- **Disco**: uso del primer punto de montaje; en el popup, lectura/escritura y dispositivos.
- **Temperaturas**: las dos primeras lecturas de hwmon; en el popup, todos los sensores.
- **GPU**: uso de la placa más cargada (AMD por sysfs, NVIDIA por `nvidia-smi` con el popup
  abierto); se oculta si no hay ninguna fuente de datos.

Todo sale de `/proc` y `/sys`; lo caro (top de procesos, VRAM, inventario de discos) sólo se
mide con su popup abierto.

## Compilar

Necesita la toolchain de Rust y los headers de desarrollo de libcosmic:

```sh
sudo dnf install rust cargo gcc pkgconf-pkg-config libxkbcommon-devel \
    wayland-devel fontconfig-devel freetype-devel systemd-devel expat-devel
cargo build --release
```

En Fedora, si no querés instalar nada de eso en el sistema, se puede compilar
dentro de un contenedor `toolbox` (podman rootless, sin sudo en el host); el
binario resultante corre en el host porque las librerías en runtime ya vienen
con COSMIC:

```sh
toolbox create -i registry.fedoraproject.org/fedora-toolbox:44 cosmic-build
toolbox run -c cosmic-build sudo dnf install -y rust cargo gcc \
    pkgconf-pkg-config libxkbcommon-devel wayland-devel fontconfig-devel \
    freetype-devel systemd-devel expat-devel
toolbox run -c cosmic-build cargo build --release
```

## Instalar

```sh
./install.sh          # instala en ~/.local (o PREFIX=/usr/local ./install.sh)
```

Después hay que agregar el id `io.github.martin_rizzi.CosmicSysMon` al panel,
desde Ajustes → Escritorio → Panel, o a mano en
`~/.config/cosmic/com.system76.CosmicPanel.Panel/v1/plugins_wings`.

Si cambiás el binario con el applet ya en el panel, COSMIC no lo relanza solo:
sacalo de `plugins_wings`, esperá unos segundos y volvé a agregarlo (escribiendo
el archivo de forma atómica, porque `cosmic-config` lo vigila y una redirección
`>` se lo deja leer vacío a mitad de camino).

## Créditos y licencia

GPL-3.0-only. El diseño y los íconos SVG (`res/icons/`)
vienen del plasmoid `com.labatata.sysmonitor` de LaBatata101, también GPL-3.0.
