use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader};

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

const TABLE_SIZE: usize = 8192; // power of 2, ~5% load factor for ~400 stations
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
        // Cheap hash: read first bytes as integer + mix with length
        let mut h: usize = name.len();
        for &b in name.iter().take(8) {
            h = h.wrapping_mul(31).wrapping_add(b as usize);
        }
        h
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

fn read_measurements(file_path: &str) -> StationTable {
    let file = File::open(file_path).expect("Failed to open file");
    let mut reader = BufReader::new(file);

    let mut table = StationTable::new();
    let mut buf: Vec<u8> = Vec::with_capacity(200);

    loop {
        buf.clear();
        let bytes_read = reader.read_until(b'\n', &mut buf).expect("Failed to read line");
        if bytes_read == 0 {
            break; // EOF
        }

        // Strip trailing newline/carriage return
        let mut len = buf.len();
        if len > 0 && buf[len - 1] == b'\n' { len -= 1; }
        if len > 0 && buf[len - 1] == b'\r' { len -= 1; }
        let bytes = &buf[..len];

        // Find ';' separator by scanning bytes
        let mut sep = 0;
        while bytes[sep] != b';' {
            sep += 1;
        }

        let name = &bytes[..sep];
        let temp = parse_temp(&bytes[sep + 1..]);

        table.lookup_or_insert(name, temp);
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
