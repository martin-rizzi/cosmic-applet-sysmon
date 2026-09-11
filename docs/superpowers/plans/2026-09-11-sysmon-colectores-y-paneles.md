# Colectores y paneles: temperaturas, red, disco, GPU y procesos — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Completar la paridad con el plasmoid: colectores de temperaturas, red, disco, GPU y
procesos leyendo `/proc` y `/sys`, sus secciones en el panel y los seis paneles de detalle.

**Architecture:** Un agregado `metrics::Metrics` reemplaza los campos sueltos de `SysMon` y
decide en cada tick qué se refresca: lo barato siempre, lo caro sólo con su panel de detalle a
la vista (spec §3.1). Cada colector separa el parseo puro (testeado con fixtures reales) del
acceso al filesystem. La UI se reparte en `ui/compact.rs` (panel) y `ui/detail/{cpu,ram,net,
storage,temp,gpu}.rs` (popup), con helpers comunes en `ui/detail/mod.rs`.

**Tech Stack:** Rust 2021, libcosmic (rev `a5267e623f356d1792cba7f480dd671cc8b52406`),
`cosmic-config`, `serde`, `rustix` 1.1 (ya en el árbol; se declara directo con `fs` + `param`).

**Spec:** `docs/superpowers/specs/2026-09-10-sysmon-paridad-plasmoid-design.md`
**Plan anterior:** `docs/superpowers/plans/2026-09-10-sysmon-fundacion-config-y-secciones.md`

## Global Constraints

- **Sin subprocesos en el muestreo.** Única excepción: `nvidia-smi`, sólo con el panel de GPU a
  la vista y si el binario está en el PATH (spec §3.5).
- **Lo caro sólo con su panel a la vista** (spec §3.1): top de procesos, VRAM por proceso,
  `/proc/cpuinfo` + uptime, todos los `statvfs` e inventario de `/sys/block`.
- **Idioma:** textos y comentarios en español. Unidades como el plasmoid ("KB", "MB", "GB").
- **Licencia:** todo archivo nuevo empieza con `// SPDX-License-Identifier: GPL-3.0-only`.
- **libcosmic pinneado:** no cambiar el `rev`.
- **Top de procesos:** cinco por lista (spec §3.7); CPU instantánea por delta.
- **Historial:** 60 puntos (`metrics::history::CAPACITY`).
- **Build y test** (no hay cargo en el host):
  ```sh
  toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo test'
  ```
- **Warnings:** los `dead_code` de API que consume una tarea posterior se toleran hasta la
  Task 11; la Task 12 exige `cargo build --release` sin ningún warning.
- **Commits:** uno por tarea, en español, **sin** `Co-Authored-By`.

## Estructura de archivos

| Archivo | Responsabilidad |
|---|---|
| `src/format.rs` | formateadores de `main.qml` (tasas, memoria, discos, reloj, uptime) |
| `src/exec.rs` | búsqueda de ejecutables en el PATH (monitor de sistema, `nvidia-smi`) |
| `src/metrics/mod.rs` | `Metrics`, `Expensive`, `expensive_for`, `read_trimmed` |
| `src/metrics/rate.rs` | tasa por delta entre dos lecturas acumuladas |
| `src/metrics/cpu.rs` | + user/system del agregado, `total_jiffies` |
| `src/metrics/cpuinfo.rs` | modelo y reloj de `/proc/cpuinfo`, `/proc/uptime` |
| `src/metrics/mem.rs` | + campo `free` |
| `src/metrics/temp.rs` | `/sys/class/hwmon` |
| `src/metrics/net.rs` | `/proc/net/dev` |
| `src/metrics/disk.rs` | `/proc/diskstats`, `/proc/self/mounts` + `statvfs`, `/sys/block` |
| `src/metrics/gpu.rs` | sysfs de amdgpu, `pci.ids`, `nvidia-smi` |
| `src/metrics/procs.rs` | `/proc/N/stat` y `/proc/N/fdinfo` |
| `src/draw.rs` | + medidor con relleno mínimo, gráfico doble, color desde hex |
| `src/ui/compact.rs` | + secciones de temperaturas, red, disco y GPU |
| `src/ui/detail/mod.rs` | ruteo, pie y widgets comunes |
| `src/ui/detail/{cpu,ram,net,storage,temp,gpu}.rs` | un panel por sección |
| `res/icons/am-{temperature,network,harddisk,gpu,up,down}-symbolic.svg` | íconos del plasmoid |
| `tests/fixtures/…` | capturas de `/proc` y árboles de `/sys` |

---

### Task 1: Agregado `Metrics` y selección de lo caro

Hoy `SysMon` guarda `cpu`, `mem` y dos historiales sueltos, y el tick los refresca a mano.
Cada colector nuevo sumaría campos y ramas a `app.rs`. Esta tarea los mueve a un agregado que
recibe qué panel está a la vista y decide qué refrescar. Sin cambio visible.

**Files:**
- Modify: `src/metrics/mod.rs`, `src/app.rs`, `src/ui/compact.rs`, `src/ui/detail/mod.rs`

**Interfaces:**
- Consumes: `metrics::cpu::Cpu`, `metrics::mem::Mem`, `metrics::history::History`, `config::Section`.
- Produces:
  - `metrics::Expensive { cpu_detail, ram_detail, storage_detail, gpu_detail: bool }`
  - `metrics::expensive_for(detail: Option<Section>) -> Expensive`
  - `metrics::Metrics` (`Default`) con campos públicos `cpu: Cpu`, `mem: Mem`,
    `cpu_history: History`, `ram_history: History`, y
    `fn tick(&mut self, now: Instant, detail: Option<Section>)`
  - `SysMon::metrics(&self) -> &Metrics`, `SysMon::visible_detail(&self) -> Option<Section>`
  - Se borran `SysMon::cpu()`, `mem()`, `cpu_history()`, `ram_history()`.

- [ ] **Step 1: Tests que fallan**

Al final de `src/metrics/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_panel_a_la_vista_no_corre_nada_caro() {
        assert_eq!(expensive_for(None), Expensive::default());
    }

    #[test]
    fn cada_panel_prende_solo_lo_suyo() {
        assert_eq!(
            expensive_for(Some(Section::Cpu)),
            Expensive { cpu_detail: true, ..Expensive::default() }
        );
        assert_eq!(
            expensive_for(Some(Section::Ram)),
            Expensive { ram_detail: true, ..Expensive::default() }
        );
        assert_eq!(
            expensive_for(Some(Section::Storage)),
            Expensive { storage_detail: true, ..Expensive::default() }
        );
        assert_eq!(
            expensive_for(Some(Section::Gpu)),
            Expensive { gpu_detail: true, ..Expensive::default() }
        );
    }

    #[test]
    fn red_y_temperaturas_no_tienen_parte_cara() {
        assert_eq!(expensive_for(Some(Section::Network)), Expensive::default());
        assert_eq!(expensive_for(Some(Section::Temps)), Expensive::default());
    }
}
```

Y en el bloque de tests de `src/app.rs`:

```rust
#[test]
fn visible_detail_sin_popup_es_none() {
    let app = con_popup_cerrado(Section::Cpu, false);
    assert_eq!(app.visible_detail(), None);
}

#[test]
fn visible_detail_con_ajustes_es_none() {
    let (app, _id) = con_popup_abierto(Section::Ram, true);
    assert_eq!(app.visible_detail(), None);
}

#[test]
fn visible_detail_con_popup_es_la_seccion_elegida() {
    let (app, _id) = con_popup_abierto(Section::Ram, false);
    assert_eq!(app.visible_detail(), Some(Section::Ram));
}
```

- [ ] **Step 2: Verificar que fallan**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo test'`
Expected: FAIL de compilación — `expensive_for`, `Expensive` y `visible_detail` no existen.

- [ ] **Step 3: Implementar `src/metrics/mod.rs`**

Reemplazar el archivo entero (se conservan los `pub mod` existentes; los tests del Step 1 van al final):

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! Colectores y el agregado que decide qué se refresca en cada tick.

pub mod cpu;
pub mod history;
pub mod mem;

use std::time::Instant;

use crate::config::Section;
use cpu::Cpu;
use history::History;
use mem::Mem;

/// Qué parte cara del muestreo corre (spec §3.1). Lo que no figura acá es barato y corre
/// en cada tick, porque lo usa la vista del panel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Expensive {
    /// Top de procesos por CPU, modelo y reloj de /proc/cpuinfo, uptime.
    pub cpu_detail: bool,
    /// Top de procesos por memoria.
    pub ram_detail: bool,
    /// Todos los puntos de montaje e inventario de /sys/block.
    pub storage_detail: bool,
    /// `nvidia-smi` y VRAM por proceso.
    pub gpu_detail: bool,
}

/// Lo caro corre sólo mientras su panel de detalle está a la vista.
pub fn expensive_for(detail: Option<Section>) -> Expensive {
    let mut e = Expensive::default();
    match detail {
        Some(Section::Cpu) => e.cpu_detail = true,
        Some(Section::Ram) => e.ram_detail = true,
        Some(Section::Storage) => e.storage_detail = true,
        Some(Section::Gpu) => e.gpu_detail = true,
        Some(Section::Network) | Some(Section::Temps) | None => {}
    }
    e
}

#[derive(Default)]
pub struct Metrics {
    pub cpu: Cpu,
    pub mem: Mem,
    /// Últimos 60 usos totales de CPU (0–1), uno por tick.
    pub cpu_history: History,
    /// Últimas 60 fracciones de RAM usada, una por tick.
    pub ram_history: History,
    last_tick: Option<Instant>,
}

impl Metrics {
    /// Un tick de muestreo. `detail` es la sección cuyo panel está a la vista, si hay alguna.
    pub fn tick(&mut self, now: Instant, detail: Option<Section>) {
        let _elapsed = self.elapsed_since_last(now);
        self.cpu.refresh();
        self.mem.refresh();
        self.cpu_history.push(self.cpu.total / 100.0);
        self.ram_history.push(self.mem.fraction());
        self.refresh_expensive(detail);
    }

    /// La parte cara, sola. `SysMon` la llama también al abrir un panel, para que no
    /// muestre la lista vacía hasta el próximo tick.
    pub fn refresh_expensive(&mut self, detail: Option<Section>) {
        let _expensive = expensive_for(detail);
    }

    /// Segundos desde el tick anterior; 0 en el primero.
    fn elapsed_since_last(&mut self, now: Instant) -> f64 {
        let elapsed = self
            .last_tick
            .map(|t| now.duration_since(t).as_secs_f64())
            .unwrap_or(0.0);
        self.last_tick = Some(now);
        elapsed
    }
}
```

`_elapsed` y `_expensive` quedan sin uso a propósito: los consumen las Tasks 3–8.

- [ ] **Step 4: Cablear `SysMon`**

En `src/app.rs`:

1. Imports: sacar `use crate::metrics::cpu::Cpu;`, `use crate::metrics::history::History;` y
   `use crate::metrics::mem::Mem;`; agregar `use crate::metrics::Metrics;` y
   `use std::time::Instant;` (junto a `use std::time::Duration;`).
2. En `struct SysMon`, reemplazar los campos `cpu`, `mem`, `cpu_history` y `ram_history`
   (con sus doc comments) por:
   ```rust
   /// Todos los colectores y sus historiales.
   metrics: Metrics,
   ```
3. Reemplazar los accesores `cpu()`, `mem()`, `cpu_history()` y `ram_history()` por:
   ```rust
   pub fn metrics(&self) -> &Metrics {
       &self.metrics
   }

   /// La sección cuyo panel de detalle se está viendo: popup abierto y no en ajustes.
   pub fn visible_detail(&self) -> Option<Section> {
       if self.popup.is_some() && !self.showing_settings {
           Some(self.selected)
       } else {
           None
       }
   }
   ```
4. En `init`, los cuatro campos pasan a `metrics: Metrics::default(),` y las dos líneas
   `app.cpu.refresh(); app.mem.refresh();` (con su comentario) pasan a:
   ```rust
   // Primera lectura: deja las líneas base de los contadores.
   app.metrics.tick(Instant::now(), None);
   ```
5. El brazo `Message::Tick` queda:
   ```rust
   Message::Tick => {
       self.metrics.tick(Instant::now(), self.visible_detail());
       Task::none()
   }
   ```
6. En el brazo `Message::OpenSection`, justo antes de cada `return`/expresión final que deja
   el popup mostrando una sección, no hace falta nada todavía: el refresco inmediato se agrega
   en la Task 3.
7. En los helpers de test `con_popup_abierto` y `con_popup_cerrado`, los cuatro campos pasan a
   `metrics: Metrics::default(),`.

- [ ] **Step 5: Actualizar a quienes leían los campos**

`src/ui/compact.rs`: `app.cpu()` → `&app.metrics().cpu` (dos lugares) y `app.mem()` →
`&app.metrics().mem`.

`src/ui/detail/mod.rs`: `app.cpu()` → `&app.metrics().cpu`, `app.cpu_history()` →
`&app.metrics().cpu_history`, `app.mem()` → `&app.metrics().mem`, `app.ram_history()` →
`&app.metrics().ram_history`.

- [ ] **Step 6: Verificar que pasan**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo test'`
Expected: PASS, 47 tests (41 previos + 3 de metrics + 3 de app).

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "Agregado Metrics que decide qué muestrear en cada tick

Mueve los colectores y los historiales de SysMon a metrics::Metrics y
agrega expensive_for, que prende la parte cara sólo con su panel a la
vista. Sin cambio visible."
```

---

### Task 2: Formateadores del plasmoid

**Files:**
- Create: `src/format.rs`
- Modify: `src/main.rs` (`mod format;`)

**Interfaces:**
- Produces (todas `pub fn … -> String`): `format::rate(f64)`, `compact_rate(f64)`,
  `memory_kib(f64)`, `memory_mib(f64)`, `gb2(mib: f64)`, `storage_bytes(f64)`,
  `percent(f32)`, `clock(mhz: f64)`, `uptime(secs: u64)`.

- [ ] **Step 1: Tests que fallan**

`src/format.rs` con sólo este bloque (más el encabezado SPDX):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_cambia_de_unidad_como_el_qml() {
        assert_eq!(rate(512.0), "512 B/s");
        assert_eq!(rate(1_024_000.0), "1000.0 KB/s");
        assert_eq!(rate(3_145_728.0), "3.0 MB/s");
    }

    #[test]
    fn compact_rate_rellena_los_bytes_a_cuatro() {
        assert_eq!(compact_rate(0.0), "0   B/s");
        assert_eq!(compact_rate(512.0), "512 B/s");
        assert_eq!(compact_rate(20_480.0), "20 KB/s");
        assert_eq!(compact_rate(3_145_728.0), "3.0 MB/s");
    }

    #[test]
    fn memory_kib_usa_un_decimal_solo_debajo_de_10_mb() {
        assert_eq!(memory_kib(512.0), "512 KB");
        assert_eq!(memory_kib(2048.0), "2.0 MB");
        assert_eq!(memory_kib(20_480.0), "20 MB");
        assert_eq!(memory_kib(2_097_152.0), "2.0 GB");
    }

    #[test]
    fn memory_mib_usa_un_decimal_solo_debajo_de_100_mb() {
        assert_eq!(memory_mib(50.0), "50.0 MB");
        assert_eq!(memory_mib(512.0), "512 MB");
        assert_eq!(memory_mib(2048.0), "2.0 GB");
    }

    #[test]
    fn gb2_es_el_renglon_de_ram() {
        assert_eq!(gb2(31_931.277), "31.18 GB");
    }

    #[test]
    fn storage_bytes_llega_a_terabytes() {
        assert_eq!(storage_bytes(500.0), "500 B");
        assert_eq!(storage_bytes(500_107_862_016.0), "465.8 GB");
        assert_eq!(storage_bytes(2_199_023_255_552.0), "2.0 TB");
    }

    #[test]
    fn percent_usa_un_decimal_solo_debajo_de_uno() {
        assert_eq!(percent(0.5), "0.5%");
        assert_eq!(percent(42.4), "42%");
    }

    #[test]
    fn clock_vacio_sin_dato() {
        assert_eq!(clock(0.0), "");
        assert_eq!(clock(800.0), "800 MHz");
        assert_eq!(clock(3222.964), "3.22 GHz");
    }

    #[test]
    fn uptime_omite_las_unidades_en_cero() {
        assert_eq!(uptime(30), "0 minutos");
        assert_eq!(uptime(1417), "23 minutos");
        assert_eq!(uptime(90_061), "1 día, 1 hora, 1 minuto");
        assert_eq!(uptime(1_300_000), "2 semanas, 1 día, 1 hora, 6 minutos");
    }
}
```

Agregar `mod format;` a `src/main.rs` (en orden alfabético, después de `mod draw;`).

- [ ] **Step 2: Verificar que fallan**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo test format'`
Expected: FAIL — las funciones no existen.

- [ ] **Step 3: Implementar**

Arriba del bloque de tests:

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! Formateadores de `main.qml` del plasmoid. Los cortes de unidad usan las mismas
//! constantes que el QML (`1.04858e+06`, `1.07374e+09`, `1.09951e+12`) en vez de potencias
//! exactas de 1024, para que cambien de unidad en el mismo valor.

const KIB: f64 = 1024.0;
const MIB: f64 = 1.04858e6;
const GIB: f64 = 1.07374e9;
const TIB: f64 = 1.09951e12;

/// `formatRate`.
pub fn rate(bytes_per_sec: f64) -> String {
    if bytes_per_sec < KIB {
        format!("{bytes_per_sec:.0} B/s")
    } else if bytes_per_sec < MIB {
        format!("{:.1} KB/s", bytes_per_sec / KIB)
    } else {
        format!("{:.1} MB/s", bytes_per_sec / MIB)
    }
}

/// `formatCompactRate`: los bytes se rellenan a cuatro caracteres para que el panel no
/// cambie de ancho en cada tick.
pub fn compact_rate(bytes_per_sec: f64) -> String {
    if bytes_per_sec < KIB {
        let value = format!("{bytes_per_sec:.0}");
        format!("{value:<4}B/s")
    } else if bytes_per_sec < MIB {
        format!("{:.0} KB/s", bytes_per_sec / KIB)
    } else {
        format!("{:.1} MB/s", bytes_per_sec / MIB)
    }
}

/// `formatMemoryKib`.
pub fn memory_kib(kib: f64) -> String {
    if kib < KIB {
        format!("{kib:.0} KB")
    } else if kib < MIB {
        if kib < 10_240.0 {
            format!("{:.1} MB", kib / KIB)
        } else {
            format!("{:.0} MB", kib / KIB)
        }
    } else {
        format!("{:.1} GB", kib / MIB)
    }
}

