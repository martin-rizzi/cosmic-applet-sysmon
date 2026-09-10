# Fundación: configuración, secciones y historial — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Que el applet tenga las ocho opciones del plasmoid funcionando, la vista compacta
armada por secciones ordenables, popup por sección con pie de botones, y gráficos de
historial — todo sobre las secciones CPU y RAM que ya existen.

**Architecture:** Se separa parseo de I/O para poder testear con fixtures, se agrega
`config.rs` sobre `cosmic-config` (una clave por archivo, recarga en vivo vía
`watch_config`), y la vista compacta pasa de dos secciones hardcodeadas a una lista
derivada de `section_order` + los `show_*`. El popup deja de ser fijo y rutea a la sección
clickeada.

**Tech Stack:** Rust 2021, libcosmic (rev `a5267e623f356d1792cba7f480dd671cc8b52406`),
`cosmic-config` con feature `macro`, `serde`. Sin dependencias nuevas fuera del árbol
existente.

**Spec:** `docs/superpowers/specs/2026-09-10-sysmon-paridad-plasmoid-design.md`

## Global Constraints

- **Sin subprocesos en el muestreo.** Todo sale de `/proc` y `/sys` leídos directo. La
  única excepción autorizada es `nvidia-smi`, que pertenece al Plan 2 y no aparece acá.
- **Idioma:** el applet y sus comentarios están en español. Mantenerlo.
- **Licencia:** todo archivo nuevo empieza con `// SPDX-License-Identifier: GPL-3.0-only`.
- **libcosmic pinneado:** no cambiar el `rev` de `Cargo.toml`.
- **Claves de config:** nombres y defaults exactos — `update_interval` 2000 (rango
  500–10000, paso 500), `show_cpu`/`show_ram`/`show_network`/`show_storage`/`show_temps`/
  `show_gpu` todos `true`, `section_order` = `"temps,network,storage,cpu,gpu,ram"`.
- **Índices de sección del plasmoid, inalterables:** `0=Cpu, 1=Ram, 2=Network,
  3=Storage, 4=Temps, 5=Gpu`.
- **Comando de build y test** (no hay cargo en el host, se compila en el toolbox):
  ```sh
  toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo test'
  ```
- **Commits:** uno por tarea, mensaje en español, sin línea `Co-Authored-By`.

## Estructura de archivos

| Archivo | Responsabilidad |
|---|---|
| `src/main.rs` | entrypoint; declara los módulos |
| `src/config.rs` | las 8 claves, `Section`, normalización de `section_order` |
| `src/metrics/mod.rs` | reexporta colectores; agrega el estado muestreado |
| `src/metrics/cpu.rs` | parseo de `/proc/stat` + deltas (viene de `proc.rs`) |
| `src/metrics/mem.rs` | parseo de `/proc/meminfo` (viene de `proc.rs`) |
| `src/metrics/history.rs` | buffer circular de 60 puntos |
| `src/ui/compact.rs` | vista del panel: secciones ordenadas y clickeables |
| `src/ui/settings.rs` | página del engranaje |
| `src/ui/detail/mod.rs` | ruteo del popup por sección + pie de botones |
| `src/app.rs` | `Application`, `Message`, estado de popup y sección activa |
| `src/draw.rs` | medidores SVG + gráfico de historial |
| `tests/fixtures/` | capturas reales de `/proc` |

---

### Task 1: Separar parseo de I/O y cubrir CPU y RAM con tests

Hoy `Cpu::refresh()` lee `/proc/stat` adentro de la misma función, así que no se puede
testear. Esta tarea parte el parseo del I/O y mueve `proc.rs` a `metrics/`. Sin esto
ninguna tarea posterior puede hacer TDD.

**Files:**
- Create: `src/metrics/mod.rs`, `src/metrics/cpu.rs`, `src/metrics/mem.rs`
- Create: `tests/fixtures/proc_stat_a`, `tests/fixtures/proc_stat_b`, `tests/fixtures/proc_meminfo`
- Delete: `src/proc.rs`
- Modify: `src/main.rs`, `src/app.rs` (imports)

**Interfaces:**
- Consumes: nada (primera tarea).
- Produces:
  - `metrics::cpu::CpuTicks { total: u64, active: u64 }` (pub)
  - `metrics::cpu::parse_stat(raw: &str) -> Vec<CpuTicks>`
  - `metrics::cpu::Cpu { pub cores: Vec<f32>, pub total: f32 }` con
    `fn apply(&mut self, now: Vec<CpuTicks>)`, `fn refresh(&mut self)`,
    `fn core_count(&self) -> usize`
  - `metrics::mem::MemFields` y `metrics::mem::parse_meminfo(raw: &str) -> MemFields`
  - `metrics::mem::Mem { pub total, used, cached, swap_total, swap_used: f64 }` con
    `fn fraction(&self) -> f32`, `fn refresh(&mut self)`
  - `metrics::mem::format_mib(mib: f64) -> String`

- [ ] **Step 1: Crear los fixtures**

`tests/fixtures/proc_stat_a` — captura real, con líneas no-`cpu` al final para verificar
que el parser corta donde debe:

```
cpu  50258281 1897484 28974876 395725085 1160894 2440253 2784590 0 0 0
cpu0 3631488 151633 2651303 32722735 193867 357924 592820 0 0 0
cpu1 4196869 154460 2281135 33136791 112593 165544 217775 0 0 0
cpu2 4221943 165213 2284805 33120102 100843 168482 188885 0 0 0
intr 1234567 0 0
ctxt 987654321
```

`tests/fixtures/proc_stat_b` — segunda muestra con deltas elegidos para dar números
redondos: agregado +25 activo / +100 total (25 %), cpu0 +50/+100 (50 %), cpu1 sin cambios
(0 %), cpu2 +100/+100 (100 %):

```
cpu  50258306 1897484 28974876 395725160 1160894 2440253 2784590 0 0 0
cpu0 3631538 151633 2651303 32722785 193867 357924 592820 0 0 0
cpu1 4196869 154460 2281135 33136791 112593 165544 217775 0 0 0
cpu2 4222043 165213 2284805 33120102 100843 168482 188885 0 0 0
intr 1234999 0 0
ctxt 987654999
```

`tests/fixtures/proc_meminfo` — captura real recortada a los campos que usamos:

```
MemTotal:       32697628 kB
MemFree:         2935564 kB
Buffers:            1912 kB
Cached:         22150232 kB
SwapCached:            0 kB
SwapTotal:       8388604 kB
SwapFree:        6245228 kB
Shmem:           2286264 kB
SReclaimable:     669284 kB
```

- [ ] **Step 2: Escribir los tests que fallan**

