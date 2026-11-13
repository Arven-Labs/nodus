//! Leitura do hardware da máquina com `sysinfo` (adaptador; o core só recebe o resultado).

#[cfg(test)]
mod tests {
    use super::*;
    use nodus_core::inventory::Disk;

    fn disco(mount: &str, total: u64) -> Disk {
        Disk {
            name: "x".into(),
            mount_point: mount.into(),
            file_system: "ext4".into(),
            total_bytes: total,
        }
    }

    #[test]
    fn descarta_discos_de_tamanho_zero_e_mantem_a_ordem() {
        let discos = vec![
            disco("/proc", 0),
            disco("/", 10),
            disco("/sys", 0),
            disco("/home", 20),
        ];
        let mantidos = drop_empty_disks(discos);
        let pontos: Vec<&str> = mantidos.iter().map(|d| d.mount_point.as_str()).collect();
        assert_eq!(pontos, vec!["/", "/home"]);
    }

    #[test]
    fn sem_discos_continua_sem_discos() {
        assert!(drop_empty_disks(vec![]).is_empty());
    }

    // Dado ao vivo da máquina: só invariantes, nunca valores.
    #[test]
    fn leitura_real_tem_invariantes_basicas() {
        let hw = read_hardware().unwrap();
        assert!(hw.cpu_threads >= 1);
        assert!(hw.memory_total_bytes > 0);
        assert!(hw.disks.iter().all(|d| d.total_bytes > 0));
    }
}