/// `formatMemoryMib`.
pub fn memory_mib(mib: f64) -> String {
    if mib < KIB {
        if mib < 100.0 {
            format!("{mib:.1} MB")
        } else {
            format!("{mib:.0} MB")
        }
    } else {
        format!("{:.1} GB", mib / KIB)
    }
}

/// Renglones de RAM y swap de `RamDetail.qml`: `(mib / 1024).toFixed(2) + " GB"`.
pub fn gb2(mib: f64) -> String {
    format!("{:.2} GB", mib / KIB)
}

/// `formatStorageBytes`.
pub fn storage_bytes(bytes: f64) -> String {
    if bytes < KIB {
        format!("{bytes:.0} B")
    } else if bytes < MIB {
        format!("{:.1} KB", bytes / KIB)
    } else if bytes < GIB {
        format!("{:.1} MB", bytes / MIB)
    } else if bytes < TIB {
        format!("{:.1} GB", bytes / GIB)
    } else {
        format!("{:.1} TB", bytes / TIB)
    }
}

/// `formatPercent`: un decimal sólo por debajo del 1 %.
pub fn percent(p: f32) -> String {
    if p < 1.0 {
        format!("{p:.1}%")
    } else {
        format!("{p:.0}%")
    }
}

/// `formatCpuClock`. Vacío sin dato, para que quien llama elija el reemplazo.
pub fn clock(mhz: f64) -> String {
    if mhz <= 0.0 {
        String::new()
    } else if mhz < 1000.0 {
        format!("{mhz:.0} MHz")
    } else {
        format!("{:.2} GHz", mhz / 1000.0)
    }
}

/// Lo que muestra `uptime -p` sin el "up", en español: las unidades en cero se omiten.
pub fn uptime(secs: u64) -> String {
    const UNITS: [(u64, &str, &str); 4] = [
        (604_800, "semana", "semanas"),
        (86_400, "día", "días"),
        (3_600, "hora", "horas"),
        (60, "minuto", "minutos"),
    ];
    let mut rest = secs;
    let mut parts = Vec::new();
    for (size, one, many) in UNITS {
        let n = rest / size;
        rest %= size;
        if n > 0 {
            parts.push(format!("{n} {}", if n == 1 { one } else { many }));
        }
    }
    if parts.is_empty() {
        "0 minutos".to_string()
    } else {
        parts.join(", ")
    }
}
```

- [ ] **Step 4: Verificar que pasan**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo test format'`
Expected: PASS, 9 tests.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "Formateadores de tasas, memoria, discos, reloj y uptime

Calcados de main.qml, con las mismas constantes de corte de unidad."
```

---
### Task 3: CPU — user/system, modelo, reloj y uptime

**Files:**
- Create: `src/metrics/cpuinfo.rs`, `tests/fixtures/proc_cpuinfo`
- Modify: `src/metrics/cpu.rs`, `src/metrics/mod.rs`, `src/app.rs`

**Interfaces:**
- Consumes: `Metrics::refresh_expensive`, `expensive_for` (Task 1).
- Produces:
  - `CpuTicks` suma `pub user: u64` (user + nice) y `pub system: u64`
  - `Cpu` suma `pub user: f32` y `pub system: f32` (porcentajes del agregado)
  - `metrics::cpuinfo::CpuInfo { pub model: String, pub mhz: f64 }` con `fn refresh(&mut self)`
  - `cpuinfo::parse_cpuinfo(&str) -> CpuInfo`, `cpuinfo::parse_uptime(&str) -> u64`,
    `cpuinfo::read_uptime() -> u64`
  - `Metrics` suma `pub cpu_info: CpuInfo` y `pub uptime_secs: u64`, refrescados sólo con
    `cpu_detail`
  - `SysMon` refresca la parte cara apenas cambia el panel a la vista (no espera al tick)

- [ ] **Step 1: Fixture**

`tests/fixtures/proc_cpuinfo` — captura real recortada a dos procesadores:

```
processor	: 0
vendor_id	: GenuineIntel
model name	: Intel(R) Core(TM) i5-10400T CPU @ 2.00GHz
cpu MHz		: 3214.864
flags		: fpu vme de pse
processor	: 1
vendor_id	: GenuineIntel
model name	: Intel(R) Core(TM) i5-10400T CPU @ 2.00GHz
cpu MHz		: 3231.064
flags		: fpu vme de pse
```

- [ ] **Step 2: Tests que fallan**

En el bloque de tests de `src/metrics/cpu.rs`:

```rust
#[test]
fn user_y_system_del_agregado() {
    // En los fixtures sólo avanza `user` (+25 de 100).
    let mut cpu = Cpu::default();
    cpu.apply(parse_stat(STAT_A));
    cpu.apply(parse_stat(STAT_B));
    assert_eq!(cpu.user, 25.0);
    assert_eq!(cpu.system, 0.0);
}

#[test]
fn user_suma_nice_y_system_va_aparte() {
    let mut cpu = Cpu::default();
    cpu.apply(vec![CpuTicks { total: 1000, active: 0, user: 0, system: 0 }]);
    cpu.apply(vec![CpuTicks { total: 1200, active: 100, user: 60, system: 40 }]);
    assert_eq!(cpu.total, 50.0);
    // 60 / 200 en f32 da 30.000002: se compara con tolerancia.
    assert!((cpu.user - 30.0).abs() < 1e-4, "{}", cpu.user);
    assert!((cpu.system - 20.0).abs() < 1e-4, "{}", cpu.system);
}
```

`src/metrics/cpuinfo.rs` con este bloque:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const CPUINFO: &str = include_str!("../../tests/fixtures/proc_cpuinfo");

    #[test]
    fn toma_el_primer_modelo_y_promedia_el_reloj() {
        let info = parse_cpuinfo(CPUINFO);
        assert_eq!(info.model, "Intel(R) Core(TM) i5-10400T CPU @ 2.00GHz");
        assert!((info.mhz - 3222.964).abs() < 0.001);
    }

    #[test]
    fn sin_model_name_queda_vacio() {
        // ARM expone `processor` y `BogoMIPS`, sin `model name` ni `cpu MHz`.
        let info = parse_cpuinfo("processor\t: 0\nBogoMIPS\t: 108.00\n");
        assert_eq!(info, CpuInfo::default());
    }

    #[test]
    fn uptime_toma_los_segundos_enteros() {
        assert_eq!(parse_uptime("1417.16 14830.28\n"), 1417);
        assert_eq!(parse_uptime(""), 0);
    }
}
```

Agregar `pub mod cpuinfo;` a `src/metrics/mod.rs`.

- [ ] **Step 3: Verificar que fallan**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo test cpu'`
Expected: FAIL — `CpuTicks` no tiene `user`/`system`, `cpuinfo` no existe.

- [ ] **Step 4: user/system en `src/metrics/cpu.rs`**

`CpuTicks` pasa a:

```rust
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct CpuTicks {
    pub total: u64,
    pub active: u64,
    /// user + nice, como `pctUser` en `parseCpuStat`.
    pub user: u64,
    pub system: u64,
}
```

En `parse_stat`, el `push` pasa a:

```rust
out.push(CpuTicks {
    total: active + idle + iowait,
    active,
    user: user + nice,
    system,
});
```

`Cpu` suma los campos:

```rust
#[derive(Default)]
pub struct Cpu {
    prev: Vec<CpuTicks>,
    pub cores: Vec<f32>,
    pub total: f32,
    /// Porcentaje del agregado en modo usuario (incluye nice).
    pub user: f32,
    /// Porcentaje del agregado en modo kernel.
    pub system: f32,
}
```

En `apply`, antes de `self.total = pcts[0];`:

```rust
let (user, system) = match self.prev.first() {
    Some(prev) if now[0].total > prev.total => {
        let dt = (now[0].total - prev.total) as f32;
        let pct = |now: u64, before: u64| {
            (now.saturating_sub(before) as f32 / dt * 100.0).clamp(0.0, 100.0)
        };
        (pct(now[0].user, prev.user), pct(now[0].system, prev.system))
    }
    _ => (0.0, 0.0),
};
self.user = user;
self.system = system;
```

- [ ] **Step 5: `src/metrics/cpuinfo.rs`**

Arriba del bloque de tests:

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! Modelo y reloj de /proc/cpuinfo, y uptime de /proc/uptime. Sólo los muestra el panel
//! de CPU, así que sólo se leen con ese panel a la vista.

use std::fs;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CpuInfo {
    /// Primer `model name`; vacío donde no existe (ARM).
    pub model: String,
    /// Promedio de los `cpu MHz` positivos; 0 si no hay ninguno.
    pub mhz: f64,
}

pub fn parse_cpuinfo(raw: &str) -> CpuInfo {
    let mut info = CpuInfo::default();
    let (mut sum, mut count) = (0.0, 0u32);
    for line in raw.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        match key.trim() {
            "model name" if info.model.is_empty() => info.model = value.trim().to_string(),
            "cpu MHz" => {
                if let Ok(mhz) = value.trim().parse::<f64>() {
                    if mhz > 0.0 {
                        sum += mhz;
                        count += 1;
                    }
                }
            }
            _ => {}
        }
    }
    if count > 0 {
        info.mhz = sum / f64::from(count);
    }
    info
}

impl CpuInfo {
    pub fn refresh(&mut self) {
        if let Ok(raw) = fs::read_to_string("/proc/cpuinfo") {
            *self = parse_cpuinfo(&raw);
        }
    }
}

/// Segundos enteros del primer campo de /proc/uptime.
pub fn parse_uptime(raw: &str) -> u64 {
    raw.split_whitespace()
        .next()
        .and_then(|v| v.parse::<f64>().ok())
        .map(|v| v as u64)
        .unwrap_or(0)
}

pub fn read_uptime() -> u64 {
    fs::read_to_string("/proc/uptime")
        .map(|raw| parse_uptime(&raw))
        .unwrap_or(0)
}
```

- [ ] **Step 6: Cablear en `Metrics`**

En `src/metrics/mod.rs`: `use cpuinfo::CpuInfo;`, los campos

```rust
/// Modelo y reloj; sólo se actualizan con el panel de CPU a la vista.
pub cpu_info: CpuInfo,
/// Segundos desde el arranque; ídem.
pub uptime_secs: u64,
```

y `refresh_expensive` pasa a:

```rust
pub fn refresh_expensive(&mut self, detail: Option<Section>) {
    let expensive = expensive_for(detail);
    if expensive.cpu_detail {
        self.cpu_info.refresh();
        self.uptime_secs = cpuinfo::read_uptime();
    }
}
```

- [ ] **Step 7: Refrescar al abrir o cambiar de panel**

En `src/app.rs`, el popup se crea de forma diferida (el closure de `open_popup_task` corre
cuando la surface existe), así que hay dos lugares:

1. En `open_popup_task`, dentro del closure, después de `app.popup.replace(new_id);`:
   ```rust
   // El popup recién existe acá: que el panel no espere al próximo tick para
   // tener sus datos caros.
   let detail = app.visible_detail();
   app.metrics.refresh_expensive(detail);
   ```
2. Renombrar el `fn update` del trait a un método inherente privado
   `fn handle(&mut self, message: Message) -> Task<Message>` (mismo cuerpo) dentro de
   `impl SysMon`, y dejar en el trait:
   ```rust
   fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
       let before = self.visible_detail();
       let task = self.handle(message);
       let after = self.visible_detail();
       // Cambio de sección con el popup abierto, o salida de ajustes: el panel
       // nuevo carga sus datos caros ya, no en el próximo tick.
       if after.is_some() && after != before {
           self.metrics.refresh_expensive(after);
       }
       task
   }
   ```
   Los tests existentes siguen llamando `cosmic::Application::update`, así que cubren el
   envoltorio.

- [ ] **Step 8: Verificar que pasan**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo test'`
Expected: PASS, 61 tests.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "CPU: user/system, modelo, reloj y uptime

user y system salen del mismo delta del agregado. Modelo, reloj y
uptime se leen sólo con el panel de CPU a la vista, y al abrirlo se
cargan en el momento en vez de esperar al tick."
```

---

### Task 4: Temperaturas desde hwmon

**Files:**
- Create: `src/metrics/temp.rs`, `tests/fixtures/hwmon/…`
- Modify: `src/metrics/mod.rs`

**Interfaces:**
- Consumes: `crate::draw::{CRITICAL, WARNING}`.
- Produces:
  - `metrics::read_trimmed(&Path) -> Option<String>` (`pub(crate)`, lo usan disk y gpu)
  - `metrics::temp::Reading { pub label: String, pub celsius: f32 }`
  - `temp::read_hwmon(root: &Path) -> Vec<Reading>`
  - `temp::severity_color(celsius: f32) -> Option<&'static str>` — `None` = color del tema
  - `Metrics` suma `pub temps: Vec<Reading>`, refrescado en cada tick (lo usa el panel)

- [ ] **Step 1: Fixture**

Árbol que imita `/sys/class/hwmon` de `casa`, más un chip `hwmon10` (orden numérico, no
lexicográfico), un `temp10` (ídem dentro del chip) y un `temp3_input` ilegible (se saltea):

```bash
F=tests/fixtures/hwmon
mkdir -p $F/hwmon0 $F/hwmon1 $F/hwmon2 $F/hwmon10
printf 'acpitz\n' > $F/hwmon0/name
printf '27800\n' > $F/hwmon0/temp1_input
printf 'pch_cometlake\n' > $F/hwmon1/name
printf '41000\n' > $F/hwmon1/temp1_input
printf 'coretemp\n' > $F/hwmon2/name
printf 'Package id 0\n' > $F/hwmon2/temp1_label
printf '33000\n' > $F/hwmon2/temp1_input
printf 'Core 0\n' > $F/hwmon2/temp2_label
printf '31000\n' > $F/hwmon2/temp2_input
printf 'sin-dato\n' > $F/hwmon2/temp3_input
printf 'Core 9\n' > $F/hwmon2/temp10_label
printf '30000\n' > $F/hwmon2/temp10_input
printf 'nvme\n' > $F/hwmon10/name
printf 'Composite\n' > $F/hwmon10/temp1_label
printf '38850\n' > $F/hwmon10/temp1_input
```

- [ ] **Step 2: Tests que fallan**

`src/metrics/temp.rs` con este bloque, y `pub mod temp;` en `src/metrics/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hwmon")
    }

    #[test]
    fn recorre_chips_y_sensores_en_orden_numerico() {
        let r = read_hwmon(&fixture());
        let labels: Vec<&str> = r.iter().map(|x| x.label.as_str()).collect();
        assert_eq!(
            labels,
            vec![
                "acpitz/temp1",
                "pch_cometlake/temp1",
                "coretemp/Package id 0",
                "coretemp/Core 0",
                "coretemp/Core 9",
                "nvme/Composite",
            ]
        );
        let celsius: Vec<f32> = r.iter().map(|x| x.celsius).collect();
        for (got, want) in celsius.iter().zip([27.8, 41.0, 33.0, 31.0, 30.0, 38.85]) {
            assert!((got - want).abs() < 0.001, "{got} != {want}");
        }
    }

    #[test]
    fn sin_hwmon_no_hay_lecturas() {
        assert!(read_hwmon(&fixture().join("no-existe")).is_empty());
    }

    #[test]
    fn colores_por_umbral_como_el_qml() {
        assert_eq!(severity_color(90.1), Some(crate::draw::CRITICAL));
        assert_eq!(severity_color(90.0), Some(crate::draw::WARNING));
        assert_eq!(severity_color(75.1), Some(crate::draw::WARNING));
        assert_eq!(severity_color(75.0), None);
    }
}
```

- [ ] **Step 3: Verificar que fallan**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo test temp'`
Expected: FAIL — `read_hwmon` no existe.

- [ ] **Step 4: `read_trimmed` en `src/metrics/mod.rs`**

Debajo de los `use`:

```rust
/// Contenido de un archivo chico de /proc o /sys, sin espacios ni salto final.
pub(crate) fn read_trimmed(path: &std::path::Path) -> Option<String> {
    std::fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}
```

- [ ] **Step 5: `src/metrics/temp.rs`**

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! Temperaturas desde /sys/class/hwmon, que es lo que lee `sensors` por debajo.
//!
//! El rótulo es `chip/sensor` como en `parseSensors` del plasmoid, con una diferencia:
//! `sensors` le agrega al chip el bus ("coretemp-isa-0000") y hwmon expone sólo el
//! nombre ("coretemp").

use std::fs;
use std::path::{Path, PathBuf};

use super::read_trimmed;

#[derive(Clone, Debug, PartialEq)]
pub struct Reading {
    pub label: String,
    pub celsius: f32,
}

/// El número entre un prefijo y un sufijo: (`hwmon12`, "hwmon", "") → 12.
fn index_between(name: &str, prefix: &str, suffix: &str) -> Option<u32> {
    name.strip_prefix(prefix)?.strip_suffix(suffix)?.parse().ok()
}

/// Entradas de `dir` cuyo nombre es `prefix<N>suffix`, ordenadas por N.
fn numbered(dir: &Path, prefix: &str, suffix: &str) -> Vec<(u32, PathBuf)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<(u32, PathBuf)> = entries
        .flatten()
        .filter_map(|e| {
            let n = index_between(e.file_name().to_str()?, prefix, suffix)?;
            Some((n, e.path()))
        })
        .collect();
    out.sort_by_key(|(n, _)| *n);
    out
}

pub fn read_hwmon(root: &Path) -> Vec<Reading> {
    let mut out = Vec::new();
    for (_, chip) in numbered(root, "hwmon", "") {
        let chip_name = read_trimmed(&chip.join("name")).unwrap_or_else(|| "hwmon".into());
        for (i, input) in numbered(&chip, "temp", "_input") {
            let Some(milli) = read_trimmed(&input).and_then(|v| v.parse::<i64>().ok()) else {
                continue;
            };
            let sensor = read_trimmed(&chip.join(format!("temp{i}_label")))
                .filter(|l| !l.is_empty())
                .unwrap_or_else(|| format!("temp{i}"));
            out.push(Reading {
                label: format!("{chip_name}/{sensor}"),
                celsius: milli as f32 / 1000.0,
            });
        }
    }
    out
}