Crear `src/metrics/cpu.rs` con el bloque de tests al final (Rust testea in-module):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const STAT_A: &str = include_str!("../../tests/fixtures/proc_stat_a");
    const STAT_B: &str = include_str!("../../tests/fixtures/proc_stat_b");

    #[test]
    fn parse_stat_corta_en_la_primera_linea_no_cpu() {
        let ticks = parse_stat(STAT_A);
        // agregado + 3 núcleos, sin `intr` ni `ctxt`
        assert_eq!(ticks.len(), 4);
    }

    #[test]
    fn parse_stat_calcula_active_y_total() {
        let ticks = parse_stat(STAT_A);
        // user+nice+system+irq+softirq
        assert_eq!(ticks[0].active, 86_355_484);
        // active + idle + iowait
        assert_eq!(ticks[0].total, 483_241_463);
    }

    #[test]
    fn primera_muestra_da_cero() {
        let mut cpu = Cpu::default();
        cpu.apply(parse_stat(STAT_A));
        assert_eq!(cpu.total, 0.0);
        assert_eq!(cpu.cores, vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn segunda_muestra_calcula_el_delta() {
        let mut cpu = Cpu::default();
        cpu.apply(parse_stat(STAT_A));
        cpu.apply(parse_stat(STAT_B));
        assert_eq!(cpu.total, 25.0);
        assert_eq!(cpu.cores, vec![50.0, 0.0, 100.0]);
    }

    #[test]
    fn contador_que_no_avanza_da_cero() {
        let mut cpu = Cpu::default();
        cpu.apply(parse_stat(STAT_A));
        cpu.apply(parse_stat(STAT_A));
        assert_eq!(cpu.total, 0.0);
    }
}
```

Y `src/metrics/mem.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const MEMINFO: &str = include_str!("../../tests/fixtures/proc_meminfo");

    #[test]
    fn parsea_los_campos_en_mib() {
        let f = parse_meminfo(MEMINFO);
        assert!((f.total - 31_931.277).abs() < 0.01);
        assert!((f.free - 2_866.769).abs() < 0.01);
    }

    #[test]
    fn cached_suma_sreclaimable_y_resta_shmem() {
        let m = Mem::from_fields(parse_meminfo(MEMINFO));
        // (22150232 + 669284 - 2286264) / 1024
        assert!((m.cached - 20_052.003).abs() < 0.01);
    }

    #[test]
    fn usada_es_total_menos_free_buffers_y_cached() {
        let m = Mem::from_fields(parse_meminfo(MEMINFO));
        assert!((m.used - 9_010.636).abs() < 0.01);
    }

    #[test]
    fn swap_usada_es_total_menos_free() {
        let m = Mem::from_fields(parse_meminfo(MEMINFO));
        assert!((m.swap_used - 2_093.140).abs() < 0.01);
    }

    #[test]
    fn no_confunde_cached_con_swapcached() {
        // SwapCached está en el fixture con valor 0; si el matcher fuera laxo,
        // `cached` cambiaría.
        let m = Mem::from_fields(parse_meminfo(MEMINFO));
        assert!(m.cached > 20_000.0);
    }

    #[test]
    fn fraction_se_mantiene_en_rango() {
        let m = Mem::from_fields(parse_meminfo(MEMINFO));
        assert!(m.fraction() > 0.0 && m.fraction() < 1.0);
    }

    #[test]
    fn format_mib_cambia_a_gib_en_1024() {
        assert_eq!(format_mib(512.0), "512 MiB");
        assert_eq!(format_mib(2048.0), "2.0 GiB");
    }
}
```

- [ ] **Step 3: Correr los tests y verificar que fallan**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo test'`
Expected: FAIL — los módulos `metrics::cpu` / `metrics::mem` todavía no existen.

- [ ] **Step 4: Implementar `src/metrics/cpu.rs`**

Mover el cuerpo de `Cpu` desde `proc.rs`, partido en `parse_stat` (puro) y `apply`
(estado), con `refresh` como el único punto que toca el filesystem:

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! Muestreo de /proc/stat.
//!
//! Fórmula del plasmoid com.labatata.sysmonitor:
//!   active = user+nice+system+irq+softirq, total = active+idle+iowait,
//!   y el porcentaje sale del delta contra la muestra anterior.

use std::fs;

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct CpuTicks {
    pub total: u64,
    pub active: u64,
}

/// Índice 0 es el agregado "cpu"; el resto son los núcleos, en el orden del archivo.
pub fn parse_stat(raw: &str) -> Vec<CpuTicks> {
    let mut out = Vec::with_capacity(8);
    for line in raw.lines() {
        if !line.starts_with("cpu") {
            break;
        }
        let mut it = line.split_whitespace();
        it.next();
        let v: Vec<u64> = it.take(7).map(|f| f.parse().unwrap_or(0)).collect();
        if v.len() < 7 {
            continue;
        }
        let (user, nice, system, idle, iowait, irq, softirq) =
            (v[0], v[1], v[2], v[3], v[4], v[5], v[6]);
        let active = user + nice + system + irq + softirq;
        out.push(CpuTicks {
            total: active + idle + iowait,
            active,
        });
    }
    out
}

#[derive(Default)]
pub struct Cpu {
    prev: Vec<CpuTicks>,
    pub cores: Vec<f32>,
    pub total: f32,
}

impl Cpu {
    pub fn apply(&mut self, now: Vec<CpuTicks>) {
        if now.is_empty() {
            return;
        }
        let mut pcts: Vec<f32> = Vec::with_capacity(now.len());
        for (i, sample) in now.iter().enumerate() {
            let pct = match self.prev.get(i) {
                Some(prev) if sample.total > prev.total => {
                    let dt = (sample.total - prev.total) as f32;
                    let da = sample.active.saturating_sub(prev.active) as f32;
                    (da / dt * 100.0).clamp(0.0, 100.0)
                }
                // Primera muestra o contador reiniciado: 0, como el plasmoid.
                _ => 0.0,
            };
            pcts.push(pct);
        }
        self.total = pcts[0];
        self.cores = pcts[1..].to_vec();
        self.prev = now;
    }

    pub fn refresh(&mut self) {
        if let Ok(raw) = fs::read_to_string("/proc/stat") {
            self.apply(parse_stat(&raw));
        }
    }

    pub fn core_count(&self) -> usize {
        self.cores.len()
    }
}
```

- [ ] **Step 5: Implementar `src/metrics/mem.rs`**

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! Muestreo de /proc/meminfo.
//!
//! Fórmula del plasmoid:
//!   usada = MemTotal - (MemFree + Buffers + Cached + SReclaimable - Shmem)

use std::fs;

/// Todos los valores ya convertidos a MiB.
#[derive(Default, Clone, Copy)]
pub struct MemFields {
    pub total: f64,
    pub free: f64,
    pub buffers: f64,
    pub cached: f64,
    pub sreclaimable: f64,
    pub shmem: f64,
    pub swap_total: f64,
    pub swap_free: f64,
}

pub fn parse_meminfo(raw: &str) -> MemFields {
    let field = |name: &str| -> f64 {
        raw.lines()
            .find(|l| l.starts_with(name) && l.as_bytes().get(name.len()) == Some(&b':'))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0)
            / 1024.0
    };
    MemFields {
        total: field("MemTotal"),
        free: field("MemFree"),
        buffers: field("Buffers"),
        cached: field("Cached"),
        sreclaimable: field("SReclaimable"),
        shmem: field("Shmem"),
        swap_total: field("SwapTotal"),
        swap_free: field("SwapFree"),
    }
}

/// Todos los valores en MiB.
#[derive(Default)]
pub struct Mem {
    pub total: f64,
    pub used: f64,
    pub cached: f64,
    pub swap_total: f64,
    pub swap_used: f64,
}

impl Mem {
    pub fn from_fields(f: MemFields) -> Self {
        let cached = f.cached + f.sreclaimable - f.shmem;
        let free = f.free + f.buffers + cached;
        Self {
            total: f.total,
            used: f.total - free,
            cached,
            swap_total: f.swap_total,
            swap_used: f.swap_total - f.swap_free,
        }
    }

    pub fn refresh(&mut self) {
        if let Ok(raw) = fs::read_to_string("/proc/meminfo") {
            *self = Self::from_fields(parse_meminfo(&raw));
        }
    }

    /// Fracción usada, 0.0..=1.0.
    pub fn fraction(&self) -> f32 {
        if self.total > 0.0 {
            (self.used / self.total).clamp(0.0, 1.0) as f32
        } else {
            0.0
        }
    }
}

/// "12.3 GiB" / "512 MiB", como el `formatMemoryMib` del plasmoid.
pub fn format_mib(mib: f64) -> String {
    if mib >= 1024.0 {
        format!("{:.1} GiB", mib / 1024.0)
    } else {
        format!("{:.0} MiB", mib)
    }
}
```

