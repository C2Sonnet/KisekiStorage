//! Windowsからディスク容量と基本情報を取得する。データ変更は行わない。
use std::{env, ffi::c_void, ptr, sync::mpsc::{self, Receiver}, thread, time::{Duration, Instant}};
use sysinfo::System;
use winreg::{enums::HKEY_LOCAL_MACHINE, RegKey};

const DRIVE_REMOVABLE: u32 = 2;
const DRIVE_FIXED: u32 = 3;
const DRIVE_REMOTE: u32 = 4;
const DRIVE_RAMDISK: u32 = 6;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetLogicalDrives() -> u32;
    fn GetDriveTypeW(root_path: *const u16) -> u32;
    fn GetVolumeInformationW(
        root_path: *const u16,
        volume_name: *mut u16,
        volume_name_size: u32,
        volume_serial: *mut u32,
        max_component_length: *mut u32,
        file_system_flags: *mut u32,
        file_system_name: *mut u16,
        file_system_name_size: u32,
    ) -> i32;
    fn GetDiskFreeSpaceExW(
        directory: *const u16,
        free_bytes_for_caller: *mut u64,
        total_bytes: *mut u64,
        total_free_bytes: *mut u64,
    ) -> i32;
    fn GetComputerNameExW(name_type: i32, name_buffer: *mut u16, size: *mut u32) -> i32;
}

#[link(name = "netapi32")]
unsafe extern "system" {
    fn NetGetJoinInformation(server: *const u16, name: *mut *mut u16, status: *mut i32) -> u32;
    fn NetApiBufferFree(buffer: *mut c_void) -> u32;
}

#[derive(Clone, Debug, Default)]
pub struct Partition {
    pub letter: char,
    pub label: String,
    pub fs: String,
    pub total: u64,
    pub free: u64,
    pub kind: u32,
    pub error: bool,
}

impl Partition {
    pub fn used(&self) -> u64 { self.total.saturating_sub(self.free) }
    pub fn fraction(&self) -> f32 {
        if self.total == 0 { 0.0 } else { self.used() as f32 / self.total as f32 }
    }
}

#[derive(Clone, Debug)]
pub struct DeviceInfo {
    pub hostname: String,
    pub workgroup: String,
    pub description: String,
    pub windows: String,
    pub user: String,
    pub cpu: String,
}

#[derive(Debug)]
pub enum Update {
    Memory { used: u64, total: u64 },
    Partitions(Vec<Partition>),
}

pub fn start_monitor() -> Receiver<Update> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut sys = System::new_all();
        let mut next_ram = Instant::now();
        let mut next_disk = Instant::now();
        loop {
            let now = Instant::now();
            if now >= next_ram {
                sys.refresh_memory();
                let total = sys.total_memory();
                let used = sys.used_memory().min(total);
                if tx.send(Update::Memory { used, total }).is_err() { break; }
                next_ram = Instant::now() + Duration::from_secs(10);
            }
            if now >= next_disk {
                let partitions = list_partitions();
                if tx.send(Update::Partitions(partitions)).is_err() { break; }
                next_disk = Instant::now() + Duration::from_secs(30);
            }
            thread::sleep(Duration::from_millis(300));
        }
    });
    rx
}

/// 全論理ドライブのうちC:～M:に存在するものを毎回列挙。
/// 光学ドライブはパーティションではないため除外。
fn list_partitions() -> Vec<Partition> {
    let mut out = Vec::new();
    let mask = unsafe { GetLogicalDrives() };
    for byte in b'C'..=b'M' {
        let letter = byte as char;
        let index = letter as u32 - 'A' as u32;
        if mask != 0 && (mask & (1u32 << index)) == 0 { continue; }
        let root: Vec<u16> = format!("{}:\\", letter).encode_utf16().chain(std::iter::once(0)).collect();
        let kind = unsafe { GetDriveTypeW(root.as_ptr()) };
        if !matches!(kind, DRIVE_REMOVABLE | DRIVE_FIXED | DRIVE_REMOTE | DRIVE_RAMDISK) { continue; }

        let mut volume_buf = [0u16; 260];
        let mut fs_buf = [0u16; 64];
        let got_label = unsafe {
            GetVolumeInformationW(
                root.as_ptr(), volume_buf.as_mut_ptr(), volume_buf.len() as u32,
                ptr::null_mut(), ptr::null_mut(), ptr::null_mut(),
                fs_buf.as_mut_ptr(), fs_buf.len() as u32,
            ) != 0
        };
        let label = if got_label { from_wide(&volume_buf) } else { String::new() };
        let fs = if got_label { from_wide(&fs_buf) } else { String::new() };
        let mut available = 0u64;
        let mut total = 0u64;
        let mut free_total = 0u64;
        let ok = unsafe {
            GetDiskFreeSpaceExW(root.as_ptr(), &mut available, &mut total, &mut free_total) != 0
        };
        // 使用量を「現在のユーザーに利用できる容量」ではなく物理使用量として算出。
        // これにより管理者予約・クォータによる見かけ上の消費増を避ける。
        let free = free_total.min(total);
        out.push(Partition { letter, label, fs, total, free, kind, error: !ok });
    }
    out
}

