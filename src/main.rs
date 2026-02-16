use std::collections::HashMap;
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
    fn new(temp: i32) -> Self {
        Self {
            min: temp,
            max: temp,
            sum: temp as i64,
            count: 1,
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

fn read_measurements(file_path: &str) -> HashMap<String, StationStats> {
    let file = File::open(file_path).expect("Failed to open file");
    let mut reader = BufReader::new(file);

    let mut stations: HashMap<String, StationStats> = HashMap::new();
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

        // SAFETY: station names are assumed to be valid UTF-8
        let name = unsafe { std::str::from_utf8_unchecked(&bytes[..sep]) };
        let temp = parse_temp(&bytes[sep + 1..]);

        if let Some(stats) = stations.get_mut(name) {
            stats.update(temp);
        } else {
            stations.insert(name.to_string(), StationStats::new(temp));
        }
    }

    stations
}

fn output_results(stations: &HashMap<String, StationStats>) {
    // Sort stations alphabetically
    let mut sorted_stations: Vec<_> = stations.iter().collect();
    sorted_stations.sort_by(|a, b| a.0.cmp(b.0));

    // Output results in the expected format
    print!("{{");
    for (i, (name, stats)) in sorted_stations.iter().enumerate() {
        if i > 0 {
            print!(", ");
        }
        print!(
            "{}={:.1}/{:.1}/{:.1}",
            name,
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

    let stations = read_measurements(file_path);
    output_results(&stations);
}