- [ ] **Step 6: Crear `src/metrics/mod.rs` y actualizar los imports**

`src/metrics/mod.rs`:

```rust
// SPDX-License-Identifier: GPL-3.0-only

pub mod cpu;
pub mod mem;
```

En `src/main.rs`, reemplazar `mod proc;` por `mod metrics;`.
En `src/app.rs`, reemplazar `use crate::proc::{format_mib, Cpu, Mem};` por:

```rust
use crate::metrics::cpu::Cpu;
use crate::metrics::mem::{format_mib, Mem};
```

Borrar `src/proc.rs`.

- [ ] **Step 7: Correr los tests y verificar que pasan**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo test'`
Expected: PASS, 12 tests (5 de cpu, 7 de mem).

- [ ] **Step 8: Verificar que el applet sigue compilando**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo build --release'`
Expected: compila sin errores ni warnings nuevos.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "Separar parseo de I/O en los colectores y cubrirlos con tests

Mueve proc.rs a metrics/{cpu,mem}.rs partiendo cada refresh en una
función pura de parseo más el acceso al filesystem, para poder testear
contra capturas reales de /proc."
```

---

### Task 2: `config.rs` — las ocho claves y la normalización del orden

**Files:**
- Create: `src/config.rs`
- Modify: `Cargo.toml` (dependencias `cosmic-config` con feature `macro`, y `serde`)
- Modify: `src/main.rs` (declarar `mod config;`)

**Interfaces:**
- Consumes: nada de tareas anteriores.
- Produces:
  - `config::Section` — enum `Cpu | Ram | Network | Storage | Temps | Gpu` con
    `fn key(&self) -> &'static str`, `fn label(&self) -> &'static str`,
    `fn index(&self) -> usize`, `fn all() -> [Section; 6]`
  - `config::Config` con los 8 campos públicos y `#[derive(CosmicConfigEntry)]`
  - `config::Config::ordered_sections(&self) -> Vec<Section>` — orden normalizado ya
    filtrado por los `show_*`
  - `config::normalize_section_order(raw: &str) -> Vec<Section>`
  - `config::DEFAULT_SECTION_ORDER: &str`

- [ ] **Step 1: Agregar las dependencias**

En `Cargo.toml`, después del bloque `[dependencies.libcosmic]`:

```toml
[dependencies.cosmic-config]
git = "https://github.com/pop-os/libcosmic.git"
rev = "a5267e623f356d1792cba7f480dd671cc8b52406"
features = ["macro"]

[dependencies.serde]
version = "1"
features = ["derive"]
```

`cosmic-config` es un miembro del workspace de libcosmic, no una feature suya: por eso va
como dependencia directa al mismo `rev`. La feature `macro` es la que habilita el derive
`CosmicConfigEntry`.

- [ ] **Step 2: Escribir los tests que fallan**

Al final de `src/config.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_default_es_el_del_plasmoid() {
        let c = Config::default();
        assert_eq!(c.update_interval, 2000);
        assert!(c.show_cpu && c.show_ram && c.show_network);
        assert!(c.show_storage && c.show_temps && c.show_gpu);
        assert_eq!(c.section_order, DEFAULT_SECTION_ORDER);
    }

    #[test]
    fn normaliza_el_orden_por_defecto() {
        let order = normalize_section_order(DEFAULT_SECTION_ORDER);
        assert_eq!(
            order,
            vec![
                Section::Temps,
                Section::Network,
                Section::Storage,
                Section::Cpu,
                Section::Gpu,
                Section::Ram,
            ]
        );
    }

    #[test]
    fn descarta_claves_desconocidas() {
        let order = normalize_section_order("cpu,inventada,ram");
        assert_eq!(order[0], Section::Cpu);
        assert_eq!(order[1], Section::Ram);
        assert_eq!(order.len(), 6);
    }

    #[test]
    fn descarta_duplicados() {
        let order = normalize_section_order("cpu,cpu,ram");
        assert_eq!(order.len(), 6);
        assert_eq!(order.iter().filter(|s| **s == Section::Cpu).count(), 1);
    }

    #[test]
    fn agrega_al_final_las_que_faltan() {
        let order = normalize_section_order("ram");
        assert_eq!(order[0], Section::Ram);
        // las otras cinco entran en el orden default
        assert_eq!(order.len(), 6);
        assert!(order.contains(&Section::Gpu));
    }

    #[test]
    fn tolera_espacios_y_cadena_vacia() {
        assert_eq!(normalize_section_order(" cpu , ram ")[0], Section::Cpu);
        assert_eq!(normalize_section_order("").len(), 6);
    }

    #[test]
    fn ordered_sections_filtra_las_apagadas() {
        let mut c = Config::default();
        c.show_temps = false;
        c.show_network = false;
        let visible = c.ordered_sections();
        assert!(!visible.contains(&Section::Temps));
        assert!(!visible.contains(&Section::Network));
        assert_eq!(visible.len(), 4);
        // conserva el orden relativo del default
        assert_eq!(visible[0], Section::Storage);
    }

    #[test]
    fn los_indices_son_los_del_plasmoid() {
        assert_eq!(Section::Cpu.index(), 0);
        assert_eq!(Section::Ram.index(), 1);
        assert_eq!(Section::Network.index(), 2);
        assert_eq!(Section::Storage.index(), 3);
        assert_eq!(Section::Temps.index(), 4);
        assert_eq!(Section::Gpu.index(), 5);
    }
}
```

- [ ] **Step 3: Correr los tests y verificar que fallan**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo test config'`
Expected: FAIL — `Config` y `Section` no existen.

- [ ] **Step 4: Implementar `src/config.rs`**

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! Las ocho opciones del plasmoid com.labatata.sysmonitor, con los mismos nombres y
//! defaults que `contents/config/main.xml`.

use cosmic_config::cosmic_config_derive::CosmicConfigEntry;
use cosmic_config::{Config as RawConfig, CosmicConfigEntry};
use serde::{Deserialize, Serialize};

pub const DEFAULT_SECTION_ORDER: &str = "temps,network,storage,cpu,gpu,ram";

/// Los índices son los del plasmoid (`FullView.qml`) y no deben cambiar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Section {
    Cpu,
    Ram,
    Network,
    Storage,
    Temps,
    Gpu,
}

impl Section {
    pub fn all() -> [Section; 6] {
        [
            Section::Cpu,
            Section::Ram,
            Section::Network,
            Section::Storage,
            Section::Temps,
            Section::Gpu,
        ]
    }