/// Umbrales de `TempDetail.qml` y `CompactView.qml`: más de 90 °C rojo, más de 75 °C
/// naranja; si no, el color normal del texto.
pub fn severity_color(celsius: f32) -> Option<&'static str> {
    if celsius > 90.0 {
        Some(crate::draw::CRITICAL)
    } else if celsius > 75.0 {
        Some(crate::draw::WARNING)
    } else {
        None
    }
}
```

- [ ] **Step 6: Cablear en `Metrics`**

Campo `pub temps: Vec<temp::Reading>,` con doc `/// Un renglón por sensor, en el orden de hwmon.`
y en `tick`, después de `self.mem.refresh();`:

```rust
self.temps = temp::read_hwmon(std::path::Path::new("/sys/class/hwmon"));
```

- [ ] **Step 7: Verificar que pasan**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo test'`
Expected: PASS, 64 tests.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "Temperaturas desde /sys/class/hwmon

Un renglón por sensor como chip/rótulo, en orden numérico de hwmon y de
sensor, con los umbrales de color del plasmoid."
```

---
### Task 5: Red desde /proc/net/dev

**Files:**
- Create: `src/metrics/rate.rs`, `src/metrics/net.rs`, `tests/fixtures/proc_net_dev_a`,
  `tests/fixtures/proc_net_dev_b`
- Modify: `src/metrics/mod.rs`

**Interfaces:**
- Consumes: `Metrics::elapsed_since_last` (Task 1).
- Produces:
  - `metrics::rate::per_second(prev: Option<(u64, u64)>, now: (u64, u64), elapsed_secs: f64) -> (f64, f64)`
  - `metrics::net::parse_net_dev(&str) -> (u64, u64)` — (rx, tx)
  - `metrics::net::Net { pub rx_rate: f64, pub tx_rate: f64 }` con
    `fn apply(&mut self, sample: (u64, u64), elapsed_secs: f64)`,
    `fn refresh(&mut self, elapsed_secs: f64)`, `fn normalized(&self) -> (f32, f32)` — (rx, tx)
  - `Metrics` suma `pub net: Net`, `pub net_up_history: History`, `pub net_down_history: History`

- [ ] **Step 1: Fixtures**

`tests/fixtures/proc_net_dev_a` — captura real de `casa`:

```
Inter-|   Receive                                                |  Transmit
 face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed
    lo:   70290     581    0    0    0     0          0         0    70290     581    0    0    0     0       0          0
enp1s0:       0       0    0    0    0     0          0         0        0       0    0    0    0     0       0          0
wlp2s0: 149127952  130129    0   39    0     0          0         0 20406898   67367    0    0    0     0       0          0
tailscale0:      86       1    0    0    0     0          0         0    62139     870    0    0    0     0       0          0
docker0:       0       0    0    0    0     0          0         0        0       0    0   37    0     0       0          0
```

`tests/fixtures/proc_net_dev_b` — igual, con `wlp2s0` en rx `151175952` (+2 048 000) y tx
`20407410` (+512), y `lo` en `99999999` en las dos columnas (tiene que ignorarse):

```
Inter-|   Receive                                                |  Transmit
 face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed
    lo: 99999999     581    0    0    0     0          0         0 99999999     581    0    0    0     0       0          0
enp1s0:       0       0    0    0    0     0          0         0        0       0    0    0    0     0       0          0
wlp2s0: 151175952  131000    0   39    0     0          0         0 20407410   67400    0    0    0     0       0          0
tailscale0:      86       1    0    0    0     0          0         0    62139     870    0    0    0     0       0          0
docker0:       0       0    0    0    0     0          0         0        0       0    0   37    0     0       0          0
```

- [ ] **Step 2: Tests que fallan**

`src/metrics/rate.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_primera_lectura_solo_fija_la_base() {
        assert_eq!(per_second(None, (1000, 1000), 2.0), (0.0, 0.0));
    }

    #[test]
    fn divide_el_delta_por_el_intervalo() {
        assert_eq!(per_second(Some((1000, 500)), (3000, 1500), 2.0), (1000.0, 500.0));
    }

    #[test]
    fn contador_que_retrocede_o_intervalo_nulo_dan_cero() {
        assert_eq!(per_second(Some((5000, 5000)), (1000, 6000), 1.0), (0.0, 1000.0));
        assert_eq!(per_second(Some((0, 0)), (1000, 1000), 0.0), (0.0, 0.0));
    }
}
```

`src/metrics/net.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const DEV_A: &str = include_str!("../../tests/fixtures/proc_net_dev_a");
    const DEV_B: &str = include_str!("../../tests/fixtures/proc_net_dev_b");

    #[test]
    fn suma_todas_las_interfaces_menos_lo() {
        // wlp2s0 + tailscale0; enp1s0 y docker0 en cero; lo afuera.
        assert_eq!(parse_net_dev(DEV_A), (149_128_038, 20_469_037));
    }

    #[test]
    fn tasa_entre_dos_lecturas() {
        let mut net = Net::default();
        net.apply(parse_net_dev(DEV_A), 0.0);
        net.apply(parse_net_dev(DEV_B), 2.0);
        assert_eq!(net.rx_rate, 1_024_000.0);
        assert_eq!(net.tx_rate, 256.0);
    }

    #[test]
    fn normaliza_contra_el_mayor_con_piso_de_un_kb() {
        let mut net = Net::default();
        net.apply(parse_net_dev(DEV_A), 0.0);
        net.apply(parse_net_dev(DEV_B), 2.0);
        assert_eq!(net.normalized(), (1.0, 0.00025));
        // Sin tráfico: el piso evita dividir por cero.
        assert_eq!(Net::default().normalized(), (0.0, 0.0));
    }
}
```

Agregar `pub mod net;` y `pub mod rate;` a `src/metrics/mod.rs`.

- [ ] **Step 3: Verificar que fallan**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo test net rate'`
(si `cargo test` rechaza dos filtros, correr `cargo test net` y `cargo test rate` por separado)
Expected: FAIL — `per_second`, `parse_net_dev` y `Net` no existen.

- [ ] **Step 4: `src/metrics/rate.rs`**

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! Tasa por delta entre dos lecturas de contadores acumulados (red y disco, spec §3.3).

/// Unidades por segundo entre dos lecturas de un par de contadores. Sin lectura previa o
/// con intervalo nulo da 0: la primera lectura sólo fija la base. Un contador que
/// retrocede (interfaz que desaparece, disco que se desconecta) también da 0, como el
/// `Math.max(0, …)` del plasmoid.
pub fn per_second(prev: Option<(u64, u64)>, now: (u64, u64), elapsed_secs: f64) -> (f64, f64) {
    match prev {
        Some((a, b)) if elapsed_secs > 0.0 => (
            now.0.saturating_sub(a) as f64 / elapsed_secs,
            now.1.saturating_sub(b) as f64 / elapsed_secs,
        ),
        _ => (0.0, 0.0),
    }
}
```

- [ ] **Step 5: `src/metrics/net.rs`**

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! Tasa de red desde /proc/net/dev. Suma todas las interfaces menos `lo`, como
//! `parseNetDev` del plasmoid: incluye tailscale0, docker0 y compañía.

use std::fs;

use super::rate::per_second;

/// (bytes recibidos, bytes enviados) acumulados, sin `lo`.
pub fn parse_net_dev(raw: &str) -> (u64, u64) {
    let (mut rx, mut tx) = (0u64, 0u64);
    for line in raw.lines() {
        let Some((iface, rest)) = line.split_once(':') else {
            continue;
        };
        if iface.trim() == "lo" {
            continue;
        }
        let fields: Vec<&str> = rest.split_whitespace().collect();
        if fields.len() < 9 {
            continue;
        }
        rx += fields[0].parse::<u64>().unwrap_or(0);
        tx += fields[8].parse::<u64>().unwrap_or(0);
    }
    (rx, tx)
}

#[derive(Default)]
pub struct Net {
    prev: Option<(u64, u64)>,
    /// Bytes por segundo recibidos.
    pub rx_rate: f64,
    /// Bytes por segundo enviados.
    pub tx_rate: f64,
}

impl Net {
    pub fn apply(&mut self, sample: (u64, u64), elapsed_secs: f64) {
        (self.rx_rate, self.tx_rate) = per_second(self.prev, sample, elapsed_secs);
        self.prev = Some(sample);
    }

    pub fn refresh(&mut self, elapsed_secs: f64) {
        if let Ok(raw) = fs::read_to_string("/proc/net/dev") {
            self.apply(parse_net_dev(&raw), elapsed_secs);
        }
    }

    /// (rx, tx) para el historial. Cada punto se divide por el mayor de los dos en ese
    /// instante, con piso de 1 KB/s — la misma normalización por muestra de
    /// `parseNetDev`: el gráfico muestra la proporción entre subida y bajada, no la
    /// magnitud.
    pub fn normalized(&self) -> (f32, f32) {
        let max = self.rx_rate.max(self.tx_rate).max(1024.0);
        ((self.rx_rate / max) as f32, (self.tx_rate / max) as f32)
    }
}
```

- [ ] **Step 6: Cablear en `Metrics`**

`use net::Net;` y los campos:

```rust
pub net: Net,
/// Subida normalizada por muestra (ver `Net::normalized`).
pub net_up_history: History,
/// Bajada normalizada por muestra.
pub net_down_history: History,
```

En `tick`, `let _elapsed = …` pasa a `let elapsed = …`, y después de las temperaturas:

```rust
self.net.refresh(elapsed);
let (down, up) = self.net.normalized();
self.net_down_history.push(down);
self.net_up_history.push(up);
```

- [ ] **Step 7: Verificar que pasan**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo test'`
Expected: PASS, 70 tests.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "Red desde /proc/net/dev

Suma de todas las interfaces menos lo, tasa por delta con el intervalo
real entre ticks e historial normalizado por muestra como el plasmoid."
```

---

### Task 6: Disco — E/S, uso por punto de montaje e inventario

**Files:**
- Create: `src/metrics/disk.rs`, `tests/fixtures/proc_diskstats`, `tests/fixtures/proc_mounts`
- Modify: `Cargo.toml`, `src/metrics/mod.rs`

**Interfaces:**
- Consumes: `rate::per_second` (Task 5), `read_trimmed` (Task 4).
- Produces:
  - `disk::is_whole_disk(&str) -> bool`, `disk::parse_diskstats(&str) -> (u64, u64)` — bytes (leídos, escritos)
  - `disk::DiskIo { pub read_rate: f64, pub write_rate: f64 }` con `apply` / `refresh(elapsed)`
  - `disk::GRAPH_SCALE: f64` y `disk::graph_fraction(rate: f64) -> f32`
  - `disk::MountUsage { device, mount: String, size, used, avail: u64, percent: u32 }`
  - `disk::unescape_mount(&str) -> String`, `disk::parse_mounts(&str) -> Vec<(String, String)>`
  - `disk::usage_from(blocks, bfree, bavail, frsize: u64) -> (u64, u64, u64, u32)`
  - `disk::BlockDevice { name, path: String, size: u64, detail: String, usage: Option<MountUsage> }`
  - `disk::transport(resolved: &str, name: &str) -> Option<&'static str>`
  - `disk::usage_for<'a>(disk: &str, partitions: &[String], usages: &'a [MountUsage]) -> Option<&'a MountUsage>`
  - `disk::Storage { pub io: DiskIo, pub mounts: Vec<MountUsage>, pub devices: Vec<BlockDevice> }`
    con `refresh_primary()`, `refresh_full()`, `primary() -> Option<&MountUsage>`
  - `Metrics` suma `pub storage: Storage`, `pub disk_read_history`, `pub disk_write_history: History`

- [ ] **Step 1: Dependencia**

En `Cargo.toml`, después de `[dependencies.serde]`:

```toml
[dependencies.rustix]
# Ya está en el árbol por libcosmic; se declara para statvfs (`fs`) y el tamaño de página (`param`).
version = "1.1"
features = ["fs", "param"]
```

- [ ] **Step 2: Fixtures**

`tests/fixtures/proc_diskstats` — `sda` y sus particiones reales de `casa`, más un NVMe, un
device-mapper y un zram inventados para cubrir el filtro:

```
   8       0 sda 107590 18970 12302680 188298 132810 32085 6085914 310964 0 78322 509941 2527 0 6509240 2338 7863 8339
   8       1 sda1 158 1200 11806 187 2 0 2 0 0 111 187 0 0 0 0 0 0
   8       3 sda3 107052 17760 12279928 187978 132786 32068 6085648 310937 0 85163 501255 2527 0 6509240 2338 0 0
 259       0 nvme0n1 5000 0 1000 0 6000 0 2000 0 0 0 0 0 0 0 0 0 0
 259       1 nvme0n1p1 4000 0 900 0 5000 0 1900 0 0 0 0 0 0 0 0 0 0
 253       0 dm-0 100 0 500 0 100 0 700 0 0 0 0 0 0 0 0 0 0
 251       0 zram0 50 0 400 0 60 0 480 0 0 0 0 0 0 0 0 0 0
```

`tests/fixtures/proc_mounts` — las líneas reales de `casa` más un USB con espacio en el nombre
y un NVMe en `/mnt`:

```
/dev/sda3 / btrfs rw,seclabel,relatime,compress=zstd:1,ssd,discard=async,space_cache=v2,subvolid=256,subvol=/root 0 0
proc /proc proc rw,nosuid,nodev,noexec,relatime 0 0
/dev/sda3 /home btrfs rw,seclabel,relatime,compress=zstd:1,ssd,discard=async,space_cache=v2,subvolid=257,subvol=/home 0 0
/dev/sda2 /boot ext4 rw,seclabel,relatime 0 0
/dev/sda1 /boot/efi vfat rw,relatime 0 0
tmpfs /tmp tmpfs rw,nosuid,nodev 0 0
/dev/sdb1 /run/media/tincho/USB\040DISK vfat rw,nosuid,nodev 0 0
/dev/nvme0n1p2 /mnt/datos ext4 rw,relatime 0 0
```

- [ ] **Step 3: Tests que fallan**

`src/metrics/disk.rs`, y `pub mod disk;` en `src/metrics/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const DISKSTATS: &str = include_str!("../../tests/fixtures/proc_diskstats");
    const MOUNTS: &str = include_str!("../../tests/fixtures/proc_mounts");

    #[test]
    fn reconoce_discos_enteros_como_el_regex_del_qml() {
        for name in ["sda", "sdab", "vda", "xvda", "hdc", "nvme0n1", "nvme12n3", "mmcblk0", "md127"] {
            assert!(is_whole_disk(name), "{name} es disco entero");
        }
    }

    #[test]
    fn descarta_particiones_y_dispositivos_virtuales() {
        for name in ["sda1", "nvme0n1p1", "mmcblk0p1", "dm-0", "zram0", "loop0", "sr0", "sd", "nvme0"] {
            assert!(!is_whole_disk(name), "{name} no es disco entero");
        }
    }

    #[test]
    fn suma_sectores_de_discos_enteros_en_bytes() {
        // sda + nvme0n1, sectores de 512 bytes.
        assert_eq!(parse_diskstats(DISKSTATS), (6_299_484_160, 3_117_011_968));
    }

    #[test]
    fn tasa_de_lectura_y_escritura() {
        let mut io = DiskIo::default();
        io.apply((0, 0), 0.0);
        io.apply((10_485_760, 5_242_880), 2.0);
        assert_eq!(io.read_rate, 5_242_880.0);
        assert_eq!(io.write_rate, 2_621_440.0);
    }

    #[test]
    fn el_grafico_satura_en_diez_megas() {
        assert_eq!(graph_fraction(0.0), 0.0);
        assert!((graph_fraction(5.24290e6) - 0.5).abs() < 0.001);
        assert_eq!(graph_fraction(1.0e9), 1.0);
    }

    #[test]
    fn des_escapa_los_octales_de_mounts() {
        assert_eq!(unescape_mount("/run/media/tincho/USB\\040DISK"), "/run/media/tincho/USB DISK");
        assert_eq!(unescape_mount("/mnt/a\\134b"), "/mnt/a\\b");
        assert_eq!(unescape_mount("/sin/escapes"), "/sin/escapes");
    }

    #[test]
    fn filtra_montajes_como_el_parse_df_del_qml() {
        assert_eq!(
            parse_mounts(MOUNTS),
            vec![
                ("/dev/sda3".to_string(), "/".to_string()),
                ("/dev/sda3".to_string(), "/home".to_string()),
                ("/dev/sdb1".to_string(), "/run/media/tincho/USB DISK".to_string()),
                ("/dev/nvme0n1p2".to_string(), "/mnt/datos".to_string()),
            ]
        );
    }

    #[test]
    fn uso_como_df() {
        // 600 bloques usados de 900 disponibles para el usuario: df redondea para arriba.
        assert_eq!(usage_from(1000, 400, 300, 4096), (4_096_000, 2_457_600, 1_228_800, 67));
        assert_eq!(usage_from(0, 0, 0, 4096), (0, 0, 0, 0));
    }

    #[test]
    fn transporte_desde_la_ruta_de_sysfs() {
        let ata = "/sys/devices/pci0000:00/0000:00:17.0/ata6/host5/target5:0:0/5:0:0:0/block/sda";
        let usb = "/sys/devices/pci0000:00/0000:00:14.0/usb2/2-1/2-1:1.0/host6/target6:0:0/6:0:0:0/block/sdb";
        let virtio = "/sys/devices/pci0000:00/0000:00:04.0/virtio1/block/vda";
        assert_eq!(transport(ata, "sda"), Some("SATA"));
        assert_eq!(transport(usb, "sdb"), Some("USB"));
        assert_eq!(transport(virtio, "vda"), Some("VIRTIO"));
        assert_eq!(transport("/sys/devices/pci0000:00/0000:01:00.0/nvme/nvme0/nvme0n1", "nvme0n1"), Some("NVME"));
        assert_eq!(transport("/sys/devices/virtual/block/xyz", "xyz"), None);
    }

    #[test]
    fn uso_del_disco_sale_de_la_primera_particion_montada() {
        let u = |device: &str, mount: &str| MountUsage {
            device: device.to_string(),
            mount: mount.to_string(),
            size: 1,
            used: 1,
            avail: 0,
            percent: 100,
        };
        let usages = vec![u("/dev/sda3", "/"), u("/dev/sda3", "/home"), u("/dev/nvme0n1p2", "/mnt/datos")];
        let parts = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(usage_for("sda", &parts(&["sda1", "sda2", "sda3"]), &usages).map(|x| x.mount.as_str()), Some("/"));
        assert_eq!(usage_for("nvme0n1", &parts(&["nvme0n1p1", "nvme0n1p2"]), &usages).map(|x| x.mount.as_str()), Some("/mnt/datos"));
        assert_eq!(usage_for("sdb", &parts(&["sdb1"]), &usages), None);
    }
}
```

