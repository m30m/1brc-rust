use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader};

#[derive(Debug)]
struct StationStats {
    min: f64,
    max: f64,
    sum: f64,
    count: u64,
}

impl StationStats {
    fn new(temp: f64) -> Self {
        Self {
            min: temp,
            max: temp,
            sum: temp,
            count: 1,
        }
    }

    fn update(&mut self, temp: f64) {
        if temp < self.min {
            self.min = temp;
        }
        if temp > self.max {
            self.max = temp;
        }
        self.sum += temp;
        self.count += 1;
    }

    fn mean(&self) -> f64 {
        self.sum / self.count as f64
    }
}

fn read_measurements(file_path: &str) -> HashMap<String, StationStats> {
    let file = File::open(file_path).expect("Failed to open file");
    let reader = BufReader::new(file);

    let mut stations: HashMap<String, StationStats> = HashMap::new();

    for line in reader.lines() {
        let line = line.expect("Failed to read line");

        // Parse line: "station_name;temperature"
        if let Some((name, temp_str)) = line.split_once(';') {
            let temp: f64 = temp_str.parse().expect("Failed to parse temperature");

            stations
                .entry(name.to_string())
                .and_modify(|stats| stats.update(temp))
                .or_insert_with(|| StationStats::new(temp));
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
            stats.min,
            stats.mean(),
            stats.max
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
