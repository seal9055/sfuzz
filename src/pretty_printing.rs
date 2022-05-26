use crate::{
    config::{COVMETHOD, PERM_CHECKS, SNAPSHOT_ADDR, NUM_THREADS, DEBUG_PRINT},
    Statistics, Corpus,
};

use core::fmt;
use std::sync::Arc;

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
    if DEBUG_PRINT {
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
                &Arc<Corpus>) {
    term.move_cursor_to(0, 2).unwrap();
    term.write_line(
        &format!("{}", Green("\t\t[ SFUZZ - https://github.com/seal9055/sfuzz ]\n"))
    ).unwrap();

    // Progress information
    term.write_line(
        &format!("\t{}\n\t   Run time [sec]: {:8.2}\n\t   Total fuzz cases: {:12} \
                \n\t   Instrs execd [mil]: {:12}", 
        Blue("Progression"), 
        elapsed_time,
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
    term.write_line(&format!("   Crashes: \t{}", stats.crashes)).unwrap();
    term.move_cursor_to(54, 7).unwrap();
    term.write_line(&format!("   Timeouts: \t{}", stats.timeouts)).unwrap();

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

    // Coverage
    term.move_cursor_to(54, 10).unwrap();
    term.write_line(&format!("{}", Blue("Coverage"))).unwrap();
    term.move_cursor_to(54, 11).unwrap();
    term.write_line(&format!("   Coverage: {}", stats.coverage)).unwrap();

    // Config information
    term.move_cursor_down(2).unwrap();
    term.write_line(
        &format!("\t{}\n\t   Num Threads: {}\n\t   Coverage type: {:?}\n\t   \
        Snapshots enabled: {}\n\t   ASAN: {}\n\t   timeout: {}",
        Blue("Config"), 
        NUM_THREADS,
        COVMETHOD,
        SNAPSHOT_ADDR.is_some(),
        PERM_CHECKS,
        timeout,
        )
    ).unwrap();

    // Corpus stats
    term.move_cursor_to(54, 14).unwrap();
    term.write_line(&format!("{}", Blue("Corpus"))).unwrap();
    term.move_cursor_to(54, 15).unwrap();
    term.write_line(&format!("   Size: {}", corpus.inputs.read().len())).unwrap();
    term.move_cursor_to(54, 16).unwrap();
    term.write_line(&format!("   Instrs per case: {}", 
                             (stats.instr_count / stats.total_cases as u64)
                             )).unwrap();

    // Flush buffer and write to terminal
    term.flush().unwrap();
}

/// Simple debug view of statistics
fn basic_stats(stats: &Statistics, elapsed_time: f64) {
    println!(
        "[{:8.2}] fuzz cases: {:12} : fcps: {:8} : coverage: {:6} : crashes: {:8} \
        \n\t   instr_cnt: {:13} : ips: {:9} : ucrashes: {:6}\n", 
        elapsed_time, 
        stats.total_cases.to_formatted_string(&Locale::en),
        (stats.total_cases / elapsed_time as usize).to_formatted_string(&Locale::en), 
        stats.coverage,
        stats.crashes,
        stats.instr_count.to_formatted_string(&Locale::en),
        (stats.instr_count / elapsed_time as u64).to_formatted_string(&Locale::en), 
        stats.ucrashes);
}

/// Wrapper for actual stat-printing functions
pub fn print_stats(term: &Term, stats: &Statistics, elapsed_time: f64, timeout: u64, 
                   corpus: &Arc<Corpus>) {
    if DEBUG_PRINT {
        basic_stats(stats, elapsed_time);
    } else {
        pretty_stats(term, stats, elapsed_time, timeout, corpus);
    }

}