- [ ] **Step 4: Verificar que fallan**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo test disk'`
Expected: FAIL — el módulo está vacío.

- [ ] **Step 5: `src/metrics/disk.rs`**

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! Disco: E/S desde /proc/diskstats, uso por punto de montaje con `statvfs` (en vez de
//! `df`) e inventario desde /sys/block (en vez de `lsblk`).

use std::fs;
use std::path::Path;

use super::rate::per_second;
use super::read_trimmed;

/// Mismo criterio que `isWholeDiskDevice` del plasmoid:
/// `^(sd[a-z]+|vd[a-z]+|xvd[a-z]+|hd[a-z]+|nvme\d+n\d+|mmcblk\d+|md\d+)$`.
pub fn is_whole_disk(name: &str) -> bool {
    fn letters(s: &str) -> bool {
        !s.is_empty() && s.bytes().all(|b| b.is_ascii_lowercase())
    }
    fn digits(s: &str) -> bool {
        !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
    }
    if let Some(rest) = name.strip_prefix("xvd") {
        return letters(rest);
    }
    for prefix in ["sd", "vd", "hd"] {
        if let Some(rest) = name.strip_prefix(prefix) {
            return letters(rest);
        }
    }
    if let Some(rest) = name.strip_prefix("nvme") {
        return rest
            .split_once('n')
            .is_some_and(|(controller, ns)| digits(controller) && digits(ns));
    }
    if let Some(rest) = name.strip_prefix("mmcblk") {
        return digits(rest);
    }
    if let Some(rest) = name.strip_prefix("md") {
        return digits(rest);
    }
    false
}

/// (bytes leídos, bytes escritos) acumulados de los discos enteros. Columnas 6 y 10 de
/// diskstats, en sectores de 512 bytes.
pub fn parse_diskstats(raw: &str) -> (u64, u64) {
    let (mut read, mut write) = (0u64, 0u64);
    for line in raw.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 10 || !is_whole_disk(f[2]) {
            continue;
        }
        read += f[5].parse::<u64>().unwrap_or(0) * 512;
        write += f[9].parse::<u64>().unwrap_or(0) * 512;
    }
    (read, write)
}

#[derive(Default)]
pub struct DiskIo {
    prev: Option<(u64, u64)>,
    pub read_rate: f64,
    pub write_rate: f64,
}

impl DiskIo {
    pub fn apply(&mut self, sample: (u64, u64), elapsed_secs: f64) {
        (self.read_rate, self.write_rate) = per_second(self.prev, sample, elapsed_secs);
        self.prev = Some(sample);
    }

    pub fn refresh(&mut self, elapsed_secs: f64) {
        if let Ok(raw) = fs::read_to_string("/proc/diskstats") {
            self.apply(parse_diskstats(&raw), elapsed_secs);
        }
    }
}

/// Escala fija del gráfico de E/S: `axisScale` de `StorageDetail.qml`, 10 MB/s por mitad.
pub const GRAPH_SCALE: f64 = 1.04858e7;

pub fn graph_fraction(rate: f64) -> f32 {
    (rate / GRAPH_SCALE).clamp(0.0, 1.0) as f32
}

#[derive(Clone, Debug, PartialEq)]
pub struct MountUsage {
    pub device: String,
    pub mount: String,
    /// Todo en bytes.
    pub size: u64,
    pub used: u64,
    pub avail: u64,
    /// Como la columna `Uso%` de df.
    pub percent: u32,
}

/// /proc/self/mounts escapa espacio, tab, salto y barra invertida como `\ooo`.
pub fn unescape_mount(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let octal = b[i] == b'\\'
            && i + 3 < b.len()
            && b[i + 1..=i + 3].iter().all(|c| (b'0'..=b'7').contains(c));
        if octal {
            let v = u32::from(b[i + 1] - b'0') * 64
                + u32::from(b[i + 2] - b'0') * 8
                + u32::from(b[i + 3] - b'0');
            out.push(v as u8);
            i += 4;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// El filtro de `parseDf`: raíz y lo que cuelgue de /home, /mnt, /media o /run/media.
fn wanted_mount(mount: &str) -> bool {
    mount == "/"
        || ["/home", "/mnt", "/media", "/run/media"]
            .iter()
            .any(|p| mount.starts_with(p))
}

/// (dispositivo, punto de montaje) de los dispositivos de bloque que mostraría
/// `df | grep '^/dev'`, en el orden del archivo. Como df, no deduplica: dos subvolúmenes
/// btrfs del mismo disco aparecen los dos.
pub fn parse_mounts(raw: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for line in raw.lines() {
        let mut f = line.split_whitespace();
        let (Some(device), Some(mount)) = (f.next(), f.next()) else {
            continue;
        };
        if !device.starts_with("/dev/") {
            continue;
        }
        let entry = (device.to_string(), unescape_mount(mount));
        if wanted_mount(&entry.1) && !out.contains(&entry) {
            out.push(entry);
        }
    }
    out
}

/// (tamaño, usado, disponible, porcentaje) como df: el porcentaje es usado sobre usado +
/// disponible para el usuario, redondeado para arriba.
pub fn usage_from(blocks: u64, bfree: u64, bavail: u64, frsize: u64) -> (u64, u64, u64, u32) {
    let size = blocks * frsize;
    let used = blocks.saturating_sub(bfree) * frsize;
    let avail = bavail * frsize;
    let denom = u128::from(used) + u128::from(avail);
    let percent = if denom == 0 {
        0
    } else {
        (u128::from(used) * 100).div_ceil(denom) as u32
    };
    (size, used, avail, percent)
}

fn read_usage(device: &str, mount: &str) -> Option<MountUsage> {
    let st = rustix::fs::statvfs(mount).ok()?;
    let (size, used, avail, percent) = usage_from(st.f_blocks, st.f_bfree, st.f_bavail, st.f_frsize);
    Some(MountUsage {
        device: device.to_string(),
        mount: mount.to_string(),
        size,
        used,
        avail,
        percent,
    })
}

fn read_mounts() -> Vec<(String, String)> {
    fs::read_to_string("/proc/self/mounts")
        .map(|raw| parse_mounts(&raw))
        .unwrap_or_default()
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockDevice {
    /// Modelo, o el nombre del kernel si no tiene.
    pub name: String,
    pub path: String,
    /// Bytes.
    pub size: u64,
    /// "SATA", "USB • Extraíble", "Disco"…
    pub detail: String,
    pub usage: Option<MountUsage>,
}

/// La columna TRAN de lsblk, deducida de la ruta real del dispositivo en sysfs. USB va
/// antes que ATA porque los puentes USB-SATA pasan por las dos.
pub fn transport(resolved: &str, name: &str) -> Option<&'static str> {
    if name.starts_with("nvme") {
        Some("NVME")
    } else if name.starts_with("mmcblk") {
        Some("MMC")
    } else if resolved.contains("/usb") {
        Some("USB")
    } else if resolved.contains("/virtio") {
        Some("VIRTIO")
    } else if resolved.contains("/ata") {
        Some("SATA")
    } else {
        None
    }
}

/// Uso del disco entero o de su primera partición con uso, en orden de partición: lo que
/// hace `usageForNode` recorriendo los hijos de lsblk.
pub fn usage_for<'a>(disk: &str, partitions: &[String], usages: &'a [MountUsage]) -> Option<&'a MountUsage> {
    std::iter::once(disk)
        .chain(partitions.iter().map(String::as_str))
        .find_map(|dev| {
            let path = format!("/dev/{dev}");
            usages.iter().find(|u| u.device == path)
        })
}

fn read_block_devices(sys_block: &Path, usages: &[MountUsage]) -> Vec<BlockDevice> {
    let Ok(entries) = fs::read_dir(sys_block) else {
        return Vec::new();
    };
    // lsblk reporta los md como TYPE raid*, no disk: el plasmoid no los lista.
    let mut names: Vec<String> = entries
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| is_whole_disk(n) && !n.starts_with("md"))
        .collect();
    names.sort();

    names
        .into_iter()
        .map(|name| {
            let dir = sys_block.join(&name);
            let resolved = fs::canonicalize(&dir)
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            let size = read_trimmed(&dir.join("size"))
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0)
                * 512;
            let model = read_trimmed(&dir.join("device/model")).unwrap_or_default();
            let removable = read_trimmed(&dir.join("removable")).as_deref() == Some("1");
            let mut partitions: Vec<String> = fs::read_dir(&dir)
                .map(|it| {
                    it.flatten()
                        .filter_map(|e| e.file_name().into_string().ok())
                        .filter(|p| p.starts_with(&name) && *p != name)
                        .collect()
                })
                .unwrap_or_default();
            // sda2 antes que sda10.
            partitions.sort_by_key(|p| (p.len(), p.clone()));
            let mut detail = transport(&resolved, &name).unwrap_or("Disco").to_string();
            if removable {
                detail.push_str(" • Extraíble");
            }
            BlockDevice {
                name: if model.is_empty() { name.clone() } else { model },
                path: format!("/dev/{name}"),
                size,
                detail,
                usage: usage_for(&name, &partitions, usages).cloned(),
            }
        })
        .collect()
}

#[derive(Default)]
pub struct Storage {
    pub io: DiskIo,
    /// En el orden de /proc/self/mounts. Con el panel de disco cerrado trae sólo el
    /// primero, que es el que muestra la vista compacta.
    pub mounts: Vec<MountUsage>,
    /// Vacío mientras el panel de disco no está a la vista.
    pub devices: Vec<BlockDevice>,
}

impl Storage {
    /// Barato: un solo `statvfs`, el del primer punto de montaje.
    pub fn refresh_primary(&mut self) {
        self.mounts = read_mounts()
            .into_iter()
            .take(1)
            .filter_map(|(d, m)| read_usage(&d, &m))
            .collect();
        self.devices.clear();
    }

    /// Caro: todos los puntos de montaje y el inventario de /sys/block.
    pub fn refresh_full(&mut self) {
        self.mounts = read_mounts()
            .into_iter()
            .filter_map(|(d, m)| read_usage(&d, &m))
            .collect();
        self.devices = read_block_devices(Path::new("/sys/block"), &self.mounts);
    }

    pub fn primary(&self) -> Option<&MountUsage> {
        self.mounts.first()
    }
}
```

- [ ] **Step 6: Cablear en `Metrics`**

`use disk::Storage;` y los campos:

```rust
pub storage: Storage,
/// Lectura sobre `disk::GRAPH_SCALE`, saturada en 1.
pub disk_read_history: History,
/// Escritura, ídem.
pub disk_write_history: History,
```

En `tick`, antes de `self.refresh_expensive(detail);`:

```rust
self.storage.io.refresh(elapsed);
self.disk_read_history.push(disk::graph_fraction(self.storage.io.read_rate));
self.disk_write_history.push(disk::graph_fraction(self.storage.io.write_rate));
if !expensive_for(detail).storage_detail {
    self.storage.refresh_primary();
}
```

Y en `refresh_expensive`:

```rust
if expensive.storage_detail {
    self.storage.refresh_full();
}
```

- [ ] **Step 7: Verificar que pasan y contrastar contra df**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo test'`
Expected: PASS, 80 tests.

Contraste manual en el host: `df -B1 --output=source,size,used,avail,pcent,target / /home`
tiene que dar los mismos bytes y porcentaje que `usage_from` sobre `stat -f -c '%b %f %a %S' /`
(bloques, libres, disponibles, tamaño de bloque).

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "Disco: E/S, uso por punto de montaje e inventario

diskstats para la tasa, statvfs vía rustix en vez de df y /sys/block en
vez de lsblk. Con el panel cerrado sólo se mide el primer montaje."
```

---
### Task 7: GPU — sysfs, pci.ids y nvidia-smi

**Files:**
- Create: `src/exec.rs`, `src/metrics/gpu.rs`, `tests/fixtures/drm/…`, `tests/fixtures/pci.ids`
- Modify: `src/main.rs`, `src/ui/detail/mod.rs`, `src/metrics/mod.rs`

**Interfaces:**
- Consumes: `read_trimmed` (Task 4), `History`.
- Produces:
  - `exec::executable_in_path(cmd: &str) -> bool` (sale de `ui::detail`)
  - `gpu::GpuDevice { id, name: String, usage: f32, clock_mhz: f64, temperature: f32, memory_used_mib, memory_total_mib: f64 }`
  - `gpu::VramProcess { pid: u32, name: String, mib: f64 }` (lo consume la Task 8)
  - `gpu::pci_name(pci_ids: &str, vendor: u16, device: u16) -> Option<String>`
  - `gpu::parse_pp_dpm_sclk(&str) -> f64`
  - `gpu::read_sysfs_gpus(drm: &Path, names: &mut HashMap<String, String>, pci_ids: Option<&Path>) -> Vec<GpuDevice>`
  - `gpu::parse_nvidia_smi(&str) -> Vec<GpuDevice>`, `gpu::parse_nvidia_apps(&str) -> Vec<VramProcess>`
  - `gpu::merge_devices(nvidia: &[GpuDevice], sysfs: &[GpuDevice]) -> Vec<GpuDevice>`
  - `gpu::Gpu` (`Default`) con `refresh_sysfs()`, `refresh_nvidia()`, `devices() -> Vec<GpuDevice>`,
    `summary() -> Option<GpuDevice>`, `available() -> bool`, `history(id: &str) -> Option<&History>`,
    `pub nvidia_processes: Vec<VramProcess>`
  - `Metrics` suma `pub gpu: Gpu`

- [ ] **Step 1: Fixtures**

Una AMD completa (`card0`), una Intel sin `gpu_busy_percent` (`card1`, tiene que ignorarse) y un
conector (`card0-DP-1`, no es una placa):

```bash
F=tests/fixtures/drm
mkdir -p $F/card0/device/hwmon/hwmon4 $F/card1/device $F/card0-DP-1
printf '0x1002\n' > $F/card0/device/vendor
printf '0x73ff\n' > $F/card0/device/device
printf '37\n' > $F/card0/device/gpu_busy_percent
printf '0: 500Mhz\n1: 1200Mhz *\n2: 1600Mhz\n' > $F/card0/device/pp_dpm_sclk
printf '2147483648\n' > $F/card0/device/mem_info_vram_used
printf '8573157376\n' > $F/card0/device/mem_info_vram_total
printf '54000\n' > $F/card0/device/hwmon/hwmon4/temp1_input
printf '0x8086\n' > $F/card1/device/vendor
printf '0x9bc8\n' > $F/card1/device/device
printf 'connected\n' > $F/card0-DP-1/status
```

`tests/fixtures/pci.ids` — recorte con el formato real (comentarios, subsistemas con dos tabs):

```
# Recorte de /usr/share/hwdata/pci.ids para tests
1002  Advanced Micro Devices, Inc. [AMD/ATI]
	73ef  Navi 23 [Radeon RX 6650 XT / 6700S / 6800S]
	73ff  Navi 23 [Radeon RX 6600/6600 XT/6600M]
		1458 2331  Subsistema que no hay que confundir
8086  Intel Corporation
	9bc8  CometLake-S GT2 [UHD Graphics 630]
```

- [ ] **Step 2: Tests que fallan**

`src/exec.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encuentra_sh_y_no_inventa() {
        assert!(executable_in_path("sh"));
        assert!(!executable_in_path("no-existe-este-binario-xyz"));
    }
}
```

`src/metrics/gpu.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    fn ids() -> String {
        fs::read_to_string(fixtures().join("pci.ids")).unwrap()
    }

    fn device(id: &str, name: &str, usage: f32) -> GpuDevice {
        GpuDevice {
            id: id.to_string(),
            name: name.to_string(),
            usage,
            clock_mhz: 0.0,
            temperature: 0.0,
            memory_used_mib: 0.0,
            memory_total_mib: 0.0,
        }
    }

    #[test]
    fn pci_name_arma_fabricante_y_dispositivo() {
        assert_eq!(
            pci_name(&ids(), 0x1002, 0x73ff).as_deref(),
            Some("Advanced Micro Devices, Inc. [AMD/ATI] Navi 23 [Radeon RX 6600/6600 XT/6600M]")
        );
        assert_eq!(
            pci_name(&ids(), 0x8086, 0x9bc8).as_deref(),
            Some("Intel Corporation CometLake-S GT2 [UHD Graphics 630]")
        );
    }

    #[test]
    fn pci_name_no_confunde_subsistemas_ni_inventa() {
        assert_eq!(pci_name(&ids(), 0x1002, 0x1458), None);
        assert_eq!(pci_name(&ids(), 0x10de, 0x2503), None);
    }

    #[test]
    fn reloj_de_pp_dpm_sclk() {
        assert_eq!(parse_pp_dpm_sclk("0: 500Mhz\n1: 1200Mhz *\n2: 1600Mhz\n"), 1200.0);
        assert_eq!(parse_pp_dpm_sclk("0: 500Mhz\n"), 0.0);
    }

    #[test]
    fn lee_solo_las_placas_con_gpu_busy_percent() {
        let mut names = HashMap::new();
        let ids_path = fixtures().join("pci.ids");
        let gpus = read_sysfs_gpus(&fixtures().join("drm"), &mut names, Some(ids_path.as_path()));
        assert_eq!(
            gpus,
            vec![GpuDevice {
                id: "card0".to_string(),
                name: "Advanced Micro Devices, Inc. [AMD/ATI] Navi 23 [Radeon RX 6600/6600 XT/6600M]".to_string(),
                usage: 37.0,
                clock_mhz: 1200.0,
                temperature: 54.0,
                memory_used_mib: 2048.0,
                memory_total_mib: 8176.0,
            }]
        );
        // El nombre queda cacheado: pci.ids no se relee en cada tick.
        assert!(names.contains_key("card0"));
    }

    #[test]
    fn parsea_nvidia_smi_y_tolera_na() {
        let raw = "NVIDIA GeForce RTX 3060, 12, 810, 48, 1024, 12288\nNVIDIA T400, [N/A], [N/A], 40, 100, 2048\nbasura\n";
        let gpus = parse_nvidia_smi(raw);
        assert_eq!(gpus.len(), 2);
        assert_eq!(gpus[0].id, "nvidia0");
        assert_eq!(gpus[0].name, "NVIDIA GeForce RTX 3060");
        assert_eq!((gpus[0].usage, gpus[0].clock_mhz, gpus[0].temperature), (12.0, 810.0, 48.0));
        assert_eq!((gpus[0].memory_used_mib, gpus[0].memory_total_mib), (1024.0, 12288.0));
        assert_eq!((gpus[1].id.as_str(), gpus[1].usage), ("nvidia1", 0.0));
    }

    #[test]
    fn parsea_procesos_de_nvidia() {
        let raw = "4321, /usr/bin/steam, 512\n0, fantasma, 10\n77, , 5\n";
        assert_eq!(
            parse_nvidia_apps(raw),
            vec![VramProcess { pid: 4321, name: "/usr/bin/steam".to_string(), mib: 512.0 }]
        );
    }

    #[test]
    fn nvidia_primero_y_sin_duplicados() {
        let nvidia = vec![device("nvidia0", "NVIDIA RTX", 10.0)];
        let sysfs = vec![device("card0", "AMD", 50.0), device("card1", "NVIDIA RTX", 5.0)];
        let merged = merge_devices(&nvidia, &sysfs);
        let ids: Vec<&str> = merged.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, vec!["nvidia0", "card0"]);
    }
}
```

Agregar `mod exec;` a `src/main.rs` y `pub mod gpu;` a `src/metrics/mod.rs`.

- [ ] **Step 3: Verificar que fallan**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo test gpu'`
y `… cargo test exec`
Expected: FAIL — nada de esto existe.

