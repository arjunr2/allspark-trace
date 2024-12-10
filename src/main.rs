//! Binary crate for running a collect an AllSpark
//! `plc-trace` instrumented program from [`wasm_instrument`]
//!
//! [`wasm_instrument`]: https://github.com/arjunr2/wasm-instrument.git
//!
//! Inputs to this program must accept post-instrumented binaries; this crate
//! **does not** provide instrumentation facilities
use clap::Parser;
use log::info;
use once_cell::sync::Lazy;

use libc::c_void;
use nix::time;
use std::error::Error;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::slice;
use uuid::Uuid;

use tempfile::env;

use wamr_rust_sdk::{instance::Instance, module::Module, runtime::Runtime};
use wamr_rust_sdk::{
    log_level_t, wasm_exec_env_t, wasm_runtime_addr_app_to_native, wasm_runtime_get_module_inst,
    LOG_LEVEL_WARNING,
};

/// A Lazy-initialized temporary disk-backed filepath
static TMP_FILEPATH: Lazy<PathBuf> = Lazy::new(|| {
    let mut temppath = env::temp_dir();
    temppath.push(Uuid::new_v4().to_string());
    info!("Intermediate tracefile: {:?}", temppath);
    temppath
});

static mut TMPFILE_WR: Lazy<BufWriter<File>> =
    Lazy::new(|| BufWriter::new(File::create(&*TMP_FILEPATH).unwrap()));

static mut TRACE_DATA: &[u8] = &[];
static mut TIMESTAMP: u64 = 0;

/// Command-Line Arguments
#[derive(Parser, Debug)]
#[command(version, about, long_about=None)]
struct CLI {
    /// Log-level within the Wasm engine
    #[arg(short, long, default_value_t = LOG_LEVEL_WARNING)]
    verbose: log_level_t,

    /// Output trace file
    #[arg(short, long, default_value_t = String::from("trace.aspk"))]
    outfile: String,

    /// Input Command (Wasm program path + Argv)
    #[arg(num_args = 1..)]
    input_command: Vec<String>,
}

impl CLI {
    /// Print the CLI configuration
    fn print(&self) {
        info!("Input Command: {:?}", self.input_command);
        info!("Outfile: {}", self.outfile);
        info!("Log Level: {:?}", self.verbose);
    }
}

/// Wasm function to dump trace
///
/// Scope ID is used to identify the scope which is being traced
/// opcount_buf_addr is a pointer to buffer containing opcode distribution
/// buf_size is the size of the buffer
/// Get the timestamp for this block too
fn wasm_prog_tracedump(exec_env: wasm_exec_env_t, ctrlflow_buf_addr: u32, buf_size: u32) {
    unsafe {
        let module = wasm_runtime_get_module_inst(exec_env);
        let buf_addr: *mut c_void =
            wasm_runtime_addr_app_to_native(module, ctrlflow_buf_addr as u64);
        TRACE_DATA = slice::from_raw_parts(buf_addr as *const u8, buf_size as usize)
    };
    let timestamp = time::clock_gettime(time::ClockId::CLOCK_MONOTONIC).unwrap();
    unsafe { TIMESTAMP = timestamp.tv_sec() as u64 * 1_000_000_000 + timestamp.tv_nsec() as u64 };
}

/// Entrypoint for trace
fn main() -> Result<(), Box<dyn Error>> {
    env_logger::builder().format_timestamp_millis().init();
    let cli = CLI::parse();
    cli.print();

    // Read arguments
    let infile = cli.input_command[0].as_str();
    let infile_path = Path::new(infile);

    let runtime = Runtime::builder()
        .use_system_allocator()
        .set_host_function_module_name("allspark:trace")
        .register_host_function("prog_tracedump", wasm_prog_tracedump as *mut c_void)
        .set_max_thread_num(100)
        .build()?;
    runtime.set_log_level(cli.verbose);
    let module = Module::from_file(&runtime, infile_path)?;
    let instance = Instance::new(&runtime, &module, 1024 * 256)?;

    let _ = instance.execute_main(&cli.input_command)?;
    info!("Wasm module execution completed");

    // unsafe {
    //    TMPFILE_WR.flush()?;
    //}
    let mut tracefile = File::create(&cli.outfile)?;
    let timestamp_slice = unsafe { TIMESTAMP.to_le_bytes() };
    tracefile.write_all(&timestamp_slice)?;
    tracefile.write_all(unsafe { TRACE_DATA })?;
    info!(
        "Written trace of size {} to file: {}",
        timestamp_slice.len() + unsafe { TRACE_DATA.len() },
        cli.outfile
    );

    Ok(())
}