    pub fn key(&self) -> &'static str {
        match self {
            Section::Cpu => "cpu",
            Section::Ram => "ram",
            Section::Network => "network",
            Section::Storage => "storage",
            Section::Temps => "temps",
            Section::Gpu => "gpu",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Section::Cpu => "CPU",
            Section::Ram => "RAM",
            Section::Network => "Red",
            Section::Storage => "Disco",
            Section::Temps => "Temperaturas",
            Section::Gpu => "GPU",
        }
    }

    pub fn index(&self) -> usize {
        match self {
            Section::Cpu => 0,
            Section::Ram => 1,
            Section::Network => 2,
            Section::Storage => 3,
            Section::Temps => 4,
            Section::Gpu => 5,
        }
    }

    fn from_key(key: &str) -> Option<Section> {
        Section::all().into_iter().find(|s| s.key() == key)
    }
}

/// Misma lógica que `normalizedSectionOrder` en `ConfigGeneral.qml`: descarta claves
/// desconocidas y duplicadas, y agrega al final las que falten en el orden default.
pub fn normalize_section_order(raw: &str) -> Vec<Section> {
    let mut out: Vec<Section> = Vec::with_capacity(6);
    for part in raw.split(',') {
        if let Some(s) = Section::from_key(part.trim()) {
            if !out.contains(&s) {
                out.push(s);
            }
        }
    }
    for s in DEFAULT_SECTION_ORDER
        .split(',')
        .filter_map(|k| Section::from_key(k.trim()))
    {
        if !out.contains(&s) {
            out.push(s);
        }
    }
    out
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, CosmicConfigEntry)]
#[version = 1]
pub struct Config {
    pub update_interval: u32,
    pub show_cpu: bool,
    pub show_ram: bool,
    pub show_network: bool,
    pub show_storage: bool,
    pub show_temps: bool,
    pub show_gpu: bool,
    pub section_order: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            update_interval: 2000,
            show_cpu: true,
            show_ram: true,
            show_network: true,
            show_storage: true,
            show_temps: true,
            show_gpu: true,
            section_order: DEFAULT_SECTION_ORDER.to_string(),
        }
    }
}

impl Config {
    pub fn shown(&self, s: Section) -> bool {
        match s {
            Section::Cpu => self.show_cpu,
            Section::Ram => self.show_ram,
            Section::Network => self.show_network,
            Section::Storage => self.show_storage,
            Section::Temps => self.show_temps,
            Section::Gpu => self.show_gpu,
        }
    }

    /// Orden normalizado, ya filtrado por los `show_*`.
    pub fn ordered_sections(&self) -> Vec<Section> {
        normalize_section_order(&self.section_order)
            .into_iter()
            .filter(|s| self.shown(*s))
            .collect()
    }

    /// Carga desde cosmic-config; ante error devuelve los defaults.
    pub fn load(app_id: &str) -> Self {
        match RawConfig::new(app_id, <Self as CosmicConfigEntry>::VERSION) {
            Ok(raw) => match Self::get_entry(&raw) {
                Ok(c) => c,
                Err((_errors, fallback)) => fallback,
            },
            Err(_) => Self::default(),
        }
    }

    /// Persiste una clave suelta. Escribe un archivo por clave, atómicamente.
    pub fn save(&self, app_id: &str) {
        if let Ok(raw) = RawConfig::new(app_id, <Self as CosmicConfigEntry>::VERSION) {
            let _ = self.write_entry(&raw);
        }
    }
}
```

En `src/main.rs`, agregar `mod config;`.

- [ ] **Step 5: Correr los tests y verificar que pasan**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo test config'`
Expected: PASS, 8 tests.

Si el derive falla con "cannot find `cosmic_config_derive`", la ruta correcta del reexport
es `cosmic_config::cosmic_config_derive::CosmicConfigEntry`; verificar contra
`~/.cargo/git/checkouts/libcosmic-*/a5267e6/cosmic-config/src/lib.rs`.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "Agregar config.rs con las ocho opciones del plasmoid

Mismos nombres y defaults que contents/config/main.xml, con la
normalización de section_order calcada de ConfigGeneral.qml."
```

---

### Task 3: Cablear la config al applet y hacer dinámico el intervalo

**Files:**
- Modify: `src/app.rs`

**Interfaces:**
- Consumes: `config::Config`, `config::Config::load`, `config::Section` (Task 2).
- Produces:
  - `SysMon.config: Config` (campo)
  - `Message::ConfigChanged(Config)`
  - `SysMon::tick_duration(&self) -> Duration`

- [ ] **Step 1: Agregar el campo y el mensaje**

En `src/app.rs`, sumar `use crate::config::Config;` y el campo:

```rust
pub struct SysMon {
    core: Core,
    popup: Option<Id>,
    config: Config,
    cpu: Cpu,
    mem: Mem,
}
```

Y la variante:

```rust
#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    TogglePopup,
    PopupClosed(Id),
    ConfigChanged(Config),
}
```

- [ ] **Step 2: Cargar la config en `init`**

Reemplazar la construcción de `SysMon` en `init` (hoy en `app.rs:103`):

```rust
let mut app = SysMon {
    core,
    popup: None,
    config: Config::load(Self::APP_ID),
    cpu: Cpu::default(),
    mem: Mem::default(),
};
```

- [ ] **Step 3: Intervalo dinámico y suscripción a la config**

Reemplazar el `subscription` actual (`app.rs:127`) y agregar el helper:

```rust
impl SysMon {
    fn tick_duration(&self) -> Duration {
        // Mismo rango que el spinner del plasmoid.
        Duration::from_millis(self.config.update_interval.clamp(500, 10_000) as u64)
    }
}
```

```rust
fn subscription(&self) -> Subscription<Message> {
    Subscription::batch([
        iced::time::every(self.tick_duration()).map(|_| Message::Tick),
        self.core
            .watch_config::<Config>(Self::APP_ID)
            .map(|update| Message::ConfigChanged(update.config)),
    ])
}
```

Borrar la constante `TICK` de `app.rs:18`, que queda sin uso.

- [ ] **Step 4: Manejar el mensaje**

En `update`, agregar el brazo:

```rust
Message::ConfigChanged(config) => {
    self.config = config;
    Task::none()
}
```

- [ ] **Step 5: Compilar**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo build --release && cargo test'`
Expected: compila y los tests siguen pasando.

- [ ] **Step 6: Verificar la recarga en vivo a mano**

```bash
./install.sh
# reiniciar el applet para tomar el binario nuevo
~/.local/bin/cosmic-reiniciar-applet 2>/dev/null || true
# cambiar el intervalo por afuera y ver que el applet acelera
mkdir -p ~/.config/cosmic/io.github.martin_rizzi.CosmicSysMon/v1
printf '500' > /tmp/ui && mv -f /tmp/ui ~/.config/cosmic/io.github.martin_rizzi.CosmicSysMon/v1/update_interval
```

Expected: las barras del panel pasan a actualizarse cuatro veces más rápido sin
reiniciar el applet. Devolver el valor a 2000 al terminar.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "Cablear la config al applet con intervalo dinámico

