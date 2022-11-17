use crate::{
    config::{COV_METHOD, NO_PERM_CHECKS, SNAPSHOT_ADDR, NUM_THREADS, DEBUG_PRINT, CMP_COV, 
        RUN_CASES, SEND_REMOTE},
    Statistics, Corpus,
};

use core::fmt;
use std::sync::Arc;
use std::time::Duration;
use std::collections::HashMap;

use console::Term;
use num_format::{Locale, ToFormattedString};

/// Different log-types that can be used to print out messages in different colors
pub enum LogType {
    Neutral = 0,
    Success = 1,
    Failure = 2,
}

/// Color a string green
pub struct Green(pub &'static str);
impl fmt::Display for Green {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
        write!(f, "\x1B[32m")?;
        write!(f, "{}", self.0)?;
        write!(f, "\x1B[0m")?;
        Ok(())
    }
}

/// Color a string blue
pub struct Blue(pub &'static str);
impl fmt::Display for Blue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
        write!(f, "\x1B[34m")?;
        write!(f, "{}", self.0)?;
        write!(f, "\x1B[0m")?;
        Ok(())
    }
}

/// Color a string red
pub struct Red(pub &'static str);
impl fmt::Display for Red {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { 
        write!(f, "\x1B[31m")?;
        write!(f, "{}", self.0)?;
        write!(f, "\x1B[0m")?;
        Ok(())
    }
}

/// Small wrapper to print out colored log messages
pub fn log(color: LogType, msg: &str) {
    if *DEBUG_PRINT.get().unwrap() {
        match color {
            LogType::Neutral => {
                println!("{} {}", Blue("[-]"), msg);
            },
            LogType::Success => {
                println!("{} {}", Green("[+]"), msg);
            },
            LogType::Failure => {
                println!("{} {}", Red("[!]"), msg);
            },
        }
    }
}

/// Print out statistics in a nicely formated static screen
fn pretty_stats(term: &Term, stats: &Statistics, elapsed_time: f64, timeout: u64, corpus: 
                &Arc<Corpus>, last_cov: f64) {

    term.clear_screen().unwrap();
    term.move_cursor_to(0, 2).unwrap();

    // Print out error message instead of standard output if the terminal size is too small to
    // properly display output
    let (x, y) = term.size();
    if x < 25 || y < 95 {
        term.write_line(&format!("Increase terminal size to 25:95 (Cur: {}:{})", x, y)).unwrap();
        term.flush().unwrap();
        return;
    }

    term.write_line(
        &format!("{}", Green("\t\t[ SFUZZ - https://github.com/seal9055/sfuzz ]\n"))
    ).unwrap();

    let duration    = Duration::from_secs_f64(elapsed_time);
    let elapsed_sec = duration.as_secs() % 60;
    let elapsed_min = (duration.as_secs() / 60) % 60;
    let elapsed_hr  = (duration.as_secs() / 60) / 60;

    // Progress information
    term.write_line(
        &format!("\t{}\n\t   Run time: {:02}:{:02}:{:02}\n\t   Total fuzz cases: {:12} \
                \n\t   Instrs execd [mil]: {:12}", 
        Blue("Progression"), 
        elapsed_hr, elapsed_min, elapsed_sec,
        stats.total_cases.to_formatted_string(&Locale::en),
        (stats.instr_count / 1_000_000).to_formatted_string(&Locale::en),
        )
    ).unwrap();

    // Results
    term.move_cursor_to(54, 4).unwrap();
    term.write_line(&format!("{}", Blue("Overall Results"))).unwrap();
    term.move_cursor_to(54, 5).unwrap();
    term.write_line(&format!("   Unique Crashes: {}", stats.ucrashes)).unwrap();
    term.move_cursor_to(54, 6).unwrap();
    term.write_line(&format!("   Crashes: \t{}", stats.crashes.to_formatted_string(&Locale::en)))
        .unwrap();
    term.move_cursor_to(54, 7).unwrap();
    term.write_line(&format!("   Timeouts: \t{}", stats.timeouts.to_formatted_string(&Locale::en)))
        .unwrap();

    // Performance numbers
    term.move_cursor_down(2).unwrap();
    term.write_line(
        &format!("\t{}\n\t   Fuzz cases per second: {:12}\n\t   \
                Instrs per second [mil]: {:12}",
        Blue("Performance measurements"), 
        (stats.total_cases / elapsed_time as usize).to_formatted_string(&Locale::en), 
        (stats.instr_count / 1_000_000 / elapsed_time as u64)
            .to_formatted_string(&Locale::en), 
        )
    ).unwrap();

    let duration  = Duration::from_secs_f64(elapsed_time - last_cov);
    let cov_sec   = duration.as_secs() % 60;
    let cov_min   = (duration.as_secs() / 60) % 60;
    let cov_hr    = (duration.as_secs() / 60) / 60;

    // Coverage
    term.move_cursor_to(54, 10).unwrap();
    term.write_line(&format!("{}", Blue("Coverage"))).unwrap();
    term.move_cursor_to(54, 11).unwrap();
    term.write_line(&format!("   Coverage: {}", stats.coverage)).unwrap();
    term.move_cursor_to(54, 12).unwrap();
    term.write_line(&format!("   CmpCov: {}", stats.cmpcov)).unwrap();
    term.move_cursor_to(54, 13).unwrap();
    term.write_line(&format!("   Time since last cov: {:02}:{:02}:{:02}", 
                    cov_hr, cov_min, cov_sec)).unwrap();

    let run_cases = match RUN_CASES.get().unwrap() {
        Some(v) => format!("{}", v),
        None => "No Limit".to_string(),
    };

    // Config information
    term.move_cursor_down(1).unwrap();
    term.write_line(
        &format!("\t{}\n\t   Num Threads: {}\n\t   Coverage type: {:?}\n\t   \
        Snapshots enabled: {}\n\t   ASAN: {}\n\t   Timeout: {}\n\t   CmpCov: {}\n\t   Max runs: {}",
        Blue("Config"), 
        NUM_THREADS.get().unwrap(),
        COV_METHOD.get().unwrap(),
        SNAPSHOT_ADDR.get().unwrap().is_some(),
        !NO_PERM_CHECKS.get().unwrap(),
        timeout.to_formatted_string(&Locale::en),
        CMP_COV.get().unwrap(),
        run_cases,
    )).unwrap();

    // Corpus stats
    term.move_cursor_to(54, 15).unwrap();
    term.write_line(&format!("{}", Blue("Corpus"))).unwrap();
    term.move_cursor_to(54, 16).unwrap();
    term.write_line(&format!("   Num Entries: {}", corpus.inputs.read().len())).unwrap();
    term.move_cursor_to(54, 17).unwrap();
    term.write_line(&format!("   Avg Instrs per case: {}", 
                             (stats.instr_count / stats.total_cases as u64)
                             )).unwrap();

    // Flush buffer and write to terminal
    term.flush().unwrap();
}

/// Simple debug view of statistics
fn basic_stats(stats: &Statistics, elapsed_time: f64) {
    println!(
        "[{:8.2}] fuzz cases: {:12} : fcps: {:8} : coverage: {:6} : crashes: {:8} \
        \n\t   instr_cnt: {:13} : ips: {:9} : ucrashes: {:6} : timeouts: {:8}", 
        elapsed_time, 
        stats.total_cases.to_formatted_string(&Locale::en),
        (stats.total_cases / elapsed_time as usize).to_formatted_string(&Locale::en), 
        stats.coverage,
        stats.crashes,
        stats.instr_count.to_formatted_string(&Locale::en),
        (stats.instr_count / elapsed_time as u64).to_formatted_string(&Locale::en), 
        stats.ucrashes,
        stats.timeouts
    );
}

fn send_remote(ip: String, port: usize, stats: &Statistics, elapsed_time: f64) {
    let request_url = format!("http://{}:{}/stats", ip, port).to_string();
    let client = reqwest::Client::new();

    let mut map = HashMap::new();
    map.insert("total_cases", stats.total_cases);
    map.insert("crashes", stats.crashes);
    map.insert("ucrashes", stats.ucrashes);
    map.insert("coverage", stats.coverage);
    map.insert("cmpcov", stats.cmpcov);
    map.insert("instr_count", stats.instr_count as usize);
    map.insert("timeouts", stats.timeouts as usize);
    map.insert("exec_time", elapsed_time as usize * 1_000);

    let _ = client.post(request_url).json(&map).send();
}

/// Wrapper for actual stat-printing functions
pub fn print_stats(term: &Term, stats: &Statistics, elapsed_time: f64, timeout: u64, 
                   corpus: &Arc<Corpus>, last_cov: f64) {
    if *DEBUG_PRINT.get().unwrap() {
        basic_stats(stats, elapsed_time);
    } else {
        pretty_stats(term, stats, elapsed_time, timeout, corpus, last_cov);
    }

    if let Some(connection_info) = SEND_REMOTE.get().unwrap() {
        let mut iter = connection_info.split(":");
        let ip   = iter.next().expect("Given ip in incorrect format").to_string();
        let port: usize = iter.next().expect("Given port in incorrect format").parse()
            .expect("Given port in incorrect format");

        assert!(port < 65536, "Invalid port number");
        send_remote(ip, port, stats, elapsed_time);
    }
}
