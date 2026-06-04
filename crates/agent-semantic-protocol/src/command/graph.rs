//! `asp graph` command adapter.

use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

use serde_json::Value;

use crate::graph::{GraphRenderOptions, render_search_graph_packet};

pub(crate) fn run_graph_command(args: &[String]) -> Result<(), String> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err(usage());
    };
    match command {
        "render" => run_graph_render_command(&args[1..]),
        "help" | "--help" | "-h" => Err(usage()),
        _ => Err(usage()),
    }
}

fn run_graph_render_command(args: &[String]) -> Result<(), String> {
    let request = GraphRenderRequest::parse(args)?;
    if request.view != "seeds" {
        return Err("graph render currently supports only --view seeds".to_string());
    }
    let packet = read_packet(&request.packet_path)?;
    let output = render_search_graph_packet(
        &packet,
        GraphRenderOptions {
            seed_limit: request.seed_limit,
        },
    );
    print!("{output}");
    Ok(())
}

struct GraphRenderRequest {
    packet_path: PathBuf,
    view: String,
    seed_limit: Option<usize>,
}

impl GraphRenderRequest {
    fn parse(args: &[String]) -> Result<Self, String> {
        let packet_path = flag_value(args, "--packet")
            .ok_or_else(|| "missing required --packet <path-or->".to_string())?;
        let view = flag_value(args, "--view").unwrap_or_else(|| "seeds".to_string());
        let seed_limit = flag_value(args, "--seeds")
            .map(|value| {
                value
                    .parse::<usize>()
                    .map_err(|error| format!("invalid --seeds value: {error}"))
            })
            .transpose()?;
        Ok(Self {
            packet_path: PathBuf::from(packet_path),
            view,
            seed_limit,
        })
    }
}

fn read_packet(path: &PathBuf) -> Result<Value, String> {
    let mut contents = String::new();
    if path.as_os_str() == "-" {
        io::stdin()
            .read_to_string(&mut contents)
            .map_err(|error| format!("failed to read graph packet from stdin: {error}"))?;
    } else {
        contents = fs::read_to_string(path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    }
    serde_json::from_str(&contents).map_err(|error| format!("invalid graph packet JSON: {error}"))
}

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|window| window[0] == flag)
        .map(|window| window[1].clone())
}

fn usage() -> String {
    "usage: asp graph render --packet <path-or-> [--view seeds] [--seeds N]".to_string()
}