El período de muestreo sale de update_interval y se recarga en vivo por
watch_config, sin reiniciar el applet."
```

---

### Task 4: Vista compacta armada por secciones

Saca las dos secciones hardcodeadas de `view()` y las arma desde `ordered_sections()`.
Las cuatro secciones que todavía no tienen colector se dibujan como placeholder gris —
el Plan 2 las reemplaza por contenido real.

**Files:**
- Create: `src/ui/mod.rs`, `src/ui/compact.rs`
- Modify: `src/app.rs` (usa `ui::compact::view`), `src/main.rs` (`mod ui;`)

**Interfaces:**
- Consumes: `config::Section`, `Config::ordered_sections` (Task 2); `SysMon.config` (Task 3).
- Produces:
  - accesores en `SysMon`: `core()`, `config()`, `cpu()`, `mem()`, `spacing()`,
    `selected()`, `showing_settings()` — los dos últimos leen campos que agregan las
    tareas 5 y 6; declararlos acá junto con sus campos, inicializados en
    `Section::Cpu` y `false`
  - `ui::compact::view<'a>(app: &'a SysMon) -> Element<'a, Message>`
  - `ui::compact::section_content<'a>(app: &'a SysMon, s: Section) -> Option<Element<'a, Message>>`
    — `None` para las secciones sin colector todavía.

- [ ] **Step 1: Mover la vista a `src/ui/compact.rs`**

`src/ui/mod.rs`:

```rust
// SPDX-License-Identifier: GPL-3.0-only

pub mod compact;
```

`src/ui/compact.rs` recibe el cuerpo de `view()` que hoy vive en `app.rs:172-240`,
reemplazando el `Row`/`Column` fijo de CPU+RAM por un loop:

```rust
// SPDX-License-Identifier: GPL-3.0-only

use cosmic::iced::{Alignment, Length};
use cosmic::widget::{self, Column, Row};
use cosmic::Element;

use crate::app::{Message, SysMon};
use crate::config::Section;

pub fn view(app: &SysMon) -> Element<'_, Message> {
    let horizontal = app.core().applet.is_horizontal();
    let spacing = app.spacing();

    let mut parts: Vec<Element<Message>> = Vec::new();
    for s in app.config().ordered_sections() {
        if let Some(content) = section_content(app, s) {
            parts.push(content);
        }
    }

    let content: Element<Message> = if horizontal {
        let mut row = Row::new().spacing(spacing * 2).align_y(Alignment::Center);
        for p in parts {
            row = row.push(p);
        }
        row.into()
    } else {
        let mut col = Column::new().spacing(spacing * 2).align_x(Alignment::Center);
        for p in parts {
            col = col.push(p);
        }
        col.into()
    };

    app.core().applet.autosize_window(content).into()
}
```

`section_content` devuelve `Some(...)` para `Section::Cpu` y `Section::Ram`, y `None` para
las otras cuatro. El cuerpo sale de lo que hoy arma `view()`, ahora parametrizado por
sección:

```rust
/// `None` para las secciones que todavía no tienen colector (Plan 2).
pub fn section_content<'a>(app: &'a SysMon, s: Section) -> Option<Element<'a, Message>> {
    let icon_size = app.core().applet.suggested_size(true).1;
    let h = f32::from(icon_size);
    let border = app.text_color_hex();
    let spacing = app.spacing();

    match s {
        Section::Cpu => {
            // Ancho de la caja: 4 px por núcleo a escala, con mínimo del 70 % de la altura.
            let scale = h / crate::app::REFERENCE_ICON_SIZE;
            let cores = app.cpu().core_count().max(1) as f32;
            let w = (h * 0.7).max(cores * crate::app::PX_PER_CORE * scale);
            let cpu = app.cpu();
            Some(app.section(
                crate::app::CPU_ICON,
                icon_size,
                crate::draw::cpu_bars(&cpu.cores, w, h, &border),
                w,
                h,
                // Como el plasmoid: un decimal sólo por debajo del 1 %.
                if cpu.total < 1.0 {
                    format!("{:.1}%", cpu.total)
                } else {
                    format!("{:.0}%", cpu.total)
                },
                spacing,
            ))
        }
        Section::Ram => {
            let w = (h * 0.7).round();
            let fraction = app.mem().fraction();
            Some(app.section(
                crate::app::RAM_ICON,
                icon_size,
                crate::draw::usage_meter(fraction, w, h, &border),
                w,
                h,
                format!("{:.0}%", fraction * 100.0),
                spacing,
            ))
        }
        // Sin colector todavía: no ocupan lugar en el panel.
        Section::Network | Section::Storage | Section::Temps | Section::Gpu => None,
    }
}
```

Para que esto compile hay que subir a `pub` en `src/app.rs` las constantes
`CPU_ICON`, `RAM_ICON`, `REFERENCE_ICON_SIZE` y `PX_PER_CORE`, y los métodos
`text_color_hex`, `section`, `label` e `icon`, que hoy son privados.

- [ ] **Step 2: Exponer los accesores que `compact.rs` necesita**

En `src/app.rs`, agregar métodos públicos para no filtrar campos privados:

```rust
impl SysMon {
    pub fn core(&self) -> &Core {
        &self.core
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn cpu(&self) -> &Cpu {
        &self.cpu
    }

    pub fn mem(&self) -> &Mem {
        &self.mem
    }

    pub fn selected(&self) -> Section {
        self.selected
    }

    pub fn showing_settings(&self) -> bool {
        self.showing_settings
    }

    /// `smallSpacing` de Kirigami escalado al tamaño de ícono del panel.
    pub fn spacing(&self) -> u16 {
        let h = f32::from(self.core.applet.suggested_size(true).1);
        (4.0 * (h / REFERENCE_ICON_SIZE)).round() as u16
    }
}
```

Nota: `Application::core()` ya existe en el trait; el inherente puede chocar. Si el
compilador se queja de ambigüedad, renombrar el inherente a `applet_core()` y usar ése
desde `compact.rs`.

En `Application for SysMon`, `fn view` queda como una línea:

```rust
fn view(&self) -> Element<'_, Self::Message> {
    crate::ui::compact::view(self)
}
```

- [ ] **Step 3: Compilar y verificar que no cambió nada visible**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo build --release && cargo test'`
Expected: compila; el panel se ve igual que antes, porque con los defaults `cpu` y `ram`
son las únicas secciones con contenido.

- [ ] **Step 4: Verificar orden y visibilidad a mano**

```bash
./install.sh
D=~/.config/cosmic/io.github.martin_rizzi.CosmicSysMon/v1
printf '"ram,cpu"' > /tmp/so && mv -f /tmp/so $D/section_order
```

Expected: la RAM pasa a dibujarse a la izquierda de la CPU, en vivo.

```bash
printf 'false' > /tmp/sc && mv -f /tmp/sc $D/show_cpu
```

Expected: desaparece la sección de CPU. Restaurar `true` y el orden default al terminar.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "Armar la vista compacta desde section_order

La vista deja de tener CPU y RAM hardcodeadas: recorre las secciones
visibles en el orden configurado. Las cuatro sin colector todavía no
dibujan nada."
```

---

### Task 5: Click por sección, ruteo del popup y pie de botones

**Files:**
- Create: `src/ui/detail/mod.rs`
- Modify: `src/app.rs`, `src/ui/compact.rs`, `src/ui/mod.rs`

**Interfaces:**
- Consumes: `config::Section` (Task 2), `ui::compact::section_content` (Task 4).
- Produces:
  - `Message::OpenSection(Section)`, `Message::OpenSystemMonitor`, `Message::OpenSettings`
  - `SysMon.selected: Section` (campo, default `Section::Cpu`)
  - `ui::detail::view<'a>(app: &'a SysMon) -> Element<'a, Message>`
  - `ui::detail::system_monitor_command() -> Option<&'static str>`