- [ ] **Step 4: `src/exec.rs` y su uso en `ui::detail`**

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! Búsqueda de ejecutables en el PATH, sin lanzar nada.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// Si hay un archivo regular **con bit de ejecución** llamado `cmd` en algún directorio
/// del PATH. `is_file()` solo no alcanza: un archivo sin permiso haría ofrecer algo cuyo
/// `spawn()` falla en silencio.
pub fn executable_in_path(cmd: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| is_executable_file(&dir.join(cmd))))
        .unwrap_or(false)
}

fn is_executable_file(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}
```

En `src/ui/detail/mod.rs`, borrar `is_executable_file` (con su doc comment) y dejar:

```rust
pub fn system_monitor_command() -> Option<&'static str> {
    CANDIDATES.into_iter().find(|c| crate::exec::executable_in_path(c))
}
```

- [ ] **Step 5: `src/metrics/gpu.rs`**

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! GPU: sysfs de amdgpu (barato, en cada tick) y `nvidia-smi` (el único fork del
//! muestreo, sólo con el panel de GPU a la vista, spec §3.5). Intel no expone
//! `gpu_busy_percent`: en una máquina sólo-Intel no hay dispositivos y la sección se
//! oculta.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::history::History;
use super::read_trimmed;

/// Donde lo dejan Fedora (hwdata) y Debian (pciutils).
const PCI_IDS: [&str; 2] = ["/usr/share/hwdata/pci.ids", "/usr/share/misc/pci.ids"];

const NVIDIA_GPU_QUERY: [&str; 2] = [
    "--query-gpu=name,utilization.gpu,clocks.current.graphics,temperature.gpu,memory.used,memory.total",
    "--format=csv,noheader,nounits",
];
const NVIDIA_APPS_QUERY: [&str; 2] = [
    "--query-compute-apps=pid,process_name,used_memory",
    "--format=csv,noheader,nounits",
];

#[derive(Clone, Debug, PartialEq)]
pub struct GpuDevice {
    pub id: String,
    pub name: String,
    /// 0–100.
    pub usage: f32,
    /// 0 si el driver no lo expone.
    pub clock_mhz: f64,
    /// °C; 0 sin sensor.
    pub temperature: f32,
    pub memory_used_mib: f64,
    pub memory_total_mib: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VramProcess {
    pub pid: u32,
    pub name: String,
    pub mib: f64,
}

fn parse_hex_id(raw: &str) -> Option<u16> {
    u16::from_str_radix(raw.trim().trim_start_matches("0x"), 16).ok()
}

/// "Fabricante Dispositivo" desde pci.ids, que es de donde lo saca `lspci`.
pub fn pci_name(pci_ids: &str, vendor: u16, device: u16) -> Option<String> {
    let vendor_hex = format!("{vendor:04x}");
    let device_hex = format!("{device:04x}");
    let mut vendor_name: Option<&str> = None;
    for line in pci_ids.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match vendor_name {
            None => {
                if let Some(rest) = line.strip_prefix(vendor_hex.as_str()) {
                    if rest.starts_with("  ") {
                        vendor_name = Some(rest.trim());
                    }
                }
            }
            Some(v) => {
                // Dos tabs: subsistema, no dispositivo.
                if line.starts_with("\t\t") {
                    continue;
                }
                // Sin tab: empezó el fabricante siguiente.
                let entry = line.strip_prefix('\t')?;
                if let Some(rest) = entry.strip_prefix(device_hex.as_str()) {
                    if rest.starts_with("  ") {
                        return Some(format!("{v} {}", rest.trim()));
                    }
                }
            }
        }
    }
    None
}

/// Reloj del renglón marcado con `*` en `pp_dpm_sclk`: `1: 1200Mhz *` → 1200.
pub fn parse_pp_dpm_sclk(raw: &str) -> f64 {
    raw.lines()
        .find(|l| l.contains('*'))
        .and_then(|l| l.split_once(':'))
        .map(|(_, v)| v.replace('*', "").trim().to_ascii_lowercase())
        .and_then(|v| v.trim_end_matches("mhz").trim().parse::<f64>().ok())
        .unwrap_or(0.0)
}

/// `cardN/device`, en orden numérico. Los conectores (`card0-DP-1`) no son placas.
fn card_dirs(drm: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = fs::read_dir(drm) else {
        return Vec::new();
    };
    let mut cards: Vec<(u32, String, PathBuf)> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let n = name.strip_prefix("card")?.parse::<u32>().ok()?;
            Some((n, name, e.path().join("device")))
        })
        .collect();
    cards.sort_by_key(|(n, _, _)| *n);
    cards.into_iter().map(|(_, name, dev)| (name, dev)).collect()
}

fn first_hwmon_temp(hwmon: &Path) -> Option<f32> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(hwmon).ok()?.flatten().map(|e| e.path()).collect();
    dirs.sort();
    dirs.iter()
        .find_map(|d| read_trimmed(&d.join("temp1_input"))?.parse::<i64>().ok())
        .map(|milli| (milli / 1000) as f32)
}

/// Lo que junta `_sysfsGpuCommand`, una placa por `gpu_busy_percent`. `names` cachea el
/// nombre por id: pci.ids pesa 1,6 MB y no tiene sentido releerlo en cada tick.
pub fn read_sysfs_gpus(
    drm: &Path,
    names: &mut HashMap<String, String>,
    pci_ids: Option<&Path>,
) -> Vec<GpuDevice> {
    let mut out = Vec::new();
    for (card, dev) in card_dirs(drm) {
        let Some(busy) = read_trimmed(&dev.join("gpu_busy_percent")).and_then(|v| v.parse::<f32>().ok())
        else {
            continue;
        };
        // El slot PCI si `device` es el symlink real de sysfs; si no, el nombre de la placa.
        let id = fs::canonicalize(&dev)
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .filter(|n| n.contains(':'))
            .unwrap_or_else(|| card.clone());
        let name = names
            .entry(id.clone())
            .or_insert_with(|| {
                let vendor = read_trimmed(&dev.join("vendor")).and_then(|v| parse_hex_id(&v));
                let device = read_trimmed(&dev.join("device")).and_then(|v| parse_hex_id(&v));
                let ids = pci_ids.and_then(|p| fs::read_to_string(p).ok());
                match (vendor, device, ids) {
                    (Some(v), Some(d), Some(ids)) => pci_name(&ids, v, d).unwrap_or_else(|| card.clone()),
                    _ => card.clone(),
                }
            })
            .clone();
        let clock_mhz = read_trimmed(&dev.join("gt_cur_freq_mhz"))
            .and_then(|v| v.parse::<f64>().ok())
            .or_else(|| fs::read_to_string(dev.join("pp_dpm_sclk")).ok().map(|raw| parse_pp_dpm_sclk(&raw)))
            .unwrap_or(0.0);
        let mib = |file: &str| {
            read_trimmed(&dev.join(file))
                .and_then(|v| v.parse::<f64>().ok())
                .map(|bytes| bytes / 1_048_576.0)
                .unwrap_or(0.0)
        };
        out.push(GpuDevice {
            id,
            name,
            usage: busy.clamp(0.0, 100.0),
            clock_mhz,
            temperature: first_hwmon_temp(&dev.join("hwmon")).unwrap_or(0.0),
            memory_used_mib: mib("mem_info_vram_used"),
            memory_total_mib: mib("mem_info_vram_total"),
        });
    }
    out
}

fn csv_fields(line: &str) -> Vec<&str> {
    line.split(',').map(str::trim).collect()
}

/// `nvidia-smi --query-gpu=…`. Un valor `[N/A]` cuenta como 0, igual que `parseFloat || 0`.
pub fn parse_nvidia_smi(raw: &str) -> Vec<GpuDevice> {
    let num = |s: &str| s.parse::<f64>().unwrap_or(0.0);
    raw.lines()
        .map(csv_fields)
        .filter(|f| f.len() >= 6)
        .enumerate()
        .map(|(i, f)| GpuDevice {
            id: format!("nvidia{i}"),
            name: f[0].to_string(),
            usage: (num(f[1]) as f32).clamp(0.0, 100.0),
            clock_mhz: num(f[2]),
            temperature: num(f[3]) as f32,
            memory_used_mib: num(f[4]),
            memory_total_mib: num(f[5]),
        })
        .collect()
}

/// `nvidia-smi --query-compute-apps=…`.
pub fn parse_nvidia_apps(raw: &str) -> Vec<VramProcess> {
    raw.lines()
        .map(csv_fields)
        .filter(|f| f.len() >= 3)
        .filter_map(|f| {
            let pid = f[0].parse::<u32>().ok().filter(|&p| p > 0)?;
            if f[1].is_empty() {
                return None;
            }
            Some(VramProcess {
                pid,
                name: f[1].to_string(),
                mib: f[2].parse().unwrap_or(0.0),
            })
        })
        .collect()
}

/// NVIDIA primero y después las de sysfs que no repitan nombre ni id: `syncGpuDevices`.
pub fn merge_devices(nvidia: &[GpuDevice], sysfs: &[GpuDevice]) -> Vec<GpuDevice> {
    let mut out = nvidia.to_vec();
    for d in sysfs {
        if !out.iter().any(|m| m.name == d.name || m.id == d.id) {
            out.push(d.clone());
        }
    }
    out
}

fn run_nvidia_smi(args: &[&str]) -> Option<String> {
    let out = Command::new("nvidia-smi").args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

pub struct Gpu {
    /// Se mira una sola vez, al arrancar.
    has_nvidia_smi: bool,
    pci_ids: Option<PathBuf>,
    names: HashMap<String, String>,
    sysfs: Vec<GpuDevice>,
    nvidia: Vec<GpuDevice>,
    /// Última lectura de procesos de `nvidia-smi`; la consume `Procs::refresh_vram`.
    pub nvidia_processes: Vec<VramProcess>,
    histories: HashMap<String, History>,
}

impl Default for Gpu {
    fn default() -> Self {
        Self {
            has_nvidia_smi: crate::exec::executable_in_path("nvidia-smi"),
            pci_ids: PCI_IDS.iter().map(PathBuf::from).find(|p| p.is_file()),
            names: HashMap::new(),
            sysfs: Vec::new(),
            nvidia: Vec::new(),
            nvidia_processes: Vec::new(),
            histories: HashMap::new(),
        }
    }
}

impl Gpu {
    /// Barato, en cada tick.
    pub fn refresh_sysfs(&mut self) {
        self.sysfs = read_sysfs_gpus(Path::new("/sys/class/drm"), &mut self.names, self.pci_ids.as_deref());
        for d in &self.sysfs {
            self.histories.entry(d.id.clone()).or_default().push(d.usage / 100.0);
        }
    }

    /// Caro: dos llamadas a `nvidia-smi`. Sólo con el panel de GPU a la vista.
    pub fn refresh_nvidia(&mut self) {
        if !self.has_nvidia_smi {
            return;
        }
        if let Some(raw) = run_nvidia_smi(&NVIDIA_GPU_QUERY) {
            self.nvidia = parse_nvidia_smi(&raw);
            for d in &self.nvidia {
                self.histories.entry(d.id.clone()).or_default().push(d.usage / 100.0);
            }
        }
        if let Some(raw) = run_nvidia_smi(&NVIDIA_APPS_QUERY) {
            self.nvidia_processes = parse_nvidia_apps(&raw);
        }
    }

    pub fn devices(&self) -> Vec<GpuDevice> {
        merge_devices(&self.nvidia, &self.sysfs)
    }

    /// La de más uso: es la que resume la sección en el panel.
    pub fn summary(&self) -> Option<GpuDevice> {
        self.devices().into_iter().max_by(|a, b| a.usage.total_cmp(&b.usage))
    }

    /// Sin ninguna fuente de datos la sección se oculta (spec §3.5).
    pub fn available(&self) -> bool {
        self.has_nvidia_smi || !self.sysfs.is_empty()
    }

    pub fn history(&self, id: &str) -> Option<&History> {
        self.histories.get(id)
    }
}
```

Nota: `nvidia-smi` bloquea el tick (~100–300 ms) mientras el panel de GPU está abierto. Es la
contracara de no tener un runtime async en el muestreo; queda registrado como desvío.

- [ ] **Step 6: Cablear en `Metrics`**

`use gpu::Gpu;`, campo `pub gpu: Gpu,`; en `tick`, antes de `self.refresh_expensive(detail);`:
`self.gpu.refresh_sysfs();`; y en `refresh_expensive`:

```rust
if expensive.gpu_detail {
    self.gpu.refresh_nvidia();
}
```

- [ ] **Step 7: Verificar que pasan**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo test'`
Expected: PASS, 88 tests.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "GPU: sysfs de amdgpu, nombres de pci.ids y nvidia-smi

Lo de sysfs se lee en cada tick; nvidia-smi sólo con el panel de GPU a
la vista. Sin ninguna fuente, la sección queda marcada como no disponible."
```

---

### Task 8: Top de procesos por CPU, memoria y VRAM

**Files:**
- Create: `src/metrics/procs.rs`, `tests/fixtures/proc_pid_stat`
- Modify: `src/metrics/cpu.rs`, `src/metrics/mod.rs`

**Interfaces:**
- Consumes: `gpu::VramProcess`, `Gpu::nvidia_processes` (Task 7), `read_trimmed` (Task 4),
  `rustix::param::page_size` (Task 6).
- Produces:
  - `Cpu::total_jiffies(&self) -> Option<u64>`
  - `procs::ProcStat { pid: u32, name: String, ticks: u64, rss_pages: u64 }`
  - `procs::parse_pid_stat(&str) -> Option<ProcStat>`
  - `procs::ProcCpu { name: String, percent: f32 }`, `procs::ProcMem { name: String, kib: f64, percent: f32 }`
  - `procs::top_cpu(prev: &HashMap<u32, u64>, now: &[ProcStat], total_delta: u64, cores: usize) -> Vec<ProcCpu>`
  - `procs::top_mem(now: &[ProcStat], page_size: u64, mem_total_kib: f64) -> Vec<ProcMem>`
  - `procs::vram_from_fdinfo<'a>(files: impl IntoIterator<Item = &'a str>) -> f64`
  - `procs::merge_vram(a: &[VramProcess], b: &[VramProcess]) -> Vec<VramProcess>`
  - `procs::Procs { pub cpu: Vec<ProcCpu>, pub mem: Vec<ProcMem>, pub vram: Vec<VramProcess> }`
    con `refresh_cpu(total_jiffies, cores)`, `forget_cpu()`, `refresh_mem(page_size, mem_total_mib)`,
    `refresh_vram(nvidia: &[VramProcess])`
  - `Metrics` suma `pub procs: Procs`

**Desvío menor del spec §3:** el RSS sale del campo 24 de `/proc/N/stat`, que ya se lee para la
CPU, en vez de `/proc/N/statm`: una lectura por proceso en lugar de dos, mismo número.

- [ ] **Step 1: Fixture**

`tests/fixtures/proc_pid_stat` — una línea real (`cat`) y dos armadas para los casos difíciles:
un nombre con espacio y dos puntos, y uno con paréntesis adentro:

```
11742 (cat) R 11293 11293 11293 0 -1 4194304 118 0 0 0 7 3 0 0 20 0 1 0 141716 236335104 474 18446744073709551615 94216996040704 94216996061985 140728589727520 0 0 0 0 0 0 0 0 0 17 5 0 0 0 0 0
4321 (tmux: server) S 1 4321 4321 0 -1 4194368 2000 0 0 0 1500 300 0 0 20 0 1 0 5000 30000000 2500 18446744073709551615 0 0 0 0 0 0 0 0 0 0 0 0 17 3 0 0 0 0 0
777 (a) b)) R 1 777 777 0 -1 0 0 0 0 0 10 5 0 0 20 0 1 0 100 1000 42 18446744073709551615 0 0 0 0 0 0 0 0 0 0 0 0 17 1 0 0 0 0 0
```

- [ ] **Step 2: Tests que fallan**

En `src/metrics/cpu.rs`:

```rust
#[test]
fn total_jiffies_es_el_agregado_de_la_ultima_muestra() {
    let mut cpu = Cpu::default();
    assert_eq!(cpu.total_jiffies(), None);
    cpu.apply(parse_stat(STAT_A));
    assert_eq!(cpu.total_jiffies(), Some(483_241_463));
}
```

`src/metrics/procs.rs`, y `pub mod procs;` en `src/metrics/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const STATS: &str = include_str!("../../tests/fixtures/proc_pid_stat");

    fn stat(pid: u32, name: &str, ticks: u64, rss_pages: u64) -> ProcStat {
        ProcStat { pid, name: name.to_string(), ticks, rss_pages }
    }

    #[test]
    fn parsea_utime_stime_y_rss() {
        let all: Vec<ProcStat> = STATS.lines().filter_map(parse_pid_stat).collect();
        assert_eq!(all[0], stat(11742, "cat", 10, 474));
        assert_eq!(all[1], stat(4321, "tmux: server", 1800, 2500));
    }

    #[test]
    fn el_nombre_llega_hasta_el_ultimo_parentesis() {
        let p = parse_pid_stat(STATS.lines().nth(2).unwrap()).unwrap();
        assert_eq!(p, stat(777, "a) b)", 15, 42));
        assert_eq!(parse_pid_stat("basura"), None);
    }

    #[test]
    fn cpu_instantanea_por_nucleo_y_sin_recien_nacidos() {
        let prev = HashMap::from([(1, 100), (2, 50)]);
        let now = vec![stat(1, "uno", 150, 0), stat(2, "dos", 50, 0), stat(3, "nuevo", 999, 0)];
        // 400 jiffies de agregado entre 4 núcleos: 100 por núcleo.
        assert_eq!(
            top_cpu(&prev, &now, 400, 4),
            vec![
                ProcCpu { name: "uno".to_string(), percent: 50.0 },
                ProcCpu { name: "dos".to_string(), percent: 0.0 },
            ]
        );
        assert!(top_cpu(&prev, &now, 0, 4).is_empty());
    }

    #[test]
    fn memoria_por_rss_con_porcentaje_y_tope_de_cinco() {
        let now: Vec<ProcStat> = (1..=7).map(|i| stat(i, &format!("p{i}"), 0, u64::from(i) * 250)).collect();
        let top = top_mem(&now, 4096, 1_000_000.0);
        assert_eq!(top.len(), TOP);
        assert_eq!(top[0], ProcMem { name: "p7".to_string(), kib: 7000.0, percent: 0.7 });
        assert_eq!(top[4].name, "p3");
        // Los hilos del kernel (RSS 0) no entran.
        assert!(top_mem(&[stat(9, "kworker", 0, 0)], 4096, 1_000_000.0).is_empty());
    }

    #[test]
    fn vram_cuenta_cada_cliente_drm_una_vez() {
        let files = [
            "drm-client-id:\t5\ndrm-memory-vram:\t204800 KiB\n",
            "drm-client-id:\t5\ndrm-memory-vram:\t204800 KiB\n",
            "drm-client-id:\t6\ndrm-memory-vram:\t102400 KiB\n",
            "pos:\t0\nflags:\t02\n",
            "drm-driver:\ti915\ndrm-client-id:\t16\ndrm-total-system0:\t259992 KiB\n",
        ];
        assert_eq!(vram_from_fdinfo(files), 300.0);
    }

    #[test]
    fn nvidia_y_drm_se_unen_por_pid_con_el_mayor() {
        let v = |pid: u32, mib: f64| VramProcess { pid, name: format!("p{pid}"), mib };
        let merged = merge_vram(&[v(1, 100.0), v(2, 50.0)], &[v(1, 300.0), v(3, 10.0)]);
        assert_eq!(merged, vec![v(1, 300.0), v(2, 50.0), v(3, 10.0)]);
    }
}
```

- [ ] **Step 3: Verificar que fallan**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo test procs'`
y `… cargo test total_jiffies`
Expected: FAIL — nada existe.

