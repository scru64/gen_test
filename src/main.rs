use std::io::prelude::*;
use std::process::ExitCode;
use std::{env, io, time};

const STATS_INTERVAL: u64 = 10 * 1000;

fn main() -> io::Result<ExitCode> {
    let mut args = env::args();
    let program = args.next();
    if let Some(arg) = args.next() {
        let usage = format!(
            "Usage: any-command-that-prints-identifiers-infinitely | {}",
            program.as_deref().unwrap_or("scru64-test")
        );
        return if arg == "-h" || arg == "--help" {
            println!("{usage}");
            Ok(ExitCode::SUCCESS)
        } else {
            eprintln!("Error: unknown argument: {arg}");
            eprintln!("{usage}");
            Ok(ExitCode::FAILURE)
        };
    }

    let mut reader = io::stdin().lock();
    let mut buffer = Vec::with_capacity(16);
    println!(
        "Reading IDs from stdin and will show stats every {} seconds. Press Ctrl-C to quit.",
        STATS_INTERVAL / 1000
    );

    let mut st = Status::default();
    let mut prev = Identifier::default();
    while {
        buffer.clear();
        reader.read_until(b'\n', &mut buffer)? > 0
    } {
        let line = match buffer.strip_suffix(b"\n") {
            Some(s) => s.strip_suffix(b"\r").unwrap_or(s),
            None => &buffer,
        };

        let Some(e) = Identifier::new(line) else {
            eprintln!("Error: invalid string representation");
            st.n_errors += 1;
            continue;
        };

        st.n_processed += 1;
        if e.str_bytes <= prev.str_bytes {
            eprintln!("Error: string representation not monotonically ordered");
            st.n_errors += 1;
            continue;
        }
        if e.int_value <= prev.int_value {
            eprintln!("Error: integer representation not monotonically ordered");
            st.n_errors += 1;
            continue;
        }
        if e.unix_ts_ms < prev.unix_ts_ms {
            eprintln!("Error: clock went backwards");
            st.n_errors += 1;
            continue;
        } else if e.unix_ts_ms == prev.unix_ts_ms && e.node_ctr < prev.node_ctr {
            eprintln!("Error: node_ctr went backwards within same timestamp");
            st.n_errors += 1;
            continue;
        }

        // Triggered per line
        if st.ts_first == 0 {
            st.ts_first = e.unix_ts_ms;
        }
        st.ts_last = e.unix_ts_ms;

        // Triggered per 256 millisecond or at node_ctr increment
        if e.node_ctr != prev.node_ctr + 1 {
            if st.ts_last_counter_update > 0 {
                st.n_counter_lo_update += 1;
                st.sum_intervals_counter_update += e.unix_ts_ms - st.ts_last_counter_update;
            }
            st.ts_last_counter_update = e.unix_ts_ms;
        }

        // Triggered per STATS_INTERVAL seconds
        if e.unix_ts_ms > st.ts_last_stats_print + STATS_INTERVAL {
            if st.ts_last_stats_print > 0 {
                st.print()?;
            }
            st.ts_last_stats_print = e.unix_ts_ms;
        }

        // Prepare for next loop
        prev = e;
    }

    if st.n_processed > 0 {
        st.print()?;
    } else {
        eprintln!("Error: no valid ID processed");
        return Ok(ExitCode::FAILURE);
    }

    if st.n_errors == 0 {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::FAILURE)
    }
}

#[derive(Debug, Default)]
struct Status {
    n_processed: usize,
    n_errors: usize,
    ts_first: u64,
    ts_last: u64,

    n_counter_lo_update: usize,
    ts_last_counter_update: u64,
    sum_intervals_counter_update: u64,

    ts_last_stats_print: u64,
}

impl Status {
    fn print(&self) -> io::Result<()> {
        let time_elapsed = self.ts_last - self.ts_first;

        let mut buf = io::stdout().lock();
        writeln!(buf)?;
        writeln!(buf, "{:<48} {:>12} {:>12}", "STAT", "EXPECTED", "ACTUAL")?;
        writeln!(
            buf,
            "{:<48} {:>12} {:>12.1}",
            "Seconds from first input ID to last (sec)",
            "NA",
            time_elapsed as f64 / 1000.0
        )?;
        writeln!(
            buf,
            "{:<48} {:>12} {:>12}",
            "Number of valid IDs processed", "NA", self.n_processed
        )?;
        writeln!(
            buf,
            "{:<48} {:>12} {:>12}",
            "Number of invalid IDs skipped", 0, self.n_errors
        )?;
        writeln!(
            buf,
            "{:<48} {:>12} {:>12}",
            "Mean number of IDs per 256 millisecond",
            "<~MAX_CTR/2",
            self.n_processed as u64 / (time_elapsed >> 8)
        )?;
        writeln!(
            buf,
            "{:<48} {:>12} {:>12.3}",
            "Current time less timestamp of last ID (sec)",
            "-10.0 - 0.0",
            get_current_time() - (self.ts_last as f64) / 1000.0
        )?;
        writeln!(
            buf,
            "{:<48} {:>12} {:>12.3}",
            "Mean interval of counter updates (msec)",
            "~256",
            self.sum_intervals_counter_update as f64 / self.n_counter_lo_update as f64
        )?;

        Ok(())
    }
}

/// Holds representations and internal field values of a SCRU64 ID.
#[derive(Clone, Eq, PartialEq, Hash, Debug, Default)]
struct Identifier {
    str_bytes: [u8; 12],
    int_value: u64,
    unix_ts_ms: u64,
    node_ctr: u32,
}

impl Identifier {
    fn new(str_bytes: &[u8]) -> Option<Self> {
        const DECODE_MAP: [u8; 256] = [
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
            0x08, 0x09, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
            0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c,
            0x1d, 0x1e, 0x1f, 0x20, 0x21, 0x22, 0x23, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x0a,
            0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18,
            0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x20, 0x21, 0x22, 0x23, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff,
        ];

        let str_bytes = <[u8; 12]>::try_from(str_bytes).ok()?;
        let mut int_value = 0u64;

        let mut i = 0;
        while i < 12 {
            let n = DECODE_MAP[str_bytes[i] as usize] as u64;
            if n == 0xff {
                return None;
            }
            int_value = int_value * 36 + n;
            i += 1;
        }

        Some(Self {
            str_bytes,
            int_value,
            unix_ts_ms: (int_value >> 24) << 8,
            node_ctr: int_value as u32 & 0xff_ffff,
        })
    }
}

fn get_current_time() -> f64 {
    time::SystemTime::now()
        .duration_since(time::UNIX_EPOCH)
        .expect("clock may have gone backwards")
        .as_secs_f64()
}