- [ ] **Step 1: Un botón por sección**

En `src/ui/compact.rs`, envolver cada sección en su propio botón en vez de un botón
único que abarca todo:

```rust
let button = widget::button::custom(content)
    .padding(padding)
    .class(cosmic::theme::Button::AppletIcon)
    .on_press(Message::OpenSection(s));
```

- [ ] **Step 2: Semántica de toggle igual a `main.qml:93-98`**

En `src/app.rs`, reemplazar el brazo `TogglePopup` por:

```rust
Message::OpenSection(section) => {
    // Mismo comportamiento que el plasmoid: la misma sección cierra,
    // otra cambia de panel sin cerrar.
    if self.popup.is_some() && self.selected == section {
        if let Some(p) = self.popup.take() {
            return cosmic::surface::surface_task(
                cosmic::surface::action::destroy_popup(p),
            );
        }
        Task::none()
    } else {
        self.selected = section;
        if self.popup.is_some() {
            Task::none()
        } else {
            cosmic::surface::surface_task(cosmic::surface::action::app_popup(
                |_| Default::default(),
                |app: &mut SysMon| {
                    let new_id = Id::unique();
                    app.popup.replace(new_id);
                    app.core.applet.get_popup_settings(
                        app.core.main_window_id().unwrap(),
                        new_id,
                        None,
                        None,
                        None,
                    )
                },
                None,
            ))
        }
    }
}
```

- [ ] **Step 3: Detectar el monitor de sistema disponible**

En `src/ui/detail/mod.rs`:

```rust
// SPDX-License-Identifier: GPL-3.0-only

/// El plasmoid lanza `kstart plasma-systemmonitor`. COSMIC no trae monitor propio,
/// así que se usa el primero disponible.
const CANDIDATES: [&str; 4] = [
    "plasma-systemmonitor",
    "gnome-system-monitor",
    "btop",
    "htop",
];

pub fn system_monitor_command() -> Option<&'static str> {
    CANDIDATES.into_iter().find(|c| {
        std::env::var_os("PATH")
            .map(|paths| {
                std::env::split_paths(&paths).any(|dir| dir.join(c).is_file())
            })
            .unwrap_or(false)
    })
}
```

- [ ] **Step 4: Ruteo y pie**

En el mismo archivo, `view` rutea por `app.selected()` y agrega el pie con los dos
botones. `cpu_detail` y `ram_detail` son el contenido que hoy arma `view_window`, partido
en dos funciones:

```rust
fn cpu_detail(app: &SysMon) -> Element<'_, Message> {
    let border = app.text_color_hex();
    let mut cores = Column::new().spacing(4);
    for (i, usage) in app.cpu().cores.iter().enumerate() {
        let bar = crate::draw::horizontal_bar(
            usage / 100.0,
            140.0,
            8.0,
            crate::draw::CORE_COLORS[i % crate::draw::CORE_COLORS.len()],
            &border,
        );
        cores = cores.push(
            Row::new()
                .push(widget::text::caption(format!("cpu{i}")).width(Length::Fixed(48.0)))
                .push(
                    widget::svg(widget::svg::Handle::from_memory(bar.into_bytes()))
                        .width(Length::Fixed(140.0))
                        .height(Length::Fixed(8.0)),
                )
                .push(widget::text::caption(format!("{usage:.0}%")).width(Length::Fixed(40.0)))
                .spacing(8)
                .align_y(Alignment::Center),
        );
    }

    Column::new()
        .spacing(8)
        .push(widget::text::title4(format!("CPU {:.0}%", app.cpu().total)))
        .push(cores)
        .into()
}

fn ram_detail(app: &SysMon) -> Element<'_, Message> {
    let mem = app.mem();
    let resumen = format!(
        "{} / {} ({:.0}%)",
        format_mib(mem.used),
        format_mib(mem.total),
        mem.fraction() * 100.0
    );

    let mut col = Column::new()
        .spacing(8)
        .push(widget::text::title4("Memoria"))
        .push(widget::text::body(resumen))
        .push(widget::text::caption(format!("En caché: {}", format_mib(mem.cached))));

    if mem.swap_total > 0.0 {
        col = col.push(widget::text::caption(format!(
            "Swap: {} / {}",
            format_mib(mem.swap_used),
            format_mib(mem.swap_total)
        )));
    }

    col.into()
}
```

Las otras cuatro secciones devuelven `widget::text::body("Sin datos todavía")`:

```rust
pub fn view(app: &SysMon) -> Element<'_, Message> {
    let body: Element<Message> = match app.selected() {
        Section::Cpu => cpu_detail(app),
        Section::Ram => ram_detail(app),
        _ => widget::text::body("Sin datos todavía").into(),
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
        .push(body)
        .push(widget::divider::horizontal::default())
        .push(footer);

    app.core().applet.popup_container(content).into()
}
```

- [ ] **Step 5: Lanzar el monitor**

En `update`:

```rust
Message::OpenSystemMonitor => {
    if let Some(cmd) = crate::ui::detail::system_monitor_command() {
        // Único fork del applet, y sólo por acción explícita del usuario.
        let _ = std::process::Command::new(cmd).spawn();
    }
    Task::none()
}
```

- [ ] **Step 6: Compilar y probar a mano**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo build --release && cargo test'`
Then: `./install.sh` y reiniciar el applet.

Expected, clickeando en el panel:
- click en CPU con el popup cerrado → abre en CPU;
- click en CPU de nuevo → cierra;
- click en CPU y después en RAM → cambia de panel sin cerrar;
- el botón del monitor abre `plasma-systemmonitor`.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "Popup por sección con la semántica de toggle del plasmoid

Cada sección del panel es su propio botón: la misma cierra el popup,
otra cambia de panel. Agrega el pie con monitor de sistema y engranaje."
```

---

### Task 6: Página de settings dentro del popup

**Files:**
- Create: `src/ui/settings.rs`
- Modify: `src/app.rs`, `src/ui/mod.rs`, `src/ui/detail/mod.rs`

**Interfaces:**
- Consumes: `Config` (Task 2), `Message::OpenSettings` (Task 5).
- Produces:
  - `SysMon.showing_settings: bool`
  - `Message::ToggleSection(Section, bool)`, `Message::SetInterval(u32)`,
    `Message::MoveSection(Section, i32)`, `Message::CloseSettings`
  - `ui::settings::view<'a>(app: &'a SysMon) -> Element<'a, Message>`
  - `config::move_in_order(order: &str, s: Section, delta: i32) -> String`

- [ ] **Step 1: Test de `move_in_order`**

En `src/config.rs`, agregar al bloque de tests:

```rust
#[test]
fn mover_una_seccion_hacia_arriba() {
    // default: temps,network,storage,cpu,gpu,ram
    let out = move_in_order(DEFAULT_SECTION_ORDER, Section::Storage, -1);
    assert_eq!(out, "temps,storage,network,cpu,gpu,ram");
}

#[test]
fn mover_una_seccion_hacia_abajo() {
    let out = move_in_order(DEFAULT_SECTION_ORDER, Section::Temps, 1);
    assert_eq!(out, "network,temps,storage,cpu,gpu,ram");
}

#[test]
fn mover_en_los_bordes_no_hace_nada() {
    assert_eq!(move_in_order(DEFAULT_SECTION_ORDER, Section::Temps, -1), DEFAULT_SECTION_ORDER);
    assert_eq!(move_in_order(DEFAULT_SECTION_ORDER, Section::Ram, 1), DEFAULT_SECTION_ORDER);
}
```

