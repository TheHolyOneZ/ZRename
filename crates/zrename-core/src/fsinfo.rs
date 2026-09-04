use crate::model::{FsProfile, LengthLimit};
use std::path::Path;

fn control_chars() -> Vec<char> {
    (0u8..0x20).map(|b| b as char).collect()
}

fn windows_illegal() -> Vec<char> {
    let mut v = vec!['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    v.extend(control_chars());
    v
}

fn windows_reserved() -> Vec<String> {
    let mut v: Vec<String> = ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    for n in 0..=9 {
        v.push(format!("COM{n}"));
        v.push(format!("LPT{n}"));
    }
    v
}

impl FsProfile {
    pub fn platform_default() -> Self {
        if cfg!(windows) {
            Self::ntfs()
        } else {
            Self::unknown()
        }
    }

    pub fn or_platform_default(self, reported: &str) -> Self {
        if self.name == "unknown" {
            let mut p = Self::platform_default();
            p.name = reported.to_string();
            p
        } else {
            self
        }
    }

    pub fn ext4() -> Self {
        Self {
            name: "ext4".into(),
            case_insensitive: false,
            illegal_chars: vec!['/', '\0'],
            reserved_stems: Vec::new(),
            strips_trailing_dot_space: false,
            max_component: LengthLimit::bytes(255),
            max_path: Some(4096),
            supports_long_path_prefix: false,
        }
    }

    pub fn ntfs() -> Self {
        Self {
            name: "NTFS".into(),
            case_insensitive: true,
            illegal_chars: windows_illegal(),
            reserved_stems: windows_reserved(),
            strips_trailing_dot_space: true,
            max_component: LengthLimit::utf16(255),
            max_path: Some(260),
            supports_long_path_prefix: true,
        }
    }

    pub fn fat32() -> Self {
        Self {
            name: "FAT32".into(),
            case_insensitive: true,
            illegal_chars: windows_illegal(),
            reserved_stems: windows_reserved(),
            strips_trailing_dot_space: true,
            max_component: LengthLimit::utf16(255),
            max_path: Some(260),
            supports_long_path_prefix: false,
        }
    }

    pub fn exfat() -> Self {
        Self {
            name: "exFAT".into(),
            ..Self::fat32()
        }
    }

    pub fn apfs() -> Self {
        Self {
            name: "APFS".into(),
            case_insensitive: true,
            illegal_chars: vec!['/', '\0', ':'],
            reserved_stems: Vec::new(),
            strips_trailing_dot_space: false,
            max_component: LengthLimit::bytes(255),
            max_path: Some(1024),
            supports_long_path_prefix: false,
        }
    }

    pub fn unknown() -> Self {
        Self {
            name: "unknown".into(),
            case_insensitive: true,
            illegal_chars: vec!['/', '\0'],
            reserved_stems: Vec::new(),
            strips_trailing_dot_space: false,
            max_component: LengthLimit::bytes(255),
            max_path: Some(4096),
            supports_long_path_prefix: false,
        }
    }

    pub fn portable() -> Self {
        Self {
            name: "portable".into(),
            max_component: LengthLimit::utf16(200),
            ..Self::fat32()
        }
    }

    pub fn from_fs_name(fs: &str) -> Self {
        match fs.trim().to_ascii_lowercase().as_str() {
            "ext2" | "ext3" | "ext4" => Self::ext4(),
            "btrfs" => Self {
                name: "btrfs".into(),
                ..Self::ext4()
            },
            "xfs" => Self {
                name: "xfs".into(),
                ..Self::ext4()
            },
            "zfs" => Self {
                name: "zfs".into(),
                ..Self::ext4()
            },
            "f2fs" => Self {
                name: "f2fs".into(),
                ..Self::ext4()
            },
            "tmpfs" => Self {
                name: "tmpfs".into(),
                ..Self::ext4()
            },
            "overlay" | "overlayfs" => Self {
                name: "overlay".into(),
                ..Self::ext4()
            },
            "ntfs" | "ntfs3" | "ntfs-3g" => Self::ntfs(),
            "vfat" | "msdos" | "fat" | "fat32" => Self::fat32(),
            "exfat" => Self::exfat(),
            "apfs" | "hfs" | "hfsplus" => Self::apfs(),
            _ => Self::unknown(),
        }
    }

    pub fn fold(&self, name: &str) -> String {
        if self.case_insensitive {
            name.to_lowercase()
        } else {
            name.to_string()
        }
    }

    pub fn effective_name(&self, name: &str) -> String {
        if self.strips_trailing_dot_space {
            let trimmed = name.trim_end_matches([' ', '.']);
            if trimmed.is_empty() {
                String::new()
            } else {
                trimmed.to_string()
            }
        } else {
            name.to_string()
        }
    }

    pub fn is_reserved(&self, name: &str) -> bool {
        if self.reserved_stems.is_empty() {
            return false;
        }
        let head = name.split('.').next().unwrap_or(name).trim_end_matches(' ');
        self.reserved_stems
            .iter()
            .any(|r| r.eq_ignore_ascii_case(head))
    }
}

pub const fn platform_reaches_long_paths() -> bool {
    cfg!(windows)
}

pub fn parse_mounts(contents: &str, path: &Path) -> Option<String> {
    let target = path.to_string_lossy();
    let mut best: Option<(usize, String)> = None;
    for line in contents.lines() {
        let mut cols = line.split_whitespace();
        let _dev = cols.next()?;
        let mount = cols.next()?;
        let fstype = cols.next()?;
        let mount = unescape_mount(mount);
        if !path_starts_with(&target, &mount) {
            continue;
        }
        if best.as_ref().is_none_or(|(len, _)| mount.len() > *len) {
            best = Some((mount.len(), fstype.to_string()));
        }
    }
    best.map(|(_, fs)| fs)
}

fn unescape_mount(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let digits: String = chars.clone().take(3).collect();
        if digits.len() == 3 && digits.chars().all(|d| ('0'..='7').contains(&d)) {
            if let Ok(byte) = u8::from_str_radix(&digits, 8) {
                out.push(byte as char);
                for _ in 0..3 {
                    chars.next();
                }
                continue;
            }
        }
        out.push(c);
    }
    out
}

