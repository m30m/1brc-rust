use std::env;
use std::fs::File;
use std::os::unix::io::AsRawFd;

#[derive(Debug)]
struct StationStats {
    min: i32,
    max: i32,
    sum: i64,
    count: u64,
}

impl StationStats {
    fn new() -> Self {
        Self {
            min: i32::MAX,
            max: i32::MIN,
            sum: 0,
            count: 0,
        }
    }

    fn update(&mut self, temp: i32) {
        if temp < self.min {
            self.min = temp;
        }
        if temp > self.max {
            self.max = temp;
        }
        self.sum += temp as i64;
        self.count += 1;
    }

    fn mean(&self) -> f64 {
        self.sum as f64 / self.count as f64 / 10.0
    }

    fn min_f64(&self) -> f64 {
        self.min as f64 / 10.0
    }

    fn max_f64(&self) -> f64 {
        self.max as f64 / 10.0
    }
}

/// Parses a temperature like "-12.3" or "4.5" as an i32 scaled by 10 (e.g. -123, 45).
/// Assumes exactly one decimal digit.
fn parse_temp(bytes: &[u8]) -> i32 {
    let (negative, start) = if bytes[0] == b'-' {
        (true, 1)
    } else {
        (false, 0)
    };

    let mut value: i32 = 0;
    let mut i = start;
    while bytes[i] != b'.' {
        value = value * 10 + (bytes[i] - b'0') as i32;
        i += 1;
    }
    // skip '.', parse the single decimal digit
    value = value * 10 + (bytes[i + 1] - b'0') as i32;

    if negative { -value } else { value }
}

const TABLE_SIZE: usize = 65536; // power of 2, handles up to ~10k stations
const TABLE_MASK: usize = TABLE_SIZE - 1;
const MAX_NAME_LEN: usize = 100;

struct Entry {
    name: [u8; MAX_NAME_LEN],
    name_len: u8,
    stats: StationStats,
}

struct StationTable {
    entries: Vec<Entry>,
}

impl StationTable {
    fn new() -> Self {
        let mut entries = Vec::with_capacity(TABLE_SIZE);
        for _ in 0..TABLE_SIZE {
            entries.push(Entry {
                name: [0; MAX_NAME_LEN],
                name_len: 0,
                stats: StationStats::new(),
            });
        }
        Self { entries }
    }

    #[inline(always)]
    fn hash(name: &[u8]) -> usize {
        // Read first 8 bytes as a u64 in one load, mix with length
        let mut buf = [0u8; 8];
        let n = name.len().min(8);
        buf[..n].copy_from_slice(&name[..n]);
        let h = u64::from_ne_bytes(buf) as usize;
        h ^ name.len()
    }

    #[inline(always)]
    fn lookup_or_insert(&mut self, name: &[u8], temp: i32) {
        let mut idx = Self::hash(name) & TABLE_MASK;

        loop {
            let entry = &mut self.entries[idx];

            if entry.name_len == 0 {
                // Empty slot — insert new entry
                entry.name[..name.len()].copy_from_slice(name);
                entry.name_len = name.len() as u8;
                entry.stats.update(temp);
                return;
            }

            if entry.name_len as usize == name.len()
                && &entry.name[..name.len()] == name
            {
                // Found existing entry
                entry.stats.update(temp);
                return;
            }

            // Collision — linear probe
            idx = (idx + 1) & TABLE_MASK;
        }
    }
}

fn mmap_file(file: &File) -> &[u8] {
    let len = file.metadata().expect("Failed to get file metadata").len() as usize;
    if len == 0 {
        return &[];
    }
    unsafe {
        let ptr = libc::mmap(
            std::ptr::null_mut(),
            len,
            libc::PROT_READ,
            libc::MAP_PRIVATE,
            file.as_raw_fd(),
            0,
        );
        assert!(ptr != libc::MAP_FAILED, "mmap failed");
        std::slice::from_raw_parts(ptr as *const u8, len)
    }
}

fn read_measurements(file_path: &str) -> StationTable {
    let file = File::open(file_path).expect("Failed to open file");
    let data = mmap_file(&file);

    let mut table = StationTable::new();
    let mut pos = 0;

    while pos < data.len() {
        // SIMD-accelerated delimiter search
        let semi = memchr::memchr(b';', &data[pos..]).unwrap() + pos;
        let end = memchr::memchr(b'\n', &data[semi + 1..])
            .map(|i| i + semi + 1)
            .unwrap_or(data.len());

        let name = &data[pos..semi];
        let temp = parse_temp(&data[semi + 1..end]);

        table.lookup_or_insert(name, temp);

        pos = end + 1;
    }

    table
}

fn output_results(table: &StationTable) {
    // Collect occupied entries
    let mut results: Vec<(&[u8], &StationStats)> = Vec::new();
    for entry in &table.entries {
        if entry.name_len > 0 {
            results.push((&entry.name[..entry.name_len as usize], &entry.stats));
        }
    }

    // Sort alphabetically by station name
    results.sort_by(|a, b| a.0.cmp(b.0));

    // Output results
    print!("{{");
    for (i, (name, stats)) in results.iter().enumerate() {
        if i > 0 {
            print!(", ");
        }
        // SAFETY: station names are assumed to be valid UTF-8
        let name_str = unsafe { std::str::from_utf8_unchecked(name) };
        print!(
            "{}={:.1}/{:.1}/{:.1}",
            name_str,
            stats.min_f64(),
            stats.mean(),
            stats.max_f64()
        );
    }
    println!("}}");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let file_path = args.get(1).map(|s| s.as_str()).unwrap_or("measurements.txt");

    let table = read_measurements(file_path);
    output_results(&table);
}