- [ ] **Step 2: Correr y verificar que falla**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo test mover'`
Expected: FAIL — `move_in_order` no existe.

- [ ] **Step 3: Implementar `move_in_order`**

En `src/config.rs`:

```rust
/// Mueve una sección dentro del orden. Equivale a `moveSection` de ConfigGeneral.qml:
/// fuera de rango no hace nada.
pub fn move_in_order(order: &str, s: Section, delta: i32) -> String {
    let mut list = normalize_section_order(order);
    let Some(from) = list.iter().position(|x| *x == s) else {
        return order.to_string();
    };
    let to = from as i32 + delta;
    if to < 0 || to >= list.len() as i32 {
        return list
            .iter()
            .map(|x| x.key())
            .collect::<Vec<_>>()
            .join(",");
    }
    let item = list.remove(from);
    list.insert(to as usize, item);
    list.iter().map(|x| x.key()).collect::<Vec<_>>().join(",")
}
```

- [ ] **Step 4: Correr y verificar que pasa**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo test mover'`
Expected: PASS, 3 tests.

- [ ] **Step 5: Escribir la página**

`src/ui/settings.rs` — seis switches, el spinner de intervalo y la lista con ↑/↓, con
los mismos rótulos y rangos que `ConfigGeneral.qml`:

```rust
// SPDX-License-Identifier: GPL-3.0-only

use cosmic::iced::Alignment;
use cosmic::widget::{self, Column, Row};
use cosmic::Element;

use crate::app::{Message, SysMon};
use crate::config::{normalize_section_order, Section};

pub fn view(app: &SysMon) -> Element<'_, Message> {
    let cfg = app.config();

    let mut mostrar = Column::new().spacing(4).push(widget::text::heading("Mostrar"));
    for s in Section::all() {
        mostrar = mostrar.push(
            widget::settings::item(
                s.label(),
                widget::toggler(cfg.shown(s))
                    .on_toggle(move |v| Message::ToggleSection(s, v)),
            ),
        );
    }

    let intervalo = widget::settings::item(
        "Intervalo de actualización",
        Row::new()
            .spacing(8)
            .align_y(Alignment::Center)
            .push(
                widget::button::icon(widget::icon::from_name("list-remove-symbolic"))
                    .on_press(Message::SetInterval(
                        cfg.update_interval.saturating_sub(500).max(500),
                    )),
            )
            .push(widget::text::body(format!("{} ms", cfg.update_interval)))
            .push(
                widget::button::icon(widget::icon::from_name("list-add-symbolic"))
                    .on_press(Message::SetInterval(
                        (cfg.update_interval + 500).min(10_000),
                    )),
            ),
    );

    let mut orden = Column::new()
        .spacing(4)
        .push(widget::text::heading("Orden en el panel"));
    for s in normalize_section_order(&cfg.section_order) {
        orden = orden.push(
            Row::new()
                .spacing(8)
                .align_y(Alignment::Center)
                .push(widget::text::body(s.label()).width(cosmic::iced::Length::Fill))
                .push(
                    widget::button::icon(widget::icon::from_name("go-up-symbolic"))
                        .on_press(Message::MoveSection(s, -1)),
                )
                .push(
                    widget::button::icon(widget::icon::from_name("go-down-symbolic"))
                        .on_press(Message::MoveSection(s, 1)),
                ),
        );
    }

    Column::new()
        .spacing(12)
        .push(
            Row::new()
                .push(widget::text::title4("Configuración"))
                .push(widget::horizontal_space())
                .push(
                    widget::button::icon(widget::icon::from_name("window-close-symbolic"))
                        .on_press(Message::CloseSettings),
                ),
        )
        .push(mostrar)
        .push(intervalo)
        .push(orden)
        .into()
}
```

- [ ] **Step 6: Cablear los mensajes**

En `src/app.rs`, agregar el campo `showing_settings: bool` (default `false`) y los brazos.
Cada uno muta la config y persiste — cosmic-config escribe un archivo por clave, así que
el `watch_config` de la Task 3 devuelve el cambio y no hace falta actualizar nada más:

```rust
Message::OpenSettings => {
    self.showing_settings = true;
    Task::none()
}
Message::CloseSettings => {
    self.showing_settings = false;
    Task::none()
}
Message::ToggleSection(section, value) => {
    match section {
        Section::Cpu => self.config.show_cpu = value,
        Section::Ram => self.config.show_ram = value,
        Section::Network => self.config.show_network = value,
        Section::Storage => self.config.show_storage = value,
        Section::Temps => self.config.show_temps = value,
        Section::Gpu => self.config.show_gpu = value,
    }
    self.config.save(Self::APP_ID);
    Task::none()
}
Message::SetInterval(ms) => {
    self.config.update_interval = ms.clamp(500, 10_000);
    self.config.save(Self::APP_ID);
    Task::none()
}
Message::MoveSection(section, delta) => {
    self.config.section_order =
        crate::config::move_in_order(&self.config.section_order, section, delta);
    self.config.save(Self::APP_ID);
    Task::none()
}
```

En `ui::detail::view`, cuando `app.showing_settings()` es `true`, devolver
`crate::ui::settings::view(app)` en lugar del detalle.

- [ ] **Step 7: Si la sección abierta se apaga, caer a la primera visible**

Al final del brazo `ToggleSection`, antes del `Task::none()`:

```rust
if !self.config.shown(self.selected) {
    if let Some(first) = self.config.ordered_sections().first() {
        self.selected = *first;
    }
}
```

- [ ] **Step 8: Compilar y probar a mano**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo build --release && cargo test'`
Then: `./install.sh` y reiniciar el applet.

Expected: el engranaje abre la página; apagar una sección la saca del panel al instante;
las flechas reordenan; el intervalo se mueve de a 500 entre 500 y 10000; cerrar vuelve
al detalle.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "Página de configuración dentro del popup

Las ocho opciones editables desde el engranaje, persistidas por
cosmic-config. Si se apaga la sección abierta, el popup cae a la
primera visible."
```

---

### Task 7: Historial y gráfico

**Files:**
- Create: `src/metrics/history.rs`
- Modify: `src/draw.rs`, `src/app.rs`, `src/ui/detail/mod.rs`, `src/metrics/mod.rs`

**Interfaces:**
- Consumes: `Cpu`, `Mem` (Task 1); `Config::update_interval` (Task 3).
- Produces:
  - `metrics::history::History` con `fn push(&mut self, v: f32)`,
    `fn points(&self) -> Vec<f32>`, `fn len(&self)`, `const CAPACITY: usize = 60`
  - `draw::history_graph(points: &[f32], w: f32, h: f32, fill: &str, border: &str) -> String`
  - `metrics::history::window_label(update_interval_ms: u32) -> String`

- [ ] **Step 1: Tests del buffer**

En `src/metrics/history.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arranca_vacio() {
        assert_eq!(History::default().points().len(), 0);
    }

    #[test]
    fn conserva_el_orden_de_llegada() {
        let mut h = History::default();
        h.push(0.1);
        h.push(0.2);
        assert_eq!(h.points(), vec![0.1, 0.2]);
    }

    #[test]
    fn descarta_el_mas_viejo_pasada_la_capacidad() {
        let mut h = History::default();
        for i in 0..(CAPACITY + 10) {
            h.push(i as f32);
        }
        let p = h.points();
        assert_eq!(p.len(), CAPACITY);
        // el más viejo que sobrevive es el 10
        assert_eq!(p[0], 10.0);
        assert_eq!(p[CAPACITY - 1], (CAPACITY + 9) as f32);
    }

    #[test]
    fn el_rotulo_sale_del_intervalo() {
        // 60 puntos × 2000 ms = 2 minutos
        assert_eq!(window_label(2000), "hace 2 min");
        // 60 × 500 ms = 30 s
        assert_eq!(window_label(500), "hace 30 s");
    }
}
```

- [ ] **Step 2: Correr y verificar que falla**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo test history'`
Expected: FAIL — el módulo no existe.