- [ ] **Step 4: `Cpu::total_jiffies`**

En `impl Cpu` de `src/metrics/cpu.rs`:

```rust
/// Jiffies acumulados del agregado en la última muestra. La consume el top de procesos
/// para saber cuánto tiempo pasó sin reloj de pared ni `CLK_TCK`.
pub fn total_jiffies(&self) -> Option<u64> {
    self.prev.first().map(|t| t.total)
}
```

- [ ] **Step 5: `src/metrics/procs.rs`**

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! Top de procesos por CPU, memoria y VRAM, leyendo /proc/N directo. Recorre cientos de
//! directorios, así que sólo corre con su panel a la vista (spec §3.1).
//!
//! La CPU es instantánea, por delta entre dos muestras, y no el promedio de vida del
//! proceso que da `ps -eo pcpu` en el plasmoid (spec §3.7).

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use super::gpu::VramProcess;
use super::read_trimmed;

/// Cinco por lista, como `count == 5` y `head -5` en los comandos del plasmoid.
pub const TOP: usize = 5;

#[derive(Clone, Debug, PartialEq)]
pub struct ProcStat {
    pub pid: u32,
    pub name: String,
    /// utime + stime, en jiffies.
    pub ticks: u64,
    /// En páginas.
    pub rss_pages: u64,
}

/// Una línea de /proc/N/stat. El nombre va del primer `(` al **último** `)`: puede tener
/// espacios y paréntesis adentro.
pub fn parse_pid_stat(raw: &str) -> Option<ProcStat> {
    let open = raw.find('(')?;
    let close = raw.rfind(')')?;
    let pid = raw[..open].trim().parse().ok()?;
    let name = raw.get(open + 1..close)?.to_string();
    let fields: Vec<&str> = raw[close + 1..].split_whitespace().collect();
    // fields[0] es el campo 3 (state): utime es el 14, stime el 15 y rss el 24.
    let num = |i: usize| fields.get(i).and_then(|v| v.parse::<u64>().ok());
    Some(ProcStat {
        pid,
        name,
        ticks: num(11)? + num(12)?,
        rss_pages: num(21)?,
    })
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProcCpu {
    pub name: String,
    /// De **un** núcleo, como top y ps: puede pasar de 100.
    pub percent: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProcMem {
    pub name: String,
    pub kib: f64,
    /// Sobre MemTotal, como la columna `pmem` de ps.
    pub percent: f32,
}

/// El tiempo por núcleo sale del delta del agregado de /proc/stat dividido por la
/// cantidad de núcleos. Los procesos sin muestra previa esperan al tick siguiente.
pub fn top_cpu(prev: &HashMap<u32, u64>, now: &[ProcStat], total_delta: u64, cores: usize) -> Vec<ProcCpu> {
    if total_delta == 0 || cores == 0 {
        return Vec::new();
    }
    let per_core = total_delta as f32 / cores as f32;
    let mut out: Vec<ProcCpu> = now
        .iter()
        .filter_map(|p| {
            let before = *prev.get(&p.pid)?;
            Some(ProcCpu {
                name: p.name.clone(),
                percent: p.ticks.saturating_sub(before) as f32 / per_core * 100.0,
            })
        })
        .collect();
    out.sort_by(|a, b| b.percent.total_cmp(&a.percent));
    out.truncate(TOP);
    out
}

pub fn top_mem(now: &[ProcStat], page_size: u64, mem_total_kib: f64) -> Vec<ProcMem> {
    let mut out: Vec<ProcMem> = now
        .iter()
        .filter(|p| p.rss_pages > 0)
        .map(|p| {
            let kib = (p.rss_pages * page_size) as f64 / 1024.0;
            let percent = if mem_total_kib > 0.0 {
                (kib / mem_total_kib * 100.0) as f32
            } else {
                0.0
            };
            ProcMem { name: p.name.clone(), kib, percent }
        })
        .collect();
    out.sort_by(|a, b| b.kib.total_cmp(&a.kib));
    out.truncate(TOP);
    out
}

/// `drm-memory-vram` de los fdinfo de **un** proceso, en MiB, contando una sola vez cada
/// `drm-client-id` (varios fd pueden ser el mismo cliente). El awk de
/// `_drmGpuProcessesCommand`.
pub fn vram_from_fdinfo<'a>(files: impl IntoIterator<Item = &'a str>) -> f64 {
    let mut seen = HashSet::new();
    let mut total_kib = 0.0;
    for file in files {
        let mut client: Option<&str> = None;
        let mut vram = 0.0;
        for line in file.lines() {
            if let Some(v) = line.strip_prefix("drm-client-id:") {
                client = Some(v.trim());
            } else if let Some(v) = line.strip_prefix("drm-memory-vram:") {
                vram += v
                    .split_whitespace()
                    .next()
                    .and_then(|n| n.parse::<f64>().ok())
                    .unwrap_or(0.0);
            }
        }
        if vram > 0.0 && client.map_or(true, |c| seen.insert(c.to_string())) {
            total_kib += vram;
        }
    }
    total_kib / 1024.0
}

/// Un proceso puede aparecer en nvidia-smi y en DRM: se queda el mayor, como
/// `syncGpuProcesses`.
pub fn merge_vram(a: &[VramProcess], b: &[VramProcess]) -> Vec<VramProcess> {
    let mut out: Vec<VramProcess> = Vec::new();
    for p in a.iter().chain(b) {
        match out.iter_mut().find(|q| q.pid == p.pid) {
            Some(q) => q.mib = q.mib.max(p.mib),
            None => out.push(p.clone()),
        }
    }
    out.sort_by(|x, y| y.mib.total_cmp(&x.mib));
    out.truncate(TOP);
    out
}

fn pids(proc_root: &Path) -> Vec<u32> {
    fs::read_dir(proc_root)
        .map(|it| it.flatten().filter_map(|e| e.file_name().to_str()?.parse().ok()).collect())
        .unwrap_or_default()
}

fn scan(proc_root: &Path) -> Vec<ProcStat> {
    pids(proc_root)
        .into_iter()
        .filter_map(|pid| {
            let raw = fs::read_to_string(proc_root.join(pid.to_string()).join("stat")).ok()?;
            parse_pid_stat(&raw)
        })
        .collect()
}

fn scan_vram(proc_root: &Path) -> Vec<VramProcess> {
    let mut out = Vec::new();
    for pid in pids(proc_root) {
        let dir = proc_root.join(pid.to_string());
        // Procesos de otros usuarios: sin permiso, se saltean.
        let Ok(fds) = fs::read_dir(dir.join("fdinfo")) else {
            continue;
        };
        let contents: Vec<String> = fds.flatten().filter_map(|e| fs::read_to_string(e.path()).ok()).collect();
        let mib = vram_from_fdinfo(contents.iter().map(String::as_str));
        if mib > 0.0 {
            let name = read_trimmed(&dir.join("comm")).unwrap_or_default();
            out.push(VramProcess { pid, name, mib });
        }
    }
    out
}

#[derive(Default)]
pub struct Procs {
    prev_ticks: HashMap<u32, u64>,
    prev_total: Option<u64>,
    pub cpu: Vec<ProcCpu>,
    pub mem: Vec<ProcMem>,
    pub vram: Vec<VramProcess>,
}

impl Procs {
    /// `total_jiffies` es el agregado actual de /proc/stat.
    pub fn refresh_cpu(&mut self, total_jiffies: u64, cores: usize) {
        let now = scan(Path::new("/proc"));
        if let Some(prev_total) = self.prev_total {
            self.cpu = top_cpu(&self.prev_ticks, &now, total_jiffies.saturating_sub(prev_total), cores);
        }
        self.prev_ticks = now.iter().map(|p| (p.pid, p.ticks)).collect();
        self.prev_total = Some(total_jiffies);
    }

    /// Con el panel de CPU cerrado se descarta la base: al reabrirlo, el primer delta
    /// sería el promedio de todo el rato que estuvo cerrado, no el uso de ahora.
    pub fn forget_cpu(&mut self) {
        self.prev_ticks.clear();
        self.prev_total = None;
        self.cpu.clear();
    }

    pub fn refresh_mem(&mut self, page_size: u64, mem_total_mib: f64) {
        self.mem = top_mem(&scan(Path::new("/proc")), page_size, mem_total_mib * 1024.0);
    }

    pub fn refresh_vram(&mut self, nvidia: &[VramProcess]) {
        self.vram = merge_vram(nvidia, &scan_vram(Path::new("/proc")));
    }
}
```

- [ ] **Step 6: Cablear en `Metrics`**

`use procs::Procs;`, campo `pub procs: Procs,`, y `refresh_expensive` completo queda:

```rust
pub fn refresh_expensive(&mut self, detail: Option<Section>) {
    let expensive = expensive_for(detail);
    if expensive.cpu_detail {
        self.cpu_info.refresh();
        self.uptime_secs = cpuinfo::read_uptime();
        if let Some(total) = self.cpu.total_jiffies() {
            self.procs.refresh_cpu(total, self.cpu.core_count());
        }
    } else {
        self.procs.forget_cpu();
    }
    if expensive.ram_detail {
        self.procs.refresh_mem(rustix::param::page_size() as u64, self.mem.total);
    }
    if expensive.storage_detail {
        self.storage.refresh_full();
    }
    if expensive.gpu_detail {
        self.gpu.refresh_nvidia();
        self.procs.refresh_vram(&self.gpu.nvidia_processes);
    }
}
```

- [ ] **Step 7: Verificar que pasan**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo test'`
Expected: PASS, 95 tests.

Contraste manual: con un `yes > /dev/null` corriendo, `top_cpu` tiene que ponerlo cerca de 100 %
(un núcleo entero), mientras `ps -eo pcpu,comm --sort=-pcpu | head -3` lo muestra subiendo de a
poco (promedio de vida): ésa es la diferencia buscada.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "Top de procesos por CPU, memoria y VRAM

CPU instantánea por delta de /proc/N/stat, RSS del mismo archivo y VRAM
de fdinfo contando cada cliente DRM una vez. Sólo con el panel a la vista."
```

---
### Task 9: Dibujo — relleno mínimo, gráfico doble y color desde hex

**Files:**
- Modify: `src/draw.rs`

**Interfaces:**
- Produces:
  - `draw::hex_color(&str) -> cosmic::iced::Color`
  - `draw::critical_only_color(fraction: f32) -> &'static str` — rojo arriba de 0,85, si no normal
  - `draw::usage_meter_with(fraction, w, h: f32, border, fill: &str, min_fill: f32) -> String`
    (`usage_meter` pasa a delegar en ella, sin cambio de salida)
  - `draw::DualGraph<'a> { top, bottom: &'a [f32], top_color, bottom_color: &'a str, gap, top_padding: f32, grid: bool }`
  - `draw::dual_history_graph(g: &DualGraph, w: f32, h: f32, border: &str) -> String`

- [ ] **Step 1: Tests que fallan**

En el bloque de tests de `src/draw.rs`:

```rust
#[test]
fn hex_color_lee_rrggbb() {
    assert_eq!(hex_color("#ff4444"), cosmic::iced::Color::from_rgb8(0xff, 0x44, 0x44));
}

#[test]
fn critical_only_nunca_da_naranja() {
    // `warningThreshold: 1` en las secciones de disco y GPU del QML.
    assert_eq!(critical_only_color(0.8), NORMAL);
    assert_eq!(critical_only_color(0.86), CRITICAL);
}

#[test]
fn el_relleno_minimo_se_respeta_con_uso_chico() {
    let svg = usage_meter_with(0.01, 10.0, 16.0, "#ffffff", NORMAL, 3.0);
    assert!(svg.contains(r#"height="3.00""#), "{svg}");
}

#[test]
fn sin_uso_no_hay_relleno_aunque_haya_minimo() {
    let svg = usage_meter_with(0.0, 10.0, 16.0, "#ffffff", NORMAL, 3.0);
    assert_eq!(svg.matches("<rect").count(), 1, "sólo el borde");
}

fn dual<'a>(top: &'a [f32], bottom: &'a [f32], grid: bool) -> DualGraph<'a> {
    DualGraph {
        top,
        bottom,
        top_color: CRITICAL,
        bottom_color: NORMAL,
        gap: 3.0,
        top_padding: 3.0,
        grid,
    }
}

#[test]
fn grafico_doble_con_grilla_divisor_y_dos_areas() {
    let svg = dual_history_graph(&dual(&[0.0, 1.0], &[1.0, 0.0], true), 100.0, 80.0, "#ffffff");
    // tres líneas de grilla por mitad + el divisor
    assert_eq!(svg.matches("<line").count(), 7);
    assert_eq!(svg.matches("<polygon").count(), 2);
    assert!(!svg.contains("NaN"));
}

#[test]
fn cada_area_queda_en_su_mitad() {
    let svg = dual_history_graph(&dual(&[1.0, 1.0], &[1.0, 1.0], false), 100.0, 80.0, "#ffffff");
    // arriba: base 40-3 = 37, alto 37-3 = 34 → techo en y = 3
    assert!(svg.contains("0.00,3.00"), "{svg}");
    // abajo: base 80, alto 80-(40+3) = 37 → techo en y = 43
    assert!(svg.contains("0.00,43.00"), "{svg}");
    assert_eq!(svg.matches("<line").count(), 1, "sin grilla queda sólo el divisor");
}

#[test]
fn grafico_doble_con_un_punto_no_dibuja_areas() {
    let svg = dual_history_graph(&dual(&[0.5], &[], true), 100.0, 80.0, "#ffffff");
    assert_eq!(svg.matches("<polygon").count(), 0);
    assert!(!svg.contains("NaN") && !svg.contains("inf"));
}
```

- [ ] **Step 2: Verificar que fallan**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo test draw'`
Expected: FAIL — las funciones nuevas no existen.

- [ ] **Step 3: Implementar**

Debajo de `threshold_color`:

```rust
/// Disco y GPU en el panel: `warningThreshold: 1`, así que nunca pasan por naranja.
pub fn critical_only_color(fraction: f32) -> &'static str {
    if fraction > CRITICAL_THRESHOLD {
        CRITICAL
    } else {
        NORMAL
    }
}

/// `#rrggbb` a color de iced, para pintar texto con los colores de umbral.
pub fn hex_color(hex: &str) -> cosmic::iced::Color {
    let v = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0x00ff_ffff);
    cosmic::iced::Color::from_rgb8((v >> 16) as u8, (v >> 8) as u8, v as u8)
}
```

`usage_meter` pasa a:

```rust
/// Medidor vertical único (RAM), con color por umbral.
pub fn usage_meter(fraction: f32, w: f32, h: f32, border: &str) -> String {
    let fraction = fraction.clamp(0.0, 1.0);
    usage_meter_with(fraction, w, h, border, threshold_color(fraction), 0.0)
}

/// Medidor vertical con color y relleno mínimo explícitos: `VerticalUsageMeter.qml`.
/// `min_fill` sólo actúa si hay uso — en cero no se dibuja nada.
pub fn usage_meter_with(fraction: f32, w: f32, h: f32, border: &str, fill: &str, min_fill: f32) -> String {
    let mut svg = String::with_capacity(240);
    header(&mut svg, w, h, border);

    let fraction = fraction.clamp(0.0, 1.0);
    if fraction > 0.0 {
        let inner_h = (h - 2.0).max(0.0);
        let bar_h = (inner_h * fraction).max(min_fill).min(inner_h);
        let _ = write!(
            svg,
            r#"<rect x="1" y="{:.2}" width="{:.2}" height="{bar_h:.2}" rx="1" ry="1" fill="{fill}"/>"#,
            1.0 + inner_h - bar_h,
            (w - 2.0).max(0.0),
        );
    }

    svg.push_str("</svg>");
    svg
}
```

Al final del archivo, antes de los tests:

```rust
/// Dos series en mitades separadas, las dos creciendo hacia arriba: el Canvas de
/// `NetworkDetail.qml` (subida arriba, bajada abajo), `StorageDetail.qml` (lectura
/// arriba, escritura abajo) y el mini gráfico de red del panel.
pub struct DualGraph<'a> {
    /// Valores 0–1.
    pub top: &'a [f32],
    pub bottom: &'a [f32],
    pub top_color: &'a str,
    pub bottom_color: &'a str,
    /// Separación de cada área respecto del divisor central.
    pub gap: f32,
    /// Margen superior del área de arriba.
    pub top_padding: f32,
    /// Tres líneas de grilla por mitad, como los paneles de detalle. El panel no la usa.
    pub grid: bool,
}

