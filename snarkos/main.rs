// Copyright (C) 2019-2023 Aleo Systems Inc.
// This file is part of the snarkOS library.

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at:
// http://www.apache.org/licenses/LICENSE-2.0

// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use snarkos_cli::{commands::CLI, helpers::Updater};

use clap::Parser;
use humansize::{format_size, BINARY};
use num_format::{Locale, ToFormattedString};
use std::process::exit;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use system_alloc_stats::SystemWithStats;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[global_allocator]
static SWS: SystemWithStats = SystemWithStats;

fn main() -> anyhow::Result<()> {
    std::thread::spawn(|| {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(60));

            tracing::info!(
                "HEAP: {} curr; {} max; {} allocs; {} deallocs",
                format_size(SWS.use_curr(), BINARY),
                format_size(SWS.use_max(), BINARY),
                SWS.alloc_count().to_formatted_string(&Locale::en),
                SWS.dealloc_count().to_formatted_string(&Locale::en)
            );
        }
    });

    // Parse the given arguments.
    let cli = CLI::parse();
    // Run the updater.
    println!("{}", Updater::print_cli());
    // Run the CLI.
    match cli.command.parse() {
        Ok(output) => println!("{output}\n"),
        Err(error) => {
            println!("⚠️  {error}\n");
            exit(1);
        }
    }
    Ok(())
}