- [ ] **Step 3: Implementar el buffer**

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! Buffer circular del historial. 60 puntos, como `main.qml` del plasmoid.

use std::collections::VecDeque;

pub const CAPACITY: usize = 60;

#[derive(Default)]
pub struct History {
    points: VecDeque<f32>,
}

impl History {
    pub fn push(&mut self, v: f32) {
        if self.points.len() == CAPACITY {
            self.points.pop_front();
        }
        self.points.push_back(v);
    }

    pub fn points(&self) -> Vec<f32> {
        self.points.iter().copied().collect()
    }

    pub fn len(&self) -> usize {
        self.points.len()
    }
}

/// Rótulo del extremo izquierdo del eje.
///
/// El plasmoid lo tiene hardcodeado y además inconsistente entre paneles ("5 mins ago"
/// en CpuDetail, "2 mins ago" en RamDetail) cuando el buffer real son 60 puntos. Acá se
/// calcula, así que sigue siendo correcto si se cambia el intervalo.
pub fn window_label(update_interval_ms: u32) -> String {
    let secs = (CAPACITY as u32 * update_interval_ms) / 1000;
    if secs >= 60 {
        format!("hace {} min", secs / 60)
    } else {
        format!("hace {secs} s")
    }
}
```

Agregar `pub mod history;` a `src/metrics/mod.rs`.

- [ ] **Step 4: Correr y verificar que pasa**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo test history'`
Expected: PASS, 4 tests.

- [ ] **Step 5: Test del gráfico**

En `src/draw.rs`, al final:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_grafico_vacio_sigue_siendo_svg_valido() {
        let svg = history_graph(&[], 100.0, 40.0, NORMAL, "#ffffff");
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(!svg.contains("NaN"));
    }

    #[test]
    fn dibuja_la_grilla_y_el_area() {
        let svg = history_graph(&[0.0, 0.5, 1.0], 100.0, 40.0, NORMAL, "#ffffff");
        // tres líneas de grilla horizontales, como el Canvas del plasmoid
        assert_eq!(svg.matches("<line").count(), 3);
        assert!(svg.contains("<polygon"));
        assert!(!svg.contains("NaN"));
    }

    #[test]
    fn un_solo_punto_no_divide_por_cero() {
        let svg = history_graph(&[0.7], 100.0, 40.0, NORMAL, "#ffffff");
        assert!(!svg.contains("NaN"));
        assert!(!svg.contains("inf"));
    }
}
```

- [ ] **Step 6: Correr y verificar que falla**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo test draw'`
Expected: FAIL — `history_graph` no existe.

- [ ] **Step 7: Implementar `history_graph`**

En `src/draw.rs`:

```rust
/// Gráfico de área con grilla, calcado del Canvas de CpuDetail.qml: fondo tenue,
/// tres líneas horizontales y el área rellena bajo la curva.
pub fn history_graph(points: &[f32], w: f32, h: f32, fill: &str, border: &str) -> String {
    let mut svg = String::with_capacity(200 + points.len() * 16);
    header(&mut svg, w, h, border);

    for i in 1..=3 {
        let y = h * (i as f32 / 4.0);
        let _ = write!(
            svg,
            r#"<line x1="1" y1="{y:.2}" x2="{:.2}" y2="{y:.2}" stroke="{border}" stroke-opacity="0.12" stroke-width="1"/>"#,
            w - 1.0
        );
    }

    if points.len() >= 2 {
        let inner_w = (w - 2.0).max(0.0);
        let inner_h = (h - 2.0).max(0.0);
        let step = inner_w / (points.len() - 1) as f32;
        let mut poly = String::with_capacity(points.len() * 14);
        for (i, p) in points.iter().enumerate() {
            let x = 1.0 + step * i as f32;
            let y = 1.0 + inner_h * (1.0 - p.clamp(0.0, 1.0));
            let _ = write!(poly, "{x:.2},{y:.2} ");
        }
        // Cierra el polígono contra la base para que quede un área.
        let _ = write!(poly, "{:.2},{:.2} 1.00,{:.2}", 1.0 + inner_w, 1.0 + inner_h, 1.0 + inner_h);
        let _ = write!(
            svg,
            r#"<polygon points="{poly}" fill="{fill}" fill-opacity="0.2" stroke="{fill}" stroke-width="1"/>"#
        );
    }

    svg.push_str("</svg>");
    svg
}
```

- [ ] **Step 8: Correr y verificar que pasa**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo test draw'`
Expected: PASS, 3 tests.

- [ ] **Step 9: Alimentar los historiales y mostrarlos**

En `src/app.rs`, agregar los campos `cpu_history: History` y `ram_history: History`, y en
el brazo `Message::Tick`, después de los `refresh`:

```rust
self.cpu_history.push(self.cpu.total / 100.0);
self.ram_history.push(self.mem.fraction());
```

En `ui::detail`, los paneles de CPU y RAM suman el gráfico y el rótulo del eje:

```rust
Row::new()
    .push(widget::text::caption(crate::metrics::history::window_label(
        app.config().update_interval,
    )))
    .push(widget::horizontal_space())
    .push(widget::text::caption("ahora"))
```

- [ ] **Step 10: Compilar, testear y verificar a mano**

Run: `toolbox run -c cosmic-build sh -c 'cd ~/Documentos/cosmic/sysmon-applet && cargo build --release && cargo test'`
Then: `./install.sh`, reiniciar el applet, abrir el popup y dejarlo un par de minutos.

Expected: el gráfico se llena de izquierda a derecha y el rótulo dice "hace 2 min" con el
intervalo default.

- [ ] **Step 11: Commit**

```bash
git add -A
git commit -m "Historial de 60 puntos con gráfico de área

Buffer circular como el del plasmoid y gráfico SVG con grilla. El
rótulo del eje se calcula desde el intervalo, en vez de estar
hardcodeado e inconsistente como en el QML original."
```

---

## Verificación final del plan

Antes de dar la fundación por terminada:

- [ ] `cargo test` pasa entero (30 tests: 12 + 8 + 3 + 4 + 3).
- [ ] `cargo build --release` sin warnings nuevos.
- [ ] Las ocho opciones se editan desde el engranaje y sobreviven a reiniciar el applet.
- [ ] El popup abre en la sección clickeada y hace toggle correctamente.
- [ ] Apagar todas las secciones no rompe el panel ni el popup.
- [ ] `git log --oneline` muestra un commit por tarea.

## Qué queda para el Plan 2

Colectores y paneles de `temps`, `network`, `storage` y `gpu`; top de procesos por CPU,
RAM y VRAM con recolección condicionada a que el panel esté abierto; y el auto-ocultado
de la sección de GPU cuando no hay fuente de datos.