pub fn dual_history_graph(g: &DualGraph, w: f32, h: f32, border: &str) -> String {
    let mut svg = String::with_capacity(400 + (g.top.len() + g.bottom.len()) * 16);
    header(&mut svg, w, h, border);

    let half = h / 2.0;
    let top_base = half - g.gap;
    let top_h = (top_base - g.top_padding).max(0.0);
    let bottom_h = (h - (half + g.gap)).max(0.0);

    if g.grid {
        for q in [0.25f32, 0.5, 0.75] {
            grid_line(&mut svg, top_base - top_h * q, w, border);
            grid_line(&mut svg, h - bottom_h * q, w, border);
        }
    }
    let _ = write!(
        svg,
        r#"<line x1="0" y1="{half:.2}" x2="{w:.2}" y2="{half:.2}" stroke="{border}" stroke-opacity="{BORDER_OPACITY}" stroke-width="1"/>"#
    );
    area(&mut svg, g.top, w, top_base, top_h, g.top_color);
    area(&mut svg, g.bottom, w, h, bottom_h, g.bottom_color);

    svg.push_str("</svg>");
    svg
}

fn grid_line(svg: &mut String, y: f32, w: f32, border: &str) {
    let _ = write!(
        svg,
        r#"<line x1="1" y1="{y:.2}" x2="{:.2}" y2="{y:.2}" stroke="{border}" stroke-opacity="0.12" stroke-width="1"/>"#,
        w - 1.0
    );
}

/// Área bajo la curva con base en `base` y `height` de alto. Con menos de dos puntos no
/// hay curva que dibujar.
fn area(svg: &mut String, points: &[f32], w: f32, base: f32, height: f32, color: &str) {
    if points.len() < 2 {
        return;
    }
    let step = w / (points.len() - 1) as f32;
    let mut poly = String::with_capacity(points.len() * 14 + 32);
    for (i, p) in points.iter().enumerate() {
        let _ = write!(poly, "{:.2},{:.2} ", step * i as f32, base - height * p.clamp(0.0, 1.0));
    }
    let _ = write!(poly, "{w:.2},{base:.2} 0.00,{base:.2}");
    let _ = write!(
        svg,
        r#"<polygon points="{poly}" fill="{color}" fill-opacity="0.2" stroke="{color}" stroke-width="1"/>"#
    );
}
```

- [ ] **Step 4: Verificar que pasan**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo test'`
Expected: PASS, 102 tests (los de `usage_meter` previos no cambian).

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "Medidor con relleno mínimo y gráfico de dos series

Lo que necesitan las secciones de disco, GPU y red: color sin naranja,
relleno mínimo como VerticalUsageMeter.qml y el Canvas partido en dos."
```

---

### Task 10: Secciones del panel para temperaturas, red, disco y GPU

**Files:**
- Create: `res/icons/am-{temperature,network,harddisk,gpu,up,down}-symbolic.svg`
- Modify: `src/app.rs`, `src/ui/compact.rs`

**Interfaces:**
- Consumes: todo `Metrics` (Tasks 1–8), `draw::*` (Task 9), `format::{percent, compact_rate}` (Task 2).
- Produces:
  - constantes `app::{TEMP_ICON, NET_ICON, DISK_ICON, GPU_ICON, UP_ICON, DOWN_ICON}: &[u8]`
  - `SysMon::icon` pasa a `pub`; `SysMon::faint_text_color(&self) -> cosmic::iced::Color`
  - `ui::compact::section_content` devuelve contenido para las seis secciones (GPU: `None`
    si `!gpu.available()`)

- [ ] **Step 1: Íconos**

```bash
S=~/.local/share/plasma/plasmoids/com.labatata.sysmonitor/contents/icons/hicolor/scalable/actions
for i in temperature network harddisk gpu up down; do cp "$S/am-$i-symbolic.svg" res/icons/; done
```

Son del mismo plasmoid GPL-3.0 que los de CPU y RAM ya incluidos.

- [ ] **Step 2: Test que falla**

En el bloque de tests de `src/ui/compact.rs`:

```rust
#[test]
fn dos_renglones_entran_en_el_alto_del_icono() {
    assert_eq!(two_line_px(16.0), 10.0);
    assert_eq!(two_line_px(32.0), 19.0);
    // nunca menos de 8 px, ilegible
    assert_eq!(two_line_px(10.0), 8.0);
}
```

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo test compact'`
Expected: FAIL — `two_line_px` no existe.

- [ ] **Step 3: `src/app.rs`**

Debajo de `RAM_ICON`:

```rust
pub const TEMP_ICON: &[u8] = include_bytes!("../res/icons/am-temperature-symbolic.svg");
pub const NET_ICON: &[u8] = include_bytes!("../res/icons/am-network-symbolic.svg");
pub const DISK_ICON: &[u8] = include_bytes!("../res/icons/am-harddisk-symbolic.svg");
pub const GPU_ICON: &[u8] = include_bytes!("../res/icons/am-gpu-symbolic.svg");
pub const UP_ICON: &[u8] = include_bytes!("../res/icons/am-up-symbolic.svg");
pub const DOWN_ICON: &[u8] = include_bytes!("../res/icons/am-down-symbolic.svg");
```

`fn icon<'a>(&self, …)` pasa a `pub fn icon<'a>(&self, …)`. Y `text_color_hex` se parte para
compartir el color base:

```rust
/// Color del texto del panel (RGB 0–1), del tema del applet.
fn on_bg(&self) -> [f32; 3] {
    let theme = self
        .core
        .applet
        .theme()
        .unwrap_or_else(cosmic::theme::active);
    let c = theme.cosmic().on_bg_color();
    [c.red, c.green, c.blue]
}

/// Color del texto del panel en hexadecimal, para los bordes de los medidores.
pub fn text_color_hex(&self) -> String {
    let [r, g, b] = self.on_bg();
    let byte = |v: f32| (v * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", byte(r), byte(g), byte(b))
}

/// Texto al 35 %: `themePlaceholderTextColor` del plasmoid.
pub fn faint_text_color(&self) -> cosmic::iced::Color {
    let [r, g, b] = self.on_bg();
    cosmic::iced::Color::from_rgba(r, g, b, 0.35)
}
```

- [ ] **Step 4: `src/ui/compact.rs`**

Imports:

```rust
use cosmic::iced::{Alignment, Length};
use cosmic::widget::{self, Column, Row};
use cosmic::Element;

use crate::app::{Message, SysMon};
use crate::config::Section;
use crate::draw::{self, DualGraph};
```

En `view`, el comentario del botón de resguardo pasa a decir que el panel se queda sin botones
cuando todos los `show_*` están apagados (o sólo queda GPU sin fuente de datos); el código no
cambia.

`section_content` se reemplaza entero, y se agregan tres helpers:

```rust
/// Contenido de una sección del panel, o `None` si no hay nada que mostrar (GPU sin
/// ninguna fuente de datos, spec §3.5).
pub fn section_content<'a>(app: &'a SysMon, s: Section) -> Option<Element<'a, Message>> {
    let icon_size = app.core().applet.suggested_size(true).1;
    let h = f32::from(icon_size);
    let scale = h / crate::app::REFERENCE_ICON_SIZE;
    let border = app.text_color_hex();
    let spacing = app.spacing();
    let m = app.metrics();

    match s {
        Section::Cpu => {
            let w = cpu_box_width(m.cpu.core_count().max(1) as f32, h, scale);
            Some(app.section(
                crate::app::CPU_ICON,
                icon_size,
                draw::cpu_bars(&m.cpu.cores, w, h, &border),
                w,
                h,
                crate::format::percent(m.cpu.total),
                spacing,
            ))
        }
        Section::Ram => {
            let w = (h * 0.7).round();
            let fraction = m.mem.fraction();
            Some(app.section(
                crate::app::RAM_ICON,
                icon_size,
                draw::usage_meter(fraction, w, h, &border),
                w,
                h,
                format!("{:.0}%", fraction * 100.0),
                spacing,
            ))
        }
        Section::Storage => {
            // `storageDevices[0]`: el primer montaje. Sin porcentaje escrito, sin naranja
            // y con relleno mínimo de 3 px, como la sección de CompactView.qml.
            let fraction = m.storage.primary().map(|u| u.percent as f32 / 100.0).unwrap_or(0.0);
            let w = (h * 0.7).round();
            let svg = draw::usage_meter_with(fraction, w, h, &border, draw::critical_only_color(fraction), 3.0 * scale);
            Some(
                Row::new()
                    .spacing(spacing)
                    .align_y(Alignment::Center)
                    .push(app.icon(crate::app::DISK_ICON, icon_size))
                    .push(meter(svg, w, h))
                    .into(),
            )
        }
        Section::Gpu => {
            if !m.gpu.available() {
                return None;
            }
            let usage = m.gpu.summary().map(|d| d.usage).unwrap_or(0.0);
            let fraction = usage / 100.0;
            let w = (h * 0.7).round();
            let svg = draw::usage_meter_with(fraction, w, h, &border, draw::critical_only_color(fraction), 0.0);
            Some(app.section(crate::app::GPU_ICON, icon_size, svg, w, h, crate::format::percent(usage), spacing))
        }
        Section::Temps => {
            // Las dos primeras lecturas, como el Repeater de `min(temperatures.length, 2)`.
            let px = two_line_px(h);
            let mut lines = Column::new();
            for i in 0..2 {
                let line: Element<'a, Message> = match m.temps.get(i) {
                    Some(r) => {
                        let text = small_text(format!("{:.1} °C", r.celsius), px);
                        match crate::metrics::temp::severity_color(r.celsius) {
                            Some(hex) => text.class(cosmic::theme::Text::Color(draw::hex_color(hex))).into(),
                            None => text.into(),
                        }
                    }
                    None => small_text("-- °C".to_string(), px)
                        .class(cosmic::theme::Text::Color(app.faint_text_color()))
                        .into(),
                };
                lines = lines.push(line);
            }
            Some(
                Row::new()
                    .spacing(spacing)
                    .align_y(Alignment::Center)
                    .push(app.icon(crate::app::TEMP_ICON, icon_size))
                    .push(lines)
                    .into(),
            )
        }
        Section::Network => {
            let px = two_line_px(h);
            let graph_w = (36.0 * scale).round();
            let graph = draw::dual_history_graph(
                &DualGraph {
                    top: &m.net_up_history.points(),
                    bottom: &m.net_down_history.points(),
                    top_color: draw::CRITICAL,
                    bottom_color: draw::NORMAL,
                    gap: 2.0,
                    top_padding: 2.0,
                    grid: false,
                },
                graph_w,
                h,
                &border,
            );
            let rates = Column::new()
                .push(rate_line(app, crate::app::UP_ICON, m.net.tx_rate, px))
                .push(rate_line(app, crate::app::DOWN_ICON, m.net.rx_rate, px));
            Some(
                Row::new()
                    .spacing(spacing)
                    .align_y(Alignment::Center)
                    .push(app.icon(crate::app::NET_ICON, icon_size))
                    .push(meter(graph, graph_w, h))
                    .push(rates)
                    .into(),
            )
        }
    }
}

fn meter<'a>(svg: String, w: f32, h: f32) -> Element<'a, Message> {
    widget::svg(widget::svg::Handle::from_memory(svg.into_bytes()))
        .width(Length::Fixed(w))
        .height(Length::Fixed(h))
        .into()
}

/// Letra de las secciones de dos renglones (temperaturas y red): 60 % del ícono, para
/// que los dos renglones entren en el alto del panel. Nunca menos de 8 px.
fn two_line_px(h: f32) -> f32 {
    (h * 0.6).round().max(8.0)
}

fn small_text<'a>(value: String, px: f32) -> widget::Text<'a, cosmic::Theme> {
    widget::text(value).size(px).font(cosmic::font::bold())
}

/// Flecha + tasa compacta, un renglón de la sección de red.
fn rate_line<'a>(app: &'a SysMon, arrow: &'static [u8], rate: f64, px: f32) -> Element<'a, Message> {
    Row::new()
        .spacing(2)
        .align_y(Alignment::Center)
        .push(app.icon(arrow, px as u16))
        .push(small_text(crate::format::compact_rate(rate), px))
        .into()
}
```

`cpu_box_width` y sus tests quedan como están.

- [ ] **Step 5: Compilar y testear**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo build --release && cargo test'`
Expected: compila; PASS, 103 tests.

- [ ] **Step 6: Verificar en el panel**

```bash
./install.sh
# Matar sólo el applet no lo relanza: cosmic-session relanza el panel entero.
pkill -x cosmic-panel; sleep 4
cosmic-screenshot --interactive=false --modal=false --notify=false -s "$SCRATCH"
```

(`$SCRATCH` = el scratchpad de la sesión.) Recortar la franja del panel y mirarla.
Expected: en orden default se ven temperaturas (dos renglones), red (mini gráfico + ↑/↓), disco
(medidor), CPU, RAM; en `casa` (Intel) no aparece GPU. Nada se sale del alto del panel.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "Secciones del panel para temperaturas, red, disco y GPU

Las cuatro secciones de CompactView.qml que faltaban. GPU se oculta si
no hay ninguna fuente de datos."
```

---
### Task 11: Los seis paneles de detalle

**Files:**
- Create: `src/ui/detail/{cpu,ram,net,storage,temp,gpu}.rs`
- Modify: `src/ui/detail/mod.rs`, `src/metrics/mem.rs`

**Interfaces:**
- Consumes: todo `Metrics`, `format::*`, `draw::*`, `SysMon::{icon, text_color_hex, config}`.
- Produces:
  - `Mem` suma `pub free: f64` (MiB, `ramFree` del QML); se borra `mem::format_mib`, que queda sin uso
  - `ui::detail::CONTENT_W: f32`
  - un `pub fn view(app: &SysMon) -> Element<'_, Message>` por panel

Rótulos traducidos de los QML: Total/Usuario/Sistema, Total/Usada/Libre/En caché, Subida/Bajada,
Lectura/Escritura, Reloj/Memoria/Temperatura/Uso, "Procesos", "Swap", "Dispositivos",
"Tiempo encendido". El título de cada panel es `Section::label()`, como `DetailHeader.qml`
(que en el QML sólo dibuja el título).

- [ ] **Step 1: Test que falla**

En `src/metrics/mem.rs`, reemplazar el test `format_mib_cambia_a_gib_en_1024` por:

```rust
#[test]
fn libre_es_free_mas_buffers_mas_cached() {
    let m = Mem::from_fields(parse_meminfo(MEMINFO));
    // (2935564 + 1912) / 1024 + 20052.004
    assert!((m.free - 22_920.633).abs() < 0.01);
    assert!((m.total - m.free - m.used).abs() < 0.001);
}
```

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo test mem'`
Expected: FAIL — `Mem` no tiene `free`.

- [ ] **Step 2: `Mem::free`**

En `struct Mem`, después de `used`: `/// Libre + buffers + caché: lo que el QML llama ramFree.` `pub free: f64,`.
En `from_fields`, el `Self { … }` suma `free,` (la variable local ya existe). Borrar `format_mib`
con su doc comment.

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo test mem'`
Expected: PASS (el build del binario falla hasta el Step 3 porque `ui::detail` usa `format_mib`).

- [ ] **Step 3: `src/ui/detail/mod.rs`**

Reemplazar todo lo que hay debajo de `system_monitor_command` (constantes de gráfico,
`history_view`, `cpu_detail`, `ram_detail` y `view`) por los helpers y el ruteo nuevos. El
encabezado del archivo queda:

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! Contenido del popup: rutea por la sección elegida (`SysMon::selected`) y agrega el pie
//! con el monitor de sistema y los ajustes. Cada panel vive en su archivo; acá quedan los
//! widgets que comparten.

mod cpu;
mod gpu;
mod net;
mod ram;
mod storage;
mod temp;

use cosmic::iced::{Alignment, Length};
use cosmic::widget::{self, Column, Row};
use cosmic::Element;

use crate::app::{Message, SysMon};
use crate::config::Section;
use crate::draw::DualGraph;
use crate::metrics::history::window_label;
```

(`CANDIDATES` y `system_monitor_command` quedan como los dejó la Task 7.) Después:

```rust
/// Ancho del contenido del popup: `gridUnit * 20` del plasmoid, menos márgenes.
pub const CONTENT_W: f32 = 300.0;

fn svg<'a>(svg: String, w: f32, h: f32) -> Element<'a, Message> {
    widget::svg(widget::svg::Handle::from_memory(svg.into_bytes()))
        .width(Length::Fixed(w))
        .height(Length::Fixed(h))
        .into()
}

/// `StatRow.qml`: rótulo a la izquierda, valor en negrita a la derecha.
fn stat_row<'a>(label: &'a str, value: String) -> Element<'a, Message> {
    Row::new()
        .width(Length::Fill)
        .push(widget::text::body(label).width(Length::Fill))
        .push(widget::text::body(value).font(cosmic::font::bold()))
        .into()
}

/// Título de bloque centrado ("Procesos", "Swap", "Dispositivos").
fn block_title(text: &str) -> Element<'_, Message> {
    widget::container(widget::text::heading(text))
        .center_x(Length::Fill)
        .into()
}

fn centered_caption<'a>(text: String) -> Element<'a, Message> {
    widget::container(widget::text::caption(text))
        .center_x(Length::Fill)
        .into()
}

/// Barra horizontal con el valor escrito a la derecha.
fn bar_row<'a>(fraction: f32, fill: &str, border: &str, value: String) -> Element<'a, Message> {
    let w = CONTENT_W - 48.0;
    Row::new()
        .spacing(8)
        .align_y(Alignment::Center)
        .push(svg(crate::draw::horizontal_bar(fraction, w, 10.0, fill, border), w, 10.0))
        .push(widget::text::caption(value))
        .into()
}

/// Rótulos del eje de tiempo. El extremo izquierdo sale del intervalo configurado.
fn time_axis(app: &SysMon) -> Element<'_, Message> {
    Row::new()
        .width(Length::Fill)
        .push(widget::text::caption(window_label(app.config().update_interval)))
        .push(widget::space::horizontal())
        .push(widget::text::caption("ahora"))
        .into()
}

