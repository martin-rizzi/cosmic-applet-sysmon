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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encuentra_sh_y_no_inventa() {
        assert!(executable_in_path("sh"));
        assert!(!executable_in_path("no-existe-este-binario-xyz"));
    }
}