pub fn disk_totals(partitions: &[Partition]) -> (u64, u64) {
    partitions.iter()
        .filter(|p| !p.error && matches!(p.kind, DRIVE_FIXED | DRIVE_RAMDISK))
        .fold((0u64, 0u64), |(used, total), p| {
            (used.saturating_add(p.used()), total.saturating_add(p.total))
        })
}

fn from_wide(buf: &[u16]) -> String {
    let len = buf.iter().position(|&x| x == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

fn full_computer_name() -> String {
    let mut buf = [0u16; 256];
    let mut len = buf.len() as u32;
    // ComputerNameDnsFullyQualified = 3
    let ok = unsafe { GetComputerNameExW(3, buf.as_mut_ptr(), &mut len) != 0 };
    if ok && len > 0 { String::from_utf16_lossy(&buf[..len as usize]) }
    else { env::var("COMPUTERNAME").unwrap_or_else(|_| "不明".into()) }
}

fn workgroup_name() -> String {
    let mut pointer: *mut u16 = ptr::null_mut();
    let mut status = 0i32;
    let result = unsafe { NetGetJoinInformation(ptr::null(), &mut pointer, &mut status) };
    if result != 0 || pointer.is_null() { return "不明".into(); }
    // Win32が確保した終端NUL付きバッファを必ず解放する。
    let name = unsafe {
        let mut len = 0usize;
        while len < 32768 && *pointer.add(len) != 0 { len += 1; }
        let slice = std::slice::from_raw_parts(pointer, len);
        let value = String::from_utf16_lossy(slice);
        NetApiBufferFree(pointer.cast());
        value
    };
    if name.is_empty() { "不明".into() } else { name }
}

fn registry_string(key: &str, name: &str) -> Option<String> {
    RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(key).ok()?.get_value(name).ok()
}

fn os_label() -> String {
    let key = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
    let mut product = registry_string(key, "ProductName").unwrap_or_else(|| "Windows".into());
    let display = registry_string(key, "DisplayVersion")
        .or_else(|| registry_string(key, "ReleaseId"))
        .unwrap_or_default();
    let build = registry_string(key, "CurrentBuildNumber")
        .and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
    if build >= 22000 { product = product.replace("Windows 10", "Windows 11"); }
    if display.is_empty() { product } else { format!("{} {}", product, display) }
}

fn user_label() -> String {
    let name = env::var("USERNAME").unwrap_or_else(|_| "不明".into());
    let domain = env::var("USERDOMAIN").unwrap_or_default();
    let computer = env::var("COMPUTERNAME").unwrap_or_default();
    if !domain.is_empty() && !domain.eq_ignore_ascii_case(&computer) {
        format!("{} @ {}", name, domain)
    } else { name }
}

pub fn get_device_info() -> DeviceInfo {
    let sys = System::new_all();
    let cpu = sys.cpus().first().map(|c| {
        let frequency_mhz = c.frequency();
        if frequency_mhz == 0 { c.brand().trim().to_owned() }
        else { format!("{} @ {:.2} GHz", c.brand().trim(), frequency_mhz as f64 / 1000.0) }
    }).unwrap_or_else(|| "不明".into());
    let description = registry_string(
        r"SYSTEM\CurrentControlSet\Services\LanmanServer\Parameters", "srvcomment"
    ).filter(|x| !x.trim().is_empty()).unwrap_or_else(|| "設定なし".into());
    DeviceInfo {
        hostname: full_computer_name(),
        workgroup: workgroup_name(),
        description,
        windows: os_label(),
        user: user_label(),
        cpu,
    }
}

/// バイトを2桁小数・自動単位で表示。Windows風の 1024 換算。
pub fn capacity(bytes: u64) -> String {
    let mut size = bytes as f64;
    if size < 1024.0 { return format!("{size:.2} B"); }
    // KiB/MiB/GiBをWindowsに近いKB/MB/GB表記として表示。
    let names = ["KB", "MB", "GB", "TB", "PB", "EB"];
    for unit in names {
        size /= 1024.0;
        if size < 1024.0 || unit == "EB" { return format!("{size:.2} {unit}"); }
    }
    format!("{size:.2} EB")
}

/// 進捗バー内は左右各最大8文字程度に収められるよう単位前の空白を省略。
pub fn short_capacity(bytes: u64) -> String { capacity(bytes).replace(' ', "") }

pub fn usage_ratio(used: u64, total: u64) -> f64 {
    if total == 0 { 0.0 } else { (used as f64 / total as f64 * 100.0).clamp(0.0, 100.0) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn formatted_values() {
        assert_eq!(capacity(1024 * 1024), "1.00 MB");
        assert_eq!(short_capacity(1024 * 1024 * 1024), "1.00GB");
        assert_eq!(usage_ratio(75, 100), 75.0);
    }
}