/// Gráfico de una serie con su eje de tiempo.
fn history_view<'a>(app: &'a SysMon, points: &[f32], h: f32) -> Element<'a, Message> {
    let graph = crate::draw::history_graph(points, CONTENT_W, h, crate::draw::NORMAL, &app.text_color_hex());
    Column::new()
        .spacing(2)
        .push(svg(graph, CONTENT_W, h))
        .push(time_axis(app))
        .into()
}

/// Gráfico de dos series con su eje de tiempo.
fn dual_view<'a>(app: &'a SysMon, graph: &DualGraph, h: f32) -> Element<'a, Message> {
    let drawn = crate::draw::dual_history_graph(graph, CONTENT_W, h, &app.text_color_hex());
    Column::new()
        .spacing(2)
        .push(svg(drawn, CONTENT_W, h))
        .push(time_axis(app))
        .into()
}

/// Muestra de color + texto: las leyendas de red y disco.
fn legend<'a>(items: Vec<(&str, String)>) -> Element<'a, Message> {
    let mut row = Row::new().spacing(6).align_y(Alignment::Center);
    for (color, text) in items {
        let swatch = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="12" height="3"><rect width="12" height="3" rx="1" fill="{color}"/></svg>"#
        );
        row = row.push(svg(swatch, 12.0, 3.0)).push(widget::text::caption(text));
    }
    row.into()
}

/// Renglones "nombre … valor" de los tops de procesos.
fn process_rows<'a>(rows: impl IntoIterator<Item = (String, String)>) -> Element<'a, Message> {
    let mut col = Column::new().spacing(2);
    for (name, value) in rows {
        col = col.push(
            Row::new()
                .width(Length::Fill)
                .push(widget::text::body(name).font(cosmic::font::bold()).width(Length::Fill))
                .push(widget::text::body(value)),
        );
    }
    col.into()
}

/// Contenido del popup: el panel de la sección elegida, o los ajustes si se abrió el
/// engranaje, más el pie.
pub fn view(app: &SysMon) -> Element<'_, Message> {
    if app.showing_settings() {
        return app
            .core()
            .applet
            .popup_container(crate::ui::settings::view(app))
            .into();
    }

    let section = app.selected();
    let body: Element<Message> = match section {
        Section::Cpu => cpu::view(app),
        Section::Ram => ram::view(app),
        Section::Network => net::view(app),
        Section::Storage => storage::view(app),
        Section::Temps => temp::view(app),
        Section::Gpu => gpu::view(app),
    };

    let mut footer = Row::new().spacing(8).align_y(Alignment::Center);
    if system_monitor_command().is_some() {
        footer = footer.push(
            widget::button::icon(widget::icon::from_name("utilities-system-monitor-symbolic"))
                .on_press(Message::OpenSystemMonitor),
        );
    }
    footer = footer.push(
        widget::button::icon(widget::icon::from_name("preferences-system-symbolic"))
            .on_press(Message::OpenSettings),
    );

    let content = Column::new()
        .spacing(8)
        .padding(12)
        .width(Length::Fixed(CONTENT_W + 24.0))
        .push(widget::container(widget::text::title4(section.label())).center_x(Length::Fill))
        .push(body)
        .push(widget::divider::horizontal::default())
        .push(widget::container(footer).center_x(Length::Fill));

    app.core().applet.popup_container(content).into()
}
```

- [ ] **Step 4: `src/ui/detail/cpu.rs`**

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! `CpuDetail.qml`: modelo y reloj, total/usuario/sistema, barra, historial, núcleos, top
//! de procesos y tiempo encendido.

use cosmic::iced::{Alignment, Length};
use cosmic::widget::{self, Column, Row};
use cosmic::Element;

use super::{bar_row, block_title, centered_caption, history_view, process_rows, stat_row, svg};
use crate::app::{Message, SysMon};
use crate::draw::{CORE_COLORS, CRITICAL, NORMAL};
use crate::format;

pub fn view(app: &SysMon) -> Element<'_, Message> {
    let m = app.metrics();
    let border = app.text_color_hex();
    let model = if m.cpu_info.model.is_empty() { "CPU" } else { m.cpu_info.model.as_str() };
    let clock = format::clock(m.cpu_info.mhz);
    let header = if clock.is_empty() { model.to_string() } else { format!("{model} @ {clock}") };
    let total = m.cpu.total;

    Column::new()
        .spacing(6)
        .push(centered_caption(header))
        .push(stat_row("Total:", format!("{total:.0}%")))
        .push(stat_row("Usuario:", format!("{:.0}%", m.cpu.user)))
        .push(stat_row("Sistema:", format!("{:.0}%", m.cpu.system)))
        .push(bar_row(total / 100.0, if total > 80.0 { CRITICAL } else { NORMAL }, &border, format!("{total:.0}%")))
        .push(history_view(app, &m.cpu_history.points(), 58.0))
        .push(cores(app, &border))
        .push(block_title("Procesos"))
        .push(process_rows(m.procs.cpu.iter().map(|p| (p.name.clone(), format!("{:.1}%", p.percent)))))
        .push(block_title("Tiempo encendido"))
        .push(centered_caption(format::uptime(m.uptime_secs)))
        .into()
}

/// Un renglón por núcleo con el color que tiene en el panel. El plasmoid los muestra en un
/// popup flotante al pasar el mouse por el gráfico (`CpuCoreInfo.qml`); un applet de
/// COSMIC no tiene esos tooltips, así que van en línea.
fn cores<'a>(app: &'a SysMon, border: &str) -> Element<'a, Message> {
    const BAR_W: f32 = 190.0;
    let mut col = Column::new().spacing(4);
    for (i, usage) in app.metrics().cpu.cores.iter().enumerate() {
        let bar = crate::draw::horizontal_bar(usage / 100.0, BAR_W, 8.0, CORE_COLORS[i % CORE_COLORS.len()], border);
        col = col.push(
            Row::new()
                .spacing(8)
                .align_y(Alignment::Center)
                .push(widget::text::caption(format!("cpu{i}")).width(Length::Fixed(48.0)))
                .push(svg(bar, BAR_W, 8.0))
                .push(widget::text::caption(format!("{usage:.0}%"))),
        );
    }
    col.into()
}
```

- [ ] **Step 5: `src/ui/detail/ram.rs`**

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! `RamDetail.qml`: total/usada/libre/en caché, historial, barra, top de procesos y swap.

use cosmic::widget::Column;
use cosmic::Element;

use super::{bar_row, block_title, history_view, process_rows, stat_row};
use crate::app::{Message, SysMon};
use crate::draw::{CRITICAL, NORMAL, WARNING};
use crate::format;

pub fn view(app: &SysMon) -> Element<'_, Message> {
    let m = app.metrics();
    let mem = &m.mem;
    let border = app.text_color_hex();
    let fraction = mem.fraction();
    let swap_fraction = if mem.swap_total > 0.0 {
        (mem.swap_used / mem.swap_total).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    // "..." mientras no hay lectura, como el QML.
    let gb = |mib: f64| if mib > 0.0 { format::gb2(mib) } else { "...".to_string() };

    Column::new()
        .spacing(6)
        .push(stat_row("Total:", gb(mem.total)))
        .push(stat_row("Usada:", gb(mem.used)))
        .push(stat_row("Libre:", gb(mem.free)))
        .push(stat_row("En caché:", gb(mem.cached)))
        .push(history_view(app, &m.ram_history.points(), 58.0))
        .push(bar_row(
            fraction,
            if fraction > 0.85 { CRITICAL } else { NORMAL },
            &border,
            format!("{:.0}%", fraction * 100.0),
        ))
        .push(block_title("Procesos"))
        .push(process_rows(m.procs.mem.iter().map(|p| {
            (p.name.clone(), format!("{}  {:.1}%", format::memory_kib(p.kib), p.percent))
        })))
        .push(block_title("Swap"))
        .push(stat_row(
            "Total:",
            if mem.swap_total > 0.0 { format::gb2(mem.swap_total) } else { "Ninguna".to_string() },
        ))
        .push(stat_row(
            "Usada:",
            if mem.swap_used > 0.0 { format::gb2(mem.swap_used) } else { "0 GB".to_string() },
        ))
        .push(bar_row(swap_fraction, WARNING, &border, format!("{:.0}%", swap_fraction * 100.0)))
        .into()
}
```

- [ ] **Step 6: `src/ui/detail/net.rs`**

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! `NetworkDetail.qml`: subida y bajada, gráfico partido y leyenda. Sin barra.

use cosmic::widget::Column;
use cosmic::Element;

use super::{dual_view, legend, stat_row};
use crate::app::{Message, SysMon};
use crate::draw::{DualGraph, CRITICAL, NORMAL};
use crate::format;

pub fn view(app: &SysMon) -> Element<'_, Message> {
    let m = app.metrics();
    Column::new()
        .spacing(6)
        .push(stat_row("Subida:", format::rate(m.net.tx_rate)))
        .push(stat_row("Bajada:", format::rate(m.net.rx_rate)))
        .push(dual_view(
            app,
            &DualGraph {
                top: &m.net_up_history.points(),
                bottom: &m.net_down_history.points(),
                top_color: CRITICAL,
                bottom_color: NORMAL,
                gap: 3.0,
                top_padding: 3.0,
                grid: true,
            },
            80.0,
        ))
        .push(legend(vec![(NORMAL, "Bajada".to_string()), (CRITICAL, "Subida".to_string())]))
        .into()
}
```

- [ ] **Step 7: `src/ui/detail/storage.rs`**

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! `StorageDetail.qml`: gráfico de lectura/escritura con escala fija, leyenda y
//! dispositivos con su uso.

use cosmic::iced::Length;
use cosmic::widget::{self, Column, Row};
use cosmic::Element;

use super::{bar_row, block_title, centered_caption, dual_view, legend};
use crate::app::{Message, SysMon};
use crate::draw::{threshold_color, DualGraph, CRITICAL, NORMAL};
use crate::format;
use crate::metrics::disk::GRAPH_SCALE;

pub fn view(app: &SysMon) -> Element<'_, Message> {
    let m = app.metrics();
    let border = app.text_color_hex();
    let io = &m.storage.io;

    let mut col = Column::new()
        .spacing(6)
        .push(dual_view(
            app,
            &DualGraph {
                top: &m.disk_read_history.points(),
                bottom: &m.disk_write_history.points(),
                top_color: NORMAL,
                bottom_color: CRITICAL,
                gap: 3.0,
                top_padding: 0.0,
                grid: true,
            },
            80.0,
        ))
        .push(legend(vec![
            (NORMAL, format!("Lectura {}", format::rate(io.read_rate))),
            (CRITICAL, format!("Escritura {}", format::rate(io.write_rate))),
        ]))
        // El QML rotula el eje a la derecha; con escala fija alcanza con decirla una vez.
        .push(widget::text::caption(format!("Escala: {} por mitad", format::rate(GRAPH_SCALE))))
        .push(block_title("Dispositivos"));

    for dev in &m.storage.devices {
        let size = match &dev.usage {
            Some(u) => format!(
                "{} / {}",
                format::storage_bytes(u.used as f64),
                format::storage_bytes(u.size as f64)
            ),
            None => format::storage_bytes(dev.size as f64),
        };
        col = col
            .push(
                Row::new()
                    .width(Length::Fill)
                    .push(widget::text::body(dev.name.as_str()).font(cosmic::font::bold()).width(Length::Fill))
                    .push(widget::text::caption(size)),
            )
            .push(widget::text::caption(format!("{} • {}", dev.path, dev.detail)));
        if let Some(u) = &dev.usage {
            let fraction = u.percent as f32 / 100.0;
            col = col.push(bar_row(fraction, threshold_color(fraction), &border, format!("{}%", u.percent)));
        }
    }
    if m.storage.devices.is_empty() {
        col = col.push(centered_caption("No se encontraron dispositivos de almacenamiento".to_string()));
    }
    col.into()
}
```

- [ ] **Step 8: `src/ui/detail/temp.rs`**

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! `TempDetail.qml`: un renglón por sensor con barra sobre 110 °C y color por umbral.

use cosmic::iced::{Alignment, Length};
use cosmic::widget::{self, Column, Row};
use cosmic::Element;

use super::{centered_caption, svg};
use crate::app::{Message, SysMon, TEMP_ICON};
use crate::draw::{hex_color, horizontal_bar, NORMAL};
use crate::metrics::temp::severity_color;

pub fn view(app: &SysMon) -> Element<'_, Message> {
    let m = app.metrics();
    let border = app.text_color_hex();
    let mut col = Column::new().spacing(4);
    for r in &m.temps {
        let color = severity_color(r.celsius);
        let bar = horizontal_bar((r.celsius / 110.0).min(1.0), 60.0, 12.0, color.unwrap_or(NORMAL), &border);
        let value = widget::text::body(format!("{:.1} °C", r.celsius)).font(cosmic::font::bold());
        let value: Element<Message> = match color {
            Some(hex) => value.class(cosmic::theme::Text::Color(hex_color(hex))).into(),
            None => value.into(),
        };
        col = col.push(
            Row::new()
                .spacing(6)
                .align_y(Alignment::Center)
                .push(app.icon(TEMP_ICON, 12))
                .push(widget::text::caption(r.label.as_str()).width(Length::Fill))
                .push(svg(bar, 60.0, 12.0))
                .push(widget::container(value).width(Length::Fixed(64.0))),
        );
    }
    // El QML pide instalar lm-sensors; acá se lee hwmon directo, así que lo único que puede
    // faltar son sensores expuestos por el kernel.
    if m.temps.is_empty() {
        col = col.push(centered_caption("No hay sensores de temperatura en /sys/class/hwmon".to_string()));
    }
    col.into()
}
```

- [ ] **Step 9: `src/ui/detail/gpu.rs`**

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! `GpuDetail.qml`: por placa, nombre, reloj/memoria/temperatura/uso, barra e historial;
//! después el top de procesos por VRAM.

use cosmic::iced::Length;
use cosmic::widget::{self, Column};
use cosmic::Element;

use super::{bar_row, block_title, centered_caption, history_view, process_rows, stat_row};
use crate::app::{Message, SysMon};
use crate::draw::{CRITICAL, NORMAL};
use crate::format;

pub fn view(app: &SysMon) -> Element<'_, Message> {
    let m = app.metrics();
    let border = app.text_color_hex();
    let unavailable = || "no disponible".to_string();
    let devices = m.gpu.devices();

    let mut col = Column::new().spacing(6);
    if devices.is_empty() {
        col = col.push(centered_caption("No se detectó ninguna GPU con datos de uso".to_string()));
    }
    for d in devices {
        let clock = format::clock(d.clock_mhz);
        let memory = if d.memory_total_mib > 0.0 {
            format!("{} / {}", format::memory_mib(d.memory_used_mib), format::memory_mib(d.memory_total_mib))
        } else {
            unavailable()
        };
        let history = m.gpu.history(&d.id).map(|h| h.points()).unwrap_or_default();
        col = col
            .push(widget::container(widget::text::heading(d.name.clone())).center_x(Length::Fill))
            .push(stat_row("Reloj:", if clock.is_empty() { unavailable() } else { clock }))
            .push(stat_row("Memoria:", memory))
            .push(stat_row(
                "Temperatura:",
                if d.temperature > 0.0 { format!("{:.0} °C", d.temperature) } else { unavailable() },
            ))
            .push(stat_row("Uso:", format::percent(d.usage)))
            .push(bar_row(
                d.usage / 100.0,
                if d.usage > 85.0 { CRITICAL } else { NORMAL },
                &border,
                format!("{:.0}%", d.usage),
            ))
            .push(history_view(app, &history, 80.0));
    }
    col.push(block_title("Procesos de GPU"))
        .push(process_rows(m.procs.vram.iter().map(|p| (p.name.clone(), format::memory_mib(p.mib)))))
        .into()
}
```

- [ ] **Step 10: Compilar y testear**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo build --release && cargo test'`
Expected: compila; PASS, 103 tests.

- [ ] **Step 11: Verificar a mano**

`./install.sh`, `pkill -x cosmic-panel`, y abrir cada sección del panel (lo hace una persona:
no hay automatización de input en Wayland). Para cada popup, captura con
`cosmic-screenshot --interactive=false --modal=false --notify=false -s "$SCRATCH"`.
Expected: los seis paneles muestran datos; CPU y RAM llenan su top de procesos al segundo tick;
Disco lista `sda` con el uso de `/`; GPU dice que no hay placa con datos en `casa`.

- [ ] **Step 12: Commit**

```bash
git add -A
git commit -m "Los seis paneles de detalle

Un archivo por panel, calcado de su QML: CPU con usuario/sistema, núcleos,
top y uptime; RAM con libre, top y swap; red y disco con gráfico partido;
temperaturas por sensor; GPU por placa con top de VRAM."
```

---

### Task 12: Cierre — warnings, README y verificación completa

**Files:**
- Modify: `src/config.rs`, `README.md`

- [ ] **Step 1: `Section::index` sólo para tests**

Los índices del plasmoid quedan congelados por el orden de `Section::all()` y por su test;
nada del binario los usa. Sobre `pub fn index`:

```rust
/// Índice del plasmoid (`FullView.qml`). Sólo lo usa el test que congela la
/// correspondencia; el binario no lo necesita.
#[cfg(test)]
```

- [ ] **Step 2: README**

Leer `README.md` y reemplazar cualquier frase que describa el applet como "CPU por núcleo y RAM"
por la lista de secciones:

```markdown
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
```

- [ ] **Step 3: Verificación completa**

```bash
toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo build --release 2>&1 | grep -c "^warning" ; cargo test 2>&1 | grep "^test result"'
```

Expected: `0` warnings; `test result: ok. 103 passed`.

Contrastes contra las herramientas que reemplazamos, con el applet instalado:
- `sensors` vs el panel de temperaturas: mismos valores por sensor.
- `df -B1 --output=source,size,used,avail,pcent,target | grep '^/dev'` vs el panel de disco.
- `cat /proc/loadavg` + `top -bn1 | head -15` vs el top de procesos por CPU (orden parecido;
  `top` también es instantáneo).

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "Cierre de la paridad: sin warnings y README con las seis secciones"
```

## Verificación final del plan

- [ ] `cargo test` pasa entero (103 tests) y `cargo build --release` sin warnings.
- [ ] Las seis secciones se ven en el panel (GPU sólo con fuente de datos) sin salirse del alto.
- [ ] Cada popup muestra su contenido; lo caro sólo corre con él abierto (se puede ver con
  `strace -f -e trace=openat -p <pid>` que `/proc/N/stat` no se lee con el popup cerrado).
- [ ] `git log --oneline` muestra un commit por tarea.