fn path_starts_with(path: &str, mount: &str) -> bool {
    if mount == "/" {
        return path.starts_with('/');
    }
    if !path.starts_with(mount) {
        return false;
    }
    matches!(path.as_bytes().get(mount.len()), None | Some(b'/'))
}

pub fn detect_profile(path: &Path) -> FsProfile {
    match detect_fs_name(path) {
        Some(name) => FsProfile::from_fs_name(&name).or_platform_default(&name),
        None => FsProfile::platform_default(),
    }
}

#[cfg(target_os = "linux")]
fn detect_fs_name(path: &Path) -> Option<String> {
    let probe = nearest_existing(path)?;
    let mounts = std::fs::read_to_string("/proc/mounts").ok()?;
    parse_mounts(&mounts, &probe)
}

#[cfg(target_os = "windows")]
fn detect_fs_name(path: &Path) -> Option<String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{GetVolumeInformationW, GetVolumePathNameW};

    let probe = nearest_existing(path)?;
    let mut wide: Vec<u16> = probe.as_os_str().encode_wide().collect();
    wide.push(0);

    let mut root = vec![0u16; 260];
    let ok = unsafe { GetVolumePathNameW(wide.as_ptr(), root.as_mut_ptr(), root.len() as u32) };
    if ok == 0 {
        return None;
    }

    let mut fs_name = vec![0u16; 64];
    let ok = unsafe {
        GetVolumeInformationW(
            root.as_ptr(),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            fs_name.as_mut_ptr(),
            fs_name.len() as u32,
        )
    };
    if ok == 0 {
        return None;
    }
    let len = fs_name
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(fs_name.len());
    Some(String::from_utf16_lossy(&fs_name[..len]))
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn detect_fs_name(_path: &Path) -> Option<String> {
    None
}

