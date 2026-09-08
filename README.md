# cosmic-applet-sysmon

Applet de panel para el escritorio COSMIC que muestra el uso de **CPU por núcleo**
en barras verticales de colores y el uso de **RAM** en un medidor que cambia de
color por umbral, con sus porcentajes al lado.

Es una réplica de la vista compacta del plasmoid de KDE
[`com.labatata.sysmonitor`](https://github.com/LaBatata101/sysmonitor-kde-plasmoid)
para quienes migran de Plasma a COSMIC y quieren el mismo indicador en la barra:
al momento de escribir esto ningún applet de COSMIC dibuja una barra por núcleo
con un color distinto cada una.

- Muestreo cada 2 s desde `/proc/stat` y `/proc/meminfo`, con las mismas fórmulas
  que el plasmoid (ver `src/proc.rs`): la CPU se calcula por delta contra la
  muestra anterior con `iowait` contando como tiempo ocioso, y la RAM usada es
  `MemTotal - (MemFree + Buffers + Cached + SReclaimable - Shmem)`.
- Geometría, colores y umbrales calcados de `CompactView.qml` y
  `VerticalUsageMeter.qml` (ver `src/draw.rs`): barras de 4 px por núcleo a
  tamaño de ícono 16, borde al 35 % del color de texto, y para la RAM azul hasta
  el 70 %, ámbar hasta el 85 % y rojo por encima.
- Al hacer click abre un popup con el detalle por núcleo, RAM usada/total, caché
  y swap.
- Se adapta al tamaño y a la orientación del panel, y al tema claro/oscuro.

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

GPL-3.0-only. El diseño de la vista compacta y los íconos SVG (`res/icons/`)
vienen del plasmoid `com.labatata.sysmonitor` de LaBatata101, también GPL-3.0.