#[allow(dead_code)]
fn nearest_existing(path: &Path) -> Option<std::path::PathBuf> {
    let mut cur = path;
    loop {
        if cur.exists() {
            return Some(cur.to_path_buf());
        }
        cur = cur.parent()?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn ntfs_folds_case_and_ext4_does_not() {
        assert_eq!(FsProfile::ntfs().fold("Photo.JPG"), "photo.jpg");
        assert_eq!(FsProfile::ext4().fold("Photo.JPG"), "Photo.JPG");
    }

    #[test]
    fn windows_device_names_are_reserved_with_any_extension() {
        let ntfs = FsProfile::ntfs();
        for name in [
            "CON",
            "con",
            "CON.txt",
            "nul.log",
            "COM1.dat",
            "LPT9",
            "aux.tar.gz",
            "CON .txt",
        ] {
            assert!(ntfs.is_reserved(name), "{name} should be reserved on NTFS");
        }
        for name in ["CONSOLE.txt", "COM10.txt", "NULL.txt", "my-con.txt", "COMA"] {
            assert!(!ntfs.is_reserved(name), "{name} should not be reserved");
        }
    }

    #[test]
    fn ext4_has_no_reserved_names() {
        assert!(!FsProfile::ext4().is_reserved("CON.txt"));
    }

    #[test]
    fn windows_strips_trailing_dots_and_spaces() {
        let ntfs = FsProfile::ntfs();
        assert_eq!(ntfs.effective_name("report."), "report");
        assert_eq!(ntfs.effective_name("report "), "report");
        assert_eq!(ntfs.effective_name("report. . "), "report");
        assert_eq!(ntfs.effective_name("..."), "");
        assert_eq!(FsProfile::ext4().effective_name("report."), "report.");
    }

    #[test]
    fn illegal_sets_differ_by_filesystem() {
        assert!(FsProfile::ntfs().illegal_chars.contains(&':'));
        assert!(FsProfile::ntfs().illegal_chars.contains(&'?'));
        assert!(!FsProfile::ext4().illegal_chars.contains(&':'));
        assert!(FsProfile::ext4().illegal_chars.contains(&'/'));
    }

    #[test]
    fn fs_names_map_to_profiles() {
        assert_eq!(FsProfile::from_fs_name("ntfs3").name, "NTFS");
        assert_eq!(FsProfile::from_fs_name("NTFS").name, "NTFS");
        assert_eq!(FsProfile::from_fs_name("vfat").name, "FAT32");
        assert_eq!(FsProfile::from_fs_name("exfat").name, "exFAT");
        assert_eq!(FsProfile::from_fs_name("ext4").name, "ext4");
        assert_eq!(FsProfile::from_fs_name("squashfs").name, "unknown");
        assert!(FsProfile::from_fs_name("fuseblk").case_insensitive);
    }

    #[test]
    fn mounts_picks_the_longest_matching_mount_point() {
        let mounts = "\
/dev/nvme0n1p4 / ext4 rw,relatime 0 0
/dev/nvme0n1p1 /boot vfat rw,relatime 0 0
/dev/sdb1 /run/media/z/USB\\040STICK exfat rw 0 0
tmpfs /tmp tmpfs rw 0 0
";
        assert_eq!(
            parse_mounts(mounts, &PathBuf::from("/home/z/a.txt")).unwrap(),
            "ext4"
        );
        assert_eq!(
            parse_mounts(mounts, &PathBuf::from("/boot/efi")).unwrap(),
            "vfat"
        );
        assert_eq!(
            parse_mounts(mounts, &PathBuf::from("/tmp/x")).unwrap(),
            "tmpfs"
        );
        assert_eq!(
            parse_mounts(mounts, &PathBuf::from("/run/media/z/USB STICK/DCIM")).unwrap(),
            "exfat"
        );
    }

    #[test]
    fn mounts_does_not_match_a_partial_component() {
        let mounts = "/dev/a / ext4 rw 0 0\n/dev/b /boot vfat rw 0 0\n";
        assert_eq!(
            parse_mounts(mounts, &PathBuf::from("/bootleg/x")).unwrap(),
            "ext4"
        );
    }

    #[test]
    fn an_unrecognised_filesystem_keeps_the_platforms_rules() {
        let p = FsProfile::from_fs_name("refs").or_platform_default("ReFS");
        assert_eq!(p.name, "ReFS");
        if cfg!(windows) {
            assert!(
                p.is_reserved("CON.txt"),
                "Win32 rules apply to every volume"
            );
            assert!(p.illegal_chars.contains(&':'));
        } else {
            assert!(!p.is_reserved("CON.txt"));
        }

        assert_eq!(
            FsProfile::from_fs_name("ext4")
                .or_platform_default("ext4")
                .name,
            "ext4"
        );
        assert_eq!(
            FsProfile::from_fs_name("ntfs")
                .or_platform_default("ntfs")
                .name,
            "NTFS"
        );
    }

    #[test]
    fn portable_is_the_strictest_profile() {
        let p = FsProfile::portable();
        assert!(p.case_insensitive);
        assert!(p.strips_trailing_dot_space);
        assert!(!p.reserved_stems.is_empty());
        assert!(p.max_component.max < FsProfile::ntfs().max_component.max);
    }
}
